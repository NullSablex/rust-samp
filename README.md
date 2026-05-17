[![CI](https://github.com/NullSablex/rust-samp/actions/workflows/rust.yml/badge.svg)](https://github.com/NullSablex/rust-samp/actions/workflows/rust.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![dependency status](https://deps.rs/repo/github/NullSablex/rust-samp/status.svg)](https://deps.rs/repo/github/NullSablex/rust-samp)
[![Benchmarks](https://img.shields.io/badge/benchmarks-criterion-blue)](https://github.com/NullSablex/rust-samp/actions/workflows/rust.yml)

# rust-samp

Rust toolkit for writing SA-MP server plugins and native Open Multiplayer
components. A single compiled binary works as a SA-MP plugin **and** as a
first-class Open Multiplayer component, with no extra configuration.

> Fork of [samp-rs](https://github.com/Pycckue-Bnepeg/samp-rs) by
> [ZOTTCE](https://github.com/ZOTTCE). Modernized for Rust edition 2024 and
> extended with a pure-Rust implementation of the Open Multiplayer component
> ABI (Itanium and MSVC), without `bindgen` or any C/C++ dependency.

## Quickstart

```sh
rustup target add i686-unknown-linux-gnu
```

`Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "v3.0.0" }
```

`src/lib.rs`:

```rust
use samp::prelude::*;
use samp::{native, initialize_plugin, SampPlugin};

#[derive(SampPlugin, Default)]
struct Hello;

impl Hello {
    #[native(name = "Hello_Greet")]
    fn greet(_amx: &Amx, name: &AmxString, out: UnsizedBuffer, size: usize) -> AmxResult<bool> {
        out.write_str(size, &format!("Hello, {}!", &**name))?;
        Ok(true)
    }
}

initialize_plugin!(type: Hello, natives: [Hello::greet]);
```

```sh
cargo build --release --target i686-unknown-linux-gnu
```

Drop the resulting `.so` into the server's `plugins/`. Full walkthrough in
[docs/first-plugin.md](docs/first-plugin.md).

## Workspace

| Crate          | Version | Purpose                                                              |
| -------------- | :-----: | -------------------------------------------------------------------- |
| `samp`         | 3.0.0   | Main crate — depend on this one.                                     |
| `samp-sdk`     | 3.0.0   | Low-level bindings: AMX VM + Open Multiplayer component ABI.        |
| `samp-codegen` | 1.3.0   | Procedural macros (`#[native]`, `initialize_plugin!`, `SampPlugin`).|

Edition 2024, workspace `resolver = "3"`. Target: **i686** — both servers
are 32-bit.

## Platform matrix

| Target                    | SA-MP | Native Open Multiplayer | Build command                                       |
| ------------------------- | :---: | :---------------------: | --------------------------------------------------- |
| `i686-unknown-linux-gnu`  |   ✅   |  ✅ (Itanium ABI)        | `cargo build`                                       |
| `i686-pc-windows-msvc`    |   ✅   |  ✅ (MSVC ABI)           | `cargo xwin build --xwin-arch x86` (from Linux)     |
| `i686-pc-windows-gnu`     |   ✅   |  ❌                     | `cargo build --features samp-only`                  |

## Feature flags

- *(default)* — SA-MP exports + Open Multiplayer `ComponentEntryPoint`.
- `samp-only` — opt out of the Open Multiplayer code path; plugin still
  loads on Open Multiplayer in legacy mode.
- `encoding` — Windows-1251 / Windows-1252 string conversion via
  `encoding_rs`.

## Examples

| Path                                      | Highlights                                                                |
| ----------------------------------------- | ------------------------------------------------------------------------- |
| [`examples/hello`](examples/hello/)       | Minimal plugin (`#[derive(SampPlugin)]`, `&AmxString`, `write_str`).      |
| [`examples/counter`](examples/counter/)   | Stateful plugin with `on_server_tick`, `Ref<i32>`, full constructor block.|
| [`examples/advanced`](examples/advanced/) | Memcache plugin: custom `AmxCell`, `encoding` feature, layered `fern`.    |

## Documentation

Full user docs under [`docs/`](docs/) (MkDocs Material). Starting points:

- [Introduction](docs/introduction.md) and [Setup](docs/setup.md).
- [First plugin](docs/first-plugin.md) and [Plugin anatomy](docs/plugin-anatomy.md).
- [Native Open Multiplayer support](docs/omp-native.md).
- [API reference](docs/api-reference.md) and [Migration guide](docs/migration.md).

## License

MIT — see [LICENSE](LICENSE).
