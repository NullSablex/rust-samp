# API reference

Quick reference of the public types, traits, and macros exposed by
`samp`.

## Prelude

```rust
use samp::prelude::*;
```

Re-exports: `Amx`, `AmxExt`, `AmxCell`, `AmxString`, `Buffer`,
`CellConvert`, `Ref`, `UnsizedBuffer`, `AmxResult`, `SampPlugin`.

## Traits

### `SampPlugin`

```rust
pub trait SampPlugin {
    fn on_load(&mut self) {}
    fn on_unload(&mut self) {}
    fn on_amx_load(&mut self, amx: &Amx) {}
    fn on_amx_unload(&mut self, amx: &Amx) {}
    fn on_server_tick(&mut self) {}

    #[cfg(not(feature = "samp-only"))]
    fn on_omp_ready(&mut self) {}
    #[cfg(not(feature = "samp-only"))]
    fn on_component_free(&mut self) {}
}
```

| Method               | Available on              | Description                                                    |
| -------------------- | ------------------------- | -------------------------------------------------------------- |
| `on_load`            | SA-MP / native Open Multiplayer | Server loaded the plugin.                                |
| `on_unload`          | SA-MP / native Open Multiplayer | Server is unloading the plugin.                          |
| `on_amx_load`        | SA-MP / native Open Multiplayer | A Pawn script (`.amx`) was loaded.                       |
| `on_amx_unload`      | SA-MP / native Open Multiplayer | A Pawn script is being unloaded.                         |
| `on_server_tick`     | SA-MP / native Open Multiplayer | Runs ~5 ms when `enable_server_tick()` was called.       |
| `on_omp_ready`       | Native Open Multiplayer only | Every Open Multiplayer component initialized.              |
| `on_component_free`  | Native Open Multiplayer only | Some Open Multiplayer component is being released.         |

### `AmxCell<'amx>`

```rust
pub trait AmxCell<'amx>: Sized {
    fn from_raw(_amx: &'amx Amx, _cell: i32) -> AmxResult<Self> { Err(AmxError::General) }
    fn as_cell(&self) -> i32;
}
```

Implementations: `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `isize`,
`usize`, `f32`, `bool`, `&T`, `&mut T`, plus the SDK types
`AmxString`, `Buffer`, `UnsizedBuffer`, `Ref<T>`.

### `CellConvert`

```rust
pub trait CellConvert: Sized {
    fn from_cell(raw: i32) -> Self;
    fn into_cell(self) -> i32;
}
```

Used by `Buffer::get_as`, `set_as`, `iter_as`. No `&Amx` required.

### `AmxPrimitive` *(`unsafe` marker)*

Marker for types that fit in a single 32-bit cell — bound on
`Ref<T>` / `Buffer::get_as` and friends.

### `AmxExt`

```rust
pub trait AmxExt {
    fn ident(&self) -> AmxIdent;
}
```

## Structs

### `Amx`

| Method                                       | Purpose                                                                |
| -------------------------------------------- | ---------------------------------------------------------------------- |
| `new(ptr, fn_table) -> Amx`                  | Build the wrapper from a raw `*mut AMX` plus the exports table.        |
| `register(natives) -> AmxResult<()>`         | Register a native table via `amx_Register`.                            |
| `exec(idx) -> AmxResult<i32>`                | Execute a function by index.                                           |
| `find_public(name) -> AmxResult<AmxExecIdx>` | Resolve a Pawn `public`.                                               |
| `find_native(name) -> AmxResult<i32>`        | Resolve a native by name.                                              |
| `find_pubvar::<T>(name) -> AmxResult<Ref<T>>`| Resolve a `pubvar` (`T: AmxPrimitive`).                                |
| `push(value) -> AmxResult<()>`               | Push a value onto the VM stack (reverse argument order).               |
| `get_ref::<T>(addr) -> AmxResult<Ref<T>>`    | Build a `Ref<T>` from an AMX address.                                  |
| `allocator() -> Allocator<'_>`               | RAII heap allocator.                                                   |
| `strlen(ptr) -> AmxResult<usize>`            | Length of an AMX string at the given pointer.                          |
| `flags() -> AmxResult<AmxFlags>`             | Flags of the loaded `.amx`.                                            |
| `amx()` / `header()`                         | Raw `*mut AMX` / `*mut AMX_HEADER` (via `NonNull`).                    |

