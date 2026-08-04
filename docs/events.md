# Events

Events are Pawn **callbacks** — `OnPlayerConnect`, `OnPlayerSpawn`,
`OnPlayerText`, and friends — observed directly in Rust. Where a
[native](natives.md) is a function the gamemode *calls into*, a callback runs in
the gamemode's own script and the plugin only gets to *watch* it. The `#[event]`
attribute is what lets a Rust plugin react to them, closing the gap between
writing a plugin and writing a whole gamemode in Rust.

## How it differs from a native

The two macros share their argument-marshalling machinery, but the direction of
the call is opposite:

| | `#[native]` | `#[event]` |
| --- | --- | --- |
| Direction | Pawn → Rust | Pawn callback → Rust observer |
| Registered under | `natives: [...]` | `events: [...]` |
| Return value | Returned to the caller | Ignored (observer — see below) |
| Delivery | Server invokes the exported FFI wrapper | SDK intercepts `amx_Exec` |

## Basic shape

```rust
impl MyPlugin {
    #[event(name = "OnPlayerConnect")]
    fn on_player_connect(&mut self, _amx: &Amx, playerid: i32) -> AmxResult<i32> {
        log::info!("player {playerid} connected");
        Ok(1)
    }
}
```

```pawn
// The gamemode still defines the callback as usual — the Rust handler
// runs *before* this public executes.
public OnPlayerConnect(playerid) {
    return 1;
}
```

### Signature rules

The signature rules are identical to `#[native]`:

- The first parameter is `&mut self` for plugin methods. Associated functions
  (no `self`) are also accepted.
- The next parameter is `&Amx` — the AMX the callback is running in. Use
  `_amx: &Amx` when it is not needed.
- Subsequent parameters are the callback arguments, parsed via the `AmxCell`
  trait exactly like native arguments (see the
  [argument type table](natives.md#argument-types)).
- Returns either `AmxResult<T>` / `Result<T, E: Display>` (the wrapper logs the
  error on `Err`) or `T` directly for infallible handlers.

`name` is validated at proc-macro time — interior NUL bytes fail compilation
rather than panic at server load.

## Registering the handler

Every handler must appear in the `events: [...]` list of `initialize_plugin!`,
alongside the existing `natives: [...]`:

```rust
initialize_plugin!(
    natives: [
        MyPlugin::some_native,
    ],
    events: [
        MyPlugin::on_player_connect,
        MyPlugin::on_player_spawn,
    ],
    {
        MyPlugin::default()
    }
);
```

Both lists are optional and independent — a plugin may declare only natives,
only events, or both.

## Observer vs. suppression

By default handlers are **observers**: a handler returning `AmxResult<T>` / `T`
has its value ignored and the gamemode's own public always runs afterwards.
Registering an observer never changes the gamemode's control flow.

To **cancel** a callback, return [`EventReturn`] instead:

```rust
use samp::prelude::*; // brings EventReturn into scope

#[event(name = "OnPlayerCommandText")]
fn on_command(&mut self, _amx: &Amx, playerid: i32, cmd: &AmxString) -> EventReturn {
    if self.is_banned(playerid) {
        EventReturn::Suppress(1) // skip the gamemode's public; callback returns 1
    } else {
        EventReturn::Continue    // run the public as usual
    }
}
```

- `EventReturn::Continue` — run the gamemode's public (same as an observer).
- `EventReturn::Suppress(value)` — skip the public entirely; the callback
  returns `value` (a raw AMX cell) to its caller.
- `EventReturn::suppress(value)` — the same, but encodes a **typed** value for
  you: `EventReturn::suppress(1.5_f32)` for a `Float:` callback,
  `EventReturn::suppress(true)` for a `bool:` one.

Multiple handlers may observe the same callback; they run in registration order,
each receiving the same argument list. The **first** handler to return
`Suppress` cancels the callback and the remaining handlers are skipped.

[`EventReturn`]: https://docs.rs/rust-samp/latest/samp/events/enum.EventReturn.html

## Raw handlers

For a variadic or protocol-specific callback, add `raw` to receive the `Args`
cursor and parse it yourself — the same escape hatch `#[native(raw)]` offers:

```rust
use samp::args::Args;

#[event(name = "OnPlayerCommandText", raw)]
fn on_command(&mut self, _amx: &Amx, args: &mut Args) -> EventReturn {
    let _playerid: Option<i32> = args.next_arg();
    let cmd: Option<AmxString> = args.next_arg();
    if let Some(cmd) = cmd {
        log::info!("command: {}", &*cmd);
    }
    EventReturn::Continue
}
```

### Reentrancy

If a handler re-enters the VM (e.g. calls a public via `exec_public!`) on the
**same** callback it is currently handling, the SDK runs that public directly
instead of dispatching into the handler again — so a handler cannot recurse into
itself without bound. Different callbacks still nest normally.

## How dispatch works

To receive a callback the SDK detours the VM's `amx_Exec` (via the
[`retour`](https://crates.io/crates/retour) detour library). Every executed
public is inspected and, when its index matches a registered event on that AMX,
the handler runs before the original public. The argument list is rebuilt from
the VM stack into the same shape a native receives, so the same `AmxCell`
parsing applies.

The detour is installed **lazily** — only when the plugin registered at least
one `#[event]` handler *and* the AMX function table is available. A plugin with
no events never touches `amx_Exec`.

### Platform scope

The detour is **x86 / x86_64 only** — the architectures SA-MP and open.mp
actually run on. On other targets (for example a 64-bit ARM host used to build
`samp::debug` tooling) the events API still compiles, but the detour is a no-op
and handlers never fire. This is a hard limitation of running on those servers,
not of the SDK.

The same detour drives both SA-MP and native open.mp (it hooks the AMX
`amx_Exec` obtained from the server either way). Event delivery has been verified
end-to-end on a live **SA-MP** server; the open.mp path uses the identical
mechanism but has not yet been validated on a live open.mp server.

## Panic safety

Like `#[native]`, the generated wrapper invokes the handler body inside
`std::panic::catch_unwind`. A panic that would otherwise cross the `extern "C"`
boundary back into the VM is captured, logged with the callback name plus
payload, and swallowed — the original public still runs.
