//! Global encoding for Rust <-> AMX conversion (only with the `encoding` feature).
//!
//! The original SA-MP operates on 8-bit encodings (Western Windows-1252 by
//! default, Windows-1251 for Cyrillic on Russian servers). This module lets the
//! plugin configure the encoding once in `on_load` — after that, `AmxString`
//! decodes and [`Buffer::write_str`] encodes using it automatically.
//!
//! [`set_default_encoding`] takes any `&'static Encoding`, so every encoding
//! `encoding_rs` implements is available; the ones SA-MP and Open Multiplayer
//! servers actually run are re-exported here, and
//! [`set_default_encoding_by_label`] resolves one by name for a plugin that
//! reads it from configuration.
//!
//! ## Multi-byte encodings
//!
//! Pawn assumes one cell holds one character. That holds for every 8-bit
//! encoding above, and **not** for UTF-8, GBK, Big5, Shift_JIS or EUC-KR, where
//! one character may take several bytes. Converting in and out of Rust stays
//! correct, but on the Pawn side `strlen` then counts bytes rather than
//! characters and `text[3]` is the fourth byte, not the fourth character. Use a
//! multi-byte encoding only when the script is written for it.
//!
//! [`Buffer::write_str`]: crate::cell::Buffer::write_str

use encoding_rs::Encoding;
use std::sync::atomic::{AtomicPtr, Ordering};

// Re-exported so a plugin does not have to depend on `encoding_rs` directly to
// name one. `set_default_encoding` takes any `&'static Encoding`, so the ones
// missing here — the whole WHATWG set — still work via `encoding_rs`, or by
// label through [`set_default_encoding_by_label`].
//
// | Encoding       | Where it is used                                    |
// | -------------- | --------------------------------------------------- |
// | `WINDOWS_1250` | Polish, Czech, Slovak, Hungarian, Romanian, Croatian |
// | `WINDOWS_1251` | Russian and other Cyrillic scripts                   |
// | `WINDOWS_1252` | Western Europe, Latin America (the default)          |
// | `WINDOWS_1253` | Greek                                                |
// | `WINDOWS_1254` | Turkish                                              |
// | `WINDOWS_1256` | Arabic                                               |
// | `WINDOWS_1257` | Baltic — Lithuanian, Latvian, Estonian               |
// | `ISO_8859_2`   | Central Europe, where a script predates the CP1250 era |
// | `UTF_8`        | Open Multiplayer scripts written in UTF-8            |
pub use encoding_rs::{
    ISO_8859_2, UTF_8, WINDOWS_1250, WINDOWS_1251, WINDOWS_1252, WINDOWS_1253, WINDOWS_1254,
    WINDOWS_1256, WINDOWS_1257,
};

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

/// Sets the global encoding from a label, the way a configuration file names it.
///
/// Accepts every label the [WHATWG Encoding Standard] defines, so
/// `"windows-1251"`, `"cp1251"` and `"cyrillic"` all resolve to the same
/// encoding, case and surrounding whitespace ignored. That is what lets a
/// plugin read the encoding from the server's own configuration instead of
/// compiling it in.
///
/// Returns the encoding that was set, or `None` when the label matches nothing
/// — in which case the current encoding is left alone, and the caller should
/// say so rather than silently run in the wrong encoding.
///
/// ```rust
/// # use samp_sdk::encoding::{set_default_encoding, set_default_encoding_by_label, WINDOWS_1252};
/// let chosen = set_default_encoding_by_label("windows-1254");
/// assert_eq!(chosen.map(Encoding::name), Some("windows-1254"));
/// assert!(set_default_encoding_by_label("not-an-encoding").is_none());
/// # set_default_encoding(WINDOWS_1252);
/// # use encoding_rs::Encoding;
/// ```
///
/// [WHATWG Encoding Standard]: https://encoding.spec.whatwg.org/#names-and-labels
pub fn set_default_encoding_by_label(label: &str) -> Option<&'static Encoding> {
    let encoding = Encoding::for_label(label.as_bytes())?;
    set_default_encoding(encoding);
    Some(encoding)
}

pub(crate) fn get() -> &'static Encoding {
    unsafe { &*DEFAULT_ENCODING.load(Ordering::Acquire) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, PoisonError};

    /// The encoding is process-wide, so the tests take turns and each one puts
    /// the default back.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_encoding_is_windows_1252() {
        let _g = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let enc = get();
        assert_eq!(enc.name(), WINDOWS_1252.name());
    }

    #[test]
    fn set_and_get_encoding() {
        let _g = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        set_default_encoding(WINDOWS_1251);
        let enc = get();
        assert_eq!(enc.name(), WINDOWS_1251.name());

        // restore default
        set_default_encoding(WINDOWS_1252);
        let enc = get();
        assert_eq!(enc.name(), WINDOWS_1252.name());
    }

    #[test]
    fn label_resolves_and_sets() {
        let _g = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let chosen = set_default_encoding_by_label("windows-1254");
        assert_eq!(chosen.map(Encoding::name), Some("windows-1254"));
        assert_eq!(get().name(), "windows-1254");
        set_default_encoding(WINDOWS_1252);
    }

    #[test]
    fn label_matching_follows_the_whatwg_aliases() {
        let _g = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        // Same encoding under three spellings a config file might carry.
        // Note `cyrillic` is *not* one of them — WHATWG maps that label to
        // ISO-8859-5, a different encoding.
        for label in ["cp1251", "WINDOWS-1251", "  x-cp1251  "] {
            let chosen = set_default_encoding_by_label(label);
            assert_eq!(
                chosen.map(Encoding::name),
                Some("windows-1251"),
                "label {label:?} should resolve to windows-1251"
            );
        }
        set_default_encoding(WINDOWS_1252);
    }

    #[test]
    fn an_unknown_label_changes_nothing() {
        let _g = TEST_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        set_default_encoding(WINDOWS_1251);
        assert!(set_default_encoding_by_label("not-an-encoding").is_none());
        assert_eq!(get().name(), WINDOWS_1251.name(), "the encoding must stay");
        set_default_encoding(WINDOWS_1252);
    }

    #[test]
    fn the_re_exports_are_the_encodings_they_claim() {
        assert_eq!(WINDOWS_1250.name(), "windows-1250");
        assert_eq!(WINDOWS_1253.name(), "windows-1253");
        assert_eq!(WINDOWS_1254.name(), "windows-1254");
        assert_eq!(WINDOWS_1256.name(), "windows-1256");
        assert_eq!(WINDOWS_1257.name(), "windows-1257");
        assert_eq!(ISO_8859_2.name(), "ISO-8859-2");
        assert_eq!(UTF_8.name(), "UTF-8");
    }
}