### `Allocator<'amx>`

| Method                                       | Purpose                                                                |
| -------------------------------------------- | ---------------------------------------------------------------------- |
| `allot::<T>(init) -> AmxResult<Ref<T>>`      | Allocate one cell, initialized.                                        |
| `allot_buffer(size) -> AmxResult<Buffer>`    | Allocate a buffer of `size` cells.                                     |
| `allot_array::<T>(slice) -> AmxResult<Buffer>` | Allocate and copy a Rust slice.                                      |
| `allot_string(s) -> AmxResult<AmxString>`    | Allocate a string in the active encoding + terminator.                 |

### `Ref<'amx, T>`

Smart pointer to a Pawn cell. Implements `Deref` and `DerefMut`.

| Method            | Purpose                                          |
| ----------------- | ------------------------------------------------ |
| `address()`       | Cell address inside the AMX address space.       |
| `as_ptr()`        | Read-only physical pointer.                      |
| `as_mut_ptr()`    | Mutable physical pointer.                        |

### `AmxString<'amx>`

Implements `Deref<Target = str>`, `Display`, `PartialEq<{&str, str, String}>`.

| Method            | Purpose                                                   |
| ----------------- | --------------------------------------------------------- |
| `as_str()`        | Explicit `&str` (forces the lazy decode).                 |
| `to_bytes()`      | Raw bytes from the underlying cells.                      |
| `len()`           | Length in characters (no terminator).                     |
| `bytes_len()`     | Size of the underlying buffer in cells.                   |
| `is_empty()`      | Empty check.                                              |

### `Buffer<'amx>`

Implements `Deref<Target = [i32]>` and `DerefMut`.

| Method                       | Purpose                                                            |
| ---------------------------- | ------------------------------------------------------------------ |
| `as_slice()`                 | `&[i32]` of every cell.                                            |
| `as_mut_slice()`             | `&mut [i32]` of every cell.                                        |
| `len()` / `is_empty()`       | Number of cells.                                                   |
| `get_as::<T>(i) -> Option<T>`| Read cell `i` as `T: CellConvert`.                                 |
| `set_as::<T>(i, v) -> bool`  | Write `v` into cell `i`; `false` when out of bounds.               |
| `iter_as::<T>()`             | Iterator producing `T` values from every cell.                     |
| `write_str(s) -> AmxResult<()>` | Encode `s` into the buffer (one byte per cell + terminator).    |

### `UnsizedBuffer<'amx>`

| Method                          | Purpose                                                        |
| ------------------------------- | -------------------------------------------------------------- |
| `into_sized_buffer(len)`        | Convert into `Buffer<'amx>` (capped at 1 MiB cells).           |
| `write_str(max_len, s)`         | `into_sized_buffer(max_len)` + `write_str(s)` in one call.     |

### `Args<'a>`

| Method                          | Purpose                                                        |
| ------------------------------- | -------------------------------------------------------------- |
| `new(amx, params) -> Args`      | Build from the raw native parameter pointer.                   |
| `next_arg::<T>() -> Option<T>`  | Advance and parse the next argument.                           |
| `get::<T>(offset) -> Option<T>` | Parse by position.                                             |
| `count() -> usize`              | Number of arguments declared by the caller.                    |
| `reset()`                       | Move the cursor back to position 0.                            |

## Enums

### `AmxError`

28 explicit variants (1–28) plus `Unknown`. Implements `Display`,
`std::error::Error`, `From<i32>`. See [Error handling](error-handling.md)
for the full table.

### `AmxExecIdx`

| Variant         | Value | Purpose                                            |
| --------------- | :---: | -------------------------------------------------- |
| `Main`          | -1    | Entry point `main`.                                |
| `Continue`      | -2    | Continue a suspended execution.                    |
| `UserDef(i32)`  | N     | Index returned by `amx_FindPublic`.                |

### `ServerData`

Offsets inside the `ppData` table passed to `Load(void**)`.

| Variant         | Offset |
| --------------- | :----: |
| `Logprintf`     | 0      |
| `AmxExports`    | 16     |
| `CallPublicFs`  | 17     |
| `CallPublicGm`  | 18     |

### `LogLevel` *(Open Multiplayer)*

