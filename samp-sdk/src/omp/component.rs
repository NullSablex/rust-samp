//! `IComponent` interface from the Open Multiplayer SDK in pure Rust.
//!
//! The memory layout and vtable indices were derived from the public
//! specification of the Open Multiplayer SDK (<https://github.com/openmultiplayer/open.mp-sdk>).
//! No SDK code was copied — only signatures and vtable layouts were used
//! as reference for this pure Rust reimplementation.
//!
//! ## Platform support
//!
//! | Target                      | SA-MP | native Open Multiplayer |
//! |-----------------------------|-------|----------------|
//! | `i686-unknown-linux-gnu`    | yes   | yes (Itanium ABI) |
//! | `i686-pc-windows-msvc`      | yes   | yes (MSVC ABI)   |
//! | `i686-pc-windows-gnu`       | yes   | no (incompatible ABI) |
//!
//! ## Multiple inheritance and vtables
//!
//! `IComponent : public IExtensible, public IUIDProvider` results in two
//! vtable pointers in the object. The offsets differ between Itanium (Linux GCC)
//! and MSVC because the `FlatHashMap` (`robin_hood::unordered_flat_map`) has a
//! different size on the two platforms — confirmed via disasm of `omp-server.exe`.
//!
//! ```text
//! Offset  Field (Linux / GCC i686)
//! ------  -----
//!  0      vtable*      (primary: IExtensible + IComponent)
//!  4..39  _misc_ext    (robin_hood::unordered_flat_map, 36 bytes)
//! 40      uid_vtable*  (secondary: IUIDProvider)
//! 44      uid          (u64 — plugin's own field)
//! 52      plugin_ptr   (*mut () — plugin's own field)
//!
//! Offset  Field (Windows / MSVC i686)
//! ------  -----
//!  0      vtable*      (primary: IExtensible + IComponent)
//!  4..55  _misc_ext    (52 bytes: padding + robin_hood + trailing padding)
//! 56      uid_vtable*  (secondary: IUIDProvider) — offset hardcoded by the server
//! 60      _uid_pad     (4 bytes to align uid (u64) to 8 bytes)
//! 64      uid          (u64 — plugin's own field)
//! 72      plugin_ptr   (*mut () — plugin's own field)
//! ```
//!
//! ## Primary vtable
//!
//! **Itanium ABI** — two destructor slots (D1 complete + D0 deleting):
//!
//! ```text
//! [0]  getExtension
//! [1]  addExtension
//! [2]  removeExtension(ext*)
//! [3]  removeExtension(uid)
//! [4]  ~destructor D1 (complete object)
//! [5]  ~destructor D0 (deleting)
//! [6]  supportedVersion
//! [7]  componentName
//! [8]  componentType
//! [9]  componentVersion
//! [10] onLoad
//! [11] onInit
//! [12] onReady
//! [13] onFree
//! [14] provideConfiguration
//! [15] free
//! [16] reset
//! ```
//!
//! **MSVC ABI** — single destructor (scalar deleting) between `IExtensible` and `IComponent`:
//!
//! ```text
//! [0]  getExtension
//! [1]  addExtension
//! [2]  removeExtension(ext*)
//! [3]  removeExtension(uid)
//! [4]  ~destructor (single scalar deleting — MSVC does not emit D0)
//! [5]  supportedVersion
//! [6]  componentName
//! [7]  componentType
//! [8]  componentVersion
//! [9]  onLoad
//! [10] onInit
//! [11] onReady
//! [12] onFree
//! [13] provideConfiguration
//! [14] free
//! [15] reset
//! ```
//!
//! The slots were confirmed by runtime + disasm of `omp-server.exe`
//! (`componentVersion` calls at `[edx+0x20]` = slot 8; `componentName` at
//! `[eax+0x18]` = slot 6).
//!
//! ## Secondary vtable — `IUIDProvider`
//!
//! **Itanium ABI** — two destructor slots before `getUID`:
//!
//! ```text
//! [0]  destructor D1 thunk
//! [1]  destructor D0 thunk
//! [2]  getUID
//! ```
//!
//! **MSVC ABI** — only `getUID` (`IUIDProvider` does not declare a virtual destructor):
//!
//! ```text
//! [0]  getUID
//! ```

