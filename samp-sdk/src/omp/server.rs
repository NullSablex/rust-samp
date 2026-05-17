//! Vtables for objects managed by the Open Multiplayer server.
//!
//! The vtable indices and signatures were derived from the public
//! specification of the Open Multiplayer SDK (<https://github.com/openmultiplayer/open.mp-sdk>).
//! No SDK code was copied.
//!
//! Unlike `component.rs` (where WE implement the vtable), here we define
//! vtables for objects created by the SERVER so we can call methods on
//! them from Rust.
//!
//! ## Indices per ABI
//!
//! **Itanium ABI** — two destructor slots (D1 + D0) interleaved after the
//! virtuals of each base class, in declaration order.
//!
//! **MSVC ABI** — a single destructor (scalar deleting) at the end of the
//! virtuals of the class that declared it (e.g. `~IExtensible` at slot [4],
//! after `removeExtension`).
//!
//! The practical difference is that on MSVC the vtable is 1 slot smaller per
//! destructor (no separate D0 slot), which shifts all subsequent methods.

use crate::raw::types::AMX;

use super::events::PawnEventHandler;
use super::types::UID;

/// Number of AMX functions exported by `IPawnComponent` (`NUM_AMX_FUNCS` in the SDK).
pub const NUM_AMX_FUNCS: usize = 52;

/// UID of the Open Multiplayer Pawn component (`PawnComponent_UID` in the SDK).
pub const PAWN_COMPONENT_UID: UID = 0x7890_6cd9_f19c_36a6;

// ---------------------------------------------------------------------------
// IComponentList — list of loaded components (server-owned object)
// ---------------------------------------------------------------------------
//
// Inheritance: IComponentList : public IExtensible
//
// Primary vtable (Itanium ABI):
//   [0-3] IExtensible (get/add/remove/remove)
//   [4]   ~destructor D1
//   [5]   ~destructor D0
//   [6]   IComponentList::queryComponent(UID) -> IComponent*
//
// Primary vtable (MSVC ABI):
//   [0-3] IExtensible (get/add/remove/remove)
//   [4]   ~destructor (single scalar deleting)
//   [5]   IComponentList::queryComponent(UID) -> IComponent*

/// Opaque handle for the server's `IComponentList*`.
#[repr(C)]
pub struct ServerComponentList {
    vtable: *const ServerComponentListVTable,
}

// IComponentList vtable layout — only the count of opaque destructor slots
// differs (Itanium: D1+D0 = 2 slots; MSVC: single scalar deleting = 1 slot).
// The calling convention also differs (Itanium "C" vs MSVC "thiscall").
//
// Opaque slots (in order):
//   [0] IExtensible::getExtension
//   [1] IExtensible::addExtension
//   [2] IExtensible::removeExtension(ptr)
//   [3] IExtensible::removeExtension(uid)
//   [4] ~destructor (D1 on Itanium, scalar deleting on MSVC)
//   [5] ~destructor D0 (Itanium-only — does not exist on MSVC)
//
// Useful slot:
//   [6 Itanium / 5 MSVC] queryComponent
#[cfg(not(target_env = "msvc"))]
type QueryComponentFn = unsafe extern "C" fn(*mut ServerComponentList, UID) -> *mut ServerComponent;
#[cfg(target_env = "msvc")]
type QueryComponentFn =
    unsafe extern "thiscall" fn(*mut ServerComponentList, UID) -> *mut ServerComponent;

#[cfg(not(target_env = "msvc"))]
const COMPONENT_LIST_PREFIX_SLOTS: usize = 6;
#[cfg(target_env = "msvc")]
const COMPONENT_LIST_PREFIX_SLOTS: usize = 5;

#[repr(C)]
struct ServerComponentListVTable {
    _prefix: [*const (); COMPONENT_LIST_PREFIX_SLOTS],
    query_component: QueryComponentFn,
}

/// Opaque handle for the `IComponent*` returned by queryComponent.
#[repr(C)]
pub struct ServerComponent {
    vtable: *const (),
}

/// Queries a component by UID in the list provided by the server.
///
/// # Safety
/// `list` must be a valid pointer to an Open Multiplayer server `IComponentList`.
pub unsafe fn query_component(list: *mut ServerComponentList, uid: UID) -> *mut ServerComponent {
    unsafe { ((*(*list).vtable).query_component)(list, uid) }
}

