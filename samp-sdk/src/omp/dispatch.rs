//! `IEventDispatcher<H>`, for any handler type `H`, and the macro that writes
//! a handler.
//!
//! Every component hands out its events through the same template: a
//! dispatcher whose first slots add, remove and look up a handler. It declares
//! no destructor and no overload, so the slots are the declaration order on
//! both ABIs, and one generic wrapper serves every event group. The accessors
//! that return a dispatcher are generated (`objects_event_dispatcher`,
//! `npcs_event_dispatcher`, ...), and so are the handlers of the groups the SDK
//! does not write by hand.

use std::marker::PhantomData;

use super::vtable::call_vtable;

/// `IEventDispatcher<H>*` — held by pointer only.
#[repr(C)]
pub struct EventDispatcher<H> {
    _opaque: [u8; 0],
    _handler: PhantomData<*mut H>,
}

/// Where a handler runs relative to the others: `EventPriority` in `events.hpp`.
pub mod priority {
    /// Before every default-priority handler.
    pub const HIGHEST: i8 = -128;
    /// Before the default.
    pub const FAIRLY_HIGH: i8 = -64;
    /// What the header's default argument gives.
    pub const DEFAULT: i8 = 0;
    /// After the default.
    pub const FAIRLY_LOW: i8 = 63;
    /// After every default-priority handler.
    pub const LOWEST: i8 = 127;
}

const SLOT_ADD: usize = 0;
const SLOT_REMOVE: usize = 1;
const SLOT_COUNT: usize = 3;

/// `addEventHandler(handler, priority)`. `false` means the handler was
/// already registered, or the dispatcher is null.
///
/// # Safety
/// `dispatcher` must come from an accessor for the same `H`, and `handler`
/// must stay alive until it is removed — the server keeps the pointer.
pub unsafe fn add_event_handler<H>(
    dispatcher: *mut EventDispatcher<H>,
    handler: *mut H,
    priority: i8,
) -> bool {
    call_vtable!(
        dispatcher.cast::<u8>(),
        0,
        SLOT_ADD,
        // Any `H*` is one pointer; the type lives in the signature above.
        (*mut u8, i8) -> bool,
        (handler.cast::<u8>(), priority),
        false
    )
}

/// `removeEventHandler(handler)`. `false` means it was not registered.
///
/// # Safety
/// As for [`add_event_handler`].
pub unsafe fn remove_event_handler<H>(
    dispatcher: *mut EventDispatcher<H>,
    handler: *mut H,
) -> bool {
    call_vtable!(
        dispatcher.cast::<u8>(),
        0,
        SLOT_REMOVE,
        (*mut u8) -> bool,
        (handler.cast::<u8>()),
        false
    )
}

/// `count()` — how many handlers are registered.
///
/// # Safety
/// `dispatcher` must come from an accessor, or be null.
#[must_use]
pub unsafe fn event_handler_count<H>(dispatcher: *mut EventDispatcher<H>) -> usize {
    call_vtable!(dispatcher.cast::<u8>(), 0, SLOT_COUNT, () -> usize, (), 0)
}

/// Writes a handler: its vtable for both ABIs, the object the server calls
/// through, and `DEFAULT`, a vtable whose every entry does what the C++
/// handler's own body does — nothing, or return its literal. Override the
/// entries you need with struct update syntax:
///
/// ```ignore
/// static VTABLE: ObjectHandlerVTable = ObjectHandlerVTable {
///     on_moved: my_on_moved,
///     ..ObjectHandlerVTable::DEFAULT
/// };
/// ```
macro_rules! event_handler {
    (
        $(#[$meta:meta])*
        $name:ident for $handler:ident {
            $(
                $(#[$fmeta:meta])*
                $field:ident: fn($($arg:ty),* $(,)?) $(-> $ret:ty)? = $default:expr
            ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[cfg(not(target_env = "msvc"))]
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: unsafe extern "C" fn(*mut $handler, $($arg),*) $(-> $ret)?),*
        }

        $(#[$meta])*
        #[cfg(target_env = "msvc")]
        #[repr(C)]
        #[derive(Clone, Copy)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: unsafe extern "thiscall" fn(*mut $handler, $($arg),*) $(-> $ret)?),*
        }

        impl $name {
            /// Every entry doing what the C++ handler's own body does.
            pub const DEFAULT: Self = Self {
                $($field: {
                    #[cfg(not(target_env = "msvc"))]
                    unsafe extern "C" fn f(_: *mut $handler, $(_: $arg),*) $(-> $ret)? { $default }
                    #[cfg(target_env = "msvc")]
                    unsafe extern "thiscall" fn f(_: *mut $handler, $(_: $arg),*) $(-> $ret)? { $default }
                    f
                }),*
            };
        }

        /// Object the server calls through [`
        #[doc = stringify!($name)]
        /// `]: the vtable pointer at offset 0, like any C++ object with virtuals.
        /// The server keeps the pointer, so it must outlive the registration.
        #[repr(C)]
        pub struct $handler {
            vtable: *const $name,
        }

        // SAFETY: handlers are only ever touched on the server's main thread.
        unsafe impl Send for $handler {}
        unsafe impl Sync for $handler {}

        impl $handler {
            /// Builds a handler backed by `vtable`.
            #[must_use]
            pub const fn new(vtable: *const $name) -> Self {
                Self { vtable }
            }
        }
    };
}

pub(crate) use event_handler;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omp::vtable::MockTable;

    struct Probe;

    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn add(_: *mut u8, handler: *mut Probe, priority: i8) -> u32 {
        // A C++ `bool` comes back in a whole register.
        u32::from(!handler.is_null() && priority == priority::FAIRLY_LOW)
    }
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn add(_: *mut u8, handler: *mut Probe, priority: i8) -> u32 {
        // A C++ `bool` comes back in a whole register.
        u32::from(!handler.is_null() && priority == priority::FAIRLY_LOW)
    }
    #[cfg(not(target_env = "msvc"))]
    unsafe extern "C" fn count(_: *mut u8) -> usize {
        7
    }
    #[cfg(target_env = "msvc")]
    unsafe extern "thiscall" fn count(_: *mut u8) -> usize {
        7
    }

    static TABLE: MockTable<4> = MockTable([
        add as *const (),
        std::ptr::null(),
        std::ptr::null(),
        count as *const (),
    ]);

    #[test]
    fn calls_reach_their_slots() {
        let mut object = [&raw const TABLE.0 as *const ()];
        let dispatcher = (&raw mut object).cast::<EventDispatcher<Probe>>();
        let mut probe = Probe;
        unsafe {
            assert!(add_event_handler(
                dispatcher,
                &raw mut probe,
                priority::FAIRLY_LOW
            ));
            assert_eq!(event_handler_count(dispatcher), 7);
            // A null slot is no call at all.
            assert!(!remove_event_handler(dispatcher, &raw mut probe));
        }
    }

    #[test]
    fn a_null_dispatcher_answers_the_defaults() {
        let dispatcher = std::ptr::null_mut::<EventDispatcher<Probe>>();
        unsafe {
            assert!(!add_event_handler(dispatcher, std::ptr::null_mut(), 0));
            assert_eq!(event_handler_count(dispatcher), 0);
        }
    }
}
