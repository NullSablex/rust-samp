# Your first plugin

This chapter walks through building a small plugin that exposes a Pawn
native callable from the script.

## Minimal skeleton

After completing the [setup](setup.md), replace the contents of
`src/lib.rs` with:

```rust
use samp::prelude::*;
use samp::{initialize_plugin, SampPlugin};

#[derive(SampPlugin, Default)]
struct MyPlugin;

initialize_plugin!(
    type: MyPlugin,
    natives: [],
);
```

That is already a valid plugin. `#[derive(SampPlugin)]` generates an
empty `impl SampPlugin for MyPlugin {}`, and the short
`initialize_plugin!(type: T, ...)` form uses `Default::default()` as the
constructor.

> If the plugin needs initialization logic (logging, encoding, server
> tick, custom state), switch to the constructor-block form described
> in [Plugin anatomy](plugin-anatomy.md).

## Adding a native

Natives are methods (or associated functions) annotated with `#[native]`:

```rust
use samp::prelude::*;
use samp::{native, initialize_plugin, SampPlugin};

#[derive(SampPlugin, Default)]
struct MyPlugin;

impl MyPlugin {
    #[native(name = "RustSayHello")]
    fn say_hello(&mut self, _amx: &Amx, name: &AmxString) -> AmxResult<bool> {
        // AmxString implements Deref<Target = str> — &str methods are
        // available without an extra allocation.
        println!("Hello, {}!", &**name);
        Ok(true)
    }
}

initialize_plugin!(
    type: MyPlugin,
    natives: [MyPlugin::say_hello],
);
```

> The `#[native]` macro detects the `&AmxString` parameter and injects
> the borrow automatically at the call site. `args.next_arg()` still
> produces the owned value; the function sees it through a reference.

## Calling from Pawn

Declare the native in the Pawn script and call it normally:

```pawn
native RustSayHello(const name[]);

public OnGameModeInit()
{
    RustSayHello("World");
    return 1;
}
```

The console prints `Hello, World!`.

## What just happened

1. `#[derive(SampPlugin)]` emits `impl SampPlugin for MyPlugin {}` —
   every lifecycle method is left at its default (empty) implementation.
2. `initialize_plugin!(type: MyPlugin, ...)` uses
   `<MyPlugin as Default>::default()` as the constructor.
3. `#[native(name = "RustSayHello")]` generates the `extern "C"` wrapper
   the server expects, parses each argument via `AmxCell`, catches
   panics, and converts the `AmxResult<bool>` return value into the
   integer expected by the AMX VM (`true` → `1`, `false` → `0`).
4. `AmxString::deref` decodes the underlying Pawn cells on first access
   (Windows-1252 by default, or whatever was configured via the
   `encoding` feature) and caches the result in a `OnceCell<String>`,
   so repeated accesses are allocation-free.

## Next steps

- [Plugin anatomy](plugin-anatomy.md) — the full lifecycle and the
  constructor-block form of `initialize_plugin!`.
- [Natives](natives.md) — every option of `#[native]` and the supported
  argument/return types.
- [Advanced examples](advanced-examples.md) — a richer plugin that uses
  memcache, encoding, and a custom `fern` logger.
