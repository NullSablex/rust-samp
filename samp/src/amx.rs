//! Re-exports the SDK's `Amx` API and adds a global registry of active
//! instances + an opaque identity to pass between callbacks.

pub use samp_sdk::amx::*;
use samp_sdk::raw::types::AMX;

use crate::runtime::Runtime;

/// Locates a live `&Amx` by its [`AmxIdent`].
///
/// Useful when the plugin stores the `ident` in a structure and needs to
/// retrieve the `Amx` later (e.g. a list of scripts subscribed to an event).
/// Returns `None` if the AMX has already been unloaded by the server.
#[inline]
#[must_use]
pub fn get<'a>(ident: AmxIdent) -> Option<&'a Amx> {
    let rt = Runtime::get();
    rt.amx_list()
        .iter()
        .find(|(k, _)| *k == ident)
        .map(|(_, v)| v)
}

/// Registers a freshly received `AMX*` in the global runtime.
///
/// Called by the `interlayer` in `AmxLoad`. Plugins normally do not invoke
/// this function directly.
#[inline]
pub fn add(amx: *mut AMX) {
    let rt = Runtime::get();
    rt.insert_amx(amx);
}

/// Stable identity of an `Amx` instance.
///
/// Wrapper around the pointer address — does not dereference, safe to keep
/// as a key in maps or pass between callbacks. To resolve back to an `&Amx`,
/// use [`get`].
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub struct AmxIdent {
    ident: usize,
}

impl From<*mut AMX> for AmxIdent {
    fn from(ptr: *mut AMX) -> AmxIdent {
        AmxIdent {
            ident: ptr as usize,
        }
    }
}

/// Extensions over `Amx` specific to the `samp` crate (not part of the base SDK).
pub trait AmxExt {
    /// Opaque identity of the `Amx` — useful for maps and cross references.
    fn ident(&self) -> AmxIdent;
}

impl AmxExt for Amx {
    #[inline]
    fn ident(&self) -> AmxIdent {
        self.amx()
            .expect("Amx::ident() called with null pointer")
            .as_ptr()
            .into()
    }
}
