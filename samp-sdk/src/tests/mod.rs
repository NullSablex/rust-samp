//! Integration test suite for `samp-sdk` — one file per area covered.

mod amx_cell;
mod amx_string;
mod buffer;
#[cfg(not(feature = "samp-only"))]
mod omp_lifecycle;
