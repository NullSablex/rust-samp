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
pub mod extensions;
pub mod players;
pub mod server;
pub mod timers;
pub mod types;
pub mod vehicles;
pub mod vtable;
pub mod world;

pub use component::{
    IComponentList, IComponentVTable, ICore, IEarlyConfig, ILogger, IUIDProviderVTable,
    OmpComponent,
};
pub use component_api::{OmpComponentHandle, component_name, component_version};
pub use core::{LogLevel, core_log_ln, core_log_ln_u8, core_print_ln, core_print_ln_u8};
pub use events::{PawnEventHandler, PawnEventHandlerVTable};
pub use extensions::extension;
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
    PlayerUpdateHandlerVTable, PoolBounds, add_player_change_handler, add_player_check_handler,
    add_player_click_handler, add_player_shot_handler, add_player_stream_handler,
    add_player_update_handler, all_players, player_armour, player_by_id, player_change_dispatcher,
    player_check_dispatcher, player_click_dispatcher, player_extension, player_give_money,
    player_health, player_id, player_interior, player_is_bot, player_kick, player_money,
    player_name, player_position, player_score, player_send_message, player_set_armour,
    player_set_controllable, player_set_drunk_level, player_set_health, player_set_interior,
    player_set_money, player_set_position, player_set_score, player_set_skin, player_set_team,
    player_set_virtual_world, player_set_wanted_level, player_set_weather, player_shot_dispatcher,
    player_skin, player_stream_dispatcher, player_team, player_update_dispatcher,
    player_virtual_world, player_wanted_level, pool_bounds,
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
pub use world::{
    ACTORS_COMPONENT_UID, GANGZONES_COMPONENT_UID, GangZonePos, IActor, IActorsComponent,
    IGangZone, IGangZonesComponent, ITextDraw, ITextDrawsComponent, TEXTDRAWS_COMPONENT_UID,
    as_actors_component, as_gangzones_component, as_textdraws_component, create_actor,
    create_gangzone, create_textdraw,
};
pub use world::{
    CLASSES_COMPONENT_UID, IClass, IClassesComponent, IMenu, IMenusComponent, ITextLabel,
    ITextLabelsComponent, MAX_WEAPON_SLOTS, MENUS_COMPONENT_UID, TEXTLABELS_COMPONENT_UID,
    WeaponSlot, as_classes_component, as_menus_component, as_textlabels_component, create_class,
    create_menu, create_textlabel,
};
pub use world::{
    IObjectsComponent, IPickup, IPickupsComponent, OBJECTS_COMPONENT_UID, PICKUPS_COMPONENT_UID,
    PickupType, as_objects_component, as_pickups_component, create_object, create_pickup,
    entity_id,
};
