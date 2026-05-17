//! Bindings for the Open Multiplayer `ITimersComponent` interface.
//!
//! Used to emulate the periodic server tick in native Open Multiplayer mode (Open Multiplayer does
//! not call `process_tick` automatically like SA-MP). The SDK creates a timer
//! with a 5ms interval in `on_omp_ready` if the plugin opted in via
//! `samp::plugin::enable_server_tick`; on each timeout, it dispatches
//! `SampPlugin::on_server_tick`. The plugin writes the callback only once —
//! it works identically on both servers.
//!
//! ## Primary `ITimersComponent` vtable (19 slots — confirmed via disasm of `Timers.dll`)
//!
//! Slots `[0..15]` are inherited from `IComponent`. New slots:
//! - **[16]** `create(handler*, Milliseconds interval, bool repeating)` -> `ITimer*`
//! - **[17]** `create(handler*, Milliseconds initial, Milliseconds interval, unsigned count)` -> `ITimer*`
//! - **[18]** `count() const` -> `size_t`
//!
//! ## `ITimer` vtable (slots starting from `IExtensible`)
//!
//! - **[0..3]** `IExtensible` (`getExtension`, `addExtension`, `removeExtension`x2)
//! - **[4]** destructor (1 slot MSVC / 2 slots Itanium)
//! - **[5]** `running()` const
//! - **[6]** `remaining()` const -> Milliseconds (8 bytes, hidden ptr)
//! - **[7]** `calls()` const
//! - **[8]** `interval()` const -> Milliseconds (8 bytes, hidden ptr)
//! - **[9]** `trigger()`
//! - **[10]** `kill()`
//! - **[11]** `handler() const`
//!
//! ## `TimerTimeOutHandler` vtable (interface provided by the plugin)
//!
//! No virtual destructor in the header -> 2 slots only:
//! - **[0]** `timeout(ITimer&)`
//! - **[1]** `free(ITimer&)`

use super::component_api::OmpComponentHandle;
use super::server::{ServerComponent, query_component};
use super::types::UID;
use std::ptr::NonNull;

/// UID of the Open Multiplayer `Timers` component.
pub const TIMERS_COMPONENT_UID: UID = 0x2ad8_124c_5ea2_57a3;

/// Slot of `create(handler, interval, repeating)` in the `ITimersComponent` vtable.
const SLOT_CREATE_INTERVAL: usize = 16;

/// Slot of `kill()` in the `ITimer` vtable.
const SLOT_TIMER_KILL: usize = 10;

/// Opaque pointer to the server's `ITimersComponent`.
#[repr(C)]
pub struct ITimersComponent {
    _opaque: [u8; 0],
}

/// Opaque pointer to the server's `ITimer` — returned by `create_timer`.
#[repr(C)]
pub struct ITimer {
    _opaque: [u8; 0],
}

/// `TimerTimeOutHandler` vtable — Itanium ABI.
#[cfg(not(target_env = "msvc"))]
#[repr(C)]
pub struct TimerHandlerVTable {
    pub timeout: unsafe extern "C" fn(*mut TimerTimeOutHandler, *mut ITimer),
    pub free: unsafe extern "C" fn(*mut TimerTimeOutHandler, *mut ITimer),
}

/// `TimerTimeOutHandler` vtable — MSVC ABI (`this` in ECX).
#[cfg(target_env = "msvc")]
#[repr(C)]
pub struct TimerHandlerVTable {
    pub timeout: unsafe extern "thiscall" fn(*mut TimerTimeOutHandler, *mut ITimer),
    pub free: unsafe extern "thiscall" fn(*mut TimerTimeOutHandler, *mut ITimer),
}

/// Object the server will invoke on each timer timeout.
///
/// `#[repr(C)]` layout: vtable pointer at offset 0 + the handler's own data.
/// The server treats it as an opaque `TimerTimeOutHandler*` and only interacts
/// via the vtable.
#[repr(C)]
pub struct TimerTimeOutHandler {
    pub vtable: *const TimerHandlerVTable,
}

