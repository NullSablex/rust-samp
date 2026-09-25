//! Stateful plugin example with full lifecycle.
//!
//! Demonstrates:
//! - State on the plugin struct (`count`, `max`, `ticks`)
//! - Manual `impl SampPlugin` with `on_load` and `process_tick`
//! - `initialize_plugin!` with constructor block
//! - `Ref<i32>` for output by reference (`&value` in PAWN)
//! - Multiple natives with real logic
//!
//! Natives exposed to PAWN:
//! ```pawn
//! native Counter_Increment();
//! native Counter_Decrement();
//! native Counter_Reset();
//! native Counter_Get(&out);
//! native Counter_SetMax(max);
//! native bool:Counter_IsAtMax();
//! native bool:Counter_WorkAsync(delay_ms);   // replies via OnCounterWorkDone(delay_ms)
//! native Counter_NotifyScore(playerid);      // fires OnCounterScored(playerid, reason[], Float:mult)
//! native Counter_ListNatives(limit);         // open.mp component only; -1 elsewhere
//! ```

use log::info;
use samp::plugin::TickContext;
use samp::prelude::*;
use samp::{event, exec_public, initialize_plugin, native};

struct Counter {
    count: i32,
    max: i32,
    ticks: u32,
}

impl SampPlugin for Counter {
    fn on_load(&mut self) {
        info!("Counter plugin loaded. Max={}", self.max);
    }

    fn on_unload(&mut self) {
        info!("Counter plugin unloaded. Final value={}", self.count);
    }

