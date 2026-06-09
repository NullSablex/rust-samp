# Migration guide

This page collects the breaking changes (and the new defaults) between
the supported releases. Pick the section that matches your current
state.

## v2.1.0 → v2.2.0

No breaking changes — only ergonomic improvements. Old code keeps
compiling; the new patterns are simpler and worth adopting.

### 1. Plugin construction — the short form

**Before:**

```rust
struct MyPlugin;

impl SampPlugin for MyPlugin {}

initialize_plugin!(
    natives: [MyPlugin::my_native],
    { return MyPlugin; }
);
```

**Now:**

```rust
#[derive(SampPlugin, Default)]
struct MyPlugin;

initialize_plugin!(
    type: MyPlugin,
    natives: [MyPlugin::my_native],
);
```

`#[derive(SampPlugin)]` emits `impl SampPlugin for T {}` for stateless
plugins. As soon as a lifecycle hook (`on_load`, `on_tick`, …)
needs an override, drop the derive and write the impl by hand — and
switch back to the constructor-block form of `initialize_plugin!`.

| Situation                                | Recommended form                                                |
| ---------------------------------------- | --------------------------------------------------------------- |
| Stateless plugin                         | `initialize_plugin!(type: T, natives: [...])`                   |
| Setup logic (`on_load`, logging, …)      | `initialize_plugin!(natives: [...], { ... })`                   |
| Initial struct state                     | `initialize_plugin!(natives: [...], { return T { ... }; })`     |

### 2. `AmxString` — use `Deref` instead of `.to_string()`

`AmxString` implements `Deref<Target = str>`, so every `&str` method is
available without an extra allocation.

**Before:**

```rust
fn say_hello(&mut self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
    let name = name.to_string();
    println!("Hello, {name}!");
    Ok(true)
}
```

**Now:**

```rust
fn say_hello(&mut self, _amx: &Amx, name: &AmxString) -> AmxResult<bool> {
    println!("Hello, {}!", &**name);
    Ok(true)
}
```

Other usage patterns:

```rust
if name.starts_with("Admin") { /* ... */ }
if name.contains("vip")      { /* ... */ }

let msg = format!("Welcome, {}!", &**name);
connect_to_server(&**name);
```

> The decoding is **lazy**: the underlying `String` is only built on
> the first `Deref` access, then cached in a `OnceCell<String>`. If the
> native never touches the content, no `String` is allocated.

### 3. Output strings — `write_str`

**Before:**

```rust
fn get_value(_amx: &Amx, buffer: UnsizedBuffer, size: usize) -> AmxResult<bool> {
    let mut buf = buffer.into_sized_buffer(size);
    let _ = samp::cell::string::put_in_buffer(&mut buf, "value");
    Ok(true)
}
```

**Now:**

```rust
fn get_value(_amx: &Amx, buffer: UnsizedBuffer, size: usize) -> AmxResult<bool> {
    buffer.write_str(size, "value")?;
    Ok(true)
}
```

The previous helper silenced the error with `let _ = …`. `write_str`
propagates `AmxError::General` through `?` when the encoded string is
too long for the buffer.

Available on both `Buffer` and `UnsizedBuffer`:

```rust
let mut buf = allocator.allot_buffer(32)?;
buf.write_str("Hello, AMX")?;
```

> Internally `put_in_buffer` still exists but is `pub(crate)` — every
> public surface goes through `write_str`.

### 4. Typed Pawn arrays — `get_as` / `set_as`

`Float:arr[]` and `bool:arr[]` no longer require manual bit
manipulation.

**Before:**

```rust
fn process_floats(_amx: &Amx, array: UnsizedBuffer, len: usize) -> AmxResult<bool> {
    let buf = array.into_sized_buffer(len);
    for i in 0..buf.len() {
        let value = f32::from_bits(buf[i] as u32); // manual conversion
        println!("{value}");
    }
    Ok(true)
}
```

**Now:**

```rust
fn process_floats(_amx: &Amx, array: UnsizedBuffer, len: usize) -> AmxResult<bool> {
    let buf = array.into_sized_buffer(len);
    for i in 0..buf.len() {
        if let Some(value) = buf.get_as::<f32>(i) {
            println!("{value}");
        }
    }
    Ok(true)
}
```