unsafe impl Send for TimerTimeOutHandler {}
unsafe impl Sync for TimerTimeOutHandler {}

/// Signature of `ITimersComponent::create(handler, interval, repeating)`.
///
/// `Milliseconds` is `std::chrono::milliseconds` in C++, a wrapper over `int64_t`.
/// At the ABI it is passed as 8 bytes on the stack (or hidden in registers,
/// depending on the compiler).
#[cfg(not(target_env = "msvc"))]
type CreateFn = unsafe extern "C" fn(
    this: *mut ITimersComponent,
    handler: *mut TimerTimeOutHandler,
    interval_ms: i64,
    repeating: bool,
) -> *mut ITimer;

#[cfg(target_env = "msvc")]
type CreateFn = unsafe extern "thiscall" fn(
    this: *mut ITimersComponent,
    handler: *mut TimerTimeOutHandler,
    interval_ms: i64,
    repeating: bool,
) -> *mut ITimer;

#[cfg(not(target_env = "msvc"))]
type KillFn = unsafe extern "C" fn(this: *mut ITimer);

#[cfg(target_env = "msvc")]
type KillFn = unsafe extern "thiscall" fn(this: *mut ITimer);

/// Queries `ITimersComponent` in the server's component list.
///
/// # Safety
/// `core` must point to a valid `ICore`. Internally uses `query_component`,
/// which casts the `ServerComponent` from the list — follows its contract.
pub unsafe fn query_timers_component(
    components: *mut super::server::ServerComponentList,
) -> *mut ITimersComponent {
    if components.is_null() {
        return std::ptr::null_mut();
    }
    let raw = unsafe { query_component(components, TIMERS_COMPONENT_UID) };
    raw.cast::<ITimersComponent>()
}

/// Creates a repeating timer on the Open Multiplayer server.
///
/// Returns the server's `ITimer*` (non-owning — the server owns it). Use
/// [`kill_timer`] at shutdown to stop it and free the server's resources.
///
/// # Safety
/// - `timers` must be a valid `ITimersComponent` pointer (from `query_timers_component`)
/// - `handler` must remain alive while the timer is active (allocate on the heap via `Box::into_raw`)
pub unsafe fn create_repeating_timer(
    timers: *mut ITimersComponent,
    handler: *mut TimerTimeOutHandler,
    interval_ms: i64,
) -> *mut ITimer {
    if handler.is_null() {
        return std::ptr::null_mut();
    }
    let Some((_, slot)) = (unsafe {
        super::vtable::secondary_call_target(timers.cast::<u8>(), 0, SLOT_CREATE_INTERVAL)
    }) else {
        return std::ptr::null_mut();
    };
    let create: CreateFn = unsafe { std::mem::transmute(slot) };
    unsafe { create(timers, handler, interval_ms, true) }
}

// ---------------------------------------------------------------------------
// TimersComponent — high-level typed wrapper
// ---------------------------------------------------------------------------

/// Typed wrapper for the Open Multiplayer server's `ITimersComponent`.
///
/// Obtained via `samp::plugin::omp_query::<TimersComponent>()`. Exposes
/// `create_repeating` for timer creation and the generic `IComponent` methods
/// (`name`, `version`).
#[derive(Debug, Clone, Copy)]
pub struct TimersComponent {
    ptr: NonNull<ServerComponent>,
}

impl OmpComponentHandle for TimersComponent {
    const UID: UID = TIMERS_COMPONENT_UID;

    unsafe fn from_raw(ptr: NonNull<ServerComponent>) -> Self {
        Self { ptr }
    }

    fn as_raw(&self) -> NonNull<ServerComponent> {
        self.ptr
    }
}

impl TimersComponent {
    /// Returns the component name.
    #[must_use]
    pub fn name(&self) -> Option<String> {
        super::component_api::component_name(self)
    }

    /// Returns the component version.
    #[must_use]
    pub fn version(&self) -> Option<super::types::SemanticVersion> {
        super::component_api::component_version(self)
    }