    /// Registers a native Open Multiplayer player handler — no Pawn involved.
    ///
    /// `on_omp_ready` is the first point where every component is up, so the
    /// player pool is reachable. On SA-MP this never runs, and the plugin keeps
    /// seeing connections through the `#[event]` detour instead.
    fn on_omp_ready(&mut self) {
        use samp::omp::{add_player_connect_handler, player_connect_dispatcher, player_pool};

        let Some(core) = samp::plugin::omp_core() else {
            return;
        };
        let pool = unsafe { player_pool(core) };
        if pool.is_null() {
            info!("[omp] player pool unavailable");
            return;
        }
        let dispatcher = unsafe { player_connect_dispatcher(pool) };
        if dispatcher.is_null() {
            info!("[omp] player connect dispatcher unavailable");
            return;
        }

        // The server keeps the pointer, so the handler must outlive this call.
        let handler = Box::leak(Box::new(samp::omp::PlayerConnectHandler::new(
            &raw const PLAYER_CONNECT_VTABLE,
        )));
        let added = unsafe { add_player_connect_handler(dispatcher, handler) };
        info!("[omp] native player handler registered: {added}");

        // The same shape for the other event groups.
        let spawn = unsafe { samp::omp::player_spawn_dispatcher(pool) };
        if !spawn.is_null() {
            let handler = Box::leak(Box::new(samp::omp::PlayerSpawnHandler::new(
                &raw const PLAYER_SPAWN_VTABLE,
            )));
            let ok = unsafe { samp::omp::add_player_spawn_handler(spawn, handler) };
            info!("[omp] spawn handler registered: {ok}");
        }

        let update = unsafe { samp::omp::player_update_dispatcher(pool) };
        if !update.is_null() {
            let handler = Box::leak(Box::new(samp::omp::PlayerUpdateHandler::new(
                &raw const PLAYER_UPDATE_VTABLE,
            )));
            let ok = unsafe { samp::omp::add_player_update_handler(update, handler) };
            info!("[omp] update handler registered: {ok}");
        }

        // Reading another component's identity — exercises componentName and
        // componentVersion, whose return convention differs per ABI.
        if let Some(timers) = samp::plugin::omp_query::<samp::omp::TimersComponent>() {
            info!(
                "[omp] Timers component: name={:?} version={:?}",
                timers.name(),
                timers.version().map(|v| (v.major, v.minor, v.patch))
            );
        }

        // Vehicles: query the component by UID, spawn one, read it back.
        if let Some(component) =
            samp::plugin::omp_query_component(samp::omp::VEHICLES_COMPONENT_UID)
        {
            let vehicles = unsafe { samp::omp::as_vehicles_component(component) };
            let vehicle = unsafe {
                samp::omp::create_vehicle(
                    vehicles,
                    411, // Infernus
                    samp::omp::types::Vector3 {
                        x: 10.0,
                        y: 20.0,
                        z: 3.0,
                    },
                    90.0,
                    -1,
                    -1,
                    -1,
                    false,
                )
            };
            // Vehicle events, same shape as the player ones.
            let dispatcher = unsafe { samp::omp::vehicle_event_dispatcher(vehicles) };
            if !dispatcher.is_null() {
                let handler = Box::leak(Box::new(samp::omp::VehicleHandler::new(
                    &raw const omp_vehicles::VTABLE,
                )));
                let ok = unsafe { samp::omp::add_vehicle_handler(dispatcher, handler) };
                info!("[omp] vehicle handler registered: {ok}");
            }

            if vehicle.is_null() {
                info!("[omp] vehicle creation refused by the server");
            } else {
                unsafe { samp::omp::vehicle_set_health(vehicle, 750.0) };
                let pos = unsafe { samp::omp::vehicle_position(vehicle) };
                let id = unsafe { samp::omp::vehicle_id(vehicle) };
                // Looking the same vehicle up by id must land on the same object.
                let looked_up = unsafe { samp::omp::vehicle_by_id(vehicles, id) };
                info!(
                    "[omp] vehicle id={id} lookup_ok={} model={} health={} pos=({:.1},{:.1},{:.1})",
                    std::ptr::eq(looked_up, vehicle),
                    unsafe { samp::omp::vehicle_model(vehicle) },
                    unsafe { samp::omp::vehicle_health(vehicle) },
                    pos.x,
                    pos.y,
                    pos.z
                );
            }
        }

        // Objects and pickups, the same query-create-read shape.
        if let Some(component) = samp::plugin::omp_query_component(samp::omp::OBJECTS_COMPONENT_UID)
        {
            let objects = unsafe { samp::omp::as_objects_component(component) };
            let zero = samp::omp::types::Vector3 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
            let object = unsafe {
                samp::omp::create_object(
                    objects,
                    1337,
                    samp::omp::types::Vector3 {
                        x: 5.0,
                        y: 6.0,
                        z: 7.0,
                    },
                    zero,
                    0.0,
                )
            };
            info!("[omp] object id={}", unsafe {
                samp::omp::entity_id(object.cast::<u8>())
            });
        }

        if let Some(component) = samp::plugin::omp_query_component(samp::omp::PICKUPS_COMPONENT_UID)
        {
            let pickups = unsafe { samp::omp::as_pickups_component(component) };
            let pickup = unsafe {
                samp::omp::create_pickup(
                    pickups,
                    1274, // money bag
                    1,    // type: pick up and respawn
                    samp::omp::types::Vector3 {
                        x: 1.0,
                        y: 2.0,
                        z: 3.0,
                    },
                    0,
                    true,
                )
            };
            info!("[omp] pickup id={}", unsafe {
                samp::omp::entity_id(pickup.cast::<u8>())
            });
        }

        // Text draws, gang zones and actors — same shape once more.
        if let Some(c) = samp::plugin::omp_query_component(samp::omp::TEXTDRAWS_COMPONENT_UID) {
            let td = unsafe {
                samp::omp::create_textdraw(
                    samp::omp::as_textdraws_component(c),
                    samp::omp::types::Vector2 { x: 320.0, y: 240.0 },
                    "rust-samp",
                )
            };
            info!("[omp] textdraw id={}", unsafe {
                samp::omp::entity_id(td.cast::<u8>())
            });
        }

        if let Some(c) = samp::plugin::omp_query_component(samp::omp::GANGZONES_COMPONENT_UID) {
            let zone = unsafe {
                samp::omp::create_gangzone(
                    samp::omp::as_gangzones_component(c),
                    samp::omp::GangZonePos {
                        min: samp::omp::types::Vector2 { x: 0.0, y: 0.0 },
                        max: samp::omp::types::Vector2 { x: 100.0, y: 100.0 },
                    },
                )
            };
            info!("[omp] gangzone id={}", unsafe {
                samp::omp::entity_id(zone.cast::<u8>())
            });
        }

        if let Some(c) = samp::plugin::omp_query_component(samp::omp::ACTORS_COMPONENT_UID) {
            let actor = unsafe {
                samp::omp::create_actor(
                    samp::omp::as_actors_component(c),
                    46,
                    samp::omp::types::Vector3 {
                        x: 2.0,
                        y: 3.0,
                        z: 4.0,
                    },
                    0.0,
                )
            };
            info!("[omp] actor id={}", unsafe {
                samp::omp::entity_id(actor.cast::<u8>())
            });
        }

        // Text labels, menus and spawn classes close the component sweep.
        if let Some(c) = samp::plugin::omp_query_component(samp::omp::TEXTLABELS_COMPONENT_UID) {
            let label = unsafe {
                samp::omp::create_textlabel(
                    samp::omp::as_textlabels_component(c),
                    "rust-samp",
                    samp::omp::types::Colour::rgba(255, 255, 255, 255),
                    samp::omp::types::Vector3 {
                        x: 1.0,
                        y: 1.0,
                        z: 1.0,
                    },
                    20.0,
                    0,
                    true,
                )
            };
            info!("[omp] textlabel id={}", unsafe {
                samp::omp::entity_id(label.cast::<u8>())
            });
        }

        if let Some(c) = samp::plugin::omp_query_component(samp::omp::MENUS_COMPONENT_UID) {
            let menu = unsafe {
                samp::omp::create_menu(
                    samp::omp::as_menus_component(c),
                    "menu",
                    samp::omp::types::Vector2 { x: 50.0, y: 180.0 },
                    1,
                    200.0,
                    0.0,
                )
            };
            info!("[omp] menu id={}", unsafe {
                samp::omp::entity_id(menu.cast::<u8>())
            });
        }

        if let Some(c) = samp::plugin::omp_query_component(samp::omp::CLASSES_COMPONENT_UID) {
            let weapons = [samp::omp::WeaponSlot::default(); samp::omp::MAX_WEAPON_SLOTS];
            let class = unsafe {
                samp::omp::create_class(
                    samp::omp::as_classes_component(c),
                    46,
                    0,
                    samp::omp::types::Vector3 {
                        x: 0.0,
                        y: 0.0,
                        z: 3.0,
                    },
                    0.0,
                    &weapons,
                )
            };
            info!("[omp] class id={}", unsafe {
                samp::omp::entity_id(class.cast::<u8>())
            });
        }

        let text = unsafe { samp::omp::player_text_dispatcher(pool) };
        if !text.is_null() {
            let handler = Box::leak(Box::new(samp::omp::PlayerTextHandler::new(
                &raw const PLAYER_TEXT_VTABLE,
            )));
            let ok = unsafe { samp::omp::add_player_text_handler(text, handler) };
            info!("[omp] text handler registered: {ok}");
        }
    }

