#!/usr/bin/env python3
"""Generate typed Rust wrappers for open.mp interfaces from the SDK headers.

Hand-writing a wrapper means copying two slot indices, a C++ signature and a
Rust one, and every one of those copies is a chance to be wrong in a way that
fails silently. The headers already say everything: clang lays out the vtable
for both ABIs (see `omp-vtable.py`) and its AST gives every parameter and
return type. This turns that into the SDK's own `slots!` / `virtual_fns!`
entries.

    scripts/omp-wrappers.py                 # regenerate samp-sdk/src/omp/generated/
    scripts/omp-wrappers.py --check         # fail if the committed files are stale
    scripts/omp-wrappers.py --only objects  # one module

What goes in is listed in `scripts/omp-wrappers.toml`. What comes out is only
what the generator can state with certainty; everything else is written into
the file as a `// skipped:` line with the reason, so a gap is visible rather
than guessed at. The rules it applies:

- Integers, `float`, `bool` and enums (as their underlying type) pass as they
  are. `Vector2`/`Vector3`/`Vector4`, `Colour` and `StringView` pass by value,
  the way the SDK already passes them.
- A reference or pointer to an interface the SDK has a handle for becomes
  `*mut Handle`.
- A struct returned by value goes through the rule each ABI applies to member
  functions: MSVC always returns it through a hidden pointer, so eight bytes or
  less needs `call_vtable_small_struct!`, which supports methods without
  arguments; larger ones work with a plain declared return type on both ABIs.
- Anything from `std::`, a reference to a struct the SDK does not mirror, and
  overloaded names are skipped.
- A name the SDK already defines by hand is skipped too: the generated code
  complements the hand-written wrappers, it does not shadow them.

The output is a map, not the proof. `check-abi-slots.py --generated` compares
every generated slot with the official binaries, and a server run is the rest.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import tomllib

REPO = pathlib.Path(__file__).resolve().parent.parent
OMP = REPO / "samp-sdk/src/omp"
OUT = OMP / "generated"

_spec = importlib.util.spec_from_file_location("omp_vtable", REPO / "scripts/omp-vtable.py")
ov = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(ov)

# Base classes whose methods are not the interface's own business: extension
# plumbing, identity and pooling are wrapped once, generically, by hand.
NOT_OURS = {
    "IExtensible", "IExtension", "IIDProvider", "IEntity", "IUIDProvider",
    "IComponent", "IPoolComponent", "IPool", "IReadOnlyPool",
}

INTEGERS = {
    "int": "i32", "int32_t": "i32", "signed int": "i32",
    "unsigned int": "u32", "uint32_t": "u32", "unsigned": "u32",
    "short": "i16", "int16_t": "i16", "unsigned short": "u16", "uint16_t": "u16",
    "char": "i8", "signed char": "i8", "int8_t": "i8",
    "unsigned char": "u8", "uint8_t": "u8",
    "long long": "i64", "int64_t": "i64",
    "unsigned long long": "u64", "uint64_t": "u64", "UID": "u64",
    "float": "f32", "bool": "bool",
}

# Value types the SDK mirrors, with their size (for the return rule) and the
# neutral value a getter answers with when the object is not there.
VALUES = {
    "Vector2": (8, None),
    "Vector3": (12, "Vector3::ZERO"),
    "Vector4": (16, "Vector4::ZERO"),
    "Colour": (4, None),
    "StringView": (8, None),
}

NEUTRAL = {"bool": "false", "f32": "0.0"}


RUST_KEYWORDS = {
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final",
    "macro", "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
}


def param_name(cpp_name: str, this: str) -> str:
    """A Rust parameter name for a C++ one, clear of keywords and of the handle.

    `attachToObject(IObject& object, ...)` on an `object` handle would repeat the
    name, so the argument becomes `other_object`."""
    name = snake(cpp_name)
    if name == this:
        return f"other_{name}"
    if name in RUST_KEYWORDS:
        return f"{name}_value"
    return name


def wrap_params(head: str, params: list[str], tail: str, indent: str = "    ") -> str:
    """One line when it fits in 100 columns, one parameter per line otherwise —
    rustfmt does not reach inside a macro, so the generator lays it out."""
    one = f"{head}({', '.join(params)}){tail}"
    if len(indent + one) <= 100:
        return indent + one
    inner = "".join(f"{indent}    {p},\n" for p in params)
    return f"{indent}{head}(\n{inner}{indent}){tail}"


class Skip(Exception):
    """A method the generator will not wrap, and why."""


def rustfmt(text: str) -> str:
    """The text as `cargo fmt` would leave it.

    The generated files live inside crates that `cargo fmt` formats; written
    unformatted, the next `cargo fmt` rewrites them and `--check` reports them
    stale forever after."""
    done = subprocess.run(["rustfmt", "--edition", "2024"], input=text, capture_output=True, text=True)
    if done.returncode != 0:
        sys.exit(f"rustfmt rejected the generated code:\n{done.stderr}")
    return done.stdout


def snake(name: str) -> str:
    """`getHealth` -> `get_health`, `NPCComponent_UID` -> `npc_component_uid`.

    A run of capitals is one word (`NPC`, `IP`), ending where a lowercase letter
    starts the next one."""
    name = re.sub(r"(?<=[a-z0-9])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])", "_", name)
    return re.sub(r"_+", "_", name).lower()


def rust_name(prefix: str, method: str) -> str:
    """`getModel` -> `object_model`, `setModel` -> `object_set_model`.

    The SDK's getters drop `get`, as `player_health` does for `getHealth`."""
    if method.startswith("get") and len(method) > 3 and method[3].isupper():
        method = method[3:]
    return f"{prefix}_{snake(method)}"


