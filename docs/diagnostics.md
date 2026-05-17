# Diagnostics

The SDK emits warnings through the standard `log::warn!` macro
whenever something in the plugin lifecycle goes wrong silently. Every
message is prefixed with `[rust-samp]` so it can be grepped quickly in
the server log:

```
[rust-samp] null ICore* in on_load — samp::plugin::omp_core() will return None
```

The prefix is a single constant in `samp/src/macros.rs`
(`SDK_LOG_PREFIX`) — do not hardcode the literal in your own logs.

## Where they fire

These warnings cover the **native Open Multiplayer lifecycle**; on
SA-MP the SDK does not emit any of them (the corresponding failure
modes do not exist in that environment).

| Trigger                                                                 | Message                                                                                                            | Consequence                                                              |
| ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------ |
| `ICore*` was null in `comp_on_load`                                     | `null ICore* in on_load — samp::plugin::omp_core() will return None`                                              | Plugin keeps running; `omp_core()` returns `None`.                       |
| `IPawnComponent` missing in `comp_on_init`                              | `IPawnComponent not found in on_init — Pawn natives unavailable`                                                  | Native registration is skipped; Pawn cannot call your natives.           |
| `IEventDispatcher<PawnEventHandler>` null in `comp_on_init`             | `null IEventDispatcher<PawnEventHandler> in on_init — on_amx_load/on_amx_unload will not be called`               | Loaded Pawn scripts do not trigger the AMX hooks.                        |
| `getAmxFunctions()` returned `0` even at `on_ready`                     | `getAmxFunctions() returned 0 in on_ready — Pawn natives unavailable`                                             | Pawn natives are not registered, even after the deferred retry.          |
| `on_ready`: `IPawnComponent` could not be queried again                 | `on_ready: IPawnComponent not found`                                                                              | Same as the previous row — natives are not registered.                   |
| `ITimersComponent` missing while `enable_server_tick()` is on           | `ITimersComponent not found — on_server_tick will not be called`                                                  | Tick callback never fires on Open Multiplayer (SA-MP unaffected).        |
| `ITimersComponent::create()` returned null                              | `failed to create timer on ITimersComponent — on_server_tick will not be called`                                  | Same as above; the heap handler is freed before the warning is emitted.  |

## Panic safety

Beyond the warnings, the SDK wraps every callback that crosses the
FFI boundary in `std::panic::catch_unwind`. If a panic escapes the
plugin code:

- **Inside a native** — the `#[native]` wrapper catches the panic,
  logs `[<NativeName>] panic in native: <payload>` through
  `samp::log::error!`, and returns `0` to the AMX VM.
- **Inside an Open Multiplayer vtable callback** (`comp_on_load`,
  `comp_on_init`, `comp_on_ready`, `comp_on_free`, `comp_free`,
  `pawn_on_amx_load`, `pawn_on_amx_unload`, `tick_handler_timeout`,
  `tick_handler_free`) — the SDK catches the panic so the server
  process is **not aborted**.

Without those guards a panic crossing `extern "C"` aborts the entire
server (Rust 1.71+ guarantee).

## Native-call error logging

When a native is declared with a `Result`/`AmxResult` return, the
generated wrapper logs `Err` automatically:

```
[<NativeName>] <Display of the error value>
```

For argument parsing failures the wrapper logs a more detailed
message that includes the positional index and the expected type:

```
[<NativeName>] failed to parse argument #1 'key' (expected type: AmxString)
```

This kicks in only in the standard `#[native]` mode. In `raw` mode
the macro hands the `Args` over verbatim and parsing is the plugin's
responsibility.

## Reading the warnings

All messages flow through whatever `log` implementation the plugin
installed. With the default routing they end up wherever the server
sends `logprintf` (SA-MP) or `ICore::logLnU8` (Open Multiplayer).
With a custom `fern::Dispatch` they follow the dispatch chain
described in [Logging](logging.md).

A practical filter in the server console:

```sh
grep -E '^\[rust-samp\]|^\[<YourNativeName>\]' server_log.txt
```

## Where to look next when something is wrong

- `omp_core()` returns `None` despite running on Open Multiplayer →
  check the first warning above. The server may have passed a null
  `ICore*`, or the plugin is in legacy mode (no `ComponentEntryPoint`).
- A Pawn `native MyNative(...)` is reported as unresolved by the
  script compiler at runtime → check the `IPawnComponent` /
  `getAmxFunctions` warnings. The dispatcher might not have fired
  `on_amx_load` for this script either.
- `on_amx_load` is never called → look for the
  `IEventDispatcher<PawnEventHandler>` warning.
- `on_server_tick` is never called on Open Multiplayer → look for the
  `ITimersComponent` warnings. Make sure
  `samp::plugin::enable_server_tick()` is called inside the
  constructor block.