| Variant   | Discriminant |
| --------- | :----------: |
| `Debug`   | 0            |
| `Message` | 1            |
| `Warning` | 2            |
| `Error`   | 3            |

## Bitflags

### `Supports`

| Flag           | Value      |
| -------------- | ---------- |
| `VERSION`      | `512`      |
| `AMX_NATIVES`  | `0x10000`  |
| `PROCESS_TICK` | `0x20000`  |

### `AmxFlags`

`DEBUG`, `COMPACT`, `BYTEOPC`, `NOCHECKS`, `NTVREG`, `JITC`, `BROWSE`,
`RELOC`.

## Macros

### `#[native]`

```rust
#[native(name = "PawnName")]          // standard native
#[native(name = "PawnName", raw)]     // raw mode with Args
```

### `initialize_plugin!`

Generates the server entry points and instantiates the plugin.

```rust
// Short form — Default::default() constructor
initialize_plugin!(
    type: MyPlugin,
    natives: [MyPlugin::method],
);

// Full form — constructor block
initialize_plugin!(
    natives: [MyPlugin::method],
    { return MyPlugin::new(); }
);
```

Optional Open Multiplayer metadata fields:

```rust
initialize_plugin!(
    uid: 0x4D455550CAFEBABE_u64,      // default: FNV-1a of CARGO_PKG_NAME@CARGO_PKG_VERSION
    component_name: "MyPlugin",        // default: CARGO_PKG_NAME
    component_version: (1, 0, 0),      // default: parsed CARGO_PKG_VERSION
    natives: [MyPlugin::method],
    { return MyPlugin::new(); }
);
```

Resolution order for each field: **macro argument >
`[package.metadata.samp]` in `Cargo.toml` > derived value**.

### `exec_public!`

```rust
exec_public!(amx, "PublicName");                    // no arguments
exec_public!(amx, "PublicName", arg1, arg2);        // AmxCell-compatible primitives
exec_public!(amx, "PublicName", text => string);    // Rust string
exec_public!(amx, "PublicName", &vec => array);     // Rust slice
```

## Module map

| Path                | Contents                                                                |
| ------------------- | ----------------------------------------------------------------------- |
| `samp::amx`         | `Amx`, `AmxExt`, `AmxIdent`, `get(ident)`, `add(ptr)`.                  |
| `samp::plugin`      | `SampPlugin`, `enable_server_tick`, `logger`, `omp_core` *, `omp_query_component` *, `omp_query` *. |
| `samp::cell`        | `AmxCell`, `CellConvert`, `AmxPrimitive`, `AmxString`, `Ref`, `Buffer`, `UnsizedBuffer`. |
| `samp::error`       | `AmxError`, `AmxResult`.                                                |
| `samp::args`        | `Args`.                                                                 |
| `samp::consts`      | `Supports`, `AmxFlags`, `AmxExecIdx`, `ServerData`.                     |
| `samp::encoding` ** | `set_default_encoding`, `WINDOWS_1251`, `WINDOWS_1252`.                 |
| `samp::omp` *       | Re-exports of `samp_sdk::omp` (component types, vtables, helpers).      |
| `samp::raw`         | Raw FFI types (`AMX`, `AMX_HEADER`, `AMX_NATIVE_INFO`) and function aliases. |

\* Available only when the `samp-only` feature is **not** set.
\** Available only when the `encoding` feature is set.

### `samp::plugin` — Open Multiplayer helpers

- `omp_core() -> Option<*mut ICore>` — `ICore*` received in `on_load`,
  or `None` when running on SA-MP or in Open Multiplayer legacy mode.
- `omp_query_component(uid) -> Option<*mut ServerComponent>` — raw
  pointer to a component, when the component list is already known
  (after `on_init`).
- `omp_query::<T>() -> Option<T>` — typed wrapper version; `T` must
  implement `OmpComponentHandle` (e.g. `PawnComponent`,
  `TimersComponent`).

## Feature flags

| Feature      | Effect                                                                                       |
| ------------ | -------------------------------------------------------------------------------------------- |
| *(default)*  | SA-MP exports + Open Multiplayer `ComponentEntryPoint` (full dual support).                  |
| `encoding`   | Enables `samp::encoding` (Windows-1251 / 1252 via `encoding_rs`).                            |
| `samp-only`  | Removes every Open Multiplayer code path — the plugin still loads on Open Multiplayer in legacy mode. |
