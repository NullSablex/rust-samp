//! String encoding.
use encoding_rs::Encoding;
pub use encoding_rs::{WINDOWS_1251, WINDOWS_1252};
use std::sync::atomic::{AtomicPtr, Ordering};

static DEFAULT_ENCODING: AtomicPtr<Encoding> =
    AtomicPtr::new(WINDOWS_1252 as *const Encoding as *mut Encoding);

pub fn set_default_encoding(encoding: &'static Encoding) {
    DEFAULT_ENCODING.store(
        encoding as *const Encoding as *mut Encoding,
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

        // restaurar padrão
        set_default_encoding(WINDOWS_1252);
        let enc = get();
        assert_eq!(enc.name(), WINDOWS_1252.name());
    }
}
