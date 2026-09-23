#!/usr/bin/env python3
"""Check the SDK's vtable slot constants against the official server binaries.

Every index the SDK hardcodes is a claim about a binary someone else built. A
wrong one fails quietly: the server returns a plausible value from the wrong
virtual function, and on Windows the stack is corrupted on top of that. Four
such defects shipped in v3.5.0 and were only found by running real servers.

This script re-derives each index from the binaries and compares it with what
the source declares. Run it before a release, and after touching anything under
`samp-sdk/src/omp/`.

    scripts/check-abi-slots.py                        # default install paths
    scripts/check-abi-slots.py --linux DIR --win DIR  # elsewhere

It is not part of CI: it needs the official servers, which cannot be
redistributed.

Itanium (Linux) checks are exact — the `.so` files keep their symbols, so each
slot is identified by name. MSVC (Windows) checks work from RTTI plus the
`ret N` of each slot (N = argument bytes under `thiscall`), which pins the
methods that take arguments and the length of each vtable. Slots that take no
arguments are indistinguishable that way, and are reported as unverifiable
rather than silently assumed.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import struct
import subprocess
import sys

REPO = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_LINUX = pathlib.Path.home() / "Downloads/open.mp-linux-x86/Server"
DEFAULT_WIN = pathlib.Path.home() / "Downloads/open.mp-win-x86/Server"


# --------------------------------------------------------------------------
# Reading the constants the SDK declares
# --------------------------------------------------------------------------
def rust_const(path: str, name: str, msvc: bool) -> int | None:
    """Value of a `const NAME: usize` in `path`, for one ABI.

    Constants are cfg-gated in pairs; the one guarded by
    `#[cfg(target_env = "msvc")]` is the MSVC value and the one guarded by
    `#[cfg(not(...))]` is the Itanium value. An ungated constant answers for
    both.
    """
    source = (REPO / path).read_text()
    pattern = re.compile(
        r'(?:#\[cfg\((?P<cfg>[^\]]*)\)\]\s*\n)?'
        r'(?:pub\s+)?const\s+' + re.escape(name) + r'\s*:\s*\w+\s*=\s*(?P<value>\d+)\s*;'
    )
    ungated = None
    for match in pattern.finditer(source):
        cfg = match.group("cfg")
        value = int(match.group("value"))
        if cfg is None:
            ungated = value
        elif msvc and 'target_env = "msvc"' in cfg and not cfg.startswith("not"):
            return value
        elif not msvc and cfg.startswith("not") and 'target_env = "msvc"' in cfg:
            return value
    return ungated


# --------------------------------------------------------------------------
# ELF (Itanium ABI): vtables with symbols
# --------------------------------------------------------------------------
def elf_vtable(so: pathlib.Path, class_name: str, count: int = 40) -> list[str]:
    """Demangled names of the first `count` slots of `vtable for <class_name>`."""
    symbols: dict[int, str] = {}
    for line in subprocess.run(
        ["nm", "-n", str(so)], capture_output=True, text=True, check=True
    ).stdout.splitlines():
        parts = line.split()
        if len(parts) == 3:
            symbols.setdefault(int(parts[0], 16), parts[2])

    address = None
    for line in subprocess.run(
        ["nm", "-C", str(so)], capture_output=True, text=True, check=True
    ).stdout.splitlines():
        if line.endswith(f"vtable for {class_name}"):
            address = int(line.split()[0], 16)
    if address is None:
        raise LookupError(f"no vtable for {class_name} in {so.name}")

    data = so.read_bytes()
    loads = []
    for line in subprocess.run(
        ["readelf", "-lW", str(so)], capture_output=True, text=True, check=True
    ).stdout.splitlines():
        parts = line.split()
        if len(parts) >= 6 and parts[0] == "LOAD":
            loads.append((int(parts[2], 16), int(parts[1], 16), int(parts[5], 16)))

    offset = next(
        off + (address - vaddr)
        for vaddr, off, memsz in loads
        if vaddr <= address < vaddr + memsz
    )
    # `vtable for X` points at the start of the structure, whose first two
    # entries are offset-to-top and the typeinfo pointer. What an object's vptr
    # holds — and what slot 0 means — starts 8 bytes later.
    offset += 8
    raw = [struct.unpack_from("<I", data, offset + 4 * i)[0] for i in range(count)]
    names = [symbols.get(p, hex(p)) for p in raw]
    return subprocess.run(
        ["c++filt"], input="\n".join(names), capture_output=True, text=True, check=True
    ).stdout.splitlines()


def slot_of(vtable: list[str], needle: str) -> int | None:
    """Index of the first slot whose demangled name contains `needle`."""
    for index, name in enumerate(vtable):
        if needle in name:
            return index
    return None


# --------------------------------------------------------------------------
# PE (MSVC ABI): vtables via RTTI, slots identified by `ret N`
# --------------------------------------------------------------------------
class PortableExecutable:
    def __init__(self, path: pathlib.Path):
        self.path = path
        self.data = path.read_bytes()
        pe = struct.unpack_from("<I", self.data, 0x3C)[0]
        sections = struct.unpack_from("<H", self.data, pe + 6)[0]
        opt_size = struct.unpack_from("<H", self.data, pe + 20)[0]
        self.base = struct.unpack_from("<I", self.data, pe + 24 + 28)[0]
        self.sections = []
        for i in range(sections):
            off = pe + 24 + opt_size + 40 * i
            name = self.data[off : off + 8].rstrip(b"\0").decode(errors="replace")
            vsize, vaddr, rawsize, raw = struct.unpack_from("<IIII", self.data, off + 8)
            self.sections.append((name, vaddr, vsize, raw, rawsize))

    def file_to_virtual(self, offset: int) -> int | None:
        for _, vaddr, _, raw, rawsize in self.sections:
            if raw <= offset < raw + rawsize:
                return self.base + vaddr + (offset - raw)
        return None

    def section_of(self, virtual: int) -> str | None:
        rva = virtual - self.base
        for name, vaddr, vsize, _, _ in self.sections:
            if vaddr <= rva < vaddr + vsize:
                return name
        return None

    def virtual_to_file(self, virtual: int) -> int | None:
        rva = virtual - self.base
        for _, vaddr, vsize, raw, rawsize in self.sections:
            if vaddr <= rva < vaddr + vsize and rva - vaddr < rawsize:
                return raw + (rva - vaddr)
        return None

    def vtable(self, class_name: str, limit: int = 64) -> list[int]:
        """Function pointers of the primary vtable of `class_name`, via RTTI."""
        marker = f".?AV{class_name}@@".encode()
        index = self.data.find(marker)
        if index < 0:
            raise LookupError(f"no RTTI for {class_name} in {self.path.name}")
        descriptor = self.file_to_virtual(index - 8)

        best: list[int] = []
        for match in re.finditer(struct.pack("<I", descriptor), self.data):
            locator = match.start() - 12
            signature, subobject, _ = struct.unpack_from("<III", self.data, locator)
            if signature != 0 or subobject != 0:
                continue  # only the primary vtable (subobject offset 0)
            for reference in re.finditer(
                struct.pack("<I", self.file_to_virtual(locator)), self.data
            ):
                start = reference.start() + 4
                slots = []
                for i in range(limit):
                    pointer = struct.unpack_from("<I", self.data, start + 4 * i)[0]
                    if not pointer or self.section_of(pointer) != ".text":
                        break
                    slots.append(pointer)
                if len(slots) > len(best):
                    best = slots
        if not best:
            raise LookupError(f"vtable for {class_name} not located")
        return best

    def ret_bytes(self, function: int, window: int = 0x600) -> int | None:
        """Argument bytes the function pops: N from its first `ret N` (0 for a bare `ret`).

        Disassembles rather than scanning for the `0xC2`/`0xC3` opcode bytes:
        on x86 those bytes occur inside other instructions all the time, and a
        raw scan reports whatever it hits first.
        """
        result = subprocess.run(
            [
                "objdump", "-d", "-b", "pei-i386", "-m", "i386",
                f"--start-address={function:#x}",
                f"--stop-address={function + window:#x}",
                str(self.path),
            ],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            return None
        for line in result.stdout.splitlines():
            match = re.search(r"\bret\s*(?:\$0x([0-9a-f]+))?\s*$", line)
            if match:
                return int(match.group(1), 16) if match.group(1) else 0
        return None


# --------------------------------------------------------------------------
# The checks
# --------------------------------------------------------------------------
class Report:
    def __init__(self) -> None:
        self.failures = 0
        self.skipped = 0

    def check(self, label: str, expected, found) -> None:
        if found is None:
            print(f"  SKIP  {label}: could not derive it from the binary")
            self.skipped += 1
        elif expected == found:
            print(f"  ok    {label}: {found}")
        else:
            print(f"  FAIL  {label}: source says {expected}, binary says {found}")
            self.failures += 1


def check_itanium(server: pathlib.Path, report: Report) -> None:
    print(f"\nItanium ABI — {server}")
    timers = elf_vtable(server / "components/Timers.so", "TimersComponent")
    report.check(
        "ITimersComponent::create(handler, interval, repeating)",
        rust_const("samp-sdk/src/omp/timers.rs", "SLOT_CREATE_INTERVAL", msvc=False),
        slot_of(timers, "TimersComponent::create(TimerTimeOutHandler*, std::chrono::duration<long long, std::ratio<1ll, 1000ll> >, bool)"),
    )
    report.check(
        "IComponent::componentName",
        rust_const("samp-sdk/src/omp/component_api.rs", "SLOT_COMPONENT_NAME", msvc=False),
        slot_of(timers, "componentName"),
    )
    report.check(
        "IComponent::componentVersion",
        rust_const("samp-sdk/src/omp/component_api.rs", "SLOT_COMPONENT_VERSION", msvc=False),
        slot_of(timers, "componentVersion"),
    )

    timer = elf_vtable(server / "components/Timers.so", "Timer")
    report.check(
        "ITimer::kill",
        rust_const("samp-sdk/src/omp/timers.rs", "SLOT_TIMER_KILL", msvc=False),
        slot_of(timer, "Timer::kill"),
    )

    pawn = elf_vtable(server / "components/Pawn.so", "PawnComponent")
    report.check(
        "IPawnComponent::getEventDispatcher",
        rust_const("samp-sdk/src/omp/server.rs", "PAWN_COMPONENT_PREFIX_SLOTS", msvc=False),
        slot_of(pawn, "getEventDispatcher"),
    )
    script = elf_vtable(server / "components/Pawn.so", "PawnScript", count=70)
    report.check(
        "IPawnScript::GetAMX",
        57,  # ungated in the SDK: no virtual destructor, same on both ABIs
        slot_of(script, "PawnScript::GetAMX"),
    )

    core = elf_vtable(server / "omp-server", "ComponentList")
    report.check(
        "IComponentList::queryComponent",
        rust_const("samp-sdk/src/omp/server.rs", "COMPONENT_LIST_PREFIX_SLOTS", msvc=False),
        slot_of(core, "queryComponent"),
    )


def check_msvc(server: pathlib.Path, report: Report) -> None:
    print(f"\nMSVC ABI — {server}")
    timers_dll = PortableExecutable(server / "components/Timers.dll")
    timers = timers_dll.vtable("TimersComponent")

    # `create(handler, Milliseconds, bool)` pops 4 + 8 + 4 = 16 bytes; the
    # four-argument overload pops 24. MSVC emits an overload set in reverse, so
    # this is exactly the pair that was swapped in v3.5.0.
    expected = rust_const("samp-sdk/src/omp/timers.rs", "SLOT_CREATE_INTERVAL", msvc=True)
    found = next(
        (i for i, f in enumerate(timers) if timers_dll.ret_bytes(f) == 16 and i >= 15),
        None,
    )
    report.check("ITimersComponent::create(handler, interval, repeating)", expected, found)

    # IExtensible::removeExtension(UID) pops 8, removeExtension(IExtension*) 4.
    report.check(
        "IExtensible::removeExtension(UID) at slot 2",
        8,
        timers_dll.ret_bytes(timers[2]) if len(timers) > 2 else None,
    )
    report.check(
        "IExtensible::removeExtension(ptr) at slot 3",
        4,
        timers_dll.ret_bytes(timers[3]) if len(timers) > 3 else None,
    )

    pawn_dll = PortableExecutable(server / "components/Pawn.dll")
    pawn = pawn_dll.vtable("PawnComponent")
    # getEventDispatcher and getAmxFunctions take no arguments, so `ret N`
    # cannot tell them apart from their neighbours. What is checkable is that
    # the vtable is long enough for the index the SDK uses.
    prefix = rust_const("samp-sdk/src/omp/server.rs", "PAWN_COMPONENT_PREFIX_SLOTS", msvc=True)
    report.check(
        f"IPawnComponent vtable reaches slot {prefix + 1}",
        True,
        len(pawn) > prefix + 1,
    )
    print("  note  getEventDispatcher/getAmxFunctions take no arguments — their")
    print("        exact index is not derivable from `ret N`; see docs/internals/omp-abi.md")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--linux", type=pathlib.Path, default=DEFAULT_LINUX)
    parser.add_argument("--win", type=pathlib.Path, default=DEFAULT_WIN)
    args = parser.parse_args()

    report = Report()
    for label, directory, check in (
        ("Linux", args.linux, check_itanium),
        ("Windows", args.win, check_msvc),
    ):
        if not directory.is_dir():
            print(f"\n{label} server not found at {directory} — skipping")
            report.skipped += 1
            continue
        check(directory, report)

    print()
    if report.failures:
        print(f"{report.failures} slot(s) disagree with the binaries.")
        return 1
    print(f"All derivable slots match the official binaries ({report.skipped} skipped).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