def sdk_names() -> set[str]:
    """Every function and handle the hand-written SDK already defines."""
    names: set[str] = set()
    for path in OMP.glob("*.rs"):
        text = path.read_text()
        names |= set(re.findall(r"\bfn ([a-z_][a-z0-9_]*)\b", text))
    return names


def sdk_consts() -> set[str]:
    """Constants and `ComponentInterface` impls the hand-written SDK defines."""
    names: set[str] = set()
    for path in OMP.glob("*.rs"):
        text = path.read_text()
        names |= set(re.findall(r"\bpub const ([A-Z_][A-Z0-9_]*)\b", text))
        names |= {f"impl:{h}" for h in re.findall(r"impl ComponentInterface for (\w+)", text)}
    return names


def uid_of(header: str, sdk: pathlib.Path, cls: str):
    """(`PROVIDE_UID` or `PROVIDE_EXT_UID`, constant name, value) for `cls`.

    Read from the header's text: the macro sits in the struct body and names a
    constant declared as `UID(0x...)` near it."""
    text = (sdk / "include" / header).read_text()
    body = re.search(rf"struct {cls}\b[^{{;]*\{{(.*?)\n\}};", text, re.S)
    if not body:
        return None
    use = re.search(r"(PROVIDE_EXT_UID|PROVIDE_UID)\((\w+)\)", body.group(1))
    if not use:
        return None
    value = re.search(rf"\b{use.group(2)}\s*=\s*UID\((0x[0-9a-fA-F]+)\)", text)
    if not value:
        return None
    return use.group(1), use.group(2), int(value.group(1), 16)


def sdk_handles() -> set[str]:
    """Interface handles the SDK declares with `opaque!`."""
    handles: set[str] = set()
    for path in [*OMP.glob("*.rs"), *OUT.glob("*.rs")]:
        for block in re.findall(r"opaque! \{(.*?)\n\}", path.read_text(), re.S):
            handles |= set(re.findall(r"^\s*pub (I\w+);", block, re.M))
    return handles


# ---------------------------------------------------------------------------
# What clang knows
# ---------------------------------------------------------------------------


def ast_of(header: str, includes: list[str]) -> dict:
    with tempfile.TemporaryDirectory() as tmp:
        probe = pathlib.Path(tmp) / "probe.cpp"
        probe.write_text(f"#include <{header}>\n")
        out = subprocess.run(
            ["clang++", "-std=c++17", "--target=i686-pc-linux-gnu", *includes,
             "-Xclang", "-ast-dump=json", "-fsyntax-only", str(probe)],
            capture_output=True, text=True,
        ).stdout
    return json.loads(out)


