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
    self, Component, NPCHandler, NPCHandlerVTable, ObjectHandler, ObjectHandlerVTable,
};
use samp::plugin::omp_query;

pub static NPC_CREATED: AtomicU32 = AtomicU32::new(0);
pub static NPC_SPAWNED: AtomicU32 = AtomicU32::new(0);
pub static NPC_DESTROYED: AtomicU32 = AtomicU32::new(0);
pub static NPC_DIED: AtomicU32 = AtomicU32::new(0);
/// The death reason the handler received, low byte of the word.
pub static NPC_DEATH_REASON: AtomicI32 = AtomicI32::new(-1);
pub static OBJECT_MOVED: AtomicU32 = AtomicU32::new(0);

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
