# VM Debugging

The SDK exposes the low-level primitives a debugger needs to inspect a
running AMX VM: read its registers, read/write cells in the data segment
with the same bounds checking as `amx_GetAddr`, install a debug hook, and
decode the `AMX_DBG` debug info the Pawn compiler emits.

These are the building blocks behind tools like the
[PawnPro Debugger](https://github.com/NullSablex/PawnPro-Debugger); before
they existed in the SDK, such tools had to hand-poke the
`#[repr(C, packed)]` `AMX` struct themselves.

## Register accessors

`Amx` reads the VM registers safely (each is an unaligned read of the
packed struct). All return `None` when the `Amx` wraps a null pointer.

| Method        | Register | Meaning                                            |
| ------------- | -------- | -------------------------------------------------- |
| `amx.cip()`   | `cip`    | Current instruction pointer (code-segment offset). |
| `amx.frame()` | `frm`    | Frame pointer; locals/args are addressed from it.  |
| `amx.stack()` | `stk`    | Stack pointer.                                     |
| `amx.heap()`  | `hea`    | Heap pointer.                                      |
| `amx.stp()`   | `stp`    | Top of the stack — upper bound of the data space.  |
| `amx.pri()`   | `pri`    | Primary accumulator — the operand the next opcode acts on. |
| `amx.alt()`   | `alt`    | Alternate accumulator — e.g. the divisor of the division opcodes. |

## Cell access

`Amx::read_cell` / `Amx::write_cell` resolve a data-segment address with
the same validation as `amx_GetAddr`: an address is rejected when it falls
in the free region between heap and stack, is negative, or is past the top
of the stack.

```rust
// Read a global/local cell by its effective data address.
if let Some(value) = amx.read_cell(addr) {
    // ...
}

// Edit a variable while the VM is paused.
let ok: bool = amx.write_cell(addr, new_value);
```

Unlike [`get_ref`](cells-and-memory.md), these work inside a debug hook,
where there is no native call context. They read/write byte-wise, so they
make no alignment assumptions.

## Reading the code segment

`Amx::read_code(offset)` reads a 32-bit cell from the **code** segment — the
instruction-side counterpart of `read_cell`. It resolves `base + header.cod +
offset` and validates the offset against the code segment `[0, header.dat -
header.cod)`, returning `None` when out of range or the VM is null.

```rust
// `cip` is a code-segment offset; read the raw cell of the next instruction.
if let Some(raw) = amx.read_code(cip) {
    // ...
}
```

### Decoding an opcode

On a server built with computed-goto threading (GCC/Clang — the SA-MP and
open.mp builds), the loader rewrites each opcode in the code segment to the
**address** of its handler label. So a `read_code` there yields a pointer, not
the opcode number. `Amx::opcode_table(count)` returns the VM's dispatch table
(`amx_opcodelist`) — `count` raw label addresses in opcode order — fetched the
way the loader itself does it (set the `BROWSE` flag, call `amx_Exec` with index
`0`, restore the flags). Invert that table (address → opcode) to recover the real
opcode behind a `read_code` value:

```rust
use std::collections::HashMap;

// Build once per VM (e.g. in on_amx_load). OP_NUM_OPCODES is the VM's opcode
// count, which the caller supplies (the SDK does not hardcode it).
let table = amx.opcode_table(OP_NUM_OPCODES).unwrap_or_default();
let inverse: HashMap<usize, i32> =
    table.iter().enumerate().map(|(op, &addr)| (addr, op as i32)).collect();

// In the hook: raw code value → opcode number.
let raw = amx.read_code(cip)?;
let opcode = inverse.get(&(raw as usize)).copied()
    .or(Some(raw)); // non-relocated image: the value is already the opcode
```

`opcode_table` does not consult the `AMX_FLAG_RELOC` header bit — it may not be
visible at `AmxLoad` time even though the table is already available. On a
non-computed-goto VM the returned addresses simply never match a real opcode, so
inverting the table is harmless.

Together with `pri()`/`alt()`, this lets a debugger predict a runtime error
*before* the VM aborts it: read the next opcode at `cip`, and if it is a division
(`OP_SDIV`/`OP_UDIV`, divisor in `alt`) or a bounds check (`OP_BOUNDS`, index in
`pri`), pause instead of letting the VM's `ABORT` return without ever calling the
hook again.

## Debug hook

A debug hook fires on every executed source line, provided the `.amx` was
compiled with `-d2`/`-d3`. There are two ways to install one.

### Turnkey: `on_debug_break`

The high-level path routes the hook into your plugin instance. Call
`samp::plugin::enable_debug_hook(amx)` for each AMX you want to debug
(typically the gamemode, in `on_amx_load`), then implement
[`SampPlugin::on_debug_break`]. The SDK owns a panic-guarded trampoline and
dispatches into your plugin — no raw `extern "C"` callback and no global
state of your own.

```rust
use samp::prelude::*;

impl SampPlugin for MyDebugger {
    fn on_amx_load(&mut self, amx: &Amx) {
        samp::plugin::enable_debug_hook(amx);
    }

    fn on_debug_break(&mut self, amx: &Amx) {
        // Runs on the VM thread, on every line — keep it cheap.
        let cip = amx.cip();
        let frm = amx.frame();
        // decide whether to pause, inspect variables, forward to a client...
    }
}
```

Call `samp::plugin::disable_debug_hook(amx)` to stop receiving callbacks.

!!! warning "Runs on the VM thread"
    `on_debug_break` is called synchronously on every executed line. Block
    here (e.g. waiting for a debugger client) only if you intend to freeze
    the server — which is the expected behaviour when single-stepping in a
    local dev session.

### Low-level: `install_debug_hook`

If you want to manage the callback yourself, `Amx::install_debug_hook(cb)`
writes a raw `extern "C"` callback into `amx->debug` (the equivalent of
`amx_SetDebugHook`), and `Amx::remove_debug_hook()` restores a no-op. The
callback crosses the FFI boundary, so it must never unwind.

## AMX_DBG parser (feature `debug`)

The `samp::debug` module decodes the debug block `pawncc -d2`/`-d3` appends
to the `.amx`, mapping a code address to source line, file, symbol and
function. It is pure logic with no extra dependencies, gated behind the
`debug` feature:

```toml
samp = { version = "3", features = ["debug"] }
```

```rust
use samp::debug::AmxDbg;

let bytes = std::fs::read("gamemode.amx")?;
let dbg = AmxDbg::from_amx(&bytes)?; // or AmxDbg::parse(&debug_block)

let line = dbg.lookup_line(addr);                 // address → source line
let file = dbg.lookup_file(addr);                 // address → source file
let func = dbg.lookup_function(addr);             // address → function name
let addr = dbg.line_to_address(line, Some(file)); // line → breakpoint address
let syms = dbg.symbols_in_scope(cip);             // variables visible at cip
```

The same parser runs host-side too: a DAP adapter (a non-`samp` binary) can
depend on `rust-samp-sdk` with `default-features = false, features =
["debug"]` to share a single source of truth for the format.

### Inspecting a variable

Combine the parser with the cell accessors. `DbgSymbol::effective_address`
resolves the address for you (global → absolute; local/argument → relative
to `frm`), so you just read the cell:

```rust
for sym in dbg.symbols_in_scope(cip) {
    if sym.is_array() {
        continue; // arrays hold a base address, not a scalar value
    }
    let value = amx.read_cell(sym.effective_address(frm));
    // interpret `value` according to sym.tag (Float bits, bool, integer...)
}
```