Types supported by `get_as` / `set_as` / `iter_as`: `i8`, `u8`, `i16`,
`u16`, `i32`, `u32`, `isize`, `usize`, `f32`, `bool`.

> `get_as` and `set_as` rely on the `CellConvert` trait, not on
> `AmxCell`. `AmxCell` converts native arguments; `CellConvert`
> converts individual cells of a buffer. They live in different layers
> intentionally — `CellConvert` does not need an `&Amx`.

## v3.0.0 → v3.1.0

No breaking changes — `rust-samp 3.1.0` adds a turnkey logger module
and bumps the workspace MSRV to **Rust 1.87** (required by stable
`i32::cast_unsigned` / `u32::cast_signed`). Existing plugins keep
compiling without edits, and the previous `samp::plugin::logger()`
helper is unchanged.

### What's new

- `samp::logger::LoggerConfig` — fluent builder for the new logger.
- `samp::enable_logger!()` — turnkey installation with sensible
  defaults (directory `logs/`, filename `{crate}.log`, prefix
  `[{crate}]`, 50 MB size-based rotation into `logs/archive/`,
  banner read from `CARGO_PKG_*`).
- `samp::enable_logger_with!(cfg)` — same pipeline with an explicit
  `LoggerConfig`. Customizable: file/server format templates with
  alignment specifiers, banner mode (off / default / custom closure),
  append-style vs shift-style rotation, runtime-adjustable level.
- `samp::logger::set_level(...)` / `samp::logger::level()` — runtime
  hooks for plugins that expose a Pawn-side level knob (the
  Pawn-side knob for log verbosity).

### Adopting the new logger

Plugins that used to build a `fern::Dispatch` by hand:

**Before:**

```rust
initialize_plugin!(
    natives: [],
    {
        let log_file = fern::log_file("my-plugin.log")
            .expect("failed to open log file");

        let _ = fern::Dispatch::new()
            .level(log::LevelFilter::Info)
            .format(|out, msg, rec| {
                out.finish(format_args!(
                    "[my-plugin][{}]: {}",
                    rec.level(), msg,
                ));
            })
            .chain(samp::plugin::logger())
            .chain(log_file)
            .apply();

        return MyPlugin::default();
    }
);
```

**Now:**

```rust
impl SampPlugin for MyPlugin {
    fn on_load(&mut self) {
        let _ = samp::enable_logger!();
    }
}

initialize_plugin!(type: MyPlugin, natives: []);
```

Same behaviour out of the box: per-plugin file under `logs/`, server
console prefixed with `[my-plugin]`, and size-based rotation into
`logs/archive/`. The new logger also provides a banner, format
templates, runtime level adjustment and append-style rotation that
never deletes archives — all opt-in. See [Logging](logging.md) for the
full reference.

The DIY `samp::plugin::logger()` path keeps working unchanged for the
plugins that need it (custom destinations, JSON output, log shippers).

### Crate names on crates.io

If `rust-samp 3.1.0` is consumed via crates.io rather than as a git
dependency, the package name is `rust-samp` while the **library** name
remains `samp`. Plugins keep writing `use samp::prelude::*;` after
adding an alias to `Cargo.toml`:

```toml
[dependencies]
samp = { package = "rust-samp", version = "3" }
```

Git-based consumers (`samp = { git = "https://github.com/NullSablex/rust-samp" }`)
do not need any change — the workspace exposes both the package alias
and the library identifier.

## v2.x → v3.0.0 — native Open Multiplayer support

No source-level breaking changes. Existing code compiles unchanged.
What changes is **what the SDK generates by default**.

### Summary

Starting with v3.0.0, every build that does **not** enable the
`samp-only` feature emits the SA-MP exports **and** the Open
Multiplayer `ComponentEntryPoint`. The same binary loads on SA-MP and
is treated as a first-class component on Open Multiplayer.

| Version                   | Generated binary                                                 |
| ------------------------- | ---------------------------------------------------------------- |
| v2.x                      | SA-MP exports only.                                              |
| v3.0.0 (default)          | SA-MP exports **and** `ComponentEntryPoint`.                     |
| v3.0.0 with `samp-only`   | SA-MP exports only (identical to v2.x).                          |

### Unified `on_tick`