    fn on_tick(&mut self, _ctx: TickContext) {
        self.ticks += 1;
        // Logs the state every ~5 seconds (1000 ticks x ~5ms)
        if self.ticks.is_multiple_of(1000) {
            info!(
                "Counter tick={} count={}/{}",
                self.ticks, self.count, self.max
            );
        }
    }
}

impl Counter {
    /// Lists the natives the script registered, via the Open Multiplayer-only
    /// `amx_GetNativeByIndex`.
    ///
    /// Returns the count, or `-1` when the extended AMX table is not available
    /// — which is every SA-MP server and every legacy plugin, since only a
    /// native Open Multiplayer component gets the 52-entry table.
    #[native(name = "Counter_ListNatives")]
    fn list_natives(&mut self, amx: &Amx, limit: i32) -> AmxResult<i32> {
        use samp::omp_amx::AmxOmpExt;

        if !samp::omp_amx::extended_table_available() {
            info!("extended AMX table unavailable (not an open.mp component)");
            return Ok(-1);
        }

        let mut listed = 0;
        for index in 0..limit.max(0) {
            let Ok(native) = amx.native_by_index(index) else {
                break;
            };
            let name = unsafe { std::ffi::CStr::from_ptr(native.name) };
            info!("native[{index}] = {}", name.to_string_lossy());
            listed += 1;
        }
        Ok(listed)
    }

