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
pub mod containers;
pub mod core;
pub mod dispatch;
pub mod events;
pub mod extensions;
pub mod generated;
pub mod players;
pub mod server;
pub mod timers;
pub mod types;
pub mod vehicles;
pub mod vtable;
pub mod world;

// Every module's public items at the root, so `samp_sdk::omp::player_name`
// works as well as the qualified path. Globs rather than lists: a list had to
// be edited alongside every new function, and grew by batches.
pub use component::*;
pub use component_api::*;
pub use containers::*;
pub use core::*;
pub use dispatch::*;
pub use events::*;
pub use extensions::*;
pub use generated::*;
pub use players::*;
pub use server::*;
pub use timers::*;
pub use types::*;
pub use vehicles::*;
pub use world::*;
