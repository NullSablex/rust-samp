//! samp is a toolkit to develop SA:MP server plugins in Rust.
//!
//! # Estrutura
//! * `samp` — glue entre os crates abaixo (é o que você precisa).
//! * `samp-codegen` — gera `extern "C"` e cuida do lado feio.
//! * `samp-sdk` — todos os tipos para interagir com a VM AMX.
//!
//! # Uso mínimo
//! ```toml
//! [lib]
//! crate-type = ["cdylib"]
//!
//! [dependencies]
//! samp = { git = "https://github.com/NullSablex/rust-samp" }
//! ```
//!
//! # Exemplo
//! ```rust,no_run
//! use samp::prelude::*;
//! use samp::{native, initialize_plugin, SampPlugin};
//!
//! // #[derive(SampPlugin)] gera impl SampPlugin para structs sem overrides
//! #[derive(SampPlugin, Default)]
//! struct MyPlugin;
//!
//! impl MyPlugin {
//!     #[native(name = "Greet")]
//!     fn greet(&mut self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
//!         // Deref<Target=str> — métodos de &str disponíveis diretamente
//!         if name.starts_with("Admin") {
//!             println!("[VIP] Bem-vindo, {}!", &*name);
//!         } else {
//!             println!("Olá, {}!", &*name);
//!         }
//!         Ok(true)
//!     }
//! }
//!
//! // Forma curta: usa Default::default() como construtor
//! initialize_plugin!(
//!     type: MyPlugin,
//!     natives: [MyPlugin::greet],
//! );
//!
//! // Forma completa (quando precisa de lógica no construtor):
//! // initialize_plugin!(
//! //     natives: [MyPlugin::greet],
//! //     {
//! //         samp::plugin::enable_process_tick();
//! //         return MyPlugin;
//! //     }
//! // );
//! ```

pub mod amx;
#[doc(hidden)]
pub mod interlayer;
pub mod plugin;
pub(crate) mod runtime;

pub use samp_codegen::{initialize_plugin, native};
/// Re-exportação do derive macro `#[derive(SampPlugin)]`.
///
/// Gera `impl SampPlugin for T {}` automaticamente para structs sem overrides.
/// Para structs que precisam sobrescrever métodos, use `impl SampPlugin for T` manualmente.
pub use samp_codegen::SampPlugin;
pub use samp_sdk::exec_public;
pub use samp_sdk::{args, cell, consts, error, exports, raw};

#[cfg(feature = "encoding")]
pub use samp_sdk::encoding;

pub mod prelude {
    //! Importações mais usadas em plugins.
    pub use crate::amx::{Amx, AmxExt};
    pub use crate::cell::{AmxCell, AmxString, Buffer, CellConvert, Ref, UnsizedBuffer};
    pub use crate::error::AmxResult;
    pub use crate::plugin::SampPlugin;
}
