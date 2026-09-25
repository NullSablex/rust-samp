//! Native bindings for the Open Multiplayer SDK.
//!
//! Independent pure-Rust implementation of the binary ABI of the Open Multiplayer
//! server: vtables, layout of `IComponent`/`ICore`/`ITimer`, calling
//! conventions, and subobject offsets. No dependency on the original C++ libs
//! (`robin_hood`, `glm`, `nonstd`) — only the types sufficient to implement a
//! component's lifecycle.
//!
//! Offsets and slots were confirmed via disasm of `Console.dll` / `Console.so`
//! and `omp-server.exe`, not by guessing from the C++ headers.
//!
//! Supports the two i686 ABIs used by the server:
//! - **Itanium** (Linux GCC) — calling convention `extern "C"`
//! - **MSVC** (Windows) — calling convention `extern "thiscall"` for virtual
//!   methods, `extern "C"` (cdecl) for variadic

pub mod component;
pub mod component_api;
pub mod core;
pub mod events;
pub mod players;
pub mod server;
pub mod timers;
pub mod types;
pub mod vehicles;
pub mod vtable;

pub use component::{
    IComponentList, IComponentVTable, ICore, IEarlyConfig, ILogger, IUIDProviderVTable,
    OmpComponent,
};
pub use component_api::{OmpComponentHandle, component_name, component_version};
pub use core::{LogLevel, core_log_ln, core_log_ln_u8, core_print_ln, core_print_ln_u8};
pub use events::{PawnEventHandler, PawnEventHandlerVTable};
pub use players::{
    DisconnectReason, IPlayer, IPlayerConnectDispatcher, IPlayerDamageDispatcher, IPlayerPool,
    IPlayerSpawnDispatcher, IPlayerTextDispatcher, PlayerConnectHandler,
    PlayerConnectHandlerVTable, PlayerDamageHandler, PlayerDamageHandlerVTable, PlayerSpawnHandler,
    PlayerSpawnHandlerVTable, PlayerTextHandler, PlayerTextHandlerVTable,
    add_player_connect_handler, add_player_damage_handler, add_player_spawn_handler,
    add_player_text_handler, player_connect_dispatcher, player_damage_dispatcher, player_pool,
    player_spawn_dispatcher, player_text_dispatcher,
};
pub use players::{
    IObject, IPlayerChangeDispatcher, IPlayerCheckDispatcher, IPlayerClickDispatcher,
    IPlayerObject, IPlayerShotDispatcher, IPlayerStreamDispatcher, IPlayerUpdateDispatcher,
    PlayerBulletData, PlayerChangeHandler, PlayerChangeHandlerVTable, PlayerCheckHandler,
    PlayerCheckHandlerVTable, PlayerClickHandler, PlayerClickHandlerVTable, PlayerShotHandler,
    PlayerShotHandlerVTable, PlayerStreamHandler, PlayerStreamHandlerVTable, PlayerUpdateHandler,
    PlayerUpdateHandlerVTable, add_player_change_handler, add_player_check_handler,
    add_player_click_handler, add_player_shot_handler, add_player_stream_handler,
    add_player_update_handler, player_armour, player_by_id, player_change_dispatcher,
    player_check_dispatcher, player_click_dispatcher, player_give_money, player_health, player_id,
    player_is_bot, player_kick, player_name, player_position, player_score, player_send_message,
    player_set_armour, player_set_health, player_set_money, player_set_position, player_set_score,
    player_set_skin, player_set_team, player_set_virtual_world, player_shot_dispatcher,
    player_stream_dispatcher, player_team, player_update_dispatcher, player_virtual_world,
};
pub use server::{
    AmxFunctionTable, IEventDispatcherPawn, IPawnScript, PAWN_COMPONENT_UID, PawnComponent,
    ServerComponentList, ServerPawnComponent, add_pawn_event_handler, get_amx_from_script,
    get_amx_functions, get_pawn_event_dispatcher, query_component, remove_pawn_event_handler,
};
pub use timers::{
    ITimer, ITimersComponent, TIMERS_COMPONENT_UID, TimerHandlerVTable, TimerTimeOutHandler,
    TimersComponent, create_repeating_timer, kill_timer, query_timers_component,
};
pub use types::{
    Colour, ComponentType, SemanticVersion, StringView, UID, Vector2, Vector3, Vector4,
};
pub use vehicles::{
    IVehicle, IVehicleDispatcher, IVehiclesComponent, VEHICLES_COMPONENT_UID, VehicleHandler,
    VehicleHandlerVTable, add_vehicle_handler, as_vehicles_component, create_vehicle,
    vehicle_by_id, vehicle_event_dispatcher, vehicle_health, vehicle_id, vehicle_model,
    vehicle_position, vehicle_set_colour, vehicle_set_health,
};