def index_ast(tree: dict):
    """Records by name, and enums by name with their underlying type."""
    records: dict[str, dict] = {}
    enums: dict[str, str] = {}

    def walk(node, scope=""):
        kind = node.get("kind")
        name = node.get("name")
        if kind == "CXXRecordDecl" and name and node.get("inner"):
            records.setdefault(name, node)
        if kind == "EnumDecl" and name:
            underlying = node.get("fixedUnderlyingType", {}).get("qualType", "int")
            enums[name] = underlying
            if scope:
                enums[f"{scope}::{name}"] = underlying
        child_scope = name if kind in ("NamespaceDecl", "CXXRecordDecl") and name else scope
        for child in node.get("inner", ()):
            walk(child, child_scope)

    walk(tree)
    return records, enums


def primary_chain(records: dict, leaf: str) -> list[str]:
    """`leaf` and the bases laid out at offset 0 of it, root last."""
    chain = []
    name = leaf
    while name and name in records:
        chain.append(name)
        bases = records[name].get("bases") or []
        if not bases:
            break
        name = re.sub(r"<.*", "", bases[0]["type"]["qualType"]).strip()
    return chain


def methods_of(record: dict) -> list[dict]:
    """The pure virtual methods `record` declares, with named parameters."""
    out = []
    for member in record.get("inner", ()):
        if member.get("kind") != "CXXMethodDecl" or not member.get("pure"):
            continue
        signature = member["type"]["qualType"]
        ret = signature.partition("(")[0].strip()
        params = [
            (p.get("name") or f"arg{i}", p["type"]["qualType"])
            for i, p in enumerate(q for q in member.get("inner", ()) if q.get("kind") == "ParmVarDecl")
        ]
        variadic = signature.partition("(")[2].rpartition(")")[0].rstrip().endswith("...")
        out.append({"name": member["name"], "ret": ret, "params": params, "variadic": variadic})
    return out


def slot_tables(header: str, leaf: str, includes: list[str], msvc_includes: list[str] | None):
    """{class: {"name(types)": slot}} for both ABIs, from one stub of `leaf`."""
    with tempfile.TemporaryDirectory() as tmp:
        work = pathlib.Path(tmp)
        probe = work / "probe.cpp"
        probe.write_text(f"#include <{header}>\n")
        stub = work / "stub.cpp"
        stub.write_text(ov.stub_source(header, leaf, ov.pure_virtuals(probe, includes, leaf)))

        def dump(flags, target):
            text = subprocess.run(
                ["clang++", "-std=c++17", *flags, f"--target={target}", "-Xclang", "-fdump-vtable-layouts",
                 "-Xclang", "-emit-llvm-only", "-w", "-c", str(stub)],
                capture_output=True, text=True,
            ).stdout
            tables: dict[str, dict[str, int]] = {}
            for block in re.finditer(r"V(?:F)?Table indices for '([^']+)' \(\d+ entries\)\.\n(.*?)(?:\n\n|\Z)", text, re.S):
                entries = {}
                for line in block.group(2).splitlines():
                    m = re.match(r"\s*(\d+) \| (.*)", line)
                    if m and "~" not in m.group(2):
                        entries[key_of(m.group(2))] = int(m.group(1))
                tables[block.group(1)] = entries
            return tables

        itanium = dump(includes, "i686-pc-linux-gnu")
        msvc = dump(msvc_includes, "i686-pc-windows-msvc") if msvc_includes else {}
        if msvc_includes and not msvc:
            sys.exit(f"clang produced no MSVC layout for {leaf}")
    return itanium, msvc


def component_offset(header: str, cls: str, flags: list[str], target: str) -> int:
    """Offset of the `IComponent` subobject inside `cls`, from clang's record layout."""
    with tempfile.TemporaryDirectory() as tmp:
        probe = pathlib.Path(tmp) / "layout.cpp"
        probe.write_text(f'#include <{header}>\nstatic_assert(sizeof({cls}) > 0, "");\n')
        dump = subprocess.run(
            ["clang++", "-std=c++17", *flags, f"--target={target}", "-Xclang", "-fdump-record-layouts",
             "-fsyntax-only", "-w", str(probe)],
            capture_output=True, text=True,
        ).stdout
    inside = False
    for line in dump.splitlines():
        if re.search(rf"\|\s*struct {cls}$", line):
            inside = True
        elif inside and "Dumping AST Record Layout" in line:
            break
        elif inside:
            m = re.match(r"\s*(\d+) \|\s+struct IComponent \(", line)
            if m:
                return int(m.group(1))
    sys.exit(f"no IComponent subobject found in {cls} for {target}")


