# samp-codegen

Procedural macros for the `rust-samp` toolkit. Generates the `extern "C"`
entry points the SA-MP server expects and the `ComponentEntryPoint` required
by native Open Multiplayer.

> Plugin authors do not depend on this crate directly. The [`samp`](../samp)
> crate re-exports every macro via `use samp::{native, initialize_plugin, SampPlugin}`.

## `#[native]`

Turns a Rust function into a Pawn native. Accepted forms:

```rust
// Method on a `SampPlugin` impl
#[native(name = "MyNative")]
fn my_native(&mut self, amx: &Amx, name: &AmxString, value: i32) -> AmxResult<bool> {
    Ok(true)
}

// Associated function (no `self`)
#[native(name = "PureNative")]
fn pure_native(_amx: &Amx, count: i32) -> i32 { count * 2 }

// Raw mode — receives `Args` directly
#[native(name = "RawNative", raw)]
fn raw_native(&mut self, amx: &Amx, args: Args) -> AmxResult<i32> {
    Ok(args.count() as i32)
}
```

Behavior:

- The first parameter is `&mut self` for methods or omitted for associated
  functions; the second (or first, in associated functions) is `&Amx`.
- Subsequent parameters are parsed via `AmxCell::from_raw` in declaration
  order. A type written as `&T` is automatically taken by reference: the
  macro materializes the owned value from `args.next_arg()` and passes
  `&local` at the call site.
- Return type detection is syntactic: if the last path segment is `Result`
  or `AmxResult`, the wrapper matches `Ok`/`Err` (`Err` is logged via
  `samp::log::error!`, the native returns `0`). Any other return type is
  treated as a direct value implementing `AmxCell`.
- Panics that cross the FFI boundary are caught via `std::panic::catch_unwind`,
  logged, and converted to a `0` return. Without this, a panic crossing an
  `extern "C"` boundary aborts the whole server process.
- The native name is validated at proc-macro time: a `\0` inside the literal
  is rejected as a compile error rather than panicking at load time.

## `initialize_plugin!`

Registers natives and generates every server entry point. Two constructor
forms:

```rust
// Short form — relies on Default::default()
initialize_plugin!(
    type: MyPlugin,
    natives: [MyPlugin::function_a, MyPlugin::function_b],
);

// Full form — initialization block (must end with `return <instance>;`)
initialize_plugin!(
    natives: [MyPlugin::function_a],
    {
        samp::plugin::enable_server_tick();
        return MyPlugin::new();
    }
);
```

Optional Open Multiplayer metadata fields:

```rust
initialize_plugin!(
    uid: 0xDEADBEEFCAFEBABE_u64,      // default: FNV-1a 64 of CARGO_PKG_NAME@CARGO_PKG_VERSION
    component_name: "MyPlugin",        // default: CARGO_PKG_NAME
    component_version: (1, 0, 0),      // default: parsed CARGO_PKG_VERSION
    natives: [MyPlugin::function_a],
    { return MyPlugin::new(); }
);
```

Resolution order for every Open Multiplayer field:
**macro argument > `[package.metadata.samp]` in `Cargo.toml` > derived value**.

If the UID is missing from both the macro and `Cargo.toml`, the SDK derives
one via FNV-1a and **writes it back** into `Cargo.toml` under
`[package.metadata.samp]` so subsequent builds reuse the same value.

### What the macro emits

- SA-MP exports — always: `Load`, `Unload`, `Supports`, `AmxLoad`,
  `AmxUnload`, `ProcessTick`.
- Open Multiplayer entry point — when the `samp-only` feature is **not**
  active: a private module `__omp_component` containing the
  `IComponent`/`IUIDProvider` vtables for the active ABI (Itanium or MSVC),
  the constructor handlers (`comp_on_load`, `comp_on_init`, `comp_on_ready`,
  `comp_on_free`, `comp_free`, `comp_reset`), naked-assembly implementations
  of `componentName` and `componentVersion` for the MSVC hidden-pointer
  return convention, and the exported `ComponentEntryPoint`.

## `#[derive(SampPlugin)]`

Generates `impl samp::prelude::SampPlugin for T {}` with every method using
its default (empty) implementation. Suitable for stateless plugins:

```rust
#[derive(SampPlugin, Default)]
struct Stateless;
```

When the plugin needs to override any lifecycle hook (`on_load`,
`on_server_tick`, …), drop the derive and write `impl SampPlugin for T { ... }`
by hand.

## License

MIT.