The trait method previously called `process_tick` is now
`on_tick(&mut self, ctx: TickContext)`. The opt-in switched from
`samp::plugin::enable_process_tick()` to
`samp::plugin::enable_tick()` (or `enable_tick_with(TickConfig)` for
custom interval / per-server control).

The unified callback fires on both servers:

- SA-MP — the `ProcessTick` export forwards to `on_tick` with
  `ctx.source == TickSource::SaMp`. Cadence is whatever the server's
  main loop is configured for.
- Native Open Multiplayer — the SDK queries `ITimersComponent` and
  creates a repeating timer whose timeout dispatches the same
  callback with `ctx.source == TickSource::OmpTimer`.
  Interval defaults to 5 ms; configurable through
  `TickConfig::omp_interval`.

`ctx.elapsed` is the wall-clock time since the previous dispatch
(zero on the first call), useful for delta-based logic without
calling `Instant::now()` in the plugin.

Common `TickConfig` patterns come with builder shortcuts:

```rust
use std::time::Duration;
use samp::plugin::{enable_tick_with, TickConfig};

// SA-MP only — Open Multiplayer timer disabled.
enable_tick_with(TickConfig::sa_mp_only());

// Open Multiplayer only, at a custom interval — SA-MP export stays inert.
enable_tick_with(TickConfig::omp_only(Duration::from_millis(50)));

// Full builder when you need both servers with tweaked Open Multiplayer cadence.
enable_tick_with(TickConfig::new().omp_interval(Duration::from_millis(20)));
```

### Targets

| Platform | Target                       | SA-MP | Native Open Multiplayer |
| -------- | ---------------------------- | :---: | :---------------------: |
| Linux    | `i686-unknown-linux-gnu`     |   ✅   |  ✅                     |
| Windows  | `i686-pc-windows-msvc`       |   ✅   |  ✅                     |
| Windows  | `i686-pc-windows-gnu`        |   ✅   |  ❌                     |

For Windows builds with native Open Multiplayer support, cross-compile
from Linux through `cargo-xwin`:

```sh
cargo install cargo-xwin
cargo xwin build --xwin-arch x86 --target i686-pc-windows-msvc
```

`i686-pc-windows-gnu` does **not** support native Open Multiplayer —
use it only for SA-MP-only builds (`--features samp-only`).

### Compile-time error after upgrading

```
error[E0080]: evaluation panicked: OmpComponent: invalid size for the
Itanium ABI. Use --target i686-unknown-linux-gnu to compile with
native Open Multiplayer support.
```

**Cause:** the build target is x86_64 instead of i686. The SDK
validates `OmpComponent`'s layout at compile time against the i686
Itanium ABI, and on x86_64 pointers are 8 bytes.

**Fix:** create `.cargo/config.toml` at the project root:

```toml
[build]
# Linux — SA-MP + native Open Multiplayer (default)
target = "i686-unknown-linux-gnu"

# Windows — SA-MP + native Open Multiplayer (requires cargo-xwin)
# target = "i686-pc-windows-msvc"

# Windows — SA-MP only (combine with the samp-only feature)
# target = "i686-pc-windows-gnu"
```

### Option A — keep v2.x behavior (SA-MP only)

Enable the `samp-only` feature. The `ComponentEntryPoint` is not
emitted; the plugin behaves exactly like in v2.x:

```toml
[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "v3.0.0", features = ["samp-only"] }
```

No other change is required.

### Option B — adopt native Open Multiplayer support

Update the dependency without `samp-only`. The SDK emits the
`ComponentEntryPoint` and, when the UID is missing, derives one via
FNV-1a and writes it back to `Cargo.toml`:

```toml
[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "v3.0.0" }
```

After the first build the `Cargo.toml` ends up with a new section:

```toml
[package.metadata.samp]
uid = "0x<generated_value>"
```

That is the only required change. The same binary loads on SA-MP and
is recognized as a native component by Open Multiplayer.

### Option B+ — react to Open Multiplayer-only events

To hook into events specific to native Open Multiplayer, add the
optional methods to the trait impl:

```rust
impl SampPlugin for MyPlugin {
    fn on_load(&mut self) {
        // called on SA-MP and on Open Multiplayer
    }

    // Every Open Multiplayer component finished initializing.
    // The #[cfg] is required only if the plugin must compile both
    // with and without the samp-only feature.
    #[cfg(not(feature = "samp-only"))]
    fn on_omp_ready(&mut self) {
        if let Some(_core) = samp::plugin::omp_core() {
            log::info!("running on native Open Multiplayer");
        }
    }

    #[cfg(not(feature = "samp-only"))]
    fn on_component_free(&mut self) {
        log::info!("an Open Multiplayer component was released");
    }
}
```

