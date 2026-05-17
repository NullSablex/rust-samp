# Introduction

**rust-samp** is a Rust toolkit for building server plugins for
[SA-MP](http://sa-mp.com) (San Andreas Multiplayer) and native components
for [Open Multiplayer](https://github.com/openmultiplayer). The same
compiled binary works on both servers: SA-MP loads it through the legacy
plugin ABI, and Open Multiplayer loads it as a first-class component
without any extra configuration.

> Fork of [samp-rs](https://github.com/Pycckue-Bnepeg/samp-rs) by
> [ZOTTCE](https://github.com/ZOTTCE). Modernized for Rust edition 2024
> and extended with a pure-Rust implementation of the Open Multiplayer
> component ABI (Itanium and MSVC), without `bindgen` or any C/C++
> dependency.

## Why Rust

Traditional plugins are written in C/C++, where memory errors (buffer
overflow, use-after-free, dangling pointers) are the leading cause of
server crashes. Rust eliminates those bugs at compile time while keeping
performance on par with C.

rust-samp adds, on top of that foundation:

- **Native derivation** via the `#[native]` attribute — no manual FFI
  boilerplate.
- **Automatic argument parsing** from AMX cells into Rust types.
- **Panic-safe wrappers** — panics inside a native are caught and logged
  instead of aborting the server process.
- **`AmxString` with `Deref<Target = str>`** — every `&str` method is
  available without `.to_string()`; the decoded string is computed once
  and cached.
- **Typed Pawn arrays** — `Buffer::get_as::<f32>` / `set_as::<bool>` /
  `iter_as::<T>` cover `Float:` and `bool:` arrays without manual bit
  manipulation.
- **Optional `encoding` feature** — Windows-1251 / Windows-1252 string
  conversion via `encoding_rs`.
- **Integrated logging** via `log` + `fern`, routed to the server's log
  sink (`logprintf` on SA-MP, `ICore::logLnU8` on native Open
  Multiplayer).
- **Native Open Multiplayer support** — pure-Rust implementation of the
  component ABI (Itanium and MSVC) so a single binary works as a SA-MP
  plugin and as an Open Multiplayer component.

## Workspace structure

| Crate          | Version | Description                                                              |
| -------------- | :-----: | ------------------------------------------------------------------------ |
| `samp`         | 3.0.0   | Main crate — depend on this one. Re-exports the SDK and the proc macros. |
| `samp-sdk`     | 3.0.0   | Low-level bindings: AMX VM + Open Multiplayer component ABI.            |
| `samp-codegen` | 1.3.0   | Procedural macros (`#[native]`, `initialize_plugin!`, `SampPlugin`).    |

In practice, only the `samp` crate is referenced by a plugin's
`Cargo.toml`. It re-exports everything from `samp-sdk` and `samp-codegen`.

## A first taste

```rust
use samp::prelude::*;
use samp::{native, initialize_plugin, SampPlugin};

#[derive(SampPlugin, Default)]
struct Plugin;

impl Plugin {
    #[native(name = "TestNative")]
    fn my_native(&mut self, _amx: &Amx, text: &AmxString) -> AmxResult<bool> {
        println!("rust plugin: {}", &**text);
        Ok(true)
    }
}

initialize_plugin!(
    type: Plugin,
    natives: [Plugin::my_native],
);
```

The next chapters set up the toolchain and walk through a plugin from
scratch.
