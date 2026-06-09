# Plugin anatomy

Every rust-samp plugin follows the same shape: a Rust type that
implements `SampPlugin`, methods annotated with `#[native]`, and a
single `initialize_plugin!` invocation that wires it all together.

## The `SampPlugin` trait

`SampPlugin` defines the plugin's lifecycle. Every method is optional —
the trait provides empty defaults.

```rust
pub trait SampPlugin {
    /// Called once after the server loads the plugin
    /// (`Load()` on SA-MP, `onLoad(ICore*)` on native Open Multiplayer).
    fn on_load(&mut self) {}

    /// Called when the server unloads the plugin.
    fn on_unload(&mut self) {}

    /// A Pawn script (`.amx`) was loaded.
    fn on_amx_load(&mut self, amx: &Amx) {}

    /// A Pawn script is being unloaded.
    fn on_amx_unload(&mut self, amx: &Amx) {}

    /// Periodic callback. Requires opting in via
    /// `samp::plugin::enable_tick()` (or `enable_tick_with(...)`).
    /// `TickContext::source` distinguishes SA-MP's main-loop tick
    /// from the SDK-owned Open Multiplayer timer; `TickContext::elapsed`
    /// is the wall-clock time since the previous dispatch.
    fn on_tick(&mut self, ctx: TickContext) {}

    /// Every Open Multiplayer component has finished initializing.
    /// Compiled only when the `samp-only` feature is **not** active.
    #[cfg(not(feature = "samp-only"))]
    fn on_omp_ready(&mut self) {}

    /// An Open Multiplayer component is being released.
    /// Compiled only when the `samp-only` feature is **not** active.
    #[cfg(not(feature = "samp-only"))]
    fn on_component_free(&mut self) {}
}
```

The two Open Multiplayer-only hooks exist only when the `samp-only`
feature is **not** set. Plugins that must compile both with and without
that feature should gate their overrides with
`#[cfg(not(feature = "samp-only"))]`.

> See [Native Open Multiplayer support](omp-native.md) for the full
> Open Multiplayer lifecycle and feature-flag matrix.

Because every method has a default, a plugin without overrides can use
the derive instead of writing the impl by hand:

```rust
#[derive(SampPlugin, Default)]
struct MyPlugin;
```

> `#[derive(SampPlugin)]` emits exactly `impl SampPlugin for T {}`. If a
> method needs custom logic (`on_load`, `on_tick`, …), write the
> impl by hand and drop the derive.

### Plugin state

The plugin struct is mutable (`&mut self`), so it can hold state across
calls:

```rust
struct MyPlugin {
    players_online: u32,
    sessions: Vec<String>,
}

impl SampPlugin for MyPlugin {
    fn on_load(&mut self) {
        self.players_online = 0;
        println!("Plugin ready.");
    }
}
```

### Order of execution

1. `initialize_plugin! { ... }` — instantiate the plugin.
2. `on_load` — once, after the server loads the plugin.
3. `on_amx_load` — each time a Pawn script is loaded.
4. `on_tick` — repeatedly, while enabled.
5. `on_amx_unload` — each time a Pawn script is unloaded.
6. `on_unload` — once, before shutdown.

On native Open Multiplayer, `on_omp_ready` fires between `on_load` and
the first `on_amx_load`, and `on_component_free` fires when any other
component is released.

## The `initialize_plugin!` macro

`initialize_plugin!` does three things:

1. Registers the plugin's natives.
2. Constructs the plugin instance.
3. Emits every server-required entry point — SA-MP exports (`Load`,
   `Unload`, `Supports`, `AmxLoad`, `AmxUnload`, `ProcessTick`) and, by
   default, the Open Multiplayer `ComponentEntryPoint`.

### Short form — `type: T`

For plugins without initialization logic. Uses `Default::default()` as
the constructor:

```rust
#[derive(SampPlugin, Default)]
struct MyPlugin;

initialize_plugin!(
    type: MyPlugin,
    natives: [
        MyPlugin::function_a,
        MyPlugin::function_b,
    ],
);
```

### Full form — constructor block

For plugins that need to set up logging, encoding, the server tick, or
that build initial state:

```rust
initialize_plugin!(
    natives: [
        MyPlugin::function_a,
        MyPlugin::function_b,
    ],
    {
        samp::plugin::enable_tick();
        samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1251);

        return MyPlugin {
            players_online: 0,
            sessions: Vec::new(),
        };
    }
);
```