#[allow(unused_imports)]
use super::types::{ComponentType, SemanticVersion, StringView, UID};

// ---------------------------------------------------------------------------
// Opaque types — pointers to server interfaces we do not implement
// ---------------------------------------------------------------------------

/// `ICore*` — opaque pointer to the Open Multiplayer server core.
/// Received in `on_load`; use only for caching or future queries.
#[repr(C)]
pub struct ICore {
    _opaque: [u8; 0],
}

/// `IComponentList*` — list of loaded components.
/// Received in `on_init`; use to query other components.
#[repr(C)]
pub struct IComponentList {
    _opaque: [u8; 0],
}

/// `ILogger*` — server logging interface.
#[repr(C)]
pub struct ILogger {
    _opaque: [u8; 0],
}

/// `IEarlyConfig*` — configuration during initialization.
#[repr(C)]
pub struct IEarlyConfig {
    _opaque: [u8; 0],
}

// ---------------------------------------------------------------------------
// Primary vtable: IExtensible + IComponent — Itanium ABI
// ---------------------------------------------------------------------------
//
// **Why duplicate the entire vtable (Itanium vs MSVC) instead of using type
// aliases?** Function signatures differ **substantially**, not just in
// calling convention:
//
//   - **MSVC** returns `StringView` (8 bytes) and `SemanticVersion` (6 bytes)
//     via hidden pointer in `[ESP+4]`. The functions become `extern "thiscall"
//     fn()` (no parameters, naked asm) so Rust emits `ret` without
//     `ret 4`. Itanium, by contrast, returns these types by value with the
//     full signature `fn(*const OmpComponent) -> StringView`.
//   - **MSVC** collapses the two destructor slots (D1+D0) into a single
//     scalar deleting.
//
// A type alias only covers ABI; here the function shape itself changes.
// Keeping the two definitions explicit is clearer than trying to abstract.

/// Primary `IComponent` vtable for the Itanium ABI (Linux).
///
/// Calling convention: `extern "C"` (cdecl).
/// Two destructor slots: D1 (complete) and D0 (deleting).
#[cfg(not(target_env = "msvc"))]
#[repr(C)]
pub struct IComponentVTable {
    // --- IExtensible [0-3] ---
    pub get_extension: unsafe extern "C" fn(*mut OmpComponent, uid: UID) -> *mut (),
    pub add_extension:
        unsafe extern "C" fn(*mut OmpComponent, ext: *mut (), auto_delete: bool) -> bool,
    pub remove_extension_ptr: unsafe extern "C" fn(*mut OmpComponent, ext: *mut ()) -> bool,
    pub remove_extension_uid: unsafe extern "C" fn(*mut OmpComponent, uid: UID) -> bool,
    /// D1 — complete object destructor (Itanium ABI).
    pub destructor: unsafe extern "C" fn(*mut OmpComponent),
    /// D0 — deleting destructor (Itanium ABI requires two slots).
    pub destructor_deleting: unsafe extern "C" fn(*mut OmpComponent),
    // --- IComponent [6-16] ---
    pub supported_version: unsafe extern "C" fn(*const OmpComponent) -> i32,
    pub component_name: unsafe extern "C" fn(*const OmpComponent) -> StringView,
    pub component_type: unsafe extern "C" fn(*const OmpComponent) -> ComponentType,
    pub component_version: unsafe extern "C" fn(*const OmpComponent) -> SemanticVersion,
    pub on_load: unsafe extern "C" fn(*mut OmpComponent, *mut ICore),
    pub on_init: unsafe extern "C" fn(*mut OmpComponent, *mut IComponentList),
    pub on_ready: unsafe extern "C" fn(*mut OmpComponent),
    pub on_free: unsafe extern "C" fn(*mut OmpComponent, *mut OmpComponent),
    pub provide_configuration:
        unsafe extern "C" fn(*mut OmpComponent, *mut ILogger, *mut IEarlyConfig, bool),
    pub free: unsafe extern "C" fn(*mut OmpComponent),
    pub reset: unsafe extern "C" fn(*mut OmpComponent),
}