    /// Fires a Pawn callback with typed arguments, through `Amx::call_public`.
    ///
    /// Shows the argument types carrying the decision: `i32` and `f32` go
    /// straight into cells, while the `&str` is copied into the AMX heap and
    /// arrives as `const reason[]`. No `=> string` marker at the call site.
    #[native(name = "Counter_NotifyScore")]
    fn notify_score(&mut self, amx: &Amx, playerid: i32) -> AmxResult<i32> {
        // forward OnCounterScored(playerid, const reason[], Float:multiplier);
        amx.call_public("OnCounterScored", (playerid, "headshot", 1.5))
    }

    /// Starts slow work on another thread and reports the result to Pawn.
    ///
    /// Returns immediately — the server is not blocked. The worker sleeps to
    /// stand in for real I/O (HTTP, a database, SMTP), then hands the result
    /// back through [`samp::mainthread::post`], which runs it on the main
    /// thread at the next tick. Only there is it safe to touch the VM, so that
    /// is where the Pawn callback is fired.
    #[native(name = "Counter_WorkAsync")]
    fn work_async(&mut self, amx: &Amx, delay_ms: i32) -> AmxResult<bool> {
        let ident = amx.ident();
        let delay = u64::try_from(delay_ms.max(0)).unwrap_or(0);

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(delay));

            samp::mainthread::post(move || {
                let Some(amx) = samp::amx::get(ident) else {
                    // The script was unloaded while the work was running.
                    return;
                };
                let _ = exec_public!(amx, "OnCounterWorkDone", delay_ms);
            });
        });

        Ok(true)
    }

    /// Increments the counter. Returns the new value, or -1 if already at the maximum.
    #[native(name = "Counter_Increment")]
    fn increment(&mut self, _amx: &Amx) -> i32 {
        if self.count >= self.max {
            return -1;
        }
        self.count += 1;
        self.count
    }

    /// Decrements the counter. Returns the new value, or -1 if already zero.
    #[native(name = "Counter_Decrement")]
    fn decrement(&mut self, _amx: &Amx) -> i32 {
        if self.count <= 0 {
            return -1;
        }
        self.count -= 1;
        self.count
    }

    /// Resets the counter. Returns the value that was discarded.
    #[native(name = "Counter_Reset")]
    fn reset(&mut self, _amx: &Amx) -> i32 {
        let old = self.count;
        self.count = 0;
        old
    }

    /// Writes the current value into `out` (output by reference).
    ///
    /// ```pawn
    /// new val;
    /// Counter_Get(val);
    /// printf("Value: %d", val);
    /// ```
    #[native(name = "Counter_Get")]
    fn get(&mut self, _amx: &Amx, mut out: Ref<i32>) -> bool {
        *out = self.count;
        true
    }

    /// Sets the maximum value of the counter.
    #[native(name = "Counter_SetMax")]
    fn set_max(&mut self, _amx: &Amx, max: i32) -> bool {
        if max <= 0 {
            return false;
        }
        self.max = max;
        if self.count > self.max {
            self.count = self.max;
        }
        true
    }

    /// Returns true if the counter is at the maximum value.
    #[native(name = "Counter_IsAtMax")]
    fn is_at_max(&mut self, _amx: &Amx) -> bool {
        self.count >= self.max
    }

    /// Observes the gamemode's `OnPlayerConnect` callback. The handler runs
    /// before the gamemode's own public — here it just logs the connecting
    /// player. Registered via the `events: [...]` list below.
    ///
    /// ```pawn
    /// public OnPlayerConnect(playerid) { return 1; }
    /// ```
    #[event(name = "OnPlayerConnect")]
    fn on_player_connect(&mut self, _amx: &Amx, playerid: i32) -> AmxResult<i32> {
        info!("[event] OnPlayerConnect: player {playerid} connected");
        Ok(1)
    }

    /// Callback suppression: while the counter sits at its maximum, cancel the
    /// gamemode's `OnPlayerText` so the chat message is dropped. Returning
    /// `EventReturn::Suppress(0)` skips the original public and makes the
    /// callback return `0`; `Continue` lets it run normally.
    ///
    /// ```pawn
    /// public OnPlayerText(playerid, text[]) { return 1; }
    /// ```
    #[event(name = "OnPlayerText")]
    fn on_player_text(&mut self, _amx: &Amx, playerid: i32, text: &AmxString) -> EventReturn {
        if self.count >= self.max {
            info!(
                "[event] OnPlayerText suppressed for player {playerid}: {}",
                &**text
            );
            EventReturn::Suppress(0)
        } else {
            EventReturn::Continue
        }
    }
}

