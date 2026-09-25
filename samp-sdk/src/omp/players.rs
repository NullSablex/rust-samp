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
use super::types::{Colour, StringView, Vector3};

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

/// Slot of `IPlayer::kick()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_PLAYER_KICK: usize = 6;
#[cfg(target_env = "msvc")]
const SLOT_PLAYER_KICK: usize = 5;

/// Slot of `IPlayer::isBot()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_PLAYER_IS_BOT: usize = 8;
#[cfg(target_env = "msvc")]
const SLOT_PLAYER_IS_BOT: usize = 7;

/// Slot of `IPlayer::getName()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_PLAYER_GET_NAME: usize = 27;
#[cfg(target_env = "msvc")]
const SLOT_PLAYER_GET_NAME: usize = 26;

/// Slot of `IPlayer::setHealth(float)`.
#[cfg(not(target_env = "msvc"))]
const SLOT_PLAYER_SET_HEALTH: usize = 77;
#[cfg(target_env = "msvc")]
const SLOT_PLAYER_SET_HEALTH: usize = 76;

/// Slot of `IPlayer::getHealth()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_PLAYER_GET_HEALTH: usize = 78;
#[cfg(target_env = "msvc")]
const SLOT_PLAYER_GET_HEALTH: usize = 77;

/// Slot of `IPlayer::setScore(int)`.
#[cfg(not(target_env = "msvc"))]
const SLOT_PLAYER_SET_SCORE: usize = 79;
#[cfg(target_env = "msvc")]
const SLOT_PLAYER_SET_SCORE: usize = 78;

/// Slot of `IPlayer::getScore()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_PLAYER_GET_SCORE: usize = 80;
#[cfg(target_env = "msvc")]
const SLOT_PLAYER_GET_SCORE: usize = 79;

/// Slot of `IPlayer::sendClientMessage(const Colour&, StringView)`.
#[cfg(not(target_env = "msvc"))]
const SLOT_PLAYER_SEND_MESSAGE: usize = 100;
#[cfg(target_env = "msvc")]
const SLOT_PLAYER_SEND_MESSAGE: usize = 99;

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

/// Slot of `IPlayerPool::getPlayerStreamDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_STREAM_DISPATCHER: usize = 11;
#[cfg(target_env = "msvc")]
const SLOT_STREAM_DISPATCHER: usize = 10;

/// Slot of `IPlayerPool::getPlayerShotDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_SHOT_DISPATCHER: usize = 13;
#[cfg(target_env = "msvc")]
const SLOT_SHOT_DISPATCHER: usize = 12;

/// Slot of `IPlayerPool::getPlayerChangeDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_CHANGE_DISPATCHER: usize = 14;
#[cfg(target_env = "msvc")]
const SLOT_CHANGE_DISPATCHER: usize = 13;

/// Slot of `IPlayerPool::getPlayerClickDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_CLICK_DISPATCHER: usize = 16;
#[cfg(target_env = "msvc")]
const SLOT_CLICK_DISPATCHER: usize = 15;

/// Slot of `IPlayerPool::getPlayerCheckDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_CHECK_DISPATCHER: usize = 17;
#[cfg(target_env = "msvc")]
const SLOT_CHECK_DISPATCHER: usize = 16;

/// Slot of `IPlayerPool::getPlayerUpdateDispatcher()`.
#[cfg(not(target_env = "msvc"))]
const SLOT_UPDATE_DISPATCHER: usize = 18;
#[cfg(target_env = "msvc")]
const SLOT_UPDATE_DISPATCHER: usize = 17;

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

/// Opaque handles for the objects a shot can hit.
#[repr(C)]
pub struct IVehicle {
    _opaque: [u8; 0],
}

/// See [`IVehicle`].
#[repr(C)]
pub struct IObject {
    _opaque: [u8; 0],
}

/// See [`IVehicle`].
#[repr(C)]
pub struct IPlayerObject {
    _opaque: [u8; 0],
}

