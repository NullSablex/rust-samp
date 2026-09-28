//! Handlers generated from the headers, registered on the server's own
//! dispatchers and fired by it.
//!
//! A handler's vtable is the plugin's side of the ABI: the server calls into
//! it by slot, so a wrong order calls the wrong function. Each event here sits
//! at a different depth of its vtable — the NPC's create at [1], destroy at
//! [2], spawn at [3], death at [8] — and the server raises all of them without
//! a client: creating, spawning, killing and destroying an NPC, and an object
//! reaching the end of a move.

use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use samp::omp::{
    self, Component, CoreHandler, CoreHandlerVTable, NPCHandler, NPCHandlerVTable, ObjectHandler,
    ObjectHandlerVTable, PlayerPoolHandler, PlayerPoolHandlerVTable,
};
use samp::plugin::omp_query;

pub static NPC_CREATED: AtomicU32 = AtomicU32::new(0);
pub static NPC_SPAWNED: AtomicU32 = AtomicU32::new(0);
pub static NPC_DESTROYED: AtomicU32 = AtomicU32::new(0);
pub static NPC_DIED: AtomicU32 = AtomicU32::new(0);
/// The death reason the handler received, low byte of the word.
pub static NPC_DEATH_REASON: AtomicI32 = AtomicI32::new(-1);
pub static OBJECT_MOVED: AtomicU32 = AtomicU32::new(0);
/// Core ticks seen, and whether every one carried a sane elapsed time and a
/// clock that moved forward — the two `std::chrono` values by value.
pub static CORE_TICKS: AtomicU32 = AtomicU32::new(0);
pub static CORE_TICKS_SANE: AtomicU32 = AtomicU32::new(0);
static LAST_NOW: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0);
pub static POOL_CREATED: AtomicU32 = AtomicU32::new(0);
/// Options the config enumerated through the callback object.
pub static OPTIONS_SEEN: AtomicU32 = AtomicU32::new(0);

macro_rules! callback {
    (fn $name:ident($($arg:ident: $ty:ty),* $(,)?) $(-> $ret:ty)? $body:block) => {
        #[cfg(not(target_env = "msvc"))]
        unsafe extern "C" fn $name($($arg: $ty),*) $(-> $ret)? $body
        #[cfg(target_env = "msvc")]
        unsafe extern "thiscall" fn $name($($arg: $ty),*) $(-> $ret)? $body
    };
}

callback!(
    fn npc_create(_h: *mut NPCHandler, _npc: *mut omp::INPC) {
        NPC_CREATED.fetch_add(1, Ordering::Release);
    }
);
callback!(
    fn npc_destroy(_h: *mut NPCHandler, _npc: *mut omp::INPC) {
        NPC_DESTROYED.fetch_add(1, Ordering::Release);
    }
);
callback!(
    fn npc_spawn(_h: *mut NPCHandler, _npc: *mut omp::INPC) {
        NPC_SPAWNED.fetch_add(1, Ordering::Release);
    }
);
callback!(
    fn npc_death(
        _h: *mut NPCHandler,
        _npc: *mut omp::INPC,
        _killer: *mut omp::IPlayer,
        reason: i32,
    ) {
        NPC_DEATH_REASON.store(reason, Ordering::Release);
        NPC_DIED.fetch_add(1, Ordering::Release);
    }
);
callback!(
    fn object_moved(_h: *mut ObjectHandler, _object: *mut omp::IObject) {
        OBJECT_MOVED.fetch_add(1, Ordering::Release);
    }
);

callback!(
    fn core_tick(_h: *mut CoreHandler, elapsed: omp::Microseconds, now: omp::TimePoint) {
        let before = LAST_NOW.swap(now.0, Ordering::AcqRel);
        // Under a second between ticks, and a steady clock never goes back.
        if (0..1_000_000).contains(&elapsed.0) && now.0 >= before {
            CORE_TICKS_SANE.fetch_add(1, Ordering::Release);
        }
        CORE_TICKS.fetch_add(1, Ordering::Release);
    }
);
callback!(
    fn pool_created(_h: *mut PlayerPoolHandler, _player: *mut omp::IPlayer) {
        POOL_CREATED.fetch_add(1, Ordering::Release);
    }
);
callback!(
    fn option_seen(
        _h: *mut omp::OptionEnumeratorCallback,
        _name: omp::StringView,
        _kind: i32,
    ) -> bool {
        OPTIONS_SEEN.fetch_add(1, Ordering::Release);
        true
    }
);

/// The config's enumerator: a pure interface the plugin implements whole.
pub static OPTION_VTABLE: omp::OptionEnumeratorCallbackVTable =
    omp::OptionEnumeratorCallbackVTable { proc: option_seen };

static CORE_VTABLE: CoreHandlerVTable = CoreHandlerVTable { on_tick: core_tick };

static POOL_VTABLE: PlayerPoolHandlerVTable = PlayerPoolHandlerVTable {
    on_pool_entry_created: pool_created,
    ..PlayerPoolHandlerVTable::DEFAULT
};

static NPC_VTABLE: NPCHandlerVTable = NPCHandlerVTable {
    on_npc_create: npc_create,
    on_npc_destroy: npc_destroy,
    on_npc_spawn: npc_spawn,
    on_npc_death: npc_death,
    ..NPCHandlerVTable::DEFAULT
};

static OBJECT_VTABLE: ObjectHandlerVTable = ObjectHandlerVTable {
    on_moved: object_moved,
    ..ObjectHandlerVTable::DEFAULT
};

/// Registers the core's tick handler and the player pool's.
///
/// # Safety
/// Called on the main thread once the components are ready.
pub unsafe fn register_core() -> (bool, bool) {
    let Some(core) = samp::plugin::omp_core() else {
        return (false, false);
    };
    let tick = Box::leak(Box::new(CoreHandler::new(&raw const CORE_VTABLE)));
    let pool_handler = Box::leak(Box::new(PlayerPoolHandler::new(&raw const POOL_VTABLE)));
    unsafe {
        let ticks = omp::add_event_handler(
            omp::core_event_dispatcher(core),
            tick,
            omp::priority::DEFAULT,
        );
        let pool = omp::player_pool(core);
        let players = omp::add_event_handler(
            omp::players_pool_event_dispatcher(pool),
            pool_handler,
            omp::priority::DEFAULT,
        );
        (ticks, players)
    }
}

/// Registers both handlers. Returns whether each dispatcher took its handler.
///
/// # Safety
/// Called on the main thread once the components are ready.
pub unsafe fn register() -> (bool, bool) {
    let mut npc = false;
    if let Some(c) = omp_query::<Component<omp::INPCComponent>>() {
        let handler = Box::leak(Box::new(NPCHandler::new(&raw const NPC_VTABLE)));
        let dispatcher = unsafe { omp::npcs_event_dispatcher(c.as_ptr()) };
        npc = unsafe { omp::add_event_handler(dispatcher, handler, omp::priority::DEFAULT) };
    }
    let mut object = false;
    if let Some(c) = omp_query::<Component<omp::IObjectsComponent>>() {
        let handler = Box::leak(Box::new(ObjectHandler::new(&raw const OBJECT_VTABLE)));
        let dispatcher = unsafe { omp::objects_event_dispatcher(c.as_ptr()) };
        object = unsafe { omp::add_event_handler(dispatcher, handler, omp::priority::DEFAULT) };
    }
    (npc, object)
}
