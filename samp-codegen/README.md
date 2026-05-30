# rust-samp-codegen

Proc macros that generate the FFI boilerplate for
[`rust-samp`](https://crates.io/crates/rust-samp) plugins.

Provides:

- `#[native(name = "Pawn_Name")]` — turns a Rust method or associated
  function into a Pawn-callable native. Generates the `extern "C"` wrapper,
  argument parsing via `AmxCell`, panic isolation, and the
  `__samp_reg_*` registration function. Detects `Result<T, E>` vs `T`
  return types automatically.
- `initialize_plugin!(type: T, natives: [...])` — emits the SA-MP entry
  points (`Load`, `Unload`, `AmxLoad`, `AmxUnload`, `Supports`,
  `ProcessTick`) and the open.mp `ComponentEntryPoint`, including the
  vtable definitions for both Itanium (Linux GCC) and MSVC ABIs.
- `#[derive(SampPlugin)]` — generates `impl SampPlugin for T {}` with all
  defaults.

Re-exported by `rust-samp`; you do not need to depend on this crate
directly unless you are building tooling on top of the codegen.

```toml
[dependencies]
# Re-exported by `rust-samp`; depend on `rust-samp-codegen` only for
# tooling that targets the macros directly.
samp-codegen = { package = "rust-samp-codegen", version = "1" }
```

## Repository

<https://github.com/NullSablex/rust-samp>