> The constructor block **must** end with `return <instance>;`. Any
> code preceding the `return` runs exactly once, when the server loads
> the plugin.

> **Where should `enable_logger!` go?** Inside `on_load`, not in the
> constructor block. The server's log sink is not connected yet during
> construction — any `log::*` call would fall back to `eprintln!`. See
> [Logging](logging.md#common-pitfalls) for the full explanation.

### Native Open Multiplayer metadata

Native Open Multiplayer mode is the **default**: every build without the
`samp-only` feature emits both the SA-MP exports and the
`ComponentEntryPoint`. No extra configuration is required.

The component UID is resolved from three sources, in priority order:

1. `uid: 0x..._u64` declared inside `initialize_plugin!`.
2. `[package.metadata.samp] uid = "0x..."` in `Cargo.toml`.
3. FNV-1a 64-bit hash of `CARGO_PKG_NAME@CARGO_PKG_VERSION`. The
   computed value is written back into `Cargo.toml` under
   `[package.metadata.samp]` on the next build, so subsequent builds
   reuse the same value.

To declare the metadata directly in `Cargo.toml`:

```toml
[package.metadata.samp]
uid     = "0x4D455550CAFEBABE"
name    = "MyPlugin"             # optional — default: CARGO_PKG_NAME
version = "1.0.0"                # optional — default: CARGO_PKG_VERSION
```

To declare it directly in code (macro values take precedence over
`Cargo.toml`):

```rust
initialize_plugin!(
    uid: 0x4D455550CAFEBABE_u64,
    component_name: "MyPlugin",     // optional
    component_version: (1, 0, 0),   // optional
    natives: [MyPlugin::function_a],
    { return MyPlugin::new(); }
);
```

See [Native Open Multiplayer support](omp-native.md) for the full
explanation.

### No natives

If the plugin only reacts to events:

```rust
// Short form
initialize_plugin!(type: MyPlugin, natives: []);

// Full form
initialize_plugin!(
    natives: [],
    { return MyPlugin; }
);
```

## Enabling the periodic tick

By default `on_tick` is **not** called. Opt in inside the constructor
block:

```rust
initialize_plugin!(
    natives: [],
    {
        samp::plugin::enable_tick();
        return MyPlugin::default();
    }
);
```

The two servers schedule the callback differently:

- **SA-MP** — `Supports::PROCESS_TICK` is advertised and the server
  calls `ProcessTick` on every iteration of its main loop. The
  cadence is whatever the server is configured for; the SDK has no
  control over it.
- **Native Open Multiplayer** — there is no native `ProcessTick`
  equivalent. The SDK queries `ITimersComponent` in `on_ready` and
  installs a repeating timer; its timeout dispatches `on_tick`. The
  interval is configurable.

For custom cadence or per-server control, use the explicit form:

```rust
use std::time::Duration;
use samp::plugin::{enable_tick_with, TickConfig};

// Both servers, but 100 ms on Open Multiplayer instead of 5 ms:
enable_tick_with(TickConfig::new().omp_interval(Duration::from_millis(100)));

// Shortcut: SA-MP only, no Open Multiplayer timer.
enable_tick_with(TickConfig::sa_mp_only());

// Shortcut: Open Multiplayer only, at the given interval.
enable_tick_with(TickConfig::omp_only(Duration::from_millis(50)));
```

`TickContext::source` (passed to `on_tick`) reports
`TickSource::SaMp` or `TickSource::OmpTimer`, so the same
method can branch on origin when needed.

## Lifecycle diagrams

### SA-MP

```
Server start
  └─ Plugin load
       ├─ initialize_plugin! { ... }    ← construct the instance
       ├─ on_load()
       ├─ Gamemode loaded → on_amx_load(amx)
       ├─ [loop] on_tick(ctx)               (if enabled — cadence dictated by the server)
       ├─ Gamemode unloaded → on_amx_unload(amx)
       └─ on_unload()
Server shutdown
```

### Native Open Multiplayer

```
Server start
  └─ Plugin load (ComponentEntryPoint)
       ├─ initialize_plugin! { ... }    ← construct the instance
       ├─ on_load()                      ← from comp_on_load(ICore*)
       ├─ on_omp_ready()                 ← every component initialized
       ├─ Script loaded → on_amx_load(amx)
       ├─ [loop] on_tick(ctx)             (if enabled — SDK-owned ITimer at configured interval)
       ├─ on_component_free()             ← another component being released
       ├─ Script unloaded → on_amx_unload(amx)
       └─ on_unload()                    ← from comp_free
Server shutdown
```