// ---------------------------------------------------------------------------
// Primary vtable: IExtensible + IComponent — MSVC ABI
// ---------------------------------------------------------------------------

/// Primary `IComponent` vtable for the MSVC ABI (Windows).
///
/// Calling convention: `extern "thiscall"` (`this` in ECX).
///
/// MSVC i686 with single inheritance generates **a single** destructor slot (scalar deleting).
/// The destructor sits at the position where `~IExtensible()` was declared (after the other
/// IExtensible virtuals):
///   [0] getExtension, [1] addExtension, [2] removeExtension(ptr),
///   [3] removeExtension(UID), [4] ~IExtensible (scalar deleting)
///   IComponent adds:
///   [5] supportedVersion, [6] componentName, [7] componentType,
///   [8] componentVersion, [9] onLoad, [10] onInit, [11] onReady,
///   [12] onFree, [13] provideConfiguration, [14] free, [15] reset
#[cfg(target_env = "msvc")]
#[repr(C)]
pub struct IComponentVTable {
    // --- IExtensible [0-4] ---
    pub get_extension: unsafe extern "thiscall" fn(*mut OmpComponent, uid: UID) -> *mut (),
    pub add_extension:
        unsafe extern "thiscall" fn(*mut OmpComponent, ext: *mut (), auto_delete: bool) -> bool,
    pub remove_extension_ptr: unsafe extern "thiscall" fn(*mut OmpComponent, ext: *mut ()) -> bool,
    pub remove_extension_uid: unsafe extern "thiscall" fn(*mut OmpComponent, uid: UID) -> bool,
    // Functions with no stack args besides this: this in ECX, no explicit parameter.
    // This prevents the compiler from emitting `ret 4` which would corrupt the stack.
    pub destructor: unsafe extern "thiscall" fn(),
    pub supported_version: unsafe extern "thiscall" fn() -> i32,
    // Naked functions: return via eax:edx, return type () in the Rust signature.
    pub component_name: unsafe extern "thiscall" fn(),
    pub component_type: unsafe extern "thiscall" fn() -> i32,
    pub component_version: unsafe extern "thiscall" fn(),
    // Functions with additional stack args: this in ECX + args on the stack (ret N correct).
    pub on_load: unsafe extern "thiscall" fn(*mut OmpComponent, *mut ICore),
    pub on_init: unsafe extern "thiscall" fn(*mut OmpComponent, *mut IComponentList),
    pub on_ready: unsafe extern "thiscall" fn(),
    pub on_free: unsafe extern "thiscall" fn(*mut OmpComponent, *mut OmpComponent),
    pub provide_configuration:
        unsafe extern "thiscall" fn(*mut OmpComponent, *mut ILogger, *mut IEarlyConfig, bool),
    pub free: unsafe extern "thiscall" fn(),
    pub reset: unsafe extern "thiscall" fn(),
}

// ---------------------------------------------------------------------------
// Secondary vtable: IUIDProvider — Itanium ABI
// ---------------------------------------------------------------------------
//
// Genuinely different layouts: Itanium has 3 slots (D1, D0, getUID);
// MSVC has 1 slot (only getUID — no virtual destructor). Kept duplicated
// because the static initializers in `samp-codegen/src/plugin.rs` are also
// cfg-gated with different field names; unifying would require changing
// both ends and would lose the clarity of the `pub destructor_*` fields on Itanium.

