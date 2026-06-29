//! Low-level layer of the `rust-samp` toolkit.
//!
//! This crate concentrates two independent sets of bindings:
//!
//! - **AMX**: pointers, cell types, function table and error codes of the
//!   Pawn VM used by SA-MP (modules [`raw`], [`amx`], [`cell`], [`args`],
//!   [`error`], [`exports`], [`consts`]).
//! - **Open Multiplayer**: vtables, binary layout of `IComponent` and typed wrappers
//!   of the native server interfaces (module [`omp`], active while the
//!   `samp-only` feature is not enabled).
//!
//! The interface of this crate works on raw pointers and is `unsafe`-friendly.
//! To write plugins, use the `samp` crate (workspace root) — it re-exports
//! this SDK and adds the plugin life cycle and native registration via
//! proc macros (`#[native]`, `initialize_plugin!`, `#[derive(SampPlugin)]`).

pub mod amx;
pub mod args;
pub mod cell;
pub mod consts;
#[cfg(feature = "debug")]
pub mod debug;
#[cfg(feature = "encoding")]
pub mod encoding;
pub mod error;
pub mod exports;
#[doc(hidden)]
pub mod macros;
#[cfg(not(feature = "samp-only"))]
pub mod omp;
pub mod raw;
#[cfg(test)]
mod tests;
