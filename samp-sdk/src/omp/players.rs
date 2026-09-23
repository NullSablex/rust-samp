//! Player events straight from the Open Multiplayer server.
//!
//! A plugin that wants `OnPlayerConnect` today goes through Pawn: the SDK
//! detours `amx_Exec` and watches the gamemode's callbacks (see
//! [`crate::omp::events`]). That works on both servers, but it only sees what
//! the script is told, and it costs a detour.
//!
//! Open Multiplayer offers the same events natively: `ICore` hands out an
//! `IPlayerPool`, the pool hands out an `IEventDispatcher<PlayerConnectEventHandler>`,
//! and a component registers a handler on it. No Pawn, no detour — the server
//! calls the plugin directly.
//!
//! ## Slots, per ABI
//!
//! | Method | Itanium | MSVC |
//! | ------ | :-----: | :--: |
//! | `ICore::getPlayers` | 8 | 7 |
//! | `IPlayerPool::getPlayerConnectDispatcher` | 10 | 9 |
//!
//! Itanium values come from the vtable dumps of the official `omp-server`
//! (`PlayerPool` and `Core` keep their symbols). MSVC shifts by one because it
//! emits a single destructor slot where Itanium emits two — the same rule as
//! everywhere else in this module, and `scripts/check-abi-slots.py` re-derives
//! it from the binaries.
//!
//! ## `PlayerConnectEventHandler`
//!
//! Declared in `player.hpp` with no virtual destructor, so four slots on both
//! ABIs:
//!
//! ```text
//! [0] onIncomingConnection(IPlayer&, StringView ip, u16 port)
//! [1] onPlayerConnect(IPlayer&)
//! [2] onPlayerDisconnect(IPlayer&, PeerDisconnectReason)
//! [3] onPlayerClientInit(IPlayer&)
//! ```

use super::component::ICore;
use super::types::StringView;

/// Slot of `ICore::getPlayers()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_GET_PLAYERS: usize = 8;
#[cfg(target_env = "msvc")]
const SLOT_GET_PLAYERS: usize = 7;

/// Slot of `IPlayerPool::getPlayerConnectDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_CONNECT_DISPATCHER: usize = 10;
#[cfg(target_env = "msvc")]
const SLOT_CONNECT_DISPATCHER: usize = 9;

/// Opaque handle for the server's `IPlayerPool*`.
#[repr(C)]
pub struct IPlayerPool {
    _opaque: [u8; 0],
}

/// Opaque handle for the server's `IPlayer*`, as received by a handler.
#[repr(C)]
pub struct IPlayer {
    _opaque: [u8; 0],
}

/// Slot of `IPlayerPool::getPlayerSpawnDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_SPAWN_DISPATCHER: usize = 9;
#[cfg(target_env = "msvc")]
const SLOT_SPAWN_DISPATCHER: usize = 8;

/// Slot of `IPlayerPool::getPlayerTextDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_TEXT_DISPATCHER: usize = 12;
#[cfg(target_env = "msvc")]
const SLOT_TEXT_DISPATCHER: usize = 11;

/// Slot of `IPlayerPool::getPlayerDamageDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_DAMAGE_DISPATCHER: usize = 15;
#[cfg(target_env = "msvc")]
const SLOT_DAMAGE_DISPATCHER: usize = 14;

/// Opaque handle for `IEventDispatcher<PlayerConnectEventHandler>*`.
#[repr(C)]
pub struct IPlayerConnectDispatcher {
    _opaque: [u8; 0],
}

/// Why the server dropped a player (`PeerDisconnectReason` in `network.hpp`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    Timeout = 0,
    Quit = 1,
    Kicked = 2,
    Custom = 3,
    ModeEnd = 4,
}

impl DisconnectReason {
    /// Maps the raw value the server passes, treating an unknown one as
    /// [`DisconnectReason::Custom`] rather than transmuting it into a variant
    /// that does not exist.
    #[must_use]
    pub fn from_raw(value: i32) -> Self {
        match value {
            0 => Self::Timeout,
            1 => Self::Quit,
            2 => Self::Kicked,
            4 => Self::ModeEnd,
            _ => Self::Custom,
        }
    }
}

/// `PlayerConnectEventHandler` vtable — Itanium ABI.
#[cfg(not(target_env = "msvc"))]
#[repr(C)]
pub struct PlayerConnectHandlerVTable {
    pub on_incoming_connection:
        unsafe extern "C" fn(*mut PlayerConnectHandler, *mut IPlayer, StringView, u16),
    pub on_player_connect: unsafe extern "C" fn(*mut PlayerConnectHandler, *mut IPlayer),
    pub on_player_disconnect: unsafe extern "C" fn(*mut PlayerConnectHandler, *mut IPlayer, i32),
    pub on_player_client_init: unsafe extern "C" fn(*mut PlayerConnectHandler, *mut IPlayer),
}

