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
pub mod server;
pub mod timers;
pub mod types;
pub mod vtable;

pub use component::{
    IComponentList, IComponentVTable, ICore, IEarlyConfig, ILogger, IUIDProviderVTable,
    OmpComponent,
};
pub use component_api::{OmpComponentHandle, component_name, component_version};
pub use core::{LogLevel, core_log_ln, core_log_ln_u8, core_print_ln, core_print_ln_u8};
pub use events::{PawnEventHandler, PawnEventHandlerVTable};
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
