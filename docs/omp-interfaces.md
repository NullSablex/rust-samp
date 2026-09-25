# Talking to the Open Multiplayer Server Directly

A plugin has always been able to reach the server through Pawn: call a native,
watch a callback. Running as a native Open Multiplayer component opens a second
route — the server's own C++ interfaces — and `samp::omp` wraps it.

The two routes coexist on purpose:

| | `#[event]` + natives (Pawn) | `samp::omp` (direct) |
| --- | --- | --- |
| Works on SA-MP | yes | no |
| Works on open.mp | yes, legacy or native | native component only |
| Sees | what the gamemode is told | the server's own events |
| Costs | a detour over `amx_Exec` | a virtual call |
| Can cancel a gamemode callback | yes | not applicable |

Reach for the direct route when the plugin runs as a component and wants to
react without a script in the middle; keep `#[event]` when it must also run on
SA-MP, or when it needs to suppress a callback the gamemode would otherwise
receive.

## Player events

`ICore` hands out the player pool, the pool hands out a dispatcher per event
group, and a handler registered there is called by the server:

```rust
use samp::omp::{add_player_connect_handler, player_connect_dispatcher, player_pool};

fn on_omp_ready(&mut self) {
    let Some(core) = samp::plugin::omp_core() else { return };
    let pool = unsafe { player_pool(core) };
    let dispatcher = unsafe { player_connect_dispatcher(pool) };

    // The server keeps the pointer, so the handler must outlive the call.
    let handler = Box::leak(Box::new(samp::omp::PlayerConnectHandler::new(
        &raw const PLAYER_CONNECT_VTABLE,
    )));
    unsafe { add_player_connect_handler(dispatcher, handler) };
}
```

The handler is a vtable of plain functions. The calling convention differs per
ABI — `extern "C"` under Itanium, `thiscall` under MSVC — so a macro that
writes each handler twice is the tidy way:

```rust
macro_rules! handler {
    (fn $name:ident($($arg:ident: $ty:ty),*) $(-> $ret:ty)? $body:block) => {
        #[cfg(not(target_env = "msvc"))]
        unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? $body
        #[cfg(target_env = "msvc")]
        unsafe extern "thiscall" fn $name($($arg: $ty),*) $(-> $ret)? $body
    };
}
```

All eleven groups are available: connect, spawn, stream, text, shot, change,
damage, click, check, update, and the pool's own. `examples/counter` registers
several of them.

## Players

Given the `IPlayer*` a handler receives, or one looked up by id:

```rust
let name = unsafe { samp::omp::player_name(player) };      // Option<String>
let id = unsafe { samp::omp::player_id(player) };
let pos = unsafe { samp::omp::player_position(player) };

unsafe {
    samp::omp::player_set_score(player, 1337);
    samp::omp::player_send_message(player, Colour::rgba(0, 255, 0, 255), "hello");
}
```

Reading back what you write is not always meaningful: health, armour and
position are resent by the client on its next sync packet, so the server's copy
is overwritten. Score, team and money belong to the server and do read back.

## Entities

Components follow one shape — query by UID, create, read:

```rust
let Some(component) = samp::plugin::omp_query_component(samp::omp::VEHICLES_COMPONENT_UID)
else { return };

let vehicles = unsafe { samp::omp::as_vehicles_component(component) };
let vehicle = unsafe {
    samp::omp::create_vehicle(vehicles, 411, position, 90.0, -1, -1, -1, false)
};
let id = unsafe { samp::omp::vehicle_id(vehicle) };
```

Wrapped so far: vehicles (with their fourteen events), objects, pickups, text
draws, text labels, gang zones, actors, menus and spawn classes. Every entity
carrying an `IEntity` answers `entity_id`.

## Finding players and vehicles

```rust
let player = unsafe { samp::omp::player_by_id(pool, 7) };       // null if absent
let everyone = unsafe { samp::omp::all_players(pool) };         // Vec<*mut IPlayer>
```

Iteration walks the id range the pool reports through `bounds()`, asking for
each one. It deliberately does not read the hash set behind `entries()`, whose
layout belongs to a vendored library.

## Per-player extensions

Components attach their per-player data with `addExtension`, which files it in a
map the virtual `getExtension` does not consult. `samp::omp::extension` walks
that map:

```rust
const CHECKPOINT_DATA_UID: u64 = 0xbc07_576a_a359_1a66;
let data = unsafe { samp::omp::extension(player.cast::<u8>(), CHECKPOINT_DATA_UID) };
```

This is the one part of the SDK whose correctness rests on a vendored library's
internals rather than an ABI. It fails closed — a bounded probe, and a candidate
returned only after `getExtensionID()` confirms it — but when in doubt, the Pawn
natives through `Amx::call_native` do not depend on any of it.

## Every pointer here fails closed

These functions take raw pointers the server owns. A null one — a player who
just disconnected, a component the server did not load — gives the documented
default: `None`, zero, a null pointer, `false`, or an empty range. That is
covered by tests, not by convention.

What no test on this side can catch is a pointer that is wrong but not null.
Against that there are the slot constants pinned per ABI,
`scripts/check-abi-slots.py` re-deriving them from the shipped binaries, and the
runs against real servers on both platforms.

## Adding an interface

The slot indices differ per ABI and are not guessable — see
[the ABI notes](internals/omp-abi.md). `scripts/omp-vtable.py` asks clang where
each method lands, in both ABIs at once, and `--rust` prints the constants ready
to paste. `CONTRIBUTING.md` has the workflow.