/// `PlayerBulletData`, passed by const reference — opaque here, since reading
/// it means pinning another layout.
#[repr(C)]
pub struct PlayerBulletData {
    _opaque: [u8; 0],
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

/// Opaque handle for `IEventDispatcher<PlayerStreamEventHandler>*`.
#[repr(C)]
pub struct IPlayerStreamDispatcher {
    _opaque: [u8; 0],
}

/// Opaque handle for `IEventDispatcher<PlayerShotEventHandler>*`.
#[repr(C)]
pub struct IPlayerShotDispatcher {
    _opaque: [u8; 0],
}

/// Opaque handle for `IEventDispatcher<PlayerChangeEventHandler>*`.
#[repr(C)]
pub struct IPlayerChangeDispatcher {
    _opaque: [u8; 0],
}

/// Opaque handle for `IEventDispatcher<PlayerClickEventHandler>*`.
#[repr(C)]
pub struct IPlayerClickDispatcher {
    _opaque: [u8; 0],
}

/// Opaque handle for `IEventDispatcher<PlayerCheckEventHandler>*`.
#[repr(C)]
pub struct IPlayerCheckDispatcher {
    _opaque: [u8; 0],
}

/// Opaque handle for `IEventDispatcher<PlayerUpdateEventHandler>*`.
#[repr(C)]
pub struct IPlayerUpdateDispatcher {
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

handler_vtable! {
    /// `PlayerStreamEventHandler` — a player entering or leaving another's
    /// stream radius.
    PlayerStreamHandlerVTable for PlayerStreamHandler {
        on_player_stream_in: fn(*mut IPlayer, *mut IPlayer),
        on_player_stream_out: fn(*mut IPlayer, *mut IPlayer),
    }
}

handler_vtable! {
    /// `PlayerShotEventHandler` — returning `false` rejects the shot.
    /// `bullet_data` points at a `PlayerBulletData` the server owns.
    PlayerShotHandlerVTable for PlayerShotHandler {
        on_player_shot_missed: fn(*mut IPlayer, *const PlayerBulletData) -> bool,
        on_player_shot_player: fn(*mut IPlayer, *mut IPlayer, *const PlayerBulletData) -> bool,
        on_player_shot_vehicle: fn(*mut IPlayer, *mut IVehicle, *const PlayerBulletData) -> bool,
        on_player_shot_object: fn(*mut IPlayer, *mut IObject, *const PlayerBulletData) -> bool,
        on_player_shot_player_object:
            fn(*mut IPlayer, *mut IPlayerObject, *const PlayerBulletData) -> bool,
    }
}

handler_vtable! {
    /// `PlayerChangeEventHandler`. `PlayerState` and the key masks arrive as
    /// plain integers.
    PlayerChangeHandlerVTable for PlayerChangeHandler {
        on_player_score_change: fn(*mut IPlayer, i32),
        on_player_name_change: fn(*mut IPlayer, StringView),
        on_player_interior_change: fn(*mut IPlayer, u32, u32),
        on_player_state_change: fn(*mut IPlayer, i32, i32),
        on_player_key_state_change: fn(*mut IPlayer, u32, u32),
    }
}

handler_vtable! {
    /// `PlayerClickEventHandler`. `Vector3` is passed by value, as the header
    /// declares it.
    PlayerClickHandlerVTable for PlayerClickHandler {
        on_player_click_map: fn(*mut IPlayer, Vector3),
        on_player_click_player: fn(*mut IPlayer, *mut IPlayer, i32),
    }
}

handler_vtable! {
    /// `PlayerCheckEventHandler` — the reply to a client check request.
    PlayerCheckHandlerVTable for PlayerCheckHandler {
        on_client_check_response: fn(*mut IPlayer, i32, i32, i32),
    }
}

handler_vtable! {
    /// `PlayerUpdateEventHandler` — fires for every player on every server
    /// tick, so keep the body short. Returning `false` drops the update.
    ///
    /// `now` is a `TimePoint` (`steady_clock`, nanoseconds): one 64-bit value
    /// passed by value, which on i686 lands on the stack either way.
    PlayerUpdateHandlerVTable for PlayerUpdateHandler {
        on_player_update: fn(*mut IPlayer, i64) -> bool,
    }
}

/// Calls `IPlayer::kick()` — drops the player from the server.
///
/// # Safety
/// `player` must be an `IPlayer*` the server handed to a handler, and still
/// connected.
pub unsafe fn player_kick(player: *mut IPlayer) {
    #[cfg(not(target_env = "msvc"))]
    type KickFn = unsafe extern "C" fn(*mut u8);
    #[cfg(target_env = "msvc")]
    type KickFn = unsafe extern "thiscall" fn(*mut u8);

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(player.cast::<u8>(), 0, SLOT_PLAYER_KICK)
    }) else {
        return;
    };
    let kick: KickFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { kick(this) };
}

/// `IPlayer::isBot()` — whether this "player" is an NPC.
///
/// # Safety
/// See [`player_kick`].
#[must_use]
pub unsafe fn player_is_bot(player: *mut IPlayer) -> bool {
    #[cfg(not(target_env = "msvc"))]
    type IsBotFn = unsafe extern "C" fn(*mut u8) -> bool;
    #[cfg(target_env = "msvc")]
    type IsBotFn = unsafe extern "thiscall" fn(*mut u8) -> bool;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(player.cast::<u8>(), 0, SLOT_PLAYER_IS_BOT)
    }) else {
        return false;
    };
    let is_bot: IsBotFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { is_bot(this) }
}

