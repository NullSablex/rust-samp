#![recursion_limit = "128"]

use proc_macro::TokenStream;

mod native;
mod plugin;

pub(crate) const NATIVE_PREFIX: &str = "__samp_native_";
pub(crate) const REG_PREFIX: &str = "__samp_reg_";

/// Generate C function that parses passed argument and calls current function.
#[proc_macro_attribute]
pub fn native(args: TokenStream, input: TokenStream) -> TokenStream {
    native::create_native(args, input)
}

/// Generates common plugin C interface.
#[proc_macro]
pub fn initialize_plugin(input: TokenStream) -> TokenStream {
    plugin::create_plugin(input)
}

/// Automatically implements [`SampPlugin`] with all default methods.
///
/// Equivalent to writing `impl SampPlugin for MyPlugin {}` manually.
/// Override individual methods in a separate `impl` block as needed.
///
/// # Example
/// ```rust,ignore
/// use samp::prelude::*;
///
/// // Plugin simples sem overrides: derive gera o impl automaticamente
/// #[derive(SampPlugin)]
/// struct SimplePlugin {
///     counter: u32,
/// }
///
/// // Plugin com overrides: escreva o impl manualmente (sem o derive)
/// struct AdvancedPlugin;
///
/// impl SampPlugin for AdvancedPlugin {
///     fn on_load(&mut self) {
///         println!("plugin carregado");
///     }
/// }
/// ```
///
/// [`SampPlugin`]: samp::prelude::SampPlugin
#[proc_macro_derive(SampPlugin)]
pub fn derive_samp_plugin(input: TokenStream) -> TokenStream {
    plugin::derive_samp_plugin(input)
}