// ---------------------------------------------------------------------------
// Native Open Multiplayer player events (no Pawn, no detour)
// ---------------------------------------------------------------------------

mod omp_players {
    use log::info;
    use samp::omp::types::StringView;
    use samp::omp::{
        DisconnectReason, IPlayer, PlayerConnectHandler, PlayerConnectHandlerVTable,
        PlayerSpawnHandler, PlayerSpawnHandlerVTable, PlayerTextHandler, PlayerTextHandlerVTable,
        PlayerUpdateHandler, PlayerUpdateHandlerVTable,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// The server calls these through a vtable, so the calling convention is
    /// the platform's: `extern "C"` under the Itanium ABI, `thiscall` under
    /// MSVC. The macro writes each handler once for both.
    macro_rules! handler {
        (fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block) => {
            #[cfg(not(target_env = "msvc"))]
            pub unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? $body

            #[cfg(target_env = "msvc")]
            pub unsafe extern "thiscall" fn $name($($arg: $ty),*) $(-> $ret)? $body
        };
    }

    handler!(
        fn on_incoming(
            _this: *mut PlayerConnectHandler,
            _player: *mut IPlayer,
            _ip: StringView,
            _port: u16,
        ) {
        }
    );

    handler!(
        fn on_connect(_this: *mut PlayerConnectHandler, player: *mut IPlayer) {
            // Reading from the IPlayer the server just handed us: the name
            // comes back as a StringView through a hidden pointer on both ABIs.
            let is_bot = unsafe { samp::omp::player_is_bot(player) };
            let name = unsafe { samp::omp::player_name(player) };
            let id = unsafe { samp::omp::player_id(player) };
            info!(
                "[omp-event] onPlayerConnect: id={id} name={:?} bot={is_bot}",
                name.as_deref().unwrap_or("<none>")
            );
        }
    );

    handler!(
        fn on_disconnect(_this: *mut PlayerConnectHandler, _player: *mut IPlayer, reason: i32) {
            info!(
                "[omp-event] onPlayerDisconnect: {:?}",
                DisconnectReason::from_raw(reason)
            );
        }
    );

    handler!(
        fn on_client_init(_this: *mut PlayerConnectHandler, _player: *mut IPlayer) {}
    );

    handler!(
        fn on_request_spawn(_this: *mut PlayerSpawnHandler, _player: *mut IPlayer) -> bool {
            true // allow
        }
    );

    handler!(
        fn on_spawn(_this: *mut PlayerSpawnHandler, player: *mut IPlayer) {
            // Acting on the player, not just observing.
            let health = unsafe { samp::omp::player_health(player) };
            unsafe {
                samp::omp::player_send_message(
                    player,
                    samp::omp::types::Colour::rgba(0, 255, 0, 255),
                    "hello from rust-samp",
                );
            }
            info!("[omp-event] onPlayerSpawn: health={health}");
        }
    );

    handler!(
        fn on_text(
            _this: *mut PlayerTextHandler,
            _player: *mut IPlayer,
            message: StringView,
        ) -> bool {
            info!("[omp-event] onPlayerText ({} bytes)", message.len);
            true // let it through
        }
    );

    handler!(
        fn on_command_text(
            _this: *mut PlayerTextHandler,
            _player: *mut IPlayer,
            _message: StringView,
        ) -> bool {
            false // not handled here
        }
    );

    handler!(
        fn on_update(_this: *mut PlayerUpdateHandler, player: *mut IPlayer, _now: i64) -> bool {
            // Fires for every player on every tick, so it only reports once.
            // The first tick writes the health, the next one reads it back:
            // the server resets health right after a spawn, so a write checked
            // inside the spawn handler proves nothing.
            static STEP: AtomicUsize = AtomicUsize::new(0);
            match STEP.fetch_add(1, Ordering::Relaxed) {
                0 => {
                    info!("[omp-event] onPlayerUpdate straight from the server");
                    // Score, not health: health comes back from the client on
                    // the next sync packet, so writing it proves nothing here.
                    unsafe { samp::omp::player_set_score(player, 1337) };
                }
                1 => {
                    let score = unsafe { samp::omp::player_score(player) };
                    let health = unsafe { samp::omp::player_health(player) };
                    // Position comes from the IEntity subobject, which needs
                    // the `this` pointer adjusted before indexing.
                    let world = unsafe { samp::omp::player_virtual_world(player) };
                    unsafe {
                        samp::omp::player_set_money(player, 5000);
                        samp::omp::player_set_team(player, 3);
                        samp::omp::player_set_armour(player, 50.0);
                    }
                    let id = unsafe { samp::omp::player_id(player) };
                    // The same object, reached through the pool by id?
                    let pool = samp::plugin::omp_core()
                        .map(|core| unsafe { samp::omp::player_pool(core) })
                        .unwrap_or(std::ptr::null_mut());
                    let looked_up = unsafe { samp::omp::player_by_id(pool, id) };
                    // Iterating the pool by id range, rather than by reading
                    // the hash set behind `entries()`.
                    let count = unsafe { samp::omp::all_players(pool) }.len();
                    // The per-player extension map: data a component
                    // attached with addExtension, which the virtual
                    // getExtension does not report.
                    const CHECKPOINT_DATA_UID: u64 = 0xbc07_576a_a359_1a66;
                    const DIALOG_DATA_UID: u64 = 0xbc03_376a_a359_1a11;
                    let checkpoints =
                        unsafe { samp::omp::extension(player.cast::<u8>(), CHECKPOINT_DATA_UID) };
                    let dialogs =
                        unsafe { samp::omp::extension(player.cast::<u8>(), DIALOG_DATA_UID) };
                    info!(
                        "[omp-event] extensions: checkpoints={} dialogs={}",
                        !checkpoints.is_null(),
                        !dialogs.is_null()
                    );

                    // The plain accessors, written then read back.
                    unsafe {
                        samp::omp::player_set_interior(player, 3);
                        samp::omp::player_set_wanted_level(player, 4);
                        samp::omp::player_set_money(player, 250);
                    }
                    info!(
                        "[omp-event] pool reports {count} player(s); interior={} wanted={} money={} skin={}",
                        unsafe { samp::omp::player_interior(player) },
                        unsafe { samp::omp::player_wanted_level(player) },
                        unsafe { samp::omp::player_money(player) },
                        unsafe { samp::omp::player_skin(player) }
                    );
                    info!(
                        "[omp-event] score={score} health={health} world={world} team={} armour={} lookup_ok={}",
                        unsafe { samp::omp::player_team(player) },
                        unsafe { samp::omp::player_armour(player) },
                        std::ptr::eq(looked_up, player)
                    );
                    let pos = unsafe { samp::omp::player_position(player) };
                    info!("[omp-event] pos=({:.1},{:.1},{:.1})", pos.x, pos.y, pos.z);
                }
                _ => {}
            }
            true
        }
    );

    pub static UPDATE_VTABLE: PlayerUpdateHandlerVTable = PlayerUpdateHandlerVTable {
        on_player_update: on_update,
    };

    pub static SPAWN_VTABLE: PlayerSpawnHandlerVTable = PlayerSpawnHandlerVTable {
        on_player_request_spawn: on_request_spawn,
        on_player_spawn: on_spawn,
    };

    pub static TEXT_VTABLE: PlayerTextHandlerVTable = PlayerTextHandlerVTable {
        on_player_text: on_text,
        on_player_command_text: on_command_text,
    };

    pub static VTABLE: PlayerConnectHandlerVTable = PlayerConnectHandlerVTable {
        on_incoming_connection: on_incoming,
        on_player_connect: on_connect,
        on_player_disconnect: on_disconnect,
        on_player_client_init: on_client_init,
    };
}

use omp_players::{
    SPAWN_VTABLE as PLAYER_SPAWN_VTABLE, TEXT_VTABLE as PLAYER_TEXT_VTABLE,
    UPDATE_VTABLE as PLAYER_UPDATE_VTABLE, VTABLE as PLAYER_CONNECT_VTABLE,
};

mod omp_vehicles {
    use log::info;
    use samp::omp::{IVehicle, VehicleHandler, VehicleHandlerVTable};