/// `PlayerConnectEventHandler` vtable — MSVC ABI (`this` in ECX).
#[cfg(target_env = "msvc")]
#[repr(C)]
pub struct PlayerConnectHandlerVTable {
    pub on_incoming_connection:
        unsafe extern "thiscall" fn(*mut PlayerConnectHandler, *mut IPlayer, StringView, u16),
    pub on_player_connect: unsafe extern "thiscall" fn(*mut PlayerConnectHandler, *mut IPlayer),
    pub on_player_disconnect:
        unsafe extern "thiscall" fn(*mut PlayerConnectHandler, *mut IPlayer, i32),
    pub on_player_client_init: unsafe extern "thiscall" fn(*mut PlayerConnectHandler, *mut IPlayer),
}

/// Object the server calls on player connection events.
///
/// Layout: vtable pointer at offset 0, like every C++ object with virtuals. The
/// server keeps the pointer, so it must outlive the registration — leak it, or
/// keep it alive for the plugin's lifetime and remove it before dropping.
#[repr(C)]
pub struct PlayerConnectHandler {
    vtable: *const PlayerConnectHandlerVTable,
}

// SAFETY: the handler is only ever touched on the server's main thread.
unsafe impl Send for PlayerConnectHandler {}
unsafe impl Sync for PlayerConnectHandler {}

impl PlayerConnectHandler {
    /// Builds a handler backed by `vtable`.
    #[must_use]
    pub fn new(vtable: *const PlayerConnectHandlerVTable) -> Self {
        Self { vtable }
    }
}

/// Opaque handle for `IEventDispatcher<PlayerSpawnEventHandler>*`.
#[repr(C)]
pub struct IPlayerSpawnDispatcher {
    _opaque: [u8; 0],
}

/// Opaque handle for `IEventDispatcher<PlayerTextEventHandler>*`.
#[repr(C)]
pub struct IPlayerTextDispatcher {
    _opaque: [u8; 0],
}

/// Opaque handle for `IEventDispatcher<PlayerDamageEventHandler>*`.
#[repr(C)]
pub struct IPlayerDamageDispatcher {
    _opaque: [u8; 0],
}

/// Writes a handler vtable struct for both ABIs — the server calls through it,
/// so the convention is the platform's: `extern "C"` on Itanium, `thiscall`
/// on MSVC. None of these handlers declares a virtual destructor, so the slot
/// numbering is the declaration order on both.
macro_rules! handler_vtable {
    (
        $(#[$meta:meta])*
        $name:ident for $handler:ident {
            $($field:ident: fn($($arg:ty),* $(,)?) $(-> $ret:ty)?),* $(,)?
        }
    ) => {
        $(#[$meta])*
        #[cfg(not(target_env = "msvc"))]
        #[repr(C)]
        pub struct $name {
            $(pub $field: unsafe extern "C" fn(*mut $handler, $($arg),*) $(-> $ret)?),*
        }

        $(#[$meta])*
        #[cfg(target_env = "msvc")]
        #[repr(C)]
        pub struct $name {
            $(pub $field: unsafe extern "thiscall" fn(*mut $handler, $($arg),*) $(-> $ret)?),*
        }

        /// Object the server calls through [`
        #[doc = stringify!($name)]
        /// `]. Layout: vtable pointer at offset 0, like any C++ object with
        /// virtuals. The server keeps the pointer, so it must outlive the
        /// registration.
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
            pub fn new(vtable: *const $name) -> Self {
                Self { vtable }
            }
        }
    };
}

handler_vtable! {
    /// `PlayerSpawnEventHandler` — `onPlayerRequestSpawn` returning `false`
    /// denies the spawn.
    PlayerSpawnHandlerVTable for PlayerSpawnHandler {
        on_player_request_spawn: fn(*mut IPlayer) -> bool,
        on_player_spawn: fn(*mut IPlayer),
    }
}

handler_vtable! {
    /// `PlayerTextEventHandler` — `onPlayerText` returning `false` blocks the
    /// message; `onPlayerCommandText` returning `true` marks the command as
    /// handled.
    PlayerTextHandlerVTable for PlayerTextHandler {
        on_player_text: fn(*mut IPlayer, StringView) -> bool,
        on_player_command_text: fn(*mut IPlayer, StringView) -> bool,
    }
}

handler_vtable! {
    /// `PlayerDamageEventHandler`. `killer` is null when nobody killed the
    /// player, and `part` is a `BodyPart` value.
    PlayerDamageHandlerVTable for PlayerDamageHandler {
        on_player_death: fn(*mut IPlayer, *mut IPlayer, i32),
        on_player_take_damage: fn(*mut IPlayer, *mut IPlayer, f32, u32, i32),
        on_player_give_damage: fn(*mut IPlayer, *mut IPlayer, f32, u32, i32),
    }
}

/// `ICore::getPlayers()` — the server's player pool.
///
/// # Safety
/// `core` must be the `ICore*` the server passed to `on_load`.
#[must_use]
pub unsafe fn player_pool(core: *mut ICore) -> *mut IPlayerPool {
    #[cfg(not(target_env = "msvc"))]
    type GetPlayersFn = unsafe extern "C" fn(*mut u8) -> *mut IPlayerPool;
    #[cfg(target_env = "msvc")]
    type GetPlayersFn = unsafe extern "thiscall" fn(*mut u8) -> *mut IPlayerPool;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(core.cast::<u8>(), 0, SLOT_GET_PLAYERS)
    }) else {
        return std::ptr::null_mut();
    };
    let get_players: GetPlayersFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get_players(this) }
}

