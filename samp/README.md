# rust-samp

Write SA-MP and open.mp plugins in safe Rust instead of C++. The same source
compiles to a single binary that runs natively on both servers; the macros
hide the FFI boilerplate and ABI-correct marshalling for Linux (Itanium) and
Windows (MSVC).

This is the main crate — depend on it from your plugin's `Cargo.toml`.

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
# Published as `rust-samp` to avoid name collision; aliased to `samp` so
# the import is short.
samp = { package = "rust-samp", version = "3" }
```

Minimal plugin:

```rust
use samp::prelude::*;
use samp::{SampPlugin, initialize_plugin, native};

#[derive(SampPlugin, Default)]
struct Hello;

impl Hello {
    #[native(name = "Hello_Echo")]
    fn echo(_amx: &Amx, text: &AmxString) -> bool {
        println!("{}", &**text);
        true
    }
}

initialize_plugin!(type: Hello, natives: [Hello::echo]);
```

Build for 32-bit (both servers are 32-bit):

```sh
cargo build --target i686-unknown-linux-gnu --release
cargo xwin build --xwin-arch x86 --target i686-pc-windows-msvc --release
```

## Related crates in this workspace

- [`rust-samp-sdk`](https://crates.io/crates/rust-samp-sdk) — low-level FFI
  bindings (AMX VM, open.mp component ABI). Depend on it directly only if
  you want raw access without the macros.
- [`rust-samp-codegen`](https://crates.io/crates/rust-samp-codegen) — the
  proc macros (`#[native]`, `initialize_plugin!`, `#[derive(SampPlugin)]`).
  Re-exported by `rust-samp`; no need to depend on it directly.

## Repository and full documentation

Source, examples and the long-form guide live at
<https://github.com/NullSablex/rust-samp>.
