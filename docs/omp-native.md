# Native Open Multiplayer support

rust-samp supports SA-MP and Open Multiplayer from the same binary.
This page explains how the native Open Multiplayer integration works,
what the plugin code needs to know about it, and how the two
environments coexist.

## Legacy mode vs. native mode

When Open Multiplayer loads a plugin that does **not** expose
`ComponentEntryPoint`, it treats it exactly the way SA-MP would —
**legacy mode**. The plugin runs, but cannot reach Open Multiplayer's
component APIs (`ICore`, `queryComponent`, etc.).

When Open Multiplayer finds `ComponentEntryPoint` in the binary, it
loads the plugin as a first-class **component**: native mode. The
plugin gets access to the server's internal APIs and an extended
lifecycle.

The decision between the two modes is automatic — the server picks the
entry point it can use.

## Default: dual support

Native mode is the **default**. Every build without the `samp-only`
feature emits the SA-MP exports **and** the `ComponentEntryPoint` for
Open Multiplayer. No extra configuration is required.

| Configuration                | Entry points generated                       | Behavior on Open Multiplayer       |
| ---------------------------- | -------------------------------------------- | ---------------------------------- |
| Default (no `samp-only`)     | SA-MP exports + `ComponentEntryPoint`        | Loaded as a native component.      |
| `samp-only` feature          | SA-MP exports only                           | Loaded in legacy mode.             |

> Enable `samp-only` only when you want to remove every byte of
> Open Multiplayer code from the binary. A plugin without the feature
> runs normally on SA-MP and is treated as a native component on
> Open Multiplayer.

## Component UID

The UID is a 64-bit identifier that uniquely names the component on
the Open Multiplayer server. UIDs must be globally unique — collisions
prevent the server from telling two components apart.

### Generating a UID

You do not need to pick one manually. When `samp-only` is not active
and no `uid` is declared in the macro or in `Cargo.toml`, the SDK
derives a value via 64-bit FNV-1a of
`CARGO_PKG_NAME@CARGO_PKG_VERSION` and writes it back into
`[package.metadata.samp]` in `Cargo.toml` on the next build, so the
value remains stable.

To set the UID by hand, any tool that emits 64 random bits will do:

```sh
openssl rand -hex 8
python3 -c "import random; print(hex(random.getrandbits(64)))"
node -e "console.log('0x'+require('crypto').randomBytes(8).toString('hex'))"
```

A UID never needs to change once chosen. Never reuse a UID across
projects.

### Where the UID can live

| Priority | `uid`                                | `component_name`                          | `component_version`                          |
| :------: | ------------------------------------ | ----------------------------------------- | -------------------------------------------- |
| 1        | `uid: 0x..._u64` in the macro        | `component_name: "..."` in the macro      | `component_version: (x, y, z)` in the macro  |
| 2        | `[package.metadata.samp] uid`        | `[package.metadata.samp] name`            | `[package.metadata.samp] version`            |
| 3        | FNV-1a of `CARGO_PKG_NAME@CARGO_PKG_VERSION` (persisted to `Cargo.toml`) | `CARGO_PKG_NAME` | parsed `CARGO_PKG_VERSION` (defaults to `1.0.0`) |

`component_name` and `component_version` are always optional. When both
the macro and `Cargo.toml` are silent, the SDK falls back to the
crate metadata automatically. The fields can be mixed — each one
resolves independently.

## What changes in the plugin code

### 1. Pick the metadata format (or accept the defaults)

#### Format A — `Cargo.toml` (recommended)

```toml
[package.metadata.samp]
uid     = "0x4D455550CAFEBABE"
name    = "MyPlugin"             # optional — default: CARGO_PKG_NAME
version = "1.0.0"                # optional — default: CARGO_PKG_VERSION
```

`initialize_plugin!` itself stays identical to a SA-MP-only plugin:

```rust
initialize_plugin!(
    natives: [MyPlugin::my_native],
    { return MyPlugin::new(); }
);
```

#### Format B — fully in code

```rust
initialize_plugin!(
    uid: 0x4D455550CAFEBABE_u64,
    component_name: "MyPlugin",     // optional
    component_version: (1, 0, 0),   // optional
    natives: [MyPlugin::my_native],
    { return MyPlugin::new(); }
);
```

