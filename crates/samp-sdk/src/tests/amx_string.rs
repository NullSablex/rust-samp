//! Integration tests for [`AmxString`]: decoding, packed vs unpacked,
//! [`Deref`], equality, palindromes via the `&str` API.
//!
//! [`AmxString`]: crate::cell::AmxString
//! [`Deref`]: std::ops::Deref

use crate::cell::{AmxString, Buffer, Ref};

/// Builds an `AmxString<'static>` from a `&str` in unpacked format
/// (1 byte per cell + `0` terminator). The returned `Vec` owns the cells:
/// moving it keeps the heap address stable, and the caller must keep it
/// alive (bind it to a named variable) while the `AmxString` is in use.
fn make_amx_string(s: &str) -> (Vec<i32>, AmxString<'static>) {
    let mut data: Vec<i32> = s.bytes().map(i32::from).collect();
    data.push(0);

    let len = data.len();
    let ptr = data.as_mut_ptr();
    let r = unsafe { Ref::new(0, ptr) };
    let buf: Buffer<'static> = Buffer::new(r, len);
    let amx_str = AmxString::from_buffer_parts(buf, len - 1);
    (data, amx_str)
}

#[test]
fn len_returns_character_count() {
    let (_cells, s) = make_amx_string("hello");
    assert_eq!(s.len(), 5);
}

#[test]
fn is_empty_for_empty_string() {
    let (_cells, s) = make_amx_string("");
    assert!(s.is_empty());
}

#[test]
fn is_empty_false_for_nonempty() {
    let (_cells, s) = make_amx_string("x");
    assert!(!s.is_empty());
}

#[test]
fn deref_returns_correct_str() {
    let (_cells, s) = make_amx_string("world");
    assert_eq!(&*s, "world");
}

#[test]
fn deref_cached_is_consistent() {
    let (_cells, s) = make_amx_string("rust");
    let first: *const str = &raw const *s;
    let second: *const str = &raw const *s;
    assert_eq!(
        first, second,
        "OnceCell should return same ptr on repeated access"
    );
}

#[test]
fn to_bytes_returns_correct_bytes() {
    let (_cells, s) = make_amx_string("abc");
    assert_eq!(s.to_bytes(), b"abc");
}

#[test]
fn to_bytes_empty() {
    let (_cells, s) = make_amx_string("");
    assert_eq!(s.to_bytes(), b"");
}

#[test]
fn as_str_matches_deref() {
    let (_cells, s) = make_amx_string("test");
    assert_eq!(s.as_str(), &*s);
}