/// Secondary `IUIDProvider` vtable for the Itanium ABI (Linux).
///
/// Two destructor thunk slots before `getUID`.
#[cfg(not(target_env = "msvc"))]
#[repr(C)]
pub struct IUIDProviderVTable {
    /// D1 thunk — never called directly by the Open Multiplayer server.
    pub destructor_complete: unsafe extern "C" fn(*mut u8),
    /// D0 thunk — never called directly by the Open Multiplayer server.
    pub destructor_deleting: unsafe extern "C" fn(*mut u8),
    /// `getUID()` — `this` points to the `IUIDProvider` subobject (offset 40 on Linux).
    pub get_uid: unsafe extern "C" fn(*const u8) -> UID,
}

// ---------------------------------------------------------------------------
// Secondary vtable: IUIDProvider — MSVC ABI
// ---------------------------------------------------------------------------

/// Secondary `IUIDProvider` vtable for the MSVC ABI (Windows).
///
/// `IUIDProvider` declares ONLY `virtual UID getUID() = 0;` — no virtual destructor.
/// Confirmed by server disasm: `add ecx, 0x38; mov eax, [esi+0x38]; call [eax]`
/// (adjusts `this` by +56, loads secondary vtable, calls slot [0]).
#[cfg(target_env = "msvc")]
#[repr(C)]
pub struct IUIDProviderVTable {
    /// Slot [0]: `getUID()` — `this` points to the IUIDProvider subobject (offset 56 on MSVC).
    pub get_uid: unsafe extern "thiscall" fn(*const u8) -> UID,
}

// ---------------------------------------------------------------------------
// Object compatible with IComponent* — per-platform layout
// ---------------------------------------------------------------------------

/// Rust object with a layout compatible with Open Multiplayer's `IComponent*`.
///
/// The layout differs between Linux (GCC i686) and Windows MSVC i686 because
/// `FlatHashMap` (robin_hood::unordered_flat_map) has a different sizeof on
/// each platform. The offset of `uid_vtable` (IUIDProvider subobject) is
/// hardcoded by the server and was confirmed via disasm:
/// - Linux/GCC i686: `uid_vtable` at offset **40**.
/// - MSVC i686: `uid_vtable` at offset **56** (server emits `add ecx, 0x38`
///   when calling `getUID()` on `IComponent*`).
///
/// Layout on i686 Linux (GCC / Itanium ABI):
/// ```text
/// offset  0: vtable*        (primary IExtensible/IComponent)
/// offset  4: _misc_ext[36]  (robin_hood::unordered_flat_map, zero-init = empty)
/// offset 40: uid_vtable*    (secondary IUIDProvider)
/// offset 44: uid             (UID = u64)
/// offset 52: plugin_ptr      (*mut ())
/// ```
///
/// Layout on i686 Windows (MSVC ABI):
/// ```text
/// offset  0: vtable*        (primary IExtensible/IComponent)
/// offset  4: _misc_ext[52]  (padding + robin_hood + trailing pad, zero-init)
/// offset 56: uid_vtable*    (secondary IUIDProvider)
/// offset 60: _uid_pad[4]    (padding to align uid (u64) to 8 bytes)
/// offset 64: uid             (UID = u64)
/// offset 72: plugin_ptr      (*mut ())
/// ```
// MSVC: server expects the IUIDProvider vptr at offset 56 (confirmed at runtime via disasm).
// Total IExtensible = 4 (vptr) + 52 (_misc_ext) = 56 bytes.
#[cfg(target_env = "msvc")]
const MISC_EXT_SIZE: usize = 52;
#[cfg(not(target_env = "msvc"))]
const MISC_EXT_SIZE: usize = 36;

#[repr(C)]
pub struct OmpComponent {
    vtable: *const IComponentVTable,
    _misc_ext: [u8; MISC_EXT_SIZE],
    uid_vtable: *const IUIDProviderVTable,
    #[cfg(target_env = "msvc")]
    _uid_pad: u32,
    /// Unique UID for this component.
    pub uid: UID,
    /// Pointer to the Rust plugin (`SampPlugin`).
    pub plugin_ptr: *mut (),
}

// SAFETY: OmpComponent is sent to the server as an opaque pointer.
// The server is single-threaded across component lifecycle calls.
unsafe impl Send for OmpComponent {}
unsafe impl Sync for OmpComponent {}