    macro_rules! vh {
        (fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block) => {
            #[cfg(not(target_env = "msvc"))]
            unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? $body
            #[cfg(target_env = "msvc")]
            unsafe extern "thiscall" fn $name($($arg: $ty),*) $(-> $ret)? $body
        };
    }

    vh!(
        fn stream_in(_t: *mut VehicleHandler, _v: *mut IVehicle, _p: *mut u8) {}
    );
    vh!(
        fn stream_out(_t: *mut VehicleHandler, _v: *mut IVehicle, _p: *mut u8) {}
    );
    vh!(
        fn death(_t: *mut VehicleHandler, _v: *mut IVehicle, _p: *mut u8) {}
    );
    vh!(
        fn enter(_t: *mut VehicleHandler, _p: *mut u8, _v: *mut IVehicle, _passenger: bool) {}
    );
    vh!(
        fn exit(_t: *mut VehicleHandler, _p: *mut u8, _v: *mut IVehicle) {}
    );
    vh!(
        fn damage_status(_t: *mut VehicleHandler, _v: *mut IVehicle, _p: *mut u8) {}
    );
    vh!(
        fn paint_job(_t: *mut VehicleHandler, _p: *mut u8, _v: *mut IVehicle, _j: i32) -> bool {
            true
        }
    );
    vh!(
        fn modification(_t: *mut VehicleHandler, _p: *mut u8, _v: *mut IVehicle, _c: i32) -> bool {
            true
        }
    );
    vh!(
        fn respray(
            _t: *mut VehicleHandler,
            _p: *mut u8,
            _v: *mut IVehicle,
            _c1: i32,
            _c2: i32,
        ) -> bool {
            true
        }
    );
    vh!(
        fn mod_shop(_t: *mut VehicleHandler, _p: *mut u8, _enter: bool, _interior: i32) {}
    );
    vh!(
        fn spawn(_t: *mut VehicleHandler, _v: *mut IVehicle) {
            info!("[omp-event] onVehicleSpawn straight from the server");
        }
    );
    vh!(
        fn unoccupied(
            _t: *mut VehicleHandler,
            _v: *mut IVehicle,
            _p: *mut u8,
            _d: *const u8,
        ) -> bool {
            true
        }
    );
    vh!(
        fn trailer(_t: *mut VehicleHandler, _p: *mut u8, _v: *mut IVehicle) -> bool {
            true
        }
    );
    vh!(
        fn siren(_t: *mut VehicleHandler, _p: *mut u8, _v: *mut IVehicle, _s: u8) -> bool {
            true
        }
    );