### Migration checklist

- [ ] Update the dependency tag to `v3.0.0`.
- [ ] Configure `.cargo/config.toml` with the correct i686 target (if
      not yet done).
- [ ] Decide: keep `samp-only` for SA-MP-only behavior, or drop the
      feature to enable dual support.
- [ ] If using dual support: rename any `process_tick` overrides to
      `on_tick(&mut self, ctx: TickContext)`, and the opt-in call to
      `enable_tick()` (or `enable_tick_with(...)` for custom interval
      / per-server control). The `TickContext` parameter is mandatory
      — use `_ctx` if you don't read it.
- [ ] Build once and verify the UID was written into
      `[package.metadata.samp]`.

## Legacy `samp_sdk` → current API

This section covers the move from the original `samp_sdk` (pre-v1)
to the current `samp` crate.

### Summary

| Before                                  | Now                                                                            |
| --------------------------------------- | ------------------------------------------------------------------------------ |
| `samp_sdk = "*"`                        | `samp = { package = "rust-samp", version = "3" }` (crates.io, v3.1.0+) — or `samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "v3.1.0" }` (any version) |
| `new_plugin!(Plugin)`                   | `initialize_plugin!(type: T, natives: [...])` or constructor-block form        |
| `define_native!(name, args)`            | `#[native(name = "Name")]`                                                     |
| `impl Default for Plugin`               | `#[derive(Default)]` or a constructor block                                    |
| `AMX` (raw)                             | `Amx` (safe wrapper)                                                           |
| `Cell`                                  | `i32`, `Ref<T>`, `AmxString`, custom `AmxCell` impls                           |
| Manual native registration              | Automatic, through `initialize_plugin!`                                        |
| `string.to_string()`                    | `&*string` via `Deref<Target = str>`                                           |
| `process_tick`                          | `on_tick(ctx: TickContext)` (unified across servers; opt in via `enable_tick()` / `enable_tick_with(TickConfig)`) |

### 1. Update `Cargo.toml`

From crates.io (v3.1.0 onwards):

```diff
- [dependencies]
- samp_sdk = "*"

+ [dependencies]
+ samp = { package = "rust-samp", version = "3" }
```

Or via git (any version, including v3.0.0 and earlier):

```diff
- [dependencies]
- samp_sdk = "*"

+ [dependencies]
+ samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "v3.1.0" }
```

### 2. Update imports

```diff
- use samp_sdk::new_plugin;
- use samp_sdk::...;

+ use samp::prelude::*;
+ use samp::{native, initialize_plugin, SampPlugin};
```

### 3. Replace `define_native!` with `#[native]`

**Before:**

```rust
define_native!(my_native, string: String);
define_native!(raw_native as raw);
```

**Now:**

```rust
#[native(name = "MyNative")]
fn my_native(&mut self, _amx: &Amx, text: &AmxString) -> AmxResult<bool> {
    println!("{}", &**text);
    Ok(true)
}

#[native(name = "RawNative", raw)]
fn raw_native(&mut self, amx: &Amx, args: Args) -> AmxResult<f32> {
    Ok(1.0)
}
```

### 4. Replace `new_plugin!` with `initialize_plugin!`

**Before:**

```rust
impl Default for Plugin {
    fn default() -> Plugin { Plugin { /* ... */ } }
}
new_plugin!(Plugin);
```

**Now (short form):**

```rust
#[derive(SampPlugin, Default)]
struct Plugin;

initialize_plugin!(
    type: Plugin,
    natives: [Plugin::my_native],
);
```

**Now (full form):**

```rust
initialize_plugin!(
    natives: [Plugin::my_native],
    { return Plugin { /* ... */ }; }
);
```

### 5. Update the lifecycle impl

```rust
// Without overrides — use the derive
#[derive(SampPlugin, Default)]
struct Plugin;

// With overrides — write the impl by hand
impl SampPlugin for Plugin {
    fn on_load(&mut self) {
        // native registration is automatic
    }
    fn on_unload(&mut self) { }
}
```