/// Compile-time check of the `OmpComponent` layout on i686 Linux (Itanium ABI).
///
/// On GCC i686, `uint64_t` is aligned to 4 bytes — without `_pad`, the `robin_hood` map
/// starts at offset 4 and `uid_vtable` (the `IUIDProvider` subobject) lands at offset 40.
#[cfg(all(target_arch = "x86", target_os = "linux"))]
const _: () = {
    assert!(
        std::mem::offset_of!(OmpComponent, uid_vtable) == 40,
        "OmpComponent: invalid offset. On GCC i686, uint64_t is aligned to 4 bytes — uid_vtable must be at offset 40."
    );
    assert!(
        std::mem::size_of::<OmpComponent>() == 56,
        "OmpComponent: invalid size for the Itanium ABI. Use --target i686-unknown-linux-gnu to compile with native Open Multiplayer support."
    );
};

/// Compile-time check of the `OmpComponent` layout on i686 Windows MSVC.
///
/// The Open Multiplayer server expects the IUIDProvider vptr at offset 56 (confirmed by disasm of
/// `omp-server.exe`: `add ecx, 0x38` when calling `getUID()` via `IComponent*`).
/// Total IExtensible = vptr(4) + miscExtensions+padding(52) = 56 bytes.
#[cfg(all(target_arch = "x86", target_env = "msvc"))]
const _: () = {
    assert!(
        std::mem::offset_of!(OmpComponent, uid_vtable) == 56,
        "OmpComponent MSVC: uid_vtable must be at offset 56 (IUIDProvider after IExtensible=56 bytes)."
    );
};

impl OmpComponent {
    /// Creates a new `OmpComponent` with a layout compatible with the platform ABI.
    #[must_use]
    pub fn new(
        vtable: *const IComponentVTable,
        uid_vtable: *const IUIDProviderVTable,
        uid: UID,
    ) -> Self {
        Self {
            vtable,
            _misc_ext: [0u8; MISC_EXT_SIZE],
            uid_vtable,
            #[cfg(target_env = "msvc")]
            _uid_pad: 0,
            uid,
            plugin_ptr: std::ptr::null_mut(),
        }
    }
}

