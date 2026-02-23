//! String encoding.
use encoding_rs::Encoding;
pub use encoding_rs::{WINDOWS_1251, WINDOWS_1252};
use std::sync::atomic::{AtomicPtr, Ordering};

static DEFAULT_ENCODING: AtomicPtr<Encoding> =
    AtomicPtr::new(WINDOWS_1252 as *const Encoding as *mut Encoding);

pub fn set_default_encoding(encoding: &'static Encoding) {
    DEFAULT_ENCODING.store(
        encoding as *const Encoding as *mut Encoding,
        Ordering::Relaxed,
    );
}

pub(crate) fn get() -> &'static Encoding {
    unsafe { &*DEFAULT_ENCODING.load(Ordering::Relaxed) }
}
