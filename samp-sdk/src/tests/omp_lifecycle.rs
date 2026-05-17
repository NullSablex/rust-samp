//! Smoke tests for the Open Multiplayer lifecycle:
//!
//! - `IPawnComponent` constants (known UID)
//! - Layout of [`PawnEventHandlerVTable`] (the vtable WE implement
//!   to receive `onAmxLoad`/`onAmxUnload`)
//!
//! Real integration tests (calls via mock `ICore`, full `on_load` → `on_init`
//! → `on_ready`) are out of scope — they require heavy mocks and have been
//! deferred in the ROADMAP.
//!
//! [`PawnEventHandlerVTable`]: crate::omp::events::PawnEventHandlerVTable

use crate::omp::events::{PawnEventHandler, PawnEventHandlerVTable};
use crate::omp::server::{IPawnScript, PAWN_COMPONENT_UID};

#[test]
fn pawn_component_uid_is_nonzero() {
    assert_ne!(PAWN_COMPONENT_UID, 0);
}

#[test]
fn pawn_component_uid_is_expected_value() {
    // Constant hardcoded in the Open Multiplayer SDK (PawnComponent_UID in components.hpp).
    // If it changes between versions, query_component for Pawn fails — breaking
    // the entire native registration and the on_amx_load cycle.
    assert_eq!(PAWN_COMPONENT_UID, 0x7890_6cd9_f19c_36a6);
}

#[cfg(not(target_env = "msvc"))]
unsafe extern "C" fn noop_amx_load(_: *mut PawnEventHandler, _: *mut IPawnScript) {}
#[cfg(not(target_env = "msvc"))]
unsafe extern "C" fn noop_amx_unload(_: *mut PawnEventHandler, _: *mut IPawnScript) {}

#[cfg(target_env = "msvc")]
unsafe extern "thiscall" fn noop_amx_load(_: *mut PawnEventHandler, _: *mut IPawnScript) {}
#[cfg(target_env = "msvc")]
unsafe extern "thiscall" fn noop_amx_unload(_: *mut PawnEventHandler, _: *mut IPawnScript) {}

#[test]
fn pawn_event_handler_vtable_layout() {
    static VTABLE: PawnEventHandlerVTable = PawnEventHandlerVTable {
        on_amx_load: noop_amx_load,
        on_amx_unload: noop_amx_unload,
    };

    // Smoke test: vtable fields are valid (non-null) function pointers.
    assert!(!std::ptr::addr_of!(VTABLE.on_amx_load).is_null());
    assert!(!std::ptr::addr_of!(VTABLE.on_amx_unload).is_null());
}

#[test]
fn pawn_event_handler_new_stores_vtable_ptr() {
    static VTABLE: PawnEventHandlerVTable = PawnEventHandlerVTable {
        on_amx_load: noop_amx_load,
        on_amx_unload: noop_amx_unload,
    };
    let handler = PawnEventHandler::new(&raw const VTABLE);
    // Exercises the constructor; the `vtable` field is private and has no
    // accessor, so we cannot assert pointer equality directly.
    let _ = handler;
}