    /// Creates a repeating timer on the server.
    ///
    /// `handler` must be heap-allocated (e.g. `Box::into_raw`) and must be
    /// dropped inside the `TimerHandlerVTable::free` callback.
    ///
    /// # Safety
    /// `handler` must point to a live [`TimerTimeOutHandler`] while the timer
    /// is active.
    pub unsafe fn create_repeating(
        &self,
        handler: *mut TimerTimeOutHandler,
        interval_ms: i64,
    ) -> *mut ITimer {
        unsafe {
            create_repeating_timer(
                self.ptr.as_ptr().cast::<ITimersComponent>(),
                handler,
                interval_ms,
            )
        }
    }
}

/// Kills an active timer, stopping future fires.
///
/// After `kill`, the server calls `TimerTimeOutHandler::free(timer)` allowing
/// the heap-allocated handler to be released. Without it, the handler leaks.
///
/// # Safety
/// `timer` must be a valid pointer returned by `create_repeating_timer`.
pub unsafe fn kill_timer(timer: *mut ITimer) {
    let Some((_, slot)) =
        (unsafe { super::vtable::secondary_call_target(timer.cast::<u8>(), 0, SLOT_TIMER_KILL) })
    else {
        return;
    };
    let kill: KillFn = unsafe { std::mem::transmute(slot) };
    unsafe { kill(timer) };
}

#[cfg(test)]
mod tests {
    //! Tests for the `timers` module.
    //!
    //! Cover: UID constant, `TimerTimeOutHandler` layout, defensive behavior
    //! of `create_repeating_timer` and `kill_timer` against null or invalid
    //! inputs.

    use super::*;

    #[test]
    fn timers_component_uid_is_known_value() {
        // Value declared in `timers.hpp:44` of the Open Multiplayer SDK.
        assert_eq!(TIMERS_COMPONENT_UID, 0x2ad8_124c_5ea2_57a3);
    }

    #[test]
    fn timers_component_uid_via_trait() {
        assert_eq!(
            <TimersComponent as OmpComponentHandle>::UID,
            TIMERS_COMPONENT_UID
        );
    }

    #[test]
    fn timer_handler_has_vtable_at_offset_zero() {
        // The server reads the vtable at offset 0 of the handler — confirm layout.
        assert_eq!(std::mem::offset_of!(TimerTimeOutHandler, vtable), 0);
    }

    #[test]
    fn timer_handler_size_is_one_pointer() {
        // No own data: only the vtable pointer.
        assert_eq!(
            std::mem::size_of::<TimerTimeOutHandler>(),
            std::mem::size_of::<*const ()>()
        );
    }

    #[test]
    fn timer_handler_vtable_has_two_slots() {
        // IUIDProvider does not declare a destructor -> 2 slots (timeout, free).
        assert_eq!(
            std::mem::size_of::<TimerHandlerVTable>(),
            2 * std::mem::size_of::<*const ()>()
        );
    }

    #[test]
    fn create_repeating_timer_returns_null_when_handler_is_null() {
        let timers = std::ptr::null_mut::<ITimersComponent>();
        let ret = unsafe { create_repeating_timer(timers, std::ptr::null_mut(), 5) };
        assert!(ret.is_null());
    }

    #[test]
    fn create_repeating_timer_returns_null_when_component_is_null() {
        // Dummy handler: since timers is null, it should not even try to deref the handler.
        let fake_handler = std::ptr::dangling_mut::<TimerTimeOutHandler>();
        let ret = unsafe { create_repeating_timer(std::ptr::null_mut(), fake_handler, 5) };
        assert!(ret.is_null());
    }

    #[test]
    fn kill_timer_is_noop_for_null_pointer() {
        // Must not panic or segfault.
        unsafe { kill_timer(std::ptr::null_mut()) };
    }

    #[test]
    fn query_timers_component_returns_null_for_null_list() {
        let ret = unsafe { query_timers_component(std::ptr::null_mut()) };
        assert!(ret.is_null());
    }
}
