//! Global encoding for Rust <-> AMX conversion (only with the `encoding` feature).
//!
//! The original SA-MP operates on 8-bit encodings (Western Windows-1252 by
//! default, Windows-1251 for Cyrillic on Russian servers). This module lets the
//! plugin configure the encoding once in `on_load` — after that, `AmxString`
//! decodes and [`Buffer::write_str`] encodes using it automatically.
//!
//! [`Buffer::write_str`]: crate::cell::Buffer::write_str

use encoding_rs::Encoding;
pub use encoding_rs::{WINDOWS_1251, WINDOWS_1252};
use std::sync::atomic::{AtomicPtr, Ordering};

static DEFAULT_ENCODING: AtomicPtr<Encoding> =
    AtomicPtr::new(std::ptr::from_ref::<Encoding>(WINDOWS_1252).cast_mut());

/// Sets the global encoding used in every AMX string conversion.
///
/// Call once during plugin initialization (`on_load`). Later changes are
/// visible immediately to any thread (`Ordering::Release`/`Acquire`).
pub fn set_default_encoding(encoding: &'static Encoding) {
    DEFAULT_ENCODING.store(
        std::ptr::from_ref::<Encoding>(encoding).cast_mut(),
        Ordering::Release,
    );
}

pub(crate) fn get() -> &'static Encoding {
    unsafe { &*DEFAULT_ENCODING.load(Ordering::Acquire) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_encoding_is_windows_1252() {
        let enc = get();
        assert_eq!(enc.name(), WINDOWS_1252.name());
    }

    #[test]
    fn set_and_get_encoding() {
        set_default_encoding(WINDOWS_1251);
        let enc = get();
        assert_eq!(enc.name(), WINDOWS_1251.name());

        // restore default
        set_default_encoding(WINDOWS_1252);
        let enc = get();
        assert_eq!(enc.name(), WINDOWS_1252.name());
    }
}