def key_of(signature: str) -> str:
    """`void IBaseObject::setModel(int)` -> `setModel(int)`, spacing removed."""
    m = re.search(r"(\w+)\((.*)\)", signature)
    return f"{m.group(1)}({m.group(2).replace(' ', '')})" if m else signature


# ---------------------------------------------------------------------------
# From C++ types to Rust
# ---------------------------------------------------------------------------


def strip_type(cpp: str) -> str:
    return re.sub(r"\b(const|volatile|struct|enum)\b", "", cpp).strip()


def param_type(cpp: str, enums: dict, handles: set) -> str:
    bare = strip_type(cpp)
    if bare.endswith("&") or bare.endswith("*"):
        target = bare.rstrip("&*").strip()
        if target in handles:
            return f"*mut {target}"
        raise Skip(f"takes `{cpp}`, which the SDK does not mirror")
    if bare in INTEGERS:
        return INTEGERS[bare]
    if bare in enums:
        return INTEGERS.get(strip_type(enums[bare]), "i32")
    if bare in VALUES:
        return bare
    raise Skip(f"takes `{cpp}`")


def return_kind(cpp: str, enums: dict, handles: set):
    """(rust type, how, neutral) where `how` picks the call form."""
    bare = strip_type(cpp)
    if bare == "void":
        return None, "plain", None
    if bare.endswith("&") or bare.endswith("*"):
        target = bare.rstrip("&*").strip()
        if target in handles:
            return f"*mut {target}", "plain", "std::ptr::null_mut()"
        raise Skip(f"returns `{cpp}`, which the SDK does not mirror")
    if bare in INTEGERS:
        rust = INTEGERS[bare]
        return rust, "plain", NEUTRAL.get(rust, "0")
    if bare in enums:
        rust = INTEGERS.get(strip_type(enums[bare]), "i32")
        return rust, "plain", NEUTRAL.get(rust, "0")
    if bare in VALUES:
        size, neutral = VALUES[bare]
        if size <= 8:
            return bare, "small", None
        return bare, "plain", neutral
    raise Skip(f"returns `{cpp}`")


# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------


