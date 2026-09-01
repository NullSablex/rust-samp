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

To read a range rather than a single cell, `Amx::read_cells(addr, count)`
returns consecutive cells and `Amx::read_bytes(addr, len)` returns raw bytes —
the backing read for a hex view. Both stop early at the first inaccessible
address and return what they got (the natural case at the end of the data
segment), returning `None` only when `addr` itself is inaccessible.
`read_bytes` needs no alignment: it starts at the enclosing cell and trims.

```rust
let cells: Vec<i32> = amx.read_cells(addr, 8).unwrap_or_default();
let bytes: Vec<u8> = amx.read_bytes(addr + 2, 16).unwrap_or_default();
```

When you hold a raw `*mut AMX` and no function table — the usual situation
while a VM is paused — build the wrapper with `Amx::data_only(ptr)`. It states
that only the data-side API is available, instead of passing a bare `0` as the
function table.

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
the opcode number. `Amx::opcode_map()` builds the inverse table (address →
opcode) for you, from the VM's own dispatch list:

```rust
use samp::debug::opcode::{OP_BOUNDS, OP_SDIV, OP_UDIV};

// Build once per VM (e.g. in on_amx_load) and keep it.
let map = amx.opcode_map();

// In the hook: raw code value → opcode number.
let raw = amx.read_code(cip)?;
match map.decode(raw)? {
    OP_SDIV | OP_UDIV => { /* divisor is in `alt` */ }
    OP_BOUNDS => { /* index is in `pri` */ }
    _ => {}
}
```

On a non-relocated image the code segment already holds opcode numbers;
`decode` passes those through, and `OpcodeMap::is_identity()` reports that case.
Under the hood the map comes from `Amx::opcode_table(count)`, which is still
public if you want the raw dispatch list — `opcode_map()` simply calls it with
`OP_NUM_OPCODES`.

`opcode_table` does not consult the `AMX_FLAG_RELOC` header bit — it may not be
visible at `AmxLoad` time even though the table is already available. On a
non-computed-goto VM the returned addresses simply never match a real opcode, so
inverting the table is harmless.

### Opcode numbers and instruction sizes

`samp::debug::opcode` carries the AMX opcode numbering (the order of the opcode
enum in `amx.c`, identical on SA-MP and open.mp) so tools do not have to
hardcode magic numbers: `OP_SDIV`, `OP_BOUNDS`, `OP_BREAK`, `OP_PROC`,
`OP_CALL`, the load/store and stack/heap opcodes, plus `STK_MARGIN` (the VM's
`STKMARGIN`) and `OP_NUM_OPCODES`.

`operand_cells(op)` gives how many inline operand cells an instruction carries,
so a scanner can step to the next instruction — an instruction occupies
`1 + operand_cells(op)` cells:

```rust
use samp::debug::{operand_cells, OP_BREAK};

let mut at = line_start;
while let Some(op) = map.decode(amx.read_code(at)?) {
    if op == OP_BREAK && at != line_start {
        break; // reached the next source line
    }
    // inspect `op` here...
    let Some(operands) = operand_cells(op) else {
        break; // variable-length instruction: cannot know where the next starts
    };
    at += 4 * (1 + u32::from(operands));
}
```

`operand_cells` returns `None` for a variable-length instruction (`casetbl`) or
an out-of-range opcode — the signal to stop scanning rather than guess.

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

## Walking the call stack

`Amx::call_stack(top_cip)` walks the AMX frame chain and returns the
`(cip, frm)` of every frame — index 0 is the top, where the VM currently is.
Inside a debug hook, `top_cip` is the address of the line's `OP_BREAK`, i.e.
`cip()` minus one cell, since the hook is entered with the instruction pointer
already past the break.

```rust
let top_cip = amx.cip()? - 4; // the break that opened this line

for (cip, frm) in amx.call_stack(top_cip) {
    let name = dbg.lookup_function(cip).unwrap_or("???");
    let line = dbg.lookup_line(cip);
    // `frm` is that frame's FRM: pass it to `effective_address` to read the
    // locals of *that* frame, not only the top one.
}
```

For each caller the `cip` is the saved return address, which maps to the line of
the call site. The walk ends at the entry public (`amx_Exec` pushes a return
address of `0` before it) and stops early — keeping what it has — if the chain
leaves the stack, stops ascending, or cannot be read; `MAX_DEPTH` caps it so a
corrupted stack cannot spin the hook.

`samp::debug::stack::walk` is the same logic with an injected cell reader, so it
can be unit-tested against a fake memory map or driven host-side.
