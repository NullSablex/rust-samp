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
import functools
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
    "size_t": "usize", "std::size_t": "usize",
    # `long` is 32 bits on i686 under both ABIs.
    "long": "i32", "unsigned long": "u32", "double": "f64",
}

# Value types the SDK mirrors, with their size (for the return rule) and the
# neutral value a getter answers with when the object is not there.
VALUES = {
    "Vector2": (8, None),
    "Vector3": (12, "Vector3::ZERO"),
    "Vector4": (16, "Vector4::ZERO"),
    "Colour": (4, None),
    "StringView": (8, None),
    # `std::chrono` durations: a class holding the count, so returned through
    # a hidden pointer like the structs above. Minutes and hours are 4 bytes
    # under MSVC, still within the rule.
    "Milliseconds": (8, None),
    "Seconds": (8, None),
    "Minutes": (8, None),
    "Hours": (8, None),
    # Written by hand in the SDK.
    "GangZonePos": (16, None),
    "GTAQuat": (16, None),
    "SemanticVersion": (6, None),
}

# Structs mirrored from the headers by `mirror`, name -> what `structs.rs` needs.
MIRRORED: dict[str, dict] = {}
# Structs `mirror` gave up on, name -> why.
NOT_MIRRORED: dict[str, str] = {}

NEUTRAL = {"bool": "false", "f32": "0.0", "f64": "0.0"}


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
        # A typedef of an integer reads as that integer, as an enum does.
        if kind in ("TypedefDecl", "TypeAliasDecl") and name:
            target = strip_type(node.get("type", {}).get("qualType", ""))
            if target in INTEGERS and INTEGERS[target] not in ("bool", "f32", "f64"):
                enums.setdefault(name, target)
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
    mirror(bare.rstrip("&*").strip())
    if bare.endswith("&") or bare.endswith("*"):
        target = bare.rstrip("&*").strip()
        if target in handles:
            return f"*mut {target}"
        # `const Vector3 &` is a pointer to one the callee only reads; a Rust
        # reference is that pointer. Without `const` the callee writes through
        # it: an out-parameter, `&mut`.
        if bare.endswith("&") and "&" not in target:
            constant = cpp.lstrip().startswith("const ")
            if target in VALUES:
                return f"&{target}" if constant else f"&mut {target}"
            if target in INTEGERS and not constant:
                return f"&mut {INTEGERS[target]}"
        raise Skip(f"takes `{cpp}`, which the SDK does not mirror")
    if bare in INTEGERS:
        return INTEGERS[bare]
    if bare in enums:
        return INTEGERS.get(strip_type(enums[bare]), "i32")
    if bare in VALUES:
        return bare
    raise Skip(f"takes `{cpp}`")


def return_kind(cpp: str, enums: dict, handles: set, chain: list):
    """(rust type, how, neutral) where `how` picks the call form."""
    bare = strip_type(cpp)
    mirror(bare.rstrip("&*").strip())
    if bare == "void":
        return None, "plain", None
    # A setter that returns the object itself, for chaining: the caller already
    # holds it, so the wrapper drops the value.
    if bare.endswith("&") and bare.rstrip("&").strip() in chain:
        return None, "plain", None
    # An event dispatcher: the generic handle, typed by its handler.
    m = re.fullmatch(r"IEventDispatcher<(\w+EventHandler)>\s*&", bare)
    if m:
        handler = handler_of(m.group(1))
        if handler is None:
            raise Skip(f"returns the dispatcher of `{m.group(1)}`, which {NOT_HANDLERS[m.group(1)]}")
        if handler in hand_types():
            raise Skip(f"`{handler}` and its dispatcher are written by hand")
        return f"*mut EventDispatcher<{handler}>", "plain", "std::ptr::null_mut()"
    # A reference to a struct the server owns: the pointer, to read in place.
    if bare.endswith("&") and bare.rstrip("&").strip() in VALUES:
        return f"*const {bare.rstrip('&').strip()}", "plain", "std::ptr::null()"
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
        return bare, "plain", neutral or default_of(bare)
    raise Skip(f"returns `{cpp}`")


# ---------------------------------------------------------------------------
# Structs the headers pass by value, mirrored
# ---------------------------------------------------------------------------

# Set by `generate` for the header in hand: what `mirror` reads the struct from.
CTX: dict = {}