// ---------------------------------------------------------------------------
// Default implementations of primary vtable functions — Itanium ABI
// ---------------------------------------------------------------------------

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn ext_get_extension(_this: *mut OmpComponent, _uid: UID) -> *mut () {
    std::ptr::null_mut()
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn ext_add_extension(
    _this: *mut OmpComponent,
    _ext: *mut (),
    _auto_delete: bool,
) -> bool {
    false
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn ext_remove_extension_ptr(_this: *mut OmpComponent, _ext: *mut ()) -> bool {
    false
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn ext_remove_extension_uid(_this: *mut OmpComponent, _uid: UID) -> bool {
    false
}

/// D1 complete object destructor — no-op: cleanup is done via `free()`.
///
/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn ext_destructor(_this: *mut OmpComponent) {}

/// D0 deleting destructor — no-op: the server must not call `delete` on the component.
///
/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn ext_destructor_deleting(_this: *mut OmpComponent) {}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
#[must_use]
pub unsafe extern "C" fn comp_supported_version(_this: *const OmpComponent) -> i32 {
    1
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
#[must_use]
pub unsafe extern "C" fn comp_component_type(_this: *const OmpComponent) -> ComponentType {
    ComponentType::Other
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn comp_on_init(_this: *mut OmpComponent, _components: *mut IComponentList) {}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn comp_on_ready(_this: *mut OmpComponent) {}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn comp_on_free(_this: *mut OmpComponent, _component: *mut OmpComponent) {}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn comp_provide_configuration(
    _this: *mut OmpComponent,
    _logger: *mut ILogger,
    _config: *mut IEarlyConfig,
    _defaults: bool,
) {
}

// ---------------------------------------------------------------------------
// Default implementations of primary vtable functions — MSVC ABI
// ---------------------------------------------------------------------------

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn ext_get_extension(_this: *mut OmpComponent, _uid: UID) -> *mut () {
    std::ptr::null_mut()
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn ext_add_extension(
    _this: *mut OmpComponent,
    _ext: *mut (),
    _auto_delete: bool,
) -> bool {
    false
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn ext_remove_extension_ptr(
    _this: *mut OmpComponent,
    _ext: *mut (),
) -> bool {
    false
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn ext_remove_extension_uid(
    _this: *mut OmpComponent,
    _uid: UID,
) -> bool {
    false
}

/// Scalar deleting destructor — no-op: cleanup is done via `free()`.
/// No explicit parameter: this in ECX, no args on the stack (avoids `ret 4`).
///
/// # Safety
/// Called by the Open Multiplayer server via vtable.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn ext_destructor() {}

/// # Safety
/// Called by the Open Multiplayer server via vtable; this in ECX (ignored), no args on the stack.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn comp_supported_version() -> i32 {
    1
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; this in ECX (ignored), no args on the stack.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn comp_component_type() -> i32 {
    0
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn comp_on_init(
    _this: *mut OmpComponent,
    _components: *mut IComponentList,
) {
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; this in ECX (ignored), no args on the stack.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn comp_on_ready() {}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn comp_on_free(
    _this: *mut OmpComponent,
    _component: *mut OmpComponent,
) {
}

/// # Safety
/// Called by the Open Multiplayer server via vtable; `_this` must be a valid pointer to `OmpComponent`.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn comp_provide_configuration(
    _this: *mut OmpComponent,
    _logger: *mut ILogger,
    _config: *mut IEarlyConfig,
    _defaults: bool,
) {
}

// ---------------------------------------------------------------------------
// Default implementations of the secondary vtable (IUIDProvider) — Itanium ABI
// ---------------------------------------------------------------------------

/// D1/D0 thunk no-op for the secondary `IUIDProvider` vtable (Itanium ABI).
///
/// # Safety
/// `_this` points to the `IUIDProvider` subobject (offset 48 of `OmpComponent` on MSVC).
#[cfg(not(target_env = "msvc"))]
pub unsafe extern "C" fn uid_destructor_noop(_this: *mut u8) {}

/// `getUID()` via the secondary `IUIDProvider` vtable (Itanium ABI).
///
/// `this` points to the `IUIDProvider` subobject (offset 44). We subtract
/// `offsetof(OmpComponent, uid_vtable)` to recover the pointer to the object.
///
/// # Safety
/// `this` must be a valid pointer to the `IUIDProvider` subobject of an `OmpComponent`.
#[cfg(not(target_env = "msvc"))]
#[must_use]
pub unsafe extern "C" fn uid_get_uid(this: *const u8) -> UID {
    let offset = std::mem::offset_of!(OmpComponent, uid_vtable);
    // FFI: `OmpComponent` is allocated via `Box::new` (alignment >= 8 bytes on
    // i686); subtracting `offsetof(uid_vtable)` recovers the start of the object.
    #[allow(clippy::cast_ptr_alignment)]
    let comp_ptr = this.wrapping_sub(offset).cast::<OmpComponent>();
    unsafe { (*comp_ptr).uid }
}

// ---------------------------------------------------------------------------
// Implementation of the secondary vtable (IUIDProvider) — MSVC ABI
// ---------------------------------------------------------------------------

/// `getUID()` via the secondary IUIDProvider vtable (MSVC ABI).
///
/// `this` points to the IUIDProvider subobject at offset 56 of `OmpComponent`.
/// We subtract `offsetof(OmpComponent, uid_vtable)` to recover the pointer to the object.
///
/// # Safety
/// `this` must be a valid pointer to the `IUIDProvider` subobject of an `OmpComponent`.
#[cfg(target_env = "msvc")]
pub unsafe extern "thiscall" fn uid_get_uid(this: *const u8) -> UID {
    let offset = std::mem::offset_of!(OmpComponent, uid_vtable);
    let comp_ptr = this.wrapping_sub(offset).cast::<OmpComponent>();
    unsafe { (*comp_ptr).uid }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(target_env = "msvc"))]
    use crate::omp::types::SemanticVersion;

    // Helper functions to assemble vtables in tests.
    // The calling convention varies per ABI: "C" on Itanium (Linux), "thiscall" on MSVC.
    // On MSVC, methods with no stack args are declared `fn()` (this lives in ECX);
    // declaring an explicit `_this` would make Rust emit `ret 4` and corrupt the stack.
    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_name(_: *const OmpComponent) -> StringView {
        StringView::from_static("test\0")
    }
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_name() {}

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_version(_: *const OmpComponent) -> SemanticVersion {
        SemanticVersion::new(1, 0, 0)
    }
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_version() {}

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_on_load(_: *mut OmpComponent, _: *mut ICore) {}
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_on_load(_: *mut OmpComponent, _: *mut ICore) {}

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_on_init(_: *mut OmpComponent, _: *mut IComponentList) {}
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_on_init(_: *mut OmpComponent, _: *mut IComponentList) {}

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_on_ready(_: *mut OmpComponent) {}
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_on_ready() {}

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_on_free(_: *mut OmpComponent, _: *mut OmpComponent) {}
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_on_free(_: *mut OmpComponent, _: *mut OmpComponent) {}

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_provide_cfg(
        _: *mut OmpComponent,
        _: *mut ILogger,
        _: *mut IEarlyConfig,
        _: bool,
    ) {
    }
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_provide_cfg(
        _: *mut OmpComponent,
        _: *mut ILogger,
        _: *mut IEarlyConfig,
        _: bool,
    ) {
    }

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_free(_: *mut OmpComponent) {}
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_free() {}

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn test_reset(_: *mut OmpComponent) {}
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn test_reset() {}

    #[cfg(not(target_env = "msvc"))]
    fn make_vtable() -> IComponentVTable {
        IComponentVTable {
            get_extension: ext_get_extension,
            add_extension: ext_add_extension,
            remove_extension_ptr: ext_remove_extension_ptr,
            remove_extension_uid: ext_remove_extension_uid,
            destructor: ext_destructor,
            destructor_deleting: ext_destructor_deleting,
            supported_version: comp_supported_version,
            component_name: test_name,
            component_type: comp_component_type,
            component_version: test_version,
            on_load: test_on_load,
            on_init: test_on_init,
            on_ready: test_on_ready,
            on_free: test_on_free,
            provide_configuration: test_provide_cfg,
            free: test_free,
            reset: test_reset,
        }
    }

    #[cfg(target_env = "msvc")]
    fn make_vtable() -> IComponentVTable {
        IComponentVTable {
            get_extension: ext_get_extension,
            add_extension: ext_add_extension,
            remove_extension_ptr: ext_remove_extension_ptr,
            remove_extension_uid: ext_remove_extension_uid,
            destructor: ext_destructor,
            supported_version: comp_supported_version,
            component_name: test_name,
            component_type: comp_component_type,
            component_version: test_version,
            on_load: test_on_load,
            on_init: test_on_init,
            on_ready: test_on_ready,
            on_free: test_on_free,
            provide_configuration: test_provide_cfg,
            free: test_free,
            reset: test_reset,
        }
    }

    #[cfg(not(target_env = "msvc"))]
    fn make_uid_vtable() -> IUIDProviderVTable {
        IUIDProviderVTable {
            destructor_complete: uid_destructor_noop,
            destructor_deleting: uid_destructor_noop,
            get_uid: uid_get_uid,
        }
    }

    #[cfg(target_env = "msvc")]
    fn make_uid_vtable() -> IUIDProviderVTable {
        IUIDProviderVTable {
            get_uid: uid_get_uid,
        }
    }

    // --- Layout ---

    #[test]
    #[cfg(all(target_arch = "x86", target_os = "linux"))]
    fn omp_component_layout_i686_linux() {
        // GCC i686: uint64_t aligned to 4 bytes -> no _pad, uid_vtable at offset 40
        assert_eq!(std::mem::offset_of!(OmpComponent, uid_vtable), 40);
        assert_eq!(std::mem::size_of::<OmpComponent>(), 56);
    }

    #[test]
    #[cfg(all(target_arch = "x86", target_env = "msvc"))]
    fn omp_component_layout_i686_msvc() {
        // Open Multiplayer server expects the IUIDProvider vptr at offset 56 (confirmed by
        // disasm: `add ecx, 0x38` when calling getUID via IComponent*).
        assert_eq!(std::mem::offset_of!(OmpComponent, uid_vtable), 56);
    }

    // --- OmpComponent::new ---

    #[test]
    fn omp_component_new_stores_uid() {
        let vt = make_vtable();
        let uvt = make_uid_vtable();
        let comp = OmpComponent::new(&raw const vt, &raw const uvt, 0xDEAD_BEEF_CAFE_BABE);
        assert_eq!(comp.uid, 0xDEAD_BEEF_CAFE_BABE);
    }

    #[test]
    fn omp_component_plugin_ptr_null_on_new() {
        let vt = make_vtable();
        let uvt = make_uid_vtable();
        let comp = OmpComponent::new(&raw const vt, &raw const uvt, 0);
        assert!(comp.plugin_ptr.is_null());
    }

    // --- uid_get_uid ---

    #[test]
    fn uid_get_uid_recovers_from_subobject_pointer() {
        let vt = make_vtable();
        let uvt = make_uid_vtable();
        let comp = OmpComponent::new(&raw const vt, &raw const uvt, 0xCAFE_BABE_u64);
        let uid_ptr = (&raw const comp.uid_vtable).cast::<u8>();
        let recovered = unsafe { uid_get_uid(uid_ptr) };
        assert_eq!(recovered, 0xCAFE_BABE_u64);
    }

    // --- Default vtable functions ---

    #[test]
    fn ext_get_extension_returns_null() {
        let vt = make_vtable();
        let uvt = make_uid_vtable();
        let mut comp = OmpComponent::new(&raw const vt, &raw const uvt, 0);
        let result = unsafe { ext_get_extension(&raw mut comp, 0) };
        assert!(result.is_null());
    }

    #[test]
    fn ext_add_extension_returns_false() {
        let vt = make_vtable();
        let uvt = make_uid_vtable();
        let mut comp = OmpComponent::new(&raw const vt, &raw const uvt, 0);
        let result = unsafe { ext_add_extension(&raw mut comp, std::ptr::null_mut(), false) };
        assert!(!result);
    }

    #[test]
    #[cfg(not(target_env = "msvc"))]
    fn comp_supported_version_is_one() {
        let vt = make_vtable();
        let uvt = make_uid_vtable();
        let comp = OmpComponent::new(&raw const vt, &raw const uvt, 0);
        assert_eq!(unsafe { comp_supported_version(&raw const comp) }, 1);
    }

    /// On MSVC `comp_supported_version` takes `this` in `ECX` with no
    /// stack args (`fn()`); the Rust call site cannot pass `_this`.
    #[test]
    #[cfg(target_env = "msvc")]
    fn comp_supported_version_is_one() {
        assert_eq!(unsafe { comp_supported_version() }, 1);
    }

    #[test]
    #[cfg(not(target_env = "msvc"))]
    fn comp_component_type_is_other() {
        let vt = make_vtable();
        let uvt = make_uid_vtable();
        let comp = OmpComponent::new(&raw const vt, &raw const uvt, 0);
        assert_eq!(
            unsafe { comp_component_type(&raw const comp) },
            ComponentType::Other
        );
    }

    /// On MSVC `comp_component_type` returns `i32` (the discriminant of
    /// `ComponentType::Other`) and takes no stack args.
    #[test]
    #[cfg(target_env = "msvc")]
    fn comp_component_type_is_other() {
        assert_eq!(
            unsafe { comp_component_type() },
            ComponentType::Other as i32
        );
    }
}
