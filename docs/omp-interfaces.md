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

## Component events

Every other component — objects, NPCs, pickups, actors, classes, dialogs,
menus, text draws, gang zones, checkpoints, the console, custom models —
hands out its events the same way, and those handlers are generated from the
headers. Each vtable comes with `DEFAULT`, whose entries do what the C++
handler's own bodies do, so a plugin writes only the events it wants:

```rust
use samp::omp::{Component, ObjectHandler, ObjectHandlerVTable, IObjectsComponent};

handler!(fn on_moved(_h: *mut ObjectHandler, object: *mut samp::omp::IObject) {
    // the object reached the end of its move
});

static VTABLE: ObjectHandlerVTable = ObjectHandlerVTable {
    on_moved,
    ..ObjectHandlerVTable::DEFAULT
};

fn on_omp_ready(&mut self) {
    let Some(objects) = samp::plugin::omp_query::<Component<IObjectsComponent>>() else { return };
    let handler = Box::leak(Box::new(ObjectHandler::new(&raw const VTABLE)));
    unsafe {
        let dispatcher = samp::omp::objects_event_dispatcher(objects.as_ptr());
        samp::omp::add_event_handler(dispatcher, handler, samp::omp::priority::DEFAULT);
    }
}
```

`add_event_handler`, `remove_event_handler` and `event_handler_count` take any
`EventDispatcher<H>`. A narrow integer a handler receives (`bool`, `uint8_t`,
`int16_t`, ...) arrives as a whole word: C++ callers need not extend it, so
the value is its low bits. The core's own `onTick` handler is not generated —
it takes a `std::chrono::microseconds` — and `process_tick` covers that need.

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

Components follow one shape — query, create, use:

```rust
use samp::omp::{Component, IVehiclesComponent};

let Some(vehicles) = samp::plugin::omp_query::<Component<IVehiclesComponent>>() else {
    return;
};
let vehicle = unsafe {
    samp::omp::create_vehicle(vehicles.as_ptr(), 411, position, 90.0, -1, -1, -1, false)
};
unsafe { samp::omp::vehicle_set_health(vehicle, 750.0) };
```

The component's UID comes from the interface type, so a lookup cannot find
one component and hand it back as another. The older
`omp_query_component(UID)` plus `as_*_component` cast still works, deprecated.

Past the `create_*` functions, nearly every method of every interface is
available as `<entity>_<method>`: `object_set_model`, `pickup_model`,
`textdraw_set_letter_colour`, `gangzone_show_for_player`, `npc_...`,
`players_...` on the pool, `core_...` on `ICore`. Getters drop the `get`, the
way `player_health` wraps `getHealth`. Every entity carrying an `IEntity`
answers `entity_id`.

A value the header takes by `const &` is a Rust reference
(`checkpoint_set_position(cp, &pos)`); a setter that returns the object for
chaining returns nothing here, and one that fills an argument (`T &`) takes a
`&mut`. Times use the SDK's `Milliseconds`, `Seconds`, `Minutes` and
`Hours`, laid out as `std::chrono` is on each ABI; rotations use `GTAQuat`.

Structs the headers pass by value — `VehicleParams`, `VehicleSpawnData`,
`PlayerClass`, `ObjectMoveData` and the rest — are mirrored from the headers
too, with their C++ defaults and an assertion of every field's offset per ABI.
A getter returning `const T &` gives a `*const T` into the server's copy:

```rust
let params = VehicleParams { engine: 1, ..VehicleParams::default() };
unsafe { samp::omp::vehicle_set_params(vehicle, &params) };
let now = unsafe { *samp::omp::vehicle_params(vehicle) };
```

## Generated wrappers

Those functions are generated from the open.mp SDK headers by
`cargo xtask gen-omp`, one module per interface under
`samp_sdk::omp::generated`. Each file lists at its end what was left out and
why — a type the SDK does not mirror (`std::` containers, references to
unmirrored structs), a C-style `...`, an overload no one has named. A gap is written
down, never guessed at.

