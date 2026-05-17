//! Pawn script events via `IEventDispatcher<PawnEventHandler>`.
//!
//! The Open Multiplayer server invokes `onAmxLoad`/`onAmxUnload` via vtable when scripts
//! are loaded/unloaded. In native component mode, these events replace SA-MP's
//! `AmxLoad`/`AmxUnload` — the SDK registers an internal handler on the
//! `IEventDispatcher` in `omp_on_init` and routes to the matching methods of
//! the `SampPlugin` trait.
//!
//! ## `PawnEventHandler` vtable
//!
//! No virtual destructor in the header, so only 2 slots — identical on Itanium
//! and MSVC (only the calling convention differs):
//!
//! ```text
//! [0] onAmxLoad(IPawnScript&)
//! [1] onAmxUnload(IPawnScript&)
//! ```

use super::server::IPawnScript;

// ---------------------------------------------------------------------------
// PawnEventHandler vtable and object (WE implement — the server calls)
// ---------------------------------------------------------------------------

/// `PawnEventHandler` vtable for the Itanium ABI (Linux).
///
/// `PawnEventHandler` does not declare a virtual destructor — no index shift
/// between ABIs. The only difference is the calling convention.
#[cfg(not(target_env = "msvc"))]
#[repr(C)]
pub struct PawnEventHandlerVTable {
    pub on_amx_load: unsafe extern "C" fn(*mut PawnEventHandler, *mut IPawnScript),
    pub on_amx_unload: unsafe extern "C" fn(*mut PawnEventHandler, *mut IPawnScript),
}

/// `PawnEventHandler` vtable for the MSVC ABI (Windows).
#[cfg(target_env = "msvc")]
#[repr(C)]
pub struct PawnEventHandlerVTable {
    pub on_amx_load: unsafe extern "thiscall" fn(*mut PawnEventHandler, *mut IPawnScript),
    pub on_amx_unload: unsafe extern "thiscall" fn(*mut PawnEventHandler, *mut IPawnScript),
}

/// Rust object compatible with Open Multiplayer's `PawnEventHandler*`.
///
/// Created by `samp` in `omp_on_init` and registered on the `IPawnComponent`
/// dispatcher. The server calls the vtable methods when Pawn scripts are
/// loaded or unloaded.
#[repr(C)]
pub struct PawnEventHandler {
    vtable: *const PawnEventHandlerVTable,
}

// SAFETY: handler is only accessed on the server's main thread.
unsafe impl Send for PawnEventHandler {}
unsafe impl Sync for PawnEventHandler {}

impl PawnEventHandler {
    /// Creates a new handler with the supplied vtable.
    #[must_use]
    pub fn new(vtable: *const PawnEventHandlerVTable) -> Self {
        Self { vtable }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn noop_amx_load(_: *mut PawnEventHandler, _: *mut IPawnScript) {}
    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn noop_amx_unload(_: *mut PawnEventHandler, _: *mut IPawnScript) {}

    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn noop_amx_load(_: *mut PawnEventHandler, _: *mut IPawnScript) {}
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn noop_amx_unload(_: *mut PawnEventHandler, _: *mut IPawnScript) {}

    static TEST_VTABLE: PawnEventHandlerVTable = PawnEventHandlerVTable {
        on_amx_load: noop_amx_load,
        on_amx_unload: noop_amx_unload,
    };

    #[test]
    fn pawn_event_handler_new_stores_vtable() {
        let handler = PawnEventHandler::new(&raw const TEST_VTABLE);
        assert_eq!(handler.vtable, &raw const TEST_VTABLE);
    }

    #[test]
    fn pawn_event_handler_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PawnEventHandler>();
    }
}
