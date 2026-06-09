# Setup

## Prerequisites

- [Rust](https://rustup.rs) (stable channel).
- The **i686** target — both SA-MP and Open Multiplayer servers are
  32-bit.

## Toolchain

```sh
# Install Rust (skip if already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Targets

| Target                    | SA-MP | Native Open Multiplayer | Notes                              |
| ------------------------- | :---: | :---------------------: | ---------------------------------- |
| `i686-unknown-linux-gnu`  |   ✅   |  ✅ (Itanium ABI)        | Default on Linux.                  |
| `i686-pc-windows-msvc`    |   ✅   |  ✅ (MSVC ABI)           | Cross-compile via `cargo-xwin`.    |
| `i686-pc-windows-gnu`     |   ✅   |  ❌                     | Use only with `--features samp-only`. |

`i686-pc-windows-gnu` does **not** support native Open Multiplayer because
the Windows Open Multiplayer server uses the MSVC ABI.

```sh
rustup target add i686-unknown-linux-gnu   # Linux
rustup target add i686-pc-windows-msvc     # Windows MSVC (requires cargo-xwin)
```

### Linux system dependencies

Multilib compilers are required for 32-bit cross-compilation:

```sh
# Debian / Ubuntu
sudo apt-get install gcc-multilib g++-multilib
```

## Project skeleton

```sh
cargo new --lib my-plugin
cd my-plugin
```

### `Cargo.toml`

There are two ways to depend on the SDK. From v3.1.0 onwards the
preferred path is crates.io; the git dependency keeps working and is
the only option for earlier releases (v3.0.0 and the v2.x line never
made it to the registry).

**From crates.io (v3.1.0 onwards):**

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
samp = { package = "rust-samp", version = "3" }
```

The package is published as `rust-samp` to avoid colliding with the
upstream `samp-rs` fork on the registry. The `package = "rust-samp"`
alias keeps the source-level `use samp::prelude::*;` imports unchanged.

**From git (any version, including v3.0.0 and earlier):**

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "v3.1.0" }
```

> Always pin the dependency with `tag` or `rev` when using git. A bare
> `git` dependency is not reproducible — a repository update can
> change behavior without any warning.
>
> ```toml
> # Pin by tag (recommended for named releases)
> samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "v3.1.0" }
>
> # Pin by commit SHA (exact — SHA visible on the GitHub releases page)
> samp = { git = "https://github.com/NullSablex/rust-samp.git", rev = "COMMIT_SHA" }
> ```

`crate-type = ["cdylib"]` makes Cargo emit a dynamic library (`.so` on
Linux, `.dll` on Windows), which is what the server loads as a plugin.

## Building

### Manual commands

```sh
# Linux — produces .so (SA-MP + native Open Multiplayer)
cargo build --release --target i686-unknown-linux-gnu

# Windows MSVC — produces .dll (SA-MP + native Open Multiplayer)
# Run on Windows or cross-compile from Linux via cargo-xwin
cargo xwin build --release --xwin-arch x86 --target i686-pc-windows-msvc

# Windows GNU — produces .dll (SA-MP only, use with samp-only)
cargo build --release --target i686-pc-windows-gnu --features samp-only
```

### Helper scripts

The repository ships with `scripts/build-linux.sh` and
`scripts/build-windows.sh` that produce both `.so` and `.dll` in a single
invocation. Read the scripts before running them — they pin
`cargo-xwin` flags and emit artefacts under `dist/`.

## Common errors

### `lld-link` cannot find Windows SDK libraries

Compiling for `i686-pc-windows-msvc` can fail with:

```
lld-link: error: could not open 'advapi32.lib': No such file or directory
lld-link: error: could not open 'kernel32.lib': No such file or directory
```

**Cause:** by default `cargo-xwin` downloads the Windows SDK libraries
for `x86_64`. The i686 target expects them under `sdk/lib/um/x86/`,
which is not populated without `--xwin-arch x86`.

**Fix:** clear the cache and recompile with the right flag.

```sh
rm -rf ~/.cache/cargo-xwin
cargo xwin build --xwin-arch x86 --target i686-pc-windows-msvc
```

### Compile-time error when the target is not i686

If you run `cargo build` or `cargo check` without `--target`, Rust
compiles for the host architecture (usually x86_64). On Linux this
fails with:

```
error[E0080]: evaluation panicked: OmpComponent: invalid size for the
Itanium ABI. Use --target i686-unknown-linux-gnu to compile with native
Open Multiplayer support.
```

**Cause:** the SDK validates `OmpComponent`'s layout at compile time
against the Itanium ABI on i686. On x86_64 pointers are 8 bytes and the
layout no longer matches.

**Fix:** set the target in `.cargo/config.toml` at the project root.

## Simplifying the build

To stop passing `--target` every time, create `.cargo/config.toml` at
the project root:

```toml
[build]
# Linux — SA-MP + native Open Multiplayer (Itanium ABI)
target = "i686-unknown-linux-gnu"

# Windows — SA-MP + native Open Multiplayer (MSVC ABI; cargo-xwin from Linux)
# target = "i686-pc-windows-msvc"

# Windows — SA-MP only (GNU ABI; combine with the samp-only feature)
# target = "i686-pc-windows-gnu"
```

`cargo build` then defaults to the correct target.

## Installing on the server

1. Build in release mode.
2. Copy the resulting `.so` / `.dll` into the server's `plugins/`
   directory.
3. Add the plugin name (no extension on Linux, `.dll` on Windows) to
   `server.cfg`:
   ```
   plugins my_plugin
   ```
4. Start the server.