/// `IPlayer::getName()` — the player's name, copied into a `String`.
///
/// The server returns a `StringView` into memory it owns, so the bytes are
/// copied out rather than borrowed. `None` when the view is empty or not valid
/// UTF-8.
///
/// Both ABIs return the 8-byte `StringView` through a hidden pointer, the same
/// shape `component_name` uses.
///
/// # Safety
/// See [`player_kick`].
#[must_use]
pub unsafe fn player_name(player: *mut IPlayer) -> Option<String> {
    // Return convention differs: the Itanium ABI hands back this 8-byte,
    // trivially copyable struct in EAX:EDX, while MSVC writes it through a
    // hidden pointer the caller supplies.
    #[cfg(not(target_env = "msvc"))]
    type GetNameFn = unsafe extern "C" fn(*mut u8) -> StringView;
    #[cfg(target_env = "msvc")]
    type GetNameFn = unsafe extern "thiscall" fn(*mut u8, *mut StringView) -> *mut StringView;

    let (this, f_ptr) = unsafe {
        super::vtable::secondary_call_target_ptr(player.cast::<u8>(), 0, SLOT_PLAYER_GET_NAME)
    }?;
    let get_name: GetNameFn = unsafe { std::mem::transmute(f_ptr) };

    #[cfg(not(target_env = "msvc"))]
    let view = unsafe { get_name(this) };

    #[cfg(target_env = "msvc")]
    let view = {
        let mut view = StringView {
            data: std::ptr::null(),
            len: 0,
        };
        unsafe { get_name(this, &raw mut view) };
        view
    };
    if view.data.is_null() || view.len == 0 {
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(view.data, view.len) };
    std::str::from_utf8(bytes).ok().map(String::from)
}

/// `IPlayer::getHealth()`.
///
/// # Safety
/// See [`player_kick`].
#[must_use]
pub unsafe fn player_health(player: *mut IPlayer) -> f32 {
    #[cfg(not(target_env = "msvc"))]
    type GetHealthFn = unsafe extern "C" fn(*mut u8) -> f32;
    #[cfg(target_env = "msvc")]
    type GetHealthFn = unsafe extern "thiscall" fn(*mut u8) -> f32;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(player.cast::<u8>(), 0, SLOT_PLAYER_GET_HEALTH)
    }) else {
        return 0.0;
    };
    let get_health: GetHealthFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get_health(this) }
}

/// `IPlayer::setHealth(float)`.
///
/// # Safety
/// See [`player_kick`].
pub unsafe fn player_set_health(player: *mut IPlayer, health: f32) {
    #[cfg(not(target_env = "msvc"))]
    type SetHealthFn = unsafe extern "C" fn(*mut u8, f32);
    #[cfg(target_env = "msvc")]
    type SetHealthFn = unsafe extern "thiscall" fn(*mut u8, f32);

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(player.cast::<u8>(), 0, SLOT_PLAYER_SET_HEALTH)
    }) else {
        return;
    };
    let set_health: SetHealthFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { set_health(this, health) };
}

/// `IPlayer::getScore()`.
///
/// # Safety
/// See [`player_kick`].
#[must_use]
pub unsafe fn player_score(player: *mut IPlayer) -> i32 {
    #[cfg(not(target_env = "msvc"))]
    type GetScoreFn = unsafe extern "C" fn(*mut u8) -> i32;
    #[cfg(target_env = "msvc")]
    type GetScoreFn = unsafe extern "thiscall" fn(*mut u8) -> i32;

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(player.cast::<u8>(), 0, SLOT_PLAYER_GET_SCORE)
    }) else {
        return 0;
    };
    let get_score: GetScoreFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get_score(this) }
}

/// `IPlayer::setScore(int)`.
///
/// Unlike health, the score is the server's own value — a client cannot
/// overwrite it on the next sync packet.
///
/// # Safety
/// See [`player_kick`].
pub unsafe fn player_set_score(player: *mut IPlayer, score: i32) {
    #[cfg(not(target_env = "msvc"))]
    type SetScoreFn = unsafe extern "C" fn(*mut u8, i32);
    #[cfg(target_env = "msvc")]
    type SetScoreFn = unsafe extern "thiscall" fn(*mut u8, i32);

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(player.cast::<u8>(), 0, SLOT_PLAYER_SET_SCORE)
    }) else {
        return;
    };
    let set_score: SetScoreFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { set_score(this, score) };
}