    pub static VTABLE: VehicleHandlerVTable = VehicleHandlerVTable {
        on_vehicle_stream_in: stream_in,
        on_vehicle_stream_out: stream_out,
        on_vehicle_death: death,
        on_player_enter_vehicle: enter,
        on_player_exit_vehicle: exit,
        on_vehicle_damage_status_update: damage_status,
        on_vehicle_paint_job: paint_job,
        on_vehicle_mod: modification,
        on_vehicle_respray: respray,
        on_enter_exit_mod_shop: mod_shop,
        on_vehicle_spawn: spawn,
        on_unoccupied_vehicle_update: unoccupied,
        on_trailer_update: trailer,
        on_vehicle_siren_state_change: siren,
    };
}

initialize_plugin!(
    natives: [
        Counter::increment,
        Counter::decrement,
        Counter::reset,
        Counter::get,
        Counter::set_max,
        Counter::is_at_max,
        Counter::work_async,
        Counter::notify_score,
        Counter::list_natives,
    ],
    events: [
        Counter::on_player_connect,
        Counter::on_player_text,
    ],
    {
        samp::plugin::enable_tick();

        // Turnkey logger: writes to `logs/counter.log`, rotates the
        // file into `logs/archive/counter.log.{N}` once it reaches 50 MB,
        // prefixes server-console output with `[counter]`, and prints a
        // banner on load. The SDK never deletes old archives by itself —
        // chain `.rotation_keep(N)` if you want size-bounded cleanup.
        let _ = samp::enable_logger!();

        return Counter {
            count: 0,
            max: 100,
            ticks: 0,
        };
    }
);
