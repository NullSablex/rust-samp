//! Direct translation of the types and function pointers from the AMX C header.
//!
//! Each item here replicates the ABI of the original Pawn interpreter —
//! changing it has a direct effect on binary compatibility with the SA-MP server.

pub mod functions;
pub mod types;
