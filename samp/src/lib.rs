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
pub mod logger;
#[cfg(not(feature = "samp-only"))]
pub(crate) mod macros;
pub mod plugin;
pub(crate) mod runtime;

pub use samp_codegen::{initialize_plugin, native};

/// Version of the `rust-samp` (`samp`) crate the plugin was compiled
/// against. Useful for diagnostic natives that report the SDK build
/// back to the gamemode (e.g. `MyPlugin_GetSdkVersion()`), bug reports
/// and runtime dashboards.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

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

#[cfg(feature = "debug")]
pub use samp_sdk::debug;

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

/// Installs the SDK logger with defaults derived from the caller's
/// `Cargo.toml`. Writes to `logs/{CARGO_PKG_NAME}.log` with size-based
/// rotation (50 MB × 5 archives) and forwards every line to the server's
/// own log prefixed with `[CARGO_PKG_NAME]`.
///
/// Returns `Result<(), samp::logger::InstallError>` — the most common
/// failures are "already installed" (a second call in the same process)
/// and "I/O" (the log directory could not be created).
///
/// # Example
/// ```rust,ignore
/// fn on_load(&mut self) {
///     let _ = samp::enable_logger!();
///     log::info!("ready");
/// }
/// ```
#[macro_export]
macro_rules! enable_logger {
    () => {
        $crate::enable_logger_with!($crate::logger::LoggerConfig::new(env!("CARGO_PKG_NAME")))
    };
}

/// Installs the SDK logger with an explicit [`LoggerConfig`].
///
/// The macro still seeds the banner metadata from the caller's
/// `CARGO_PKG_*` values before delegating to [`logger::install`], so the
/// startup banner reports the user's plugin even when every other field
/// is overridden.
///
/// [`LoggerConfig`]: crate::logger::LoggerConfig
/// [`logger::install`]: crate::logger::install
#[macro_export]
macro_rules! enable_logger_with {
    ($cfg:expr) => {{
        $crate::logger::__set_banner_metadata($crate::logger::BannerMetadata::new(
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_AUTHORS"),
            env!("CARGO_PKG_REPOSITORY"),
        ));
        $crate::logger::install($cfg)
    }};
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_matches_cargo_pkg_version() {
        assert_eq!(super::version(), env!("CARGO_PKG_VERSION"));
        assert!(!super::version().is_empty());
    }
}