### 2. Implement the optional Open Multiplayer hooks

The trait exposes two extra methods when `samp-only` is not active:

```rust
impl SampPlugin for MyPlugin {
    fn on_load(&mut self) {
        log::info!("plugin loaded");
    }

    // All Open Multiplayer components have finished initializing.
    #[cfg(not(feature = "samp-only"))]
    fn on_omp_ready(&mut self) {
        log::info!("Open Multiplayer: every component ready");
    }

    // Some Open Multiplayer component is being released.
    #[cfg(not(feature = "samp-only"))]
    fn on_component_free(&mut self) {
        log::info!("Open Multiplayer: a component was released");
    }
}
```

> The `#[cfg(not(feature = "samp-only"))]` attributes are required
> only if the plugin must compile both with and without the feature.
> If you never enable `samp-only`, the cfg gates are optional.

### 3. Gate Open Multiplayer helpers with `cfg` if needed

`samp::plugin::omp_core`, `omp_query_component`, and `omp_query` are
compiled only when `samp-only` is not active:

```rust
#[cfg(not(feature = "samp-only"))]
fn on_omp_ready(&mut self) {
    if let Some(_core) = samp::plugin::omp_core() {
        log::info!("ICore available");
    }
}
```

## End-to-end example

```toml
# Cargo.toml
[package.metadata.samp]
uid = "0x4D455550CAFEBABE"
```

```rust
use samp::prelude::*;
use samp::{initialize_plugin, native};

struct MyPlugin {
    count: u32,
}

impl MyPlugin {
    fn new() -> Self { MyPlugin { count: 0 } }

    #[native(name = "IncrementCount")]
    fn increment(&mut self, _amx: &Amx) -> i32 {
        self.count += 1;
        i32::try_from(self.count).unwrap_or(i32::MAX)
    }
}

impl SampPlugin for MyPlugin {
    fn on_load(&mut self)            { log::info!("MyPlugin loaded"); }
    fn on_unload(&mut self)          { log::info!("MyPlugin unloaded"); }
    fn on_amx_load(&mut self, _: &Amx) { log::info!("AMX script loaded"); }

    #[cfg(not(feature = "samp-only"))]
    fn on_omp_ready(&mut self) {
        log::info!("Open Multiplayer native: every component ready");
    }
}

initialize_plugin!(
    natives: [MyPlugin::increment],
    { return MyPlugin::new(); }
);
```

## Lifecycle compared

### SA-MP

```
Server start
  └─ Supports()
  └─ Load()             → on_load()
  └─ AmxLoad()          → on_amx_load(amx)
  └─ [loop] ProcessTick()   → on_server_tick() (when enabled)
  └─ AmxUnload()        → on_amx_unload(amx)
  └─ Unload()           → on_unload()
Server shutdown
```

### Native Open Multiplayer

```
Server start
  └─ ComponentEntryPoint()                  → plugin constructed
  └─ comp_on_load(ICore*)                   → on_load()
  └─ comp_on_init(IComponentList*)          → [SDK registers PawnEventHandler]
  └─ comp_on_ready()                        → [SDK stores getAmxFunctions(); creates ITimer if enabled] → on_omp_ready()
  └─ pawn_on_amx_load(IPawnScript*)         → on_amx_load(amx)
  └─ [loop] ITimer timeout (5 ms)           → on_server_tick() (when enabled)
  └─ pawn_on_amx_unload(IPawnScript*)       → on_amx_unload(amx)
  └─ comp_on_free()                         → on_component_free()
  └─ comp_free()                            → [SDK kills the timer, removes the dispatcher handler] → on_unload()
Server shutdown
```

### Unified `on_server_tick`

The same callback fires on both servers:

- **SA-MP** advertises `Supports::PROCESS_TICK` and routes the
  `ProcessTick` export into `on_server_tick`.
- **Native Open Multiplayer** queries `ITimersComponent` in `on_ready`
  and creates a repeating timer at 5 ms whose timeout dispatches
  `on_server_tick`.

Both paths require `samp::plugin::enable_server_tick()` to opt in
inside the constructor block.

## Runtime detection