def generate(entry: dict, sdk, includes, msvc_includes, taken: set, handles: set, consts: set, declared: set) -> str:
    header, leaf, handle, prefix = entry["header"], entry["class"], entry["handle"], entry["prefix"]
    arg = entry.get("this", prefix)
    skip = set(entry.get("skip", ()))

    records, enums = index_ast(ast_of(header, includes))
    chain = [c for c in primary_chain(records, leaf) if c not in NOT_OURS]
    itanium, msvc = slot_tables(header, leaf, includes, msvc_includes)

    # Overloads anywhere in the chain are left to hand-written code: their MSVC
    # order is reversed, and a generated name would have to invent a suffix.
    counts: dict[str, int] = {}
    for cls in chain:
        for m in methods_of(records[cls]):
            counts[m["name"]] = counts.get(m["name"], 0) + 1

    slots, fns, notes = [], [], []
    for cls in reversed(chain):
        for m in methods_of(records[cls]):
            shown = [t for _, t in m["params"]] + (["..."] if m["variadic"] else [])
            signature = f"{m['ret']} {cls}::{m['name']}({', '.join(shown)})"
            name = rust_name(prefix, m["name"])
            try:
                if m["variadic"]:
                    raise Skip("variadic — a C-style `...` cannot go through a typed wrapper")
                if m["name"] in skip:
                    raise Skip("covered by a hand-written wrapper under another name")
                if counts[m["name"]] > 1:
                    raise Skip("overloaded")
                if name in taken:
                    raise Skip(f"`{name}` is written by hand")
                key = key_of(signature)

                # When the stub overrides everything the interface declares and
                # adds nothing, clang files the index table under the stub.
                def lookup(tables):
                    found = tables.get(cls, {}).get(key)
                    return found if found is not None else tables.get("__Stub", {}).get(key)

                slot_i = lookup(itanium)
                slot_m = lookup(msvc) if msvc else slot_i
                if slot_i is None or slot_m is None:
                    raise Skip("clang reported no slot")
                params = [(param_name(n, arg), param_type(t, enums, handles)) for n, t in m["params"]]
                ret, how, neutral = return_kind(m["ret"], enums, handles)
                if how == "small" and params:
                    raise Skip("returns a small struct and takes arguments")
            except Skip as why:
                notes.append(f"// skipped: `{signature}` — {why}")
                continue

            const = "SLOT_" + snake(m["name"]).upper()
            slots.append(f"    /// `{signature}`")
            slots.append(f"    {const}: usize = {slot_i}, {slot_m};")
            doc = [
                f"    /// `{signature}`.",
                "    ///",
                "    /// # Safety",
                f"    /// `{arg}` must be a live `{handle}`.",
            ]
            if how == "small":
                body_ret = "Option<String>" if ret == "StringView" else f"Option<{ret}>"
                empty = "StringView::EMPTY" if ret == "StringView" else f"{ret}::default()"
                call = (f"call_vtable_small_struct!({arg}.cast::<u8>(), 0, {const}, {ret}, {empty})")
                # The macro carries its own `unsafe` blocks; only the copy out of
                # the server's memory needs one here.
                body = (f"    let view = {call}?;\n    unsafe {{ view.to_owned_string() }}"
                        if ret == "StringView" else f"    {call}")
                fns.append(("plain", "\n".join(d[4:] for d in doc)
                            + f"\n#[must_use]\npub unsafe fn {name}({arg}: *mut {handle}) -> {body_ret} {{\n{body}\n}}"))
            else:
                tail = (f" -> {ret}" if ret else "") + f" = [0, {const}]" + (f" or {neutral}" if ret else "") + ";"
                line = wrap_params(f"pub fn {name}", [f"{arg}: {handle}", *(f"{n}: {t}" for n, t in params)], tail)
                attrs = ["    #[must_use]"] if ret else []
                fns.append(("macro", "\n".join(doc + attrs + [line])))
            taken.add(name)

    out = [
        f"//! `{leaf}` — generated by `scripts/omp-wrappers.py` from `{header}`.",
        f"//! Implemented by `{entry['impl']}` in the server's `{entry['library']}`.",
        "//!",
        "//! Do not edit: change `scripts/omp-wrappers.toml` or the generator and",
        "//! regenerate. Slots are clang's layout of the header for each ABI, checked",
        "//! against the official binaries by `scripts/check-abi-slots.py --generated`.",
        "",
        "#![allow(unused_imports)]",
        "",
        "use crate::omp::types::{Colour, StringView, UID, Vector2, Vector3, Vector4};",
        "use crate::omp::vtable::{call_vtable_small_struct, opaque, slots, virtual_fns};",
        "use crate::omp::*;",
        "",
    ]
    # The handle, when no other module declares it.
    if handle not in declared:
        out += ["opaque! {", f"    /// Opaque handle for `{handle}*`.", f"    pub {handle};", "}", ""]
        declared.add(handle)

    # The UID that finds it: a component through `omp_query::<Component<_>>()`,
    # a per-player extension through `player_extension`.
    uid = uid_of(header, sdk, leaf)
    if uid is not None:
        macro, cpp_name, value = uid
        # Named after the prefix rather than the header's constant: the headers
        # are not consistent (`SomePlayerData_UID` for the vehicles' player
        # data), and the prefix rule reproduces the names the SDK already had
        # (`OBJECTS_COMPONENT_UID`), so an existing constant is recognised
        # instead of duplicated under a second name.
        const = f"{prefix.upper()}_COMPONENT_UID" if macro == "PROVIDE_UID" else f"{prefix.upper()}_UID"
        if const not in consts:
            out += [f"/// `{cpp_name}` in `{header}`.", f"pub const {const}: UID = {value:#018x};", ""]
            consts.add(const)
        if macro == "PROVIDE_UID" and f"impl:{handle}" not in consts:
            ita = component_offset(header, leaf, includes, "i686-pc-linux-gnu")
            mso = component_offset(header, leaf, msvc_includes, "i686-pc-windows-msvc") if msvc_includes else ita
            impl = [f"impl ComponentInterface for {handle} {{", f"    const UID: UID = {const};"]
            if ita or mso:
                impl += [
                    "    // `IComponent` is not this interface's first base.",
                    '    #[cfg(not(target_env = "msvc"))]',
                    f"    const COMPONENT_OFFSET: isize = {ita};",
                    '    #[cfg(target_env = "msvc")]',
                    f"    const COMPONENT_OFFSET: isize = {mso};",
                ]
            out += [*impl, "}", ""]
        elif macro == "PROVIDE_EXT_UID":
            records_bases = (records[leaf].get("bases") or [{}])[0].get("type", {}).get("qualType", "")
            if records_bases.strip() == "IExtension" and prefix not in taken:
                out += [
                    f"/// The player's `{leaf}`, or null when the component providing it is not loaded.",
                    "///",
                    "/// Found by walking the player's extension map for `" + const + "` — the",
                    "/// virtual `getExtension` does not consult that map. `IExtension`",
                    f"/// is `{leaf}`'s first base, so the extension pointer is the interface pointer.",
                    "///",
                    "/// # Safety",
                    "/// `player` must be a live `IPlayer`.",
                    "#[must_use]",
                    f"pub unsafe fn {prefix}(player: *mut IPlayer) -> *mut {handle} {{",
                    f"    unsafe {{ extension(player.cast::<u8>(), {const}) }}.cast::<{handle}>()",
                    "}",
                    "",
                ]
                taken.add(prefix)
            else:
                notes.append(f"// skipped: a `player` accessor — `IExtension` is not `{leaf}`'s first base")

    if slots:
        out += ["slots! {", *slots, "}", ""]
    macro = [body for kind, body in fns if kind == "macro"]
    if macro:
        out += ["virtual_fns! {", "\n\n".join(macro), "}", ""]
    for kind, body in fns:
        if kind == "plain":
            out += [body, ""]
    if notes:
        out += ["// What the generator left out, and why.", *notes, ""]
    return "\n".join(out)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--check", action="store_true", help="fail if the committed files differ")
    parser.add_argument("--only", help="generate one module")
    parser.add_argument("--sdk", type=pathlib.Path)
    parser.add_argument("--xwin", type=pathlib.Path)
    args = parser.parse_args()

    spec = tomllib.loads((REPO / "scripts/omp-wrappers.toml").read_text())
    sdk = ov.find_sdk(args.sdk)
    xwin = ov.find_xwin(args.xwin)
    if xwin is None:
        sys.exit("the MSVC column needs the Windows headers: run `cargo xwin build` once")
    includes = ov.include_flags(sdk)

    taken = sdk_names()
    consts = sdk_consts()
    # Handles the hand-written SDK declares; every generated module's own
    # handle is added up front, so modules can take each other's.
    declared = {h for h in sdk_handles() if not any((OUT / f"{e['module']}.rs").exists() and
                f"pub {h};" in (OUT / f"{e['module']}.rs").read_text() for e in spec["interface"])}
    handles = declared | {e["handle"] for e in spec["interface"]}
    stale = []
    with tempfile.TemporaryDirectory() as tmp:
        msvc_includes = includes + ov.msvc_flags(xwin, pathlib.Path(tmp) / "casefix")
        modules = []
        for entry in spec["interface"]:
            if args.only and entry["module"] != args.only:
                continue
            text = rustfmt(generate(entry, sdk, includes, msvc_includes, taken, handles, consts, declared))
            path = OUT / f"{entry['module']}.rs"
            modules.append(entry["module"])
            if args.check:
                if not path.exists() or path.read_text() != text:
                    stale.append(path.name)
            else:
                OUT.mkdir(exist_ok=True)
                path.write_text(text)
                print(f"wrote {path.relative_to(REPO)}")
    index = rustfmt("\n".join([
        "//! Wrappers generated by `scripts/omp-wrappers.py` — one module per interface",
        "//! listed in `scripts/omp-wrappers.toml`. Do not edit.",
        "",
        *(f"pub mod {m};" for m in modules),
        "",
        # A module whose every method was skipped still exists, for its list of
        # what was left out; there is nothing in it to re-export.
        *(f"pub use {m}::*;" for m in modules if re.search(r"^(pub |impl |opaque!|virtual_fns!)",
                                                          (OUT / f"{m}.rs").read_text(), re.M)),
        "",
    ]))
    if not args.only:
        index_path = OUT / "mod.rs"
        if args.check:
            if not index_path.exists() or index_path.read_text() != index:
                stale.append(index_path.name)
        else:
            index_path.write_text(index)
    if args.check:
        if stale:
            print("stale:", ", ".join(stale))
            return 1
        print("generated wrappers are current")
    return 0


if __name__ == "__main__":
    sys.exit(main())