// ---------------------------------------------------------------------------
// IPawnComponent — access to the PAWN/AMX subsystem (server-owned object)
// ---------------------------------------------------------------------------
//
// Inheritance: IPawnComponent : public IComponent : public IExtensible, IUIDProvider
//
// Primary vtable (Itanium ABI) — confirmed by runtime dump (Open Multiplayer 1.5.8):
//   [0-4]  IExtensible + server-internal slots
//   [5]    ~PawnComponent D1
//   [6]    ~PawnComponent D0 (deleting)
//   [7]    componentName
//   [8]    (unknown)
//   [9]    componentVersion
//   [10]   onLoad
//   [11]   (unknown)
//   [12]   onReady
//   [13]   onFree
//   [14]   (unknown)
//   [15]   free
//   [16]   reset
//   [17]   (unknown)
//   [18]   IPawnComponent::getEventDispatcher  <- confirmed at runtime
//   [19]   IPawnComponent::getAmxFunctions     <- confirmed at runtime
//
// Primary vtable (MSVC ABI):
//   [0-3]  IExtensible (get/add/remove/remove)
//   [4]    ~destructor (single scalar deleting)
//   [5-15] IComponent (supportedVersion..reset)
//   [16]   IPawnComponent::getEventDispatcher
//   [17]   IPawnComponent::getAmxFunctions

/// Opaque handle for the server's `IPawnComponent*`.
#[repr(C)]
pub struct ServerPawnComponent {
    vtable: *const ServerPawnComponentVTable,
    // IUIDProvider secondary vtable — we do not access it directly
    _uid_vtable: *const (),
}

// IPawnComponent vtable layout — useful slots ([18-19] Itanium / [16-17] MSVC)
// and shared trailing opaques. The prefix difference comes from how each ABI
// emits destructors (Itanium D1+D0 + unknown slots confirmed in the dump).
//
// Opaque prefix slots (Itanium ABI, 18 slots — confirmed by runtime dump):
//   [0-4]   IExtensible (4 methods + 1 unknown slot)
//   [5]     ~PawnComponent D1
//   [6]     ~PawnComponent D0 (deleting)
//   [7]     componentName
//   [8]     (unknown)
//   [9]     componentVersion
//   [10]    onLoad
//   [11]    (unknown)
//   [12]    onReady
//   [13]    onFree
//   [14]    (unknown)
//   [15]    free
//   [16]    reset
//   [17]    (unknown)
//
// Opaque prefix slots (MSVC ABI, 16 slots):
//   [0-3]   IExtensible (get/add/removeExt(ptr)/removeExt(uid))
//   [4]     ~destructor (single scalar deleting)
//   [5-15]  IComponent (supportedVersion, componentName, componentType,
//           componentVersion, onLoad, onInit, onReady, onFree,
//           provideConfiguration, free, reset)
//
// Useful slots (in both):
//   [18-19 Itanium / 16-17 MSVC] getEventDispatcher, getAmxFunctions
//
// Trailing opaques (4 slots, identical in both ABIs):
//   getScript(const), getScript(mut), mainScript, sideScripts
#[cfg(not(target_env = "msvc"))]
type GetEventDispatcherFn =
    unsafe extern "C" fn(*mut ServerPawnComponent) -> *mut IEventDispatcherPawn;
#[cfg(target_env = "msvc")]
type GetEventDispatcherFn =
    unsafe extern "thiscall" fn(*mut ServerPawnComponent) -> *mut IEventDispatcherPawn;

#[cfg(not(target_env = "msvc"))]
type GetAmxFunctionsFn =
    unsafe extern "C" fn(*const ServerPawnComponent) -> *const AmxFunctionTable;
#[cfg(target_env = "msvc")]
type GetAmxFunctionsFn =
    unsafe extern "thiscall" fn(*const ServerPawnComponent) -> *const AmxFunctionTable;

#[cfg(not(target_env = "msvc"))]
const PAWN_COMPONENT_PREFIX_SLOTS: usize = 18;
#[cfg(target_env = "msvc")]
const PAWN_COMPONENT_PREFIX_SLOTS: usize = 16;

#[repr(C)]
struct ServerPawnComponentVTable {
    _prefix: [*const (); PAWN_COMPONENT_PREFIX_SLOTS],
    get_event_dispatcher: GetEventDispatcherFn,
    get_amx_functions: GetAmxFunctionsFn,
    // Trailing opaques common to both ABIs.
    _get_script_const: *const (),
    _get_script_mut: *const (),
    _main_script: *const (),
    _side_scripts: *const (),
}

