# samp-sdk

Low-level layer of the `rust-samp` toolkit. Two independent binding sets live
in a single crate:

- **AMX VM** (SA-MP) — pointers, cell types, function table, error codes
  (modules `raw`, `amx`, `cell`, `args`, `error`, `exports`, `consts`).
- **Open Multiplayer** — pure-Rust implementation of the component ABI:
  vtables, `IComponent` memory layout, `IUIDProvider`, `IPawnComponent`,
  `IPawnScript`, `IEventDispatcher<PawnEventHandler>`, `ITimersComponent`,
  `ICore::ILogger`. Compiled when the `samp-only` feature is **not** active.

> Plugin authors normally do not depend on `samp-sdk` directly. Use the
> [`samp`](../samp) crate, which re-exports this SDK plus the proc macros.

## Highlights

| Item            | Module               | Purpose                                                                 |
| --------------- | -------------------- | ----------------------------------------------------------------------- |
| `Amx`           | `amx`                | Safe wrapper around `*mut AMX` + the `amx_Exports` table.               |
| `Allocator`     | `amx`                | RAII heap allocator (`allot`, `allot_buffer`, `allot_array`, `allot_string`). |
| `Args`          | `args`               | Iterator over the cells of a native call (`next_arg`, `get`, `count`).  |
| `AmxString`     | `cell::string`       | Pawn string with `Deref<Target = str>`, packed/unpacked, lazy decode.   |
| `Buffer`        | `cell::buffer`       | Sized cell vector; typed accessors via `CellConvert`.                   |
| `UnsizedBuffer` | `cell::buffer`       | Unsized array argument; converts to `Buffer` with `into_sized_buffer`.  |
| `Ref<T>`        | `cell`               | Typed pointer to a Pawn cell (output by reference).                     |
| `AmxCell`       | `cell::repr`         | Argument/return conversion trait used by `#[native]`.                   |
| `CellConvert`   | `cell::repr`         | Per-cell conversion trait used by `Buffer::get_as`/`set_as`/`iter_as`.  |
| `AmxPrimitive`  | `cell::repr`         | Marker for types that fit in one 32-bit cell.                           |
| `AmxError`      | `error`              | All 28 codes returned by `amx_*` functions + `Unknown`. Implements `std::error::Error`. |
| `Export` trait  | `exports`            | On-demand resolution of pointers in the `amx_Exports` table.            |
| `Supports`      | `consts`             | Bitflags reported by the `Supports()` export.                           |
| `OmpComponent`  | `omp::component`     | Memory layout compatible with Open Multiplayer's `IComponent`.          |
| `IComponentVTable` / `IUIDProviderVTable` | `omp::component` | Primary/secondary vtables, gated per ABI (`target_env = "msvc"`). |
| `OmpComponentHandle` trait | `omp::component_api` | Typed wrapper over the raw `*mut ServerComponent`.                  |
| `PawnComponent` / `TimersComponent` | `omp::server`, `omp::timers` | Concrete `OmpComponentHandle` implementations. |
| `core_print_ln` / `core_log_ln` / `*_u8` | `omp::core` | Bindings for `ICore::ILogger` log calls.                         |
| `vtable::secondary_call_target` | `omp::vtable` | Helper to resolve secondary-base vtable slots safely.                |

## `AmxCell` vs `CellConvert`

| Trait         | Where it's used                                            | Needs `&Amx`?           |
| ------------- | ---------------------------------------------------------- | ----------------------- |
| `AmxCell`     | Argument and return types of `#[native]`                   | Yes (for complex types) |
| `CellConvert` | Elements of a `Buffer` (`get_as`, `set_as`, `iter_as`)     | No                      |

Both ship with implementations for `i8`, `u8`, `i16`, `u16`, `i32`, `u32`,
`isize`, `usize`, `f32`, and `bool`. `f32` round-trips through
`to_bits`/`from_bits`; `bool` follows the C convention (`0` is false, any
other value is true).

## Feature flags

| Flag         | Effect                                                                 |
| ------------ | ---------------------------------------------------------------------- |
| *(none)*     | Default: AMX bindings + Open Multiplayer component ABI.                |
| `encoding`   | Enables `encoding_rs` and the `encoding` module (Windows-1251 / 1252). |
| `samp-only`  | Removes the `omp` module entirely; only the AMX bindings remain.       |

## Platform notes

The Open Multiplayer module ships layouts for two i686 ABIs:

- **Itanium ABI** (Linux GCC) — `extern "C"` calling convention; dual
  destructor slots (D1 + D0); `IUIDProvider` subobject at offset 40 of
  `OmpComponent`.
- **MSVC ABI** (Windows i686) — `extern "thiscall"`; single scalar deleting
  destructor; `IUIDProvider` subobject at offset 56; `componentName` and
  `componentVersion` returned via hidden pointer in `[ESP+4]`.

Both layouts are validated at compile time (`const _: () = { ... }` asserts
on `offset_of!(uid_vtable)` and `size_of::<OmpComponent>()`). `i686-pc-windows-gnu`
is **not** supported for native Open Multiplayer — use it only for SA-MP-only
builds.

## License

MIT.