Three things stand behind every generated function:

- **The layout is clang's**, for both ABIs, from the same headers the server is
  built from.
- **Every slot is checked against the official binaries** by
  `cargo xtask check-abi --generated`: by method name on Linux, whose
  libraries keep their symbols, and by the argument bytes each method pops on
  Windows (`ret N` under `thiscall`).
- **`examples/omp-showcase` round-trips every setter that has a getter** on a
  running server — set a value, read it back, restore it. It is run on open.mp
  Linux and Windows before a release.

## Containers

The headers pass a few generic types, mirrored in `samp::omp::containers`:

| C++ | Rust |
| --- | --- |
| `Pair<A, B>` | `Pair<A, B>` (`first`, `second`) |
| `Span<T>` | `Span<T>` — build one with `Span::of(&mut slice)` |
| `HybridString<N>` | `HybridString<N>` — `new(&str)` inline, `to_string_lossy()` |
| `StaticArray<T, N>` | `[T; N]` |
| `FlatPtrHashSet<T>`, `FlatHashSet<T*>` | `FlatSet<T>` — read with `flat_set_entries` |

```rust
let bots = unsafe { samp::omp::flat_set_entries(samp::omp::players_bots(pool)) };
```

`flat_set_entries` reads a `robin_hood` table's internals, not an ABI, so it
fails closed: bounded by the table's own allocation, and empty when the
counts disagree.

## Finding players and vehicles

```rust
let player = unsafe { samp::omp::player_by_id(pool, 7) };       // null if absent
let everyone = unsafe { samp::omp::all_players(pool) };         // Vec<*mut IPlayer>
```

Iteration walks the id range the pool reports through `bounds()`, asking for
each one. It deliberately does not read the hash set behind `entries()`, whose
layout belongs to a vendored library.

## Per-player extensions

Components attach their per-player data — checkpoints, dialogs, objects,
variables — as extensions. Each generated data interface comes with an accessor
that finds it:

```rust
let dialogs = unsafe { samp::omp::player_dialogs(player) };   // *mut IPlayerDialogData
if !dialogs.is_null() {
    let id = unsafe { samp::omp::player_dialogs_active_id(dialogs) };
}
```

Components file that data with `addExtension`, in a map the virtual
`getExtension` does not consult, so the accessor walks the map
(`samp::omp::extension`). It is the one part of the SDK whose correctness rests
on a vendored library's internals rather than an ABI. It fails closed — a
bounded probe, and a candidate returned only after `getExtensionID()` confirms
it — and the accessor is generated only where `IExtension` is the interface's
first base, which is what makes the extension pointer the interface pointer.

## Every pointer here fails closed

These functions take raw pointers the server owns. A null one — a player who
just disconnected, a component the server did not load — gives the documented
default: `None`, zero, a null pointer, `false`, or an empty range. That is
covered by tests, not by convention.

What no test on this side can catch is a pointer that is wrong but not null.
Against that there are the slot constants pinned per ABI,
`cargo xtask check-abi` re-deriving them from the shipped binaries, and the
runs against real servers on both platforms.

## Adding an interface

List it in `xtask/omp-wrappers.toml` — the interface, its header, the handle
the wrappers take, their prefix, and the class implementing it in the server's
binaries; an overloaded method is wrapped once each overload gets a name under
`overloads` (`"create(Vector2,int)" = "create_preview"`) — then:

```sh
cargo xtask gen-omp                        # regenerate samp-sdk/src/omp/generated/
cargo xtask check-abi --generated          # every slot against the binaries
cargo xtask roundtrip                      # refresh the showcase's round trips
```

and run `omp-showcase` on both servers. The generator needs clang and an open.mp
SDK checkout with its submodules (`--sdk`, or `$OPENMP_SDK`), and the Windows
headers `cargo xwin` downloads. What the generator skips can still be
written by hand; the slot indices differ per ABI and are not guessable — see
[the ABI notes](internals/omp-abi.md), and `cargo xtask vtable` for the
layout of one class.