/// `IPlayer::sendClientMessage(const Colour&, StringView)` — a chat line for
/// this player only.
///
/// `colour` is RGBA, the order [`Colour`] stores. The text is borrowed for the
/// duration of the call: the server copies what it needs, and a `StringView`
/// carries a length, so no NUL terminator is required.
///
/// # Safety
/// See [`player_kick`].
pub unsafe fn player_send_message(player: *mut IPlayer, colour: Colour, text: &str) {
    #[cfg(not(target_env = "msvc"))]
    type SendMessageFn = unsafe extern "C" fn(*mut u8, *const Colour, StringView);
    #[cfg(target_env = "msvc")]
    type SendMessageFn = unsafe extern "thiscall" fn(*mut u8, *const Colour, StringView);

    let Some((this, f_ptr)) = (unsafe {
        super::vtable::secondary_call_target_ptr(player.cast::<u8>(), 0, SLOT_PLAYER_SEND_MESSAGE)
    }) else {
        return;
    };
    let send: SendMessageFn = unsafe { std::mem::transmute(f_ptr) };

    let view = StringView {
        data: text.as_ptr(),
        len: text.len(),
    };
    unsafe { send(this, &raw const colour, view) };
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

dispatcher_pair!(player_stream_dispatcher -> IPlayerStreamDispatcher @ SLOT_STREAM_DISPATCHER,
                 add_player_stream_handler(PlayerStreamHandler));
dispatcher_pair!(player_shot_dispatcher -> IPlayerShotDispatcher @ SLOT_SHOT_DISPATCHER,
                 add_player_shot_handler(PlayerShotHandler));
dispatcher_pair!(player_change_dispatcher -> IPlayerChangeDispatcher @ SLOT_CHANGE_DISPATCHER,
                 add_player_change_handler(PlayerChangeHandler));
dispatcher_pair!(player_click_dispatcher -> IPlayerClickDispatcher @ SLOT_CLICK_DISPATCHER,
                 add_player_click_handler(PlayerClickHandler));
dispatcher_pair!(player_check_dispatcher -> IPlayerCheckDispatcher @ SLOT_CHECK_DISPATCHER,
                 add_player_check_handler(PlayerCheckHandler));
dispatcher_pair!(player_update_dispatcher -> IPlayerUpdateDispatcher @ SLOT_UPDATE_DISPATCHER,
                 add_player_update_handler(PlayerUpdateHandler));

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
    fn the_remaining_dispatcher_slots_match_the_dump() {
        // PlayerPool vtable of the official `omp-server`: [11] stream,
        // [13] shot, [14] change, [16] click, [17] check, [18] update.
        #[cfg(not(target_env = "msvc"))]
        let expected = [11, 13, 14, 16, 17, 18];
        #[cfg(target_env = "msvc")]
        let expected = [10, 12, 13, 15, 16, 17];
        assert_eq!(
            [
                SLOT_STREAM_DISPATCHER,
                SLOT_SHOT_DISPATCHER,
                SLOT_CHANGE_DISPATCHER,
                SLOT_CLICK_DISPATCHER,
                SLOT_CHECK_DISPATCHER,
                SLOT_UPDATE_DISPATCHER,
            ],
            expected
        );
    }

    #[test]
    fn the_remaining_vtables_have_the_slots_their_headers_declare() {
        let p = std::mem::size_of::<*const ()>();
        assert_eq!(std::mem::size_of::<PlayerStreamHandlerVTable>(), 2 * p);
        assert_eq!(std::mem::size_of::<PlayerShotHandlerVTable>(), 5 * p);
        assert_eq!(std::mem::size_of::<PlayerChangeHandlerVTable>(), 5 * p);
        assert_eq!(std::mem::size_of::<PlayerClickHandlerVTable>(), 2 * p);
        assert_eq!(std::mem::size_of::<PlayerCheckHandlerVTable>(), p);
        assert_eq!(std::mem::size_of::<PlayerUpdateHandlerVTable>(), p);
    }

    #[test]
    fn player_method_slots_match_the_dump() {
        // `Player` vtable of the official `omp-server`: [6] kick, [8] isBot,
        // [27] getName. MSVC drops the second destructor slot, and no
        // `IEntity` override (secondary base) sits before any of these, so the
        // shift is exactly one. Unlike the component classes, the Windows
        // server carries no RTTI for `Player`, so these cannot be re-derived
        // from the binary — `scripts/omp-vtable.py` derives them from the
        // headers with clang, and a running server proves them.
        #[cfg(not(target_env = "msvc"))]
        let expected = [6, 8, 27, 77, 78, 79, 80, 100];
        #[cfg(target_env = "msvc")]
        let expected = [5, 7, 26, 76, 77, 78, 79, 99];
        assert_eq!(
            [
                SLOT_PLAYER_KICK,
                SLOT_PLAYER_IS_BOT,
                SLOT_PLAYER_GET_NAME,
                SLOT_PLAYER_SET_HEALTH,
                SLOT_PLAYER_GET_HEALTH,
                SLOT_PLAYER_SET_SCORE,
                SLOT_PLAYER_GET_SCORE,
                SLOT_PLAYER_SEND_MESSAGE,
            ],
            expected
        );
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