ZERO = {"bool": "false", "f32": "0.0", "f64": "0.0", "Vector2": "Vector2::default()",
        "Vector3": "Vector3::ZERO", "Vector4": "Vector4::ZERO", "Colour": "Colour::default()",
        "GTAQuat": "GTAQuat::IDENTITY",
        **{d: f"{d}(0)" for d in ("Milliseconds", "Seconds", "Minutes", "Hours")}}


def zero_of(rust: str) -> str | None:
    if rust in INTEGERS.values():
        return ZERO.get(rust, "0")
    if rust in ZERO:
        return ZERO[rust]
    m = re.fullmatch(r"\[(.+); (\d+)\]", rust)
    if m:
        inner = zero_of(m.group(1))
        return f"[{inner}; {m.group(2)}]" if inner else None
    if rust in MIRRORED and MIRRORED[rust]["default"]:
        return f"{rust}::default()"
    return None


def default_of(rust: str) -> str:
    """An expression for a value of `rust` to stand in when there is none."""
    if rust in MIRRORED and not MIRRORED[rust]["default"]:
        # Every mirrored field is an integer, a float, a bool or a struct of
        # them: all zero is a valid value.
        return f"unsafe {{ std::mem::zeroed::<{rust}>() }}"
    if rust == "SemanticVersion":
        return "SemanticVersion::new(0, 0, 0)"
    return f"{rust}::default()"


def literal(node: dict) -> str | None:
    """A field's in-class or constructor initializer, when it is a plain literal."""
    kind = node.get("kind")
    if kind in ("ImplicitCastExpr", "ParenExpr", "ConstantExpr", "CXXFunctionalCastExpr", "ExprWithCleanups"):
        inner = node.get("inner") or []
        return literal(inner[0]) if len(inner) == 1 else None
    if kind in ("IntegerLiteral", "FloatingLiteral"):
        return str(node["value"])
    if kind == "CXXBoolLiteralExpr":
        return "true" if node["value"] else "false"
    if kind == "UnaryOperator" and node.get("opcode") == "-":
        inner = literal(node["inner"][0])
        return f"-{inner}" if inner else None
    return None


def field_type(qual: str, desugared: str | None) -> str | None:
    enums = CTX["enums"]
    for t in (strip_type(qual), strip_type(desugared or "")):
        if not t:
            continue
        if t in INTEGERS:
            return INTEGERS[t]
        if t in enums:
            return INTEGERS.get(strip_type(enums[t]), "i32")
        if t in VALUES and t != "StringView" and t not in ("GangZonePos",):
            return t
        m = re.fullmatch(r"(?:std::)?array<(.+),\s*(\d+)>", t) or re.fullmatch(r"(.+?)\s*\[(\d+)\]", t)
        if m:
            inner = field_type(m.group(1), None)
            return f"[{inner}; {m.group(2)}]" if inner else None
        if mirror(t):
            return t
    return None


