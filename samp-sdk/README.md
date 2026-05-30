# rust-samp-sdk

Low-level FFI bindings for the SA-MP AMX virtual machine and the open.mp
native component ABI.

Used internally by [`rust-samp`](https://crates.io/crates/rust-samp). Depend
on it directly only if you need raw access — for example, to bind a new
open.mp `IComponent` interface that the high-level crate does not expose
yet, or to write a custom plugin runtime that bypasses the macros and
lifecycle management.

```toml
[dependencies]
samp-sdk = { package = "rust-samp-sdk", version = "3" }
```

## What it provides

- Type-safe wrappers around the `amx_*` exports (`Amx::register`, `exec`,
  `find_native`, `find_public`, `find_pubvar`, `allot*`, `push`, `strlen`).
- AMX cell conversion: `AmxCell` and `CellConvert` traits for primitives,
  references, buffers and strings.
- `Buffer`, `UnsizedBuffer`, `AmxString` with lazy UTF-8 decoding and
  ergonomic `write_str` / `iter_as` helpers.
- open.mp native component bindings: `IComponent`, `IUIDProvider`,
  `IPawnComponent`, `IPawnScript`, `IEventDispatcher`, `ITimersComponent`,
  `ILogger`. Vtables are verified against both Itanium (Linux GCC) and
  MSVC ABIs at runtime.
- `OmpComponentHandle` trait + `omp_query` for typed access to any
  component by UID.

## Target

The SDK targets `i686-unknown-linux-gnu`, `i686-pc-windows-msvc` and
`i686-pc-windows-gnu`. The latter only supports SA-MP — native open.mp
requires an MSVC- or Itanium-compatible ABI.

## Repository

<https://github.com/NullSablex/rust-samp>
