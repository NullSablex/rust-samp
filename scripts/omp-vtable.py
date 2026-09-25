#!/usr/bin/env python3
"""Ask clang for the vtable layout of an open.mp interface, in both ABIs.

Reading slot indices out of the shipped binaries works, but only Linux keeps
its symbols — the Windows server carries RTTI for three classes and nothing
else, so MSVC indices had to be derived by hand from the ABI rules and then
proven by running a server.

The headers are public, and clang implements both ABIs. So instead of reading
binaries, this asks the compiler: it generates a stub deriving from the
interface, compiles it twice (`i686-pc-linux-gnu` and `i686-pc-windows-msvc`)
with `-fdump-vtable-layouts`, and prints the slot each method lands on.

    scripts/omp-vtable.py IPlayerPool
    scripts/omp-vtable.py IPlayer --header player.hpp

The answer is what the compiler would emit, which is what the server was built
with. It still does not replace running a server: the server may have been
built from a different revision of the headers. Treat this as the map and the
server as the proof.

Requires clang, the open.mp SDK sources, and (for the MSVC side) the Windows
headers cargo-xwin already downloads.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import subprocess
import sys
import tempfile

HOME = pathlib.Path.home()
SDK = HOME / "Documentos/Projetos/Github/NullSablex/open-sa/reference/openmultiplayer/open.mp-sdk"
XWIN = HOME / ".cache/cargo-xwin/xwin"


def include_flags(sdk: pathlib.Path) -> list[str]:
    lib = sdk / "lib"
    return [
        f"-I{sdk / 'include'}",
        f"-I{lib / 'glm'}",
        f"-I{lib / 'robin-hood-hashing/src/include'}",
        f"-I{lib / 'span-lite/include'}",
        f"-I{lib / 'string-view-lite/include'}",
    ]


def msvc_flags(xwin: pathlib.Path, casefix: pathlib.Path) -> list[str]:
    """Windows headers, plus a directory of case-corrected symlinks.

    The SDK includes `Winsock2.h`; the downloaded kit ships `winsock2.h`. On a
    case-sensitive filesystem that is a missing file, so the links bridge it.
    """
    casefix.mkdir(parents=True, exist_ok=True)
    for wanted, actual in (("Winsock2.h", "winsock2.h"),):
        link = casefix / wanted
        target = xwin / "sdk/include/um" / actual
        if target.exists() and not link.exists():
            link.symlink_to(target)
    return [
        f"-I{casefix}",
        f"-isystem{xwin / 'crt/include'}",
        f"-isystem{xwin / 'sdk/include/ucrt'}",
        f"-isystem{xwin / 'sdk/include/um'}",
        f"-isystem{xwin / 'sdk/include/shared'}",
        "-fms-compatibility",
        "-fms-extensions",
    ]


def pure_virtuals(source: pathlib.Path, includes: list[str], class_name: str) -> list[str]:
    """Every pure virtual `class_name` must implement, its bases included.

    A stub that only overrides the class's own pure virtuals stays abstract:
    these interfaces inherit from others (`IExtensible`, `IReadOnlyPool<T>`),
    and whatever those leave pure has to be overridden too."""
    ast = subprocess.run(
        ["clang++", "-std=c++17", *includes, "-Xclang", "-ast-dump=json", "-fsyntax-only", str(source)],
        capture_output=True,
        text=True,
    ).stdout
    tree = json.loads(ast)

    # Index every record in the translation unit, so bases can be looked up by
    # name — including template specializations such as `IReadOnlyPool<IPlayer>`.
    records: dict[str, dict] = {}

    def key_of(node) -> str | None:
        """`IReadOnlyPool<IPlayer>` for a specialization, the plain name otherwise.

        Indexing a template by its bare name would hand back the uninstantiated
        declaration, whose methods still mention `T` — useless for generating a
        stub."""
        name = node.get("name")
        if not name:
            return None
        if node.get("kind") == "ClassTemplateSpecializationDecl":
            args = [
                a.get("type", {}).get("qualType", "")
                for a in node.get("inner", ())
                if a.get("kind") == "TemplateArgument"
            ]
            if args:
                return f"{name}<{','.join(args)}>".replace(" ", "")
        return name

    def index(node) -> None:
        kind = node.get("kind")
        if kind in ("CXXRecordDecl", "ClassTemplateSpecializationDecl") and node.get("inner"):
            key = key_of(node)
            if key:
                records.setdefault(key, node)
        for child in node.get("inner", ()):
            index(child)

    index(tree)
    if class_name not in records:
        sys.exit(f"class {class_name} not found in the AST")

    methods: list[str] = []
    seen: set[str] = set()

    def collect(record: dict) -> None:
        for base in record.get("bases", ()):
            qual = base.get("type", {}).get("qualType", "").replace(" ", "")
            base_record = records.get(qual) or records.get(re.sub(r"<.*", "", qual))
            if base_record is not None:
                collect(base_record)

        for member in record.get("inner", ()):
            if member.get("kind") != "CXXMethodDecl" or not member.get("pure"):
                continue
            signature = member["type"]["qualType"]  # e.g. "void (IPlayer &) const"
            name = member["name"]
            head, _, tail = signature.partition("(")
            args, _, qualifiers = tail.rpartition(")")
            rendered = f"{head.strip()} {name}({args}) {qualifiers.strip()}".strip()
            if rendered not in seen:
                seen.add(rendered)
                methods.append(rendered)

    collect(records[class_name])
    return methods


def stub_source(header: str, class_name: str, methods: list[str]) -> str:
    # `__builtin_trap()` stands in for a body: the stub is never run, it exists
    # so the compiler has a concrete class whose vtable it must lay out.
    overrides = "\n".join(f"    {m} override {{ __builtin_trap(); }}" for m in methods)
    return (
        f"#include <{header}>\n"
        f"struct __Stub final : {class_name} {{\n{overrides}\n}};\n"
        f"__Stub __stub_instance;\n"
    )


def layout(source: pathlib.Path, flags: list[str], target: str, class_name: str) -> list[str]:
    """Method names in slot order, as clang lays the class out for `target`."""
    dump = subprocess.run(
        [
            "clang++", "-std=c++17", *flags, f"--target={target}",
            "-Xclang", "-fdump-vtable-layouts", "-Xclang", "-emit-llvm-only",
            "-w", "-c", str(source),
        ],
        capture_output=True,
        text=True,
    ).stdout

    # The "indices" block lists the slots as a caller would index them, which is
    # what a vtable wrapper needs — the layout block above it counts RTTI and
    # offset-to-top entries that no call goes through.
    pattern = re.compile(
        rf"V(?:F)?Table indices for '{re.escape(class_name)}' \((\d+) entries\)\.\n(.*?)(?:\n\n|\Z)",
        re.S,
    )
    match = pattern.search(dump)
    if not match:
        return []
    slots: list[str] = []
    for line in match.group(2).splitlines():
        entry = re.match(r"\s*(\d+) \| (.*)", line)
        if entry:
            index = int(entry.group(1))
            while len(slots) < index:
                slots.append("")
            slots.append(entry.group(2).strip())
    return slots


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("class_name")
    parser.add_argument("--header", default="player.hpp", help="header declaring it")
    parser.add_argument("--sdk", type=pathlib.Path, default=SDK)
    parser.add_argument("--xwin", type=pathlib.Path, default=XWIN)
    args = parser.parse_args()

    includes = include_flags(args.sdk)
    with tempfile.TemporaryDirectory() as tmp:
        work = pathlib.Path(tmp)
        probe = work / "probe.cpp"
        probe.write_text(f"#include <{args.header}>\n")
        methods = pure_virtuals(probe, includes, args.class_name)

        stub = work / "stub.cpp"
        stub.write_text(stub_source(args.header, args.class_name, methods))

        itanium = layout(stub, includes, "i686-pc-linux-gnu", args.class_name)
        msvc_available = (args.xwin / "crt/include").is_dir()
        msvc = (
            layout(stub, includes + msvc_flags(args.xwin, work / "casefix"),
                   "i686-pc-windows-msvc", args.class_name)
            if msvc_available
            else []
        )

    if not itanium:
        sys.exit(f"clang emitted no vtable for {args.class_name}")

    # Match by method signature: the same method sits at different indices.
    msvc_index = {name: i for i, name in enumerate(msvc) if name}
    print(f"{args.class_name}  (Itanium / MSVC)")
    for index, name in enumerate(itanium):
        if not name:
            continue
        other = msvc_index.get(name)
        shown = str(other) if other is not None else "-"
        print(f"  {index:3} / {shown:>3}   {name}")
    if not msvc_available:
        print("\n(no MSVC headers under", args.xwin, "— run `cargo xwin build` once)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