```rust
#[cfg(not(feature = "samp-only"))]
fn on_load(&mut self) {
    if samp::plugin::omp_core().is_some() {
        log::info!("running on native Open Multiplayer");
    } else {
        log::info!("running on SA-MP (or Open Multiplayer legacy mode)");
    }
}
```

`omp_core()` returns `Some` only when the plugin was loaded through
`ComponentEntryPoint`.

## Open Multiplayer APIs

Available under `samp::plugin` when `samp-only` is not active:

### `omp_core() -> Option<*mut ICore>`

Returns the `ICore` pointer cached in `on_load`. `None` when the
plugin runs on SA-MP or in Open Multiplayer's legacy mode.

### `omp_query_component(uid: UID) -> Option<*mut ServerComponent>`

Looks up a component by UID in the list received in `on_init`.

```rust
#[cfg(not(feature = "samp-only"))]
fn on_omp_ready(&mut self) {
    let other_uid: samp_sdk::omp::types::UID = 0x12345678DEADBEEF_u64;
    if let Some(_comp) = samp::plugin::omp_query_component(other_uid) {
        log::info!("component found");
    }
}
```

### `omp_query::<T>() -> Option<T>`

Typed variant that requires `T: OmpComponentHandle`. Ships with two
ready-made wrappers:

- `samp_sdk::omp::PawnComponent` (UID `0x7890_6cd9_f19c_36a6`) —
  exposes `event_dispatcher`, `amx_functions`, `name`, `version`.
- `samp_sdk::omp::TimersComponent` (UID `0x2ad8_124c_5ea2_57a3`) —
  exposes `create_repeating`, `name`, `version`.

External plugins can implement the trait with their own UID to plug
into the same API.

> Call these helpers inside `on_omp_ready`, not `on_load`. Other
> components may still be initializing earlier in the cycle.

## Diagnostics

The SDK emits warnings via the standard `log::warn!` macro when
something in the Open Multiplayer lifecycle goes wrong. Messages are
prefixed with `[rust-samp]`.

| Situation                                    | Message                                                                                                  | Consequence                                            |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `ICore*` null in `on_load`                   | `null ICore* in on_load — samp::plugin::omp_core() will return None`                                     | Plugin runs, but `omp_core()` is `None`.               |
| `IPawnComponent` missing in `on_init`        | `IPawnComponent not found in on_init — Pawn natives unavailable`                                         | Natives are not registered.                            |
| `getAmxFunctions()` returns `0` in `on_ready`| `getAmxFunctions() returned 0 in on_ready — Pawn natives unavailable`                                    | Natives are not registered.                            |
| `IEventDispatcher` null in `on_init`         | `null IEventDispatcher<PawnEventHandler> in on_init — on_amx_load/on_amx_unload will not be called`      | Loaded scripts do not trigger the AMX hooks.           |
| `ITimersComponent` missing (when tick on)    | `ITimersComponent not found — on_server_tick will not be called`                                         | Tick callback does not fire on Open Multiplayer.       |

## Platform support

| Target                    | SA-MP | Native Open Multiplayer |
| ------------------------- | :---: | :----------------------: |
| `i686-unknown-linux-gnu`  |   ✅   |  ✅                      |
| `i686-pc-windows-msvc`    |   ✅   |  ✅                      |
| `i686-pc-windows-gnu`     |   ✅   |  ❌                      |

`i686-pc-windows-msvc` requires cross-compilation from Linux via
`cargo-xwin` (see [Setup](setup.md)). `i686-pc-windows-gnu` does not
support native Open Multiplayer — use it only for SA-MP-only builds.

## Feature flags

| Feature      | Effect                                                                                       |
| ------------ | -------------------------------------------------------------------------------------------- |
| *(default)*  | Full dual support: SA-MP + native Open Multiplayer.                                          |
| `samp-only`  | Removes every Open Multiplayer code path; plugin still loads on Open Multiplayer in legacy mode. |
| `encoding`   | Enables `encoding_rs`-based string conversion (independent of the Open Multiplayer mode).    |

> `samp-only` does **not** prevent the plugin from running on
> Open Multiplayer — it forces legacy mode. Use it when you want to
> guarantee no Open Multiplayer code is compiled into the binary.