/// `IPlayerPool::getPlayerConnectDispatcher()`.
///
/// # Safety
/// `pool` must come from [`player_pool`].
#[must_use]
pub unsafe fn player_connect_dispatcher(pool: *mut IPlayerPool) -> *mut IPlayerConnectDispatcher {
    #[cfg(not(target_env = "msvc"))]
    type GetDispatcherFn = unsafe extern "C" fn(*mut u8) -> *mut IPlayerConnectDispatcher;
    #[cfg(target_env = "msvc")]
    type GetDispatcherFn = unsafe extern "thiscall" fn(*mut u8) -> *mut IPlayerConnectDispatcher;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(pool.cast::<u8>(), 0, SLOT_CONNECT_DISPATCHER)
    }) else {
        return std::ptr::null_mut();
    };
    let get_dispatcher: GetDispatcherFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get_dispatcher(this) }
}

/// Registers `handler` on the dispatcher (`addEventHandler`, slot [0]).
///
/// Returns what the server returned: `false` means the handler was already
/// registered.
///
/// # Safety
/// Both pointers must be valid, and `handler` must outlive the registration.
pub unsafe fn add_player_connect_handler(
    dispatcher: *mut IPlayerConnectDispatcher,
    handler: *mut PlayerConnectHandler,
) -> bool {
    #[cfg(not(target_env = "msvc"))]
    type AddFn = unsafe extern "C" fn(*mut u8, *mut PlayerConnectHandler, i8) -> bool;
    #[cfg(target_env = "msvc")]
    type AddFn = unsafe extern "thiscall" fn(*mut u8, *mut PlayerConnectHandler, i8) -> bool;

    // `IEventDispatcher<T>` declares no destructor, so `addEventHandler` is
    // slot [0] on both ABIs — same layout the Pawn dispatcher uses.
    let Some((this, f_ptr)) =
        (unsafe { super::vtable::secondary_call_target_ptr(dispatcher.cast::<u8>(), 0, 0) })
    else {
        return false;
    };
    let add: AddFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { add(this, handler, 0) }
}

/// Writes the `get<X>Dispatcher` + `add_<x>_handler` pair for one event group.
macro_rules! dispatcher_pair {
    ($getter:ident -> $dispatcher:ident @ $slot:ident, $adder:ident($handler:ident)) => {
        /// The pool's dispatcher for this event group.
        ///
        /// # Safety
        /// `pool` must come from [`player_pool`].
        #[must_use]
        pub unsafe fn $getter(pool: *mut IPlayerPool) -> *mut $dispatcher {
            #[cfg(not(target_env = "msvc"))]
            type GetFn = unsafe extern "C" fn(*mut u8) -> *mut $dispatcher;
            #[cfg(target_env = "msvc")]
            type GetFn = unsafe extern "thiscall" fn(*mut u8) -> *mut $dispatcher;

            let Some((this, f_ptr)) =
                (unsafe { super::vtable::secondary_call_target_ptr(pool.cast::<u8>(), 0, $slot) })
            else {
                return std::ptr::null_mut();
            };
            let get: GetFn = unsafe { std::mem::transmute(f_ptr) };
            unsafe { get(this) }
        }

        /// Registers `handler` on the dispatcher (`addEventHandler`, slot [0]).
        ///
        /// Returns what the server returned: `false` means it was already
        /// registered.
        ///
        /// # Safety
        /// Both pointers must be valid, and `handler` must outlive the
        /// registration.
        pub unsafe fn $adder(dispatcher: *mut $dispatcher, handler: *mut $handler) -> bool {
            #[cfg(not(target_env = "msvc"))]
            type AddFn = unsafe extern "C" fn(*mut u8, *mut $handler, i8) -> bool;
            #[cfg(target_env = "msvc")]
            type AddFn = unsafe extern "thiscall" fn(*mut u8, *mut $handler, i8) -> bool;

            let Some((this, f_ptr)) = (unsafe {
                super::vtable::secondary_call_target_ptr(dispatcher.cast::<u8>(), 0, 0)
            }) else {
                return false;
            };
            let add: AddFn = unsafe { std::mem::transmute(f_ptr) };
            unsafe { add(this, handler, 0) }
        }
    };
}