/// Table of 52 AMX function pointers (`StaticArray<void*, NUM_AMX_FUNCS>`).
pub type AmxFunctionTable = [*mut (); NUM_AMX_FUNCS];

// ---------------------------------------------------------------------------
// IPawnScript — opaque handle; we only use GetAMX() at index [57]
// ---------------------------------------------------------------------------
//
// IPawnScript does not declare a virtual destructor.
// Methods [0..56] are opaque; [57] is GetAMX().
// Index 57 is identical on Itanium and MSVC (no virtual destructor = no shift).

/// Opaque handle for the server's `IPawnScript*`.
#[repr(C)]
pub struct IPawnScript {
    vtable: *const IPawnScriptVTable,
}

// IPawnScript does not declare a virtual destructor — identical layout on
// Itanium and MSVC. Slot [57] is GetAMX(); only the calling convention differs.
#[cfg(not(target_env = "msvc"))]
type GetAmxFn = unsafe extern "C" fn(*mut IPawnScript) -> *mut AMX;
#[cfg(target_env = "msvc")]
type GetAmxFn = unsafe extern "thiscall" fn(*mut IPawnScript) -> *mut AMX;

#[repr(C)]
struct IPawnScriptVTable {
    _prefix: [*const (); 57],
    get_amx: GetAmxFn,
}

/// Extracts the `AMX*` pointer from an `IPawnScript*`.
///
/// # Safety
/// `script` must be a valid pointer to an Open Multiplayer server `IPawnScript`.
pub unsafe fn get_amx_from_script(script: *mut IPawnScript) -> *mut AMX {
    unsafe { ((*(*script).vtable).get_amx)(script) }
}

// ---------------------------------------------------------------------------
// IEventDispatcher<PawnEventHandler> — server-side vtable
// ---------------------------------------------------------------------------
//
// IEventDispatcher<T> does not declare a virtual destructor.
// Vtable:
//   [0] addEventHandler(handler*, priority: i8) -> bool
//   [1] removeEventHandler(handler*) -> bool
//   [2] hasEventHandler (unused)
//   [3] count (unused)
//
// No virtual destructor = no shift between Itanium and MSVC.
// Only the calling convention differs.

/// Opaque handle for the server's `IEventDispatcher<PawnEventHandler>*`.
#[repr(C)]
pub struct IEventDispatcherPawn {
    vtable: *const IEventDispatcherPawnVTable,
}

// IEventDispatcher<T> does not declare a virtual destructor — identical layout
// on both ABIs. Slots [0..3]: addEventHandler, removeEventHandler,
// hasEventHandler, count. Only the first two are used; only the calling
// convention differs.
#[cfg(not(target_env = "msvc"))]
type AddEventHandlerFn =
    unsafe extern "C" fn(*mut IEventDispatcherPawn, *mut PawnEventHandler, i8) -> bool;
#[cfg(target_env = "msvc")]
type AddEventHandlerFn =
    unsafe extern "thiscall" fn(*mut IEventDispatcherPawn, *mut PawnEventHandler, i8) -> bool;

#[cfg(not(target_env = "msvc"))]
type RemoveEventHandlerFn =
    unsafe extern "C" fn(*mut IEventDispatcherPawn, *mut PawnEventHandler) -> bool;
#[cfg(target_env = "msvc")]
type RemoveEventHandlerFn =
    unsafe extern "thiscall" fn(*mut IEventDispatcherPawn, *mut PawnEventHandler) -> bool;

#[repr(C)]
struct IEventDispatcherPawnVTable {
    add_event_handler: AddEventHandlerFn,
    remove_event_handler: RemoveEventHandlerFn,
    _has_event_handler: *const (),
    _count: *const (),
}

/// Registers a Pawn event handler in the dispatcher.
///
/// # Safety
/// Both pointers must be valid. `handler` must outlive the dispatcher.
pub unsafe fn add_pawn_event_handler(
    dispatcher: *mut IEventDispatcherPawn,
    handler: *mut PawnEventHandler,
) {
    unsafe { ((*(*dispatcher).vtable).add_event_handler)(dispatcher, handler, 0) };
}

/// Removes a Pawn event handler from the dispatcher.
///
/// # Safety
/// Both pointers must be valid.
pub unsafe fn remove_pawn_event_handler(
    dispatcher: *mut IEventDispatcherPawn,
    handler: *mut PawnEventHandler,
) {
    unsafe { ((*(*dispatcher).vtable).remove_event_handler)(dispatcher, handler) };
}