def record_layout(name: str, flags: list[str], target: str) -> tuple[int, list[int]] | None:
    """(size, field offsets) in bytes, from clang's record layout of `name`."""
    with tempfile.TemporaryDirectory() as tmp:
        probe = pathlib.Path(tmp) / "layout.cpp"
        probe.write_text(f'#include <{CTX["header"]}>\nstatic_assert(sizeof({name}) > 0, "");\n')
        dump = subprocess.run(
            ["clang++", "-std=c++17", *flags, f"--target={target}", "-Xclang", "-fdump-record-layouts-simple",
             "-fsyntax-only", "-w", str(probe)],
            capture_output=True, text=True,
        ).stdout
    m = re.search(rf"Type: (?:struct|class) {name}\n\nLayout: <ASTRecordLayout\n\s+Size:(\d+)\n.*?FieldOffsets: \[([^\]]*)\]", dump, re.S)
    if not m:
        return None
    offsets = [int(x) // 8 for x in m.group(2).split(",") if x.strip()]
    return int(m.group(1)) // 8, offsets


@functools.cache
def hand_types() -> frozenset[str]:
    """Types the hand-written SDK already declares — a struct, or an opaque
    handle — which a mirror would shadow."""
    names = set()
    for path in OMP.glob("*.rs"):
        text = path.read_text()
        names |= set(re.findall(r"^pub struct (\w+)", text, re.M))
        # Handlers written through `handler_vtable!`: `XVTable for X {`.
        for vtable, handler in re.findall(r"^\s*(\w+) for (\w+) \{", text, re.M):
            names |= {vtable, handler}
        for block in re.findall(r"opaque! \{(.*?)\n\}", text, re.S):
            names |= set(re.findall(r"^\s*pub (\w+);", block, re.M))
    return frozenset(names)


def mirror(name: str) -> bool:
    """Mirror the struct `name` as a `#[repr(C)]` Rust struct, if every field
    is something the SDK can state with certainty. Registers it in `VALUES`."""
    if name in MIRRORED:
        return True
    if name in NOT_MIRRORED or not CTX or name in hand_types():
        return False
    rec = CTX["records"].get(name)
    if not rec or rec.get("tagUsed") not in ("struct", "class"):
        return False
    if rec.get("definitionData", {}).get("isPolymorphic") or rec.get("definitionData", {}).get("isAbstract"):
        return False  # an interface, held by pointer

    def give_up(why: str) -> bool:
        NOT_MIRRORED[name] = why
        return False

    NOT_MIRRORED[name] = "refers to itself"
    if rec.get("bases"):
        return give_up("has a base class")
    data = rec.get("definitionData", {})
    if not data.get("isTriviallyCopyable") or not data.get("canPassInRegisters"):
        # Itanium passes such a class by hidden reference, not by value.
        return give_up("is not trivially copyable")
    access = "private" if rec["tagUsed"] == "class" else "public"
    fields, inits, ctor_inits = [], {}, {}
    for member in rec.get("inner", ()):
        kind = member.get("kind")
        if kind == "AccessSpecDecl":
            access = member.get("access", access)
        elif kind == "CXXMethodDecl" and member.get("virtual"):
            return give_up("has virtual methods")
        elif kind == "CXXConstructorDecl" and not member.get("isImplicit"):
            params = [q for q in member.get("inner", ()) if q.get("kind") == "ParmVarDecl"]
            if not params:
                for init in member.get("inner", ()):
                    if init.get("kind") == "CXXCtorInitializer" and "anyInit" in init:
                        ctor_inits[init["anyInit"]["name"]] = literal(init["inner"][0]) if init.get("inner") else None
                ctor_inits.setdefault("__user_default__", None)
        elif kind == "FieldDecl":
            if access != "public" or not member.get("name"):
                return give_up("has a private or anonymous member")
            t = member["type"]
            rust = field_type(t["qualType"], t.get("desugaredQualType"))
            if rust is None:
                return give_up(f"has a `{t['qualType']}` member")
            fields.append((member["name"], snake(member["name"]), rust))
            if member.get("hasInClassInitializer"):
                inits[member["name"]] = literal(member["inner"][0]) if member.get("inner") else None
    if not fields:
        return give_up("has no fields")

    ita = record_layout(name, CTX["includes"], "i686-pc-linux-gnu")
    mso = record_layout(name, CTX["msvc_includes"], "i686-pc-windows-msvc")
    if not ita or not mso or len(ita[1]) != len(fields) or len(mso[1]) != len(fields):
        return give_up("clang gave no layout for it")

    # `Default` only where the C++ default is known: in-class or default
    # constructor initializers that are literals, zero for the rest.
    default = []
    for cpp, _, rust in fields:
        given = ctor_inits.get(cpp, inits.get(cpp, "0"))
        if given is None:
            default = None
            break
        if given == "0":
            value = zero_of(rust)
        elif rust in ("f32", "f64"):
            value = given if re.search(r"[.eE]", given) else f"{given}.0"
            value = value.rstrip("fF")
        elif rust == "bool":
            value = {"0": "false", "1": "true"}.get(given, given)
        else:
            value = given
        if value is None:
            default = None
            break
        default.append(value)

    declared_in = CTX["header"]
    for path in sorted((CTX["sdk"] / "include").rglob("*.hpp")):
        if re.search(rf"^(?:struct|class) {name}\b", path.read_text(errors="replace"), re.M):
            declared_in = str(path.relative_to(CTX["sdk"] / "include"))
            break
    MIRRORED[name] = {"header": declared_in, "fields": fields, "default": default,
                      "itanium": ita, "msvc": mso}
    del NOT_MIRRORED[name]
    VALUES[name] = (mso[0], None)
    return True


def render_structs() -> str:
    out = [
        "//! Structs the open.mp headers pass by value — generated by",
        "//! `scripts/omp-wrappers.py`. Do not edit.",
        "//!",
        "//! Each mirrors its C++ struct field for field. The layout is clang's for",
        "//! both ABIs, and the assertions below each struct fail the build if the",
        "//! Rust one differs from it.",
        "",
        "#![allow(unused_imports)]",
        "",
        "use crate::omp::types::{Colour, GTAQuat, Hours, Milliseconds, Minutes, Seconds, Vector2, Vector3, Vector4};",
        "use crate::omp::vtable::VirtualReturn;",
        "use crate::omp::*;",
        "",
    ]
    for name in sorted(MIRRORED):
        info = MIRRORED[name]
        derives = "Debug, Clone, Copy, PartialEq"
        out += [f"/// `{name}` in `{info['header']}`.", "#[repr(C)]", f"#[derive({derives})]", f"pub struct {name} {{"]
        for cpp, rust_field, rust in info["fields"]:
            field = f"r#{rust_field}" if rust_field in RUST_KEYWORDS else rust_field
            out.append(f"    /// `{cpp}`.")
            out.append(f"    pub {field}: {rust},")
        out += ["}", ""]
        if info["default"] is not None:
            out += [f"impl Default for {name} {{", "    /// The C++ struct's own defaults.", "    fn default() -> Self {", "        Self {"]
            for (cpp, rust_field, _), value in zip(info["fields"], info["default"]):
                field = f"r#{rust_field}" if rust_field in RUST_KEYWORDS else rust_field
                out.append(f"            {field}: {value},")
            out += ["        }", "    }", "}", ""]
        for cfg, (size, offsets) in (('not(target_env = "msvc")', info["itanium"]), ('target_env = "msvc"', info["msvc"])):
            out += [f'#[cfg(all(target_arch = "x86", {cfg}))]', "const _: () = {",
                    f"    assert!(std::mem::size_of::<{name}>() == {size});"]
            for (cpp, rust_field, _), off in zip(info["fields"], offsets):
                field = f"r#{rust_field}" if rust_field in RUST_KEYWORDS else rust_field
                out.append(f"    assert!(std::mem::offset_of!({name}, {field}) == {off});")
            out += ["};", ""]
        out += [f"impl VirtualReturn for {name} {{", "    type Raw = Self;", "    fn from_raw(raw: Self) -> Self {", "        raw", "    }", "}", ""]
        out += [f"// msvc-size: {name} = {info['msvc'][0]}", ""]
    if NOT_MIRRORED:
        out += ["// What the generator did not mirror, and why."]
        out += [f"// skipped: `{n}` — {why}" for n, why in sorted(NOT_MIRRORED.items())]
        out.append("")
    return "\n".join(out)


# ---------------------------------------------------------------------------
# Event handlers
# ---------------------------------------------------------------------------

# Handlers written into `handlers.rs`: C++ name -> what the macro needs.
HANDLERS: dict[str, dict] = {}
NOT_HANDLERS: dict[str, str] = {}

# Narrow integers a callback receives: C++ callers need not extend them to a
# word, and a Rust `bool` or `u8` argument assumes they did. The whole word
# comes in, and the value is its low bits — the rule `VirtualReturn` applies to
# return values.
WIDENED = {"bool": "u32", "u8": "u32", "u16": "u32", "i8": "i32", "i16": "i32"}


def callback_arg(cpp: str) -> str | None:
    bare = strip_type(cpp)
    constant = cpp.lstrip().startswith("const ")
    if bare.endswith("&") or bare.endswith("*"):
        target = bare.rstrip("&*").strip()
        mut = "*const" if constant else "*mut"
        if "&" in target or "*" in target or "<" in target:
            return f"{mut} std::ffi::c_void"
        mirror(target)
        if target in CTX["handles"] or target in hand_types() or target in VALUES:
            return f"*mut {target}" if target in CTX["handles"] else f"{mut} {target}"
        if target in INTEGERS:
            return f"{mut} {INTEGERS[target]}"
        return f"{mut} std::ffi::c_void"
    mirror(bare)
    if bare in INTEGERS:
        return WIDENED.get(INTEGERS[bare], INTEGERS[bare])
    if bare in CTX["enums"]:
        rust = INTEGERS.get(strip_type(CTX["enums"][bare]), "i32")
        return WIDENED.get(rust, rust)
    if bare in VALUES:
        return bare
    return None


def body_literal(method: dict) -> str | None:
    """What a handler method's inline body returns: `()` for an empty body,
    the literal of a lone `return`, otherwise nothing."""
    body = next((n for n in method.get("inner", ()) if n.get("kind") == "CompoundStmt"), None)
    if body is None:
        return None
    stmts = body.get("inner", [])
    if not stmts:
        return "()"
    if len(stmts) == 1 and stmts[0].get("kind") == "ReturnStmt" and stmts[0].get("inner"):
        return literal(stmts[0]["inner"][0])
    return None


def handler_of(cpp: str) -> str | None:
    """The Rust handler type for `cpp` (`ObjectEventHandler` -> `ObjectHandler`),
    written into `handlers.rs` unless the SDK has it by hand; `None` when the
    handler cannot be stated with certainty."""
    rust = re.sub(r"EventHandler$", "Handler", cpp)
    if rust in hand_types() or cpp in HANDLERS:
        return rust
    if cpp in NOT_HANDLERS:
        return None
    rec = CTX["records"].get(cpp)

    def give_up(why: str):
        NOT_HANDLERS[cpp] = why
        return None

    if not rec:
        return give_up("not declared where its dispatcher is")
    if rec.get("bases"):
        return give_up("has a base class")
    methods = [m for m in rec.get("inner", ()) if m.get("kind") == "CXXMethodDecl" and m.get("virtual")]
    if any(m.get("kind") == "CXXDestructorDecl" and m.get("virtual") for m in rec.get("inner", ())):
        return give_up("has a virtual destructor")
    names = [m["name"] for m in methods]
    if len(set(names)) != len(names):
        return give_up("overloads a method, which MSVC reorders")
    entries = []
    for m in methods:
        params = [q for q in m.get("inner", ()) if q.get("kind") == "ParmVarDecl"]
        args = [callback_arg(q["type"]["qualType"]) for q in params]
        if None in args:
            return give_up(f"`{m['name']}` takes a value the SDK does not mirror")
        ret_cpp = strip_type(m["type"]["qualType"].partition("(")[0])
        if ret_cpp == "void":
            ret = None
        elif ret_cpp in INTEGERS:
            ret = INTEGERS[ret_cpp]
        else:
            return give_up(f"`{m['name']}` returns `{ret_cpp}`")
        default = body_literal(m)
        if default is None or (ret is None) != (default == "()"):
            return give_up(f"`{m['name']}` has a body that is not a plain literal")
        if ret in ("f32", "f64") and not re.search(r"[.eE]", default):
            default += ".0"
        shown = ", ".join(q["type"]["qualType"] for q in params)
        entries.append((f"{ret_cpp} {m['name']}({shown})", snake(m["name"]), args, ret, default))
    if not entries:
        return give_up("declares no methods")
    declared_in = CTX["header"]
    for path in sorted((CTX["sdk"] / "include").rglob("*.hpp")):
        if re.search(rf"^struct {cpp}\b", path.read_text(errors="replace"), re.M):
            declared_in = str(path.relative_to(CTX["sdk"] / "include"))
            break
    HANDLERS[cpp] = {"rust": rust, "header": declared_in, "entries": entries}
    return rust


def render_handlers() -> str:
    out = [
        "//! Event handlers of the groups the SDK does not write by hand — generated",
        "//! by `scripts/omp-wrappers.py` from the open.mp headers. Do not edit.",
        "//!",
        "//! Each handler's vtable follows its C++ declaration order, which is the",
        "//! slot order on both ABIs: none of these declares a destructor or an",
        "//! overload. `DEFAULT` does what each C++ body does. Narrow integer",
        "//! arguments arrive as a whole word; the value is its low bits.",
        "",
        "#![allow(unused_imports)]",
        "",
        "use crate::omp::dispatch::event_handler;",
        "use crate::omp::types::{Colour, GTAQuat, Hours, Milliseconds, Minutes, Seconds, StringView, Vector2, Vector3, Vector4};",
        "use crate::omp::*;",
        "",
    ]
    for cpp in sorted(HANDLERS):
        info = HANDLERS[cpp]
        out += ["event_handler! {", f"    /// `{cpp}` in `{info['header']}`.", f"    {info['rust']}VTable for {info['rust']} {{"]
        for signature, field, args, ret, default in info["entries"]:
            tail = f" -> {ret}" if ret else ""
            out.append(f"        /// `{signature}`.")
            out.append(f"        {field}: fn({', '.join(args)}){tail} = {default},")
        out += ["    }", "}", ""]
    if NOT_HANDLERS:
        out += ["// What the generator did not write, and why."]
        out += [f"// skipped: `{n}` — {why}" for n, why in sorted(NOT_HANDLERS.items())]
        out.append("")
    return "\n".join(out)


# ---------------------------------------------------------------------------
# Output
# ---------------------------------------------------------------------------


def generate(entry: dict, sdk, includes, msvc_includes, taken: set, handles: set, consts: set, declared: set) -> str:
    header, leaf, handle, prefix = entry["header"], entry["class"], entry["handle"], entry["prefix"]
    arg = entry.get("this", prefix)
    skip = set(entry.get("skip", ()))
    overloads = entry.get("overloads", {})

    records, enums = index_ast(ast_of(header, includes))
    CTX.update(records=records, enums=enums, header=header, sdk=sdk, handles=handles, includes=includes, msvc_includes=msvc_includes)
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
            const = "SLOT_" + snake(m["name"]).upper()
            try:
                if m["variadic"]:
                    raise Skip("variadic — a C-style `...` cannot go through a typed wrapper")
                if m["name"] in skip and key_of(signature) not in overloads:
                    raise Skip("covered by a hand-written wrapper under another name")
                if counts[m["name"]] > 1:
                    # Named one by one in the TOML; clang's layout places each
                    # overload, reversed order under MSVC included.
                    own = overloads.get(key_of(signature))
                    if own is None:
                        raise Skip("overloaded")
                    name = f"{prefix}_{own}"
                    const = "SLOT_" + own.upper()
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
                ret, how, neutral = return_kind(m["ret"], enums, handles, chain)
            except Skip as why:
                notes.append(f"// skipped: `{signature}` — {why}")
                continue

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
                empty = "StringView::EMPTY" if ret == "StringView" else default_of(ret)
                types = ", ".join(t for _, t in params)
                names = ", ".join(n for n, _ in params)
                extra = f", ({types}) ({names})" if params else ""
                call = (f"call_vtable_small_struct!({arg}.cast::<u8>(), 0, {const}, {ret}, {empty}{extra})")
                # The macro carries its own `unsafe` blocks; only the copy out of
                # the server's memory needs one here.
                body = (f"    let view = {call}?;\n    unsafe {{ view.to_owned_string() }}"
                        if ret == "StringView" else f"    {call}")
                fns.append(("plain", "\n".join(d[4:] for d in doc)
                            + f"\n#[must_use]\npub unsafe fn {name}({', '.join([f'{arg}: *mut {handle}', *(f'{n}: {t}' for n, t in params)])}) -> {body_ret} {{\n{body}\n}}"))
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
        "use crate::omp::types::{Colour, GTAQuat, Hours, Milliseconds, Minutes, Seconds, StringView, UID, Vector2, Vector3, Vector4};",
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
        handlers = rustfmt(render_handlers())
        path = OUT / "handlers.rs"
        if not args.only:
            modules.append("handlers")
            if args.check:
                if not path.exists() or path.read_text() != handlers:
                    stale.append(path.name)
            else:
                path.write_text(handlers)
                print(f"wrote {path.relative_to(REPO)}")
        structs = rustfmt(render_structs())
        path = OUT / "structs.rs"
        if not args.only:
            modules.append("structs")
            if args.check:
                if not path.exists() or path.read_text() != structs:
                    stale.append(path.name)
            else:
                path.write_text(structs)
                print(f"wrote {path.relative_to(REPO)}")
    index = rustfmt("\n".join([
        "//! Wrappers generated by `scripts/omp-wrappers.py` — one module per interface",
        "//! listed in `scripts/omp-wrappers.toml`. Do not edit.",
        "",
        *(f"pub mod {m};" for m in modules),
        "",
        # A module whose every method was skipped still exists, for its list of
        # what was left out; there is nothing in it to re-export.
        *(f"pub use {m}::*;" for m in modules if re.search(r"^(pub |impl |opaque!|virtual_fns!|event_handler!)",
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
