//! Rust toolkit for developing SA-MP plugins and native Open Multiplayer components.
//!
//! # Workspace structure
//!
//! - `samp` — main crate; re-exports SDK + codegen and exposes the API the plugin uses.
//! - `samp-codegen` — proc macros (`#[native]`, `initialize_plugin!`,
//!   `#[derive(SampPlugin)]`) that generate FFI entry points and argument parsing.
//! - `samp-sdk` — low-level bindings for the AMX VM (SA-MP) and for the component
//!   ABI (Open Multiplayer).
//!
//! # Minimal `Cargo.toml` setup
//!
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! samp = { git = "https://github.com/NullSablex/rust-samp" }
//! ```
//!
//! # Plugin example
//!
//! ```rust,ignore
//! use samp::prelude::*;
//! use samp::{native, initialize_plugin, SampPlugin};
//!
//! #[derive(SampPlugin, Default)]
//! struct MyPlugin;
//!
//! impl MyPlugin {
//!     #[native(name = "Greet")]
//!     fn greet(&mut self, _amx: &Amx, name: &AmxString) -> AmxResult<bool> {
//!         if name.starts_with("Admin") {
//!             println!("[VIP] Welcome, {}!", &**name);
//!         } else {
//!             println!("Hello, {}!", &**name);
//!         }
//!         Ok(true)
//!     }
//! }
//!
//! // Short form — default constructor via Default::default().
//! initialize_plugin!(
//!     type: MyPlugin,
//!     natives: [MyPlugin::greet],
//! );
//!
//! // Full form when there is setup in the constructor (logger, tick, etc):
//! // initialize_plugin!(
//! //     natives: [MyPlugin::greet],
//! //     {
//! //         samp::plugin::enable_tick();
//! //         return MyPlugin;
//! //     }
//! // );
//! ```

pub mod amx;
#[doc(hidden)]
pub mod interlayer;
#[cfg(not(feature = "samp-only"))]
pub(crate) mod macros;
pub mod plugin;
pub(crate) mod runtime;

pub use samp_codegen::{initialize_plugin, native};

// Re-export so the generated macro does not leak the `log` dep into the user's Cargo.toml.
#[doc(hidden)]
pub use log;

/// Derive macro that generates an empty `impl SampPlugin for T {}` for structs
/// that do not need to customize any trait method. For structs with logic in
/// `on_load`/`on_tick`/etc, declare `impl SampPlugin for T { ... }`
/// manually instead of using the derive.
pub use samp_codegen::SampPlugin;
pub use samp_sdk::exec_public;
pub use samp_sdk::{args, cell, consts, error, exports, raw};

#[cfg(feature = "encoding")]
pub use samp_sdk::encoding;

#[cfg(not(feature = "samp-only"))]
pub use samp_sdk::omp;

pub mod prelude {
    //! Most commonly used imports in plugins.
    pub use crate::amx::{Amx, AmxExt};
    pub use crate::cell::{AmxCell, AmxString, Buffer, CellConvert, Ref, UnsizedBuffer};
    pub use crate::error::AmxResult;
    pub use crate::plugin::SampPlugin;
}