/// Gets the Pawn event dispatcher from the `IPawnComponent`.
///
/// # Safety
/// `pawn` must be a valid pointer to an Open Multiplayer server `IPawnComponent`.
pub unsafe fn get_pawn_event_dispatcher(pawn: *mut ServerComponent) -> *mut IEventDispatcherPawn {
    let pawn = pawn.cast::<ServerPawnComponent>();
    unsafe { ((*(*pawn).vtable).get_event_dispatcher)(pawn) }
}

/// Gets the pointer to the AMX function table from the `IPawnComponent`.
///
/// # Safety
/// `pawn` must be a valid pointer to an Open Multiplayer server `IPawnComponent`.
pub unsafe fn get_amx_functions(pawn: *mut ServerComponent) -> usize {
    let pawn = pawn as *const ServerPawnComponent;
    let table_ptr = unsafe { ((*(*pawn).vtable).get_amx_functions)(pawn) };
    table_ptr as usize
}

// ---------------------------------------------------------------------------
// PawnComponent — high-level typed wrapper
// ---------------------------------------------------------------------------

use super::component_api::OmpComponentHandle;
use std::ptr::NonNull;

/// Typed wrapper for the Open Multiplayer server's `IPawnComponent`.
///
/// Obtained via `samp::plugin::omp_query::<PawnComponent>()`. Exposes the
/// Pawn-specific methods (event dispatcher, AMX functions) in addition to the
/// generic `IComponent` ones (`name()`, `version()` via `component_api`).
#[derive(Debug, Clone, Copy)]
pub struct PawnComponent {
    ptr: NonNull<ServerComponent>,
}

impl OmpComponentHandle for PawnComponent {
    const UID: UID = PAWN_COMPONENT_UID;

    unsafe fn from_raw(ptr: NonNull<ServerComponent>) -> Self {
        Self { ptr }
    }

    fn as_raw(&self) -> NonNull<ServerComponent> {
        self.ptr
    }
}

impl PawnComponent {
    /// Returns the component name — equivalent to `component_name(&self)`.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        super::component_api::component_name(self)
    }

    /// Returns the component version — equivalent to `component_version(&self)`.
    #[must_use]
    pub fn version(&self) -> Option<super::types::SemanticVersion> {
        super::component_api::component_version(self)
    }

    /// Returns the component's `IEventDispatcher<PawnEventHandler>`.
    ///
    /// Use it to register AMX event handlers (load/unload).
    #[must_use]
    pub fn event_dispatcher(&self) -> *mut IEventDispatcherPawn {
        unsafe { get_pawn_event_dispatcher(self.ptr.as_ptr()) }
    }

    /// Returns the AMX function table as `usize` (raw pointer).
    ///
    /// Available only after `on_omp_ready` — before that callback,
    /// `getAmxFunctions()` returns 0 (server behavior).
    #[must_use]
    pub fn amx_functions(&self) -> usize {
        unsafe { get_amx_functions(self.ptr.as_ptr()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pawn_component_uid_is_nonzero() {
        assert_ne!(PAWN_COMPONENT_UID, 0);
    }

    #[test]
    fn pawn_component_uid_matches_known_value() {
        // Value derived from the Open Multiplayer SDK (PawnComponent_UID).
        // If it changes, the vtable indices and the entire Open Multiplayer integration break.
        assert_eq!(PAWN_COMPONENT_UID, 0x7890_6cd9_f19c_36a6);
    }

    #[test]
    fn num_amx_funcs_is_52() {
        assert_eq!(NUM_AMX_FUNCS, 52);
    }

    #[test]
    fn pawn_component_uid_via_trait_matches_constant() {
        assert_eq!(
            <PawnComponent as OmpComponentHandle>::UID,
            PAWN_COMPONENT_UID
        );
    }

    #[test]
    fn pawn_component_is_copy() {
        // Sanity: the wrapper should be Copy so it can be used freely in closures.
        fn assert_copy<T: Copy>() {}
        assert_copy::<PawnComponent>();
    }

    #[test]
    fn pawn_component_size_is_one_pointer() {
        // Only stores a pointer — no overhead vs `*mut ServerComponent`.
        assert_eq!(
            std::mem::size_of::<PawnComponent>(),
            std::mem::size_of::<*const ()>()
        );
    }
}