dispatcher_pair!(player_spawn_dispatcher -> IPlayerSpawnDispatcher @ SLOT_SPAWN_DISPATCHER,
                 add_player_spawn_handler(PlayerSpawnHandler));
dispatcher_pair!(player_text_dispatcher -> IPlayerTextDispatcher @ SLOT_TEXT_DISPATCHER,
                 add_player_text_handler(PlayerTextHandler));
dispatcher_pair!(player_damage_dispatcher -> IPlayerDamageDispatcher @ SLOT_DAMAGE_DISPATCHER,
                 add_player_damage_handler(PlayerDamageHandler));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_match_the_official_binaries() {
        // Dumped from `omp-server`: Core [8] getPlayers, PlayerPool [10]
        // getPlayerConnectDispatcher. MSVC drops one destructor slot.
        #[cfg(not(target_env = "msvc"))]
        {
            assert_eq!(SLOT_GET_PLAYERS, 8);
            assert_eq!(SLOT_CONNECT_DISPATCHER, 10);
        }
        #[cfg(target_env = "msvc")]
        {
            assert_eq!(SLOT_GET_PLAYERS, 7);
            assert_eq!(SLOT_CONNECT_DISPATCHER, 9);
        }
    }

    #[test]
    fn the_other_dispatcher_slots_match_the_dump() {
        // From the `PlayerPool` vtable of the official `omp-server`:
        // [9] spawn, [10] connect, [12] text, [15] damage.
        #[cfg(not(target_env = "msvc"))]
        {
            assert_eq!(SLOT_SPAWN_DISPATCHER, 9);
            assert_eq!(SLOT_TEXT_DISPATCHER, 12);
            assert_eq!(SLOT_DAMAGE_DISPATCHER, 15);
        }
        #[cfg(target_env = "msvc")]
        {
            assert_eq!(SLOT_SPAWN_DISPATCHER, 8);
            assert_eq!(SLOT_TEXT_DISPATCHER, 11);
            assert_eq!(SLOT_DAMAGE_DISPATCHER, 14);
        }
    }

    #[test]
    fn every_handler_vtable_has_the_slots_its_header_declares() {
        let pointer = std::mem::size_of::<*const ()>();
        assert_eq!(std::mem::size_of::<PlayerSpawnHandlerVTable>(), 2 * pointer);
        assert_eq!(std::mem::size_of::<PlayerTextHandlerVTable>(), 2 * pointer);
        assert_eq!(
            std::mem::size_of::<PlayerDamageHandlerVTable>(),
            3 * pointer
        );
        assert_eq!(std::mem::offset_of!(PlayerSpawnHandler, vtable), 0);
        assert_eq!(std::mem::offset_of!(PlayerTextHandler, vtable), 0);
        assert_eq!(std::mem::offset_of!(PlayerDamageHandler, vtable), 0);
    }

    #[test]
    fn handler_layout_is_a_bare_vtable_pointer() {
        assert_eq!(std::mem::offset_of!(PlayerConnectHandler, vtable), 0);
        assert_eq!(
            std::mem::size_of::<PlayerConnectHandler>(),
            std::mem::size_of::<*const ()>()
        );
        assert_eq!(
            std::mem::size_of::<PlayerConnectHandlerVTable>(),
            4 * std::mem::size_of::<*const ()>(),
            "four slots: no virtual destructor in the header"
        );
    }

    #[test]
    fn unknown_disconnect_reason_does_not_invent_a_variant() {
        assert_eq!(DisconnectReason::from_raw(1), DisconnectReason::Quit);
        assert_eq!(DisconnectReason::from_raw(99), DisconnectReason::Custom);
    }
}
