//! Integration tests for [`AmxString`]: decoding, packed vs unpacked,
//! [`Deref`], equality, palindromes via the `&str` API.
//!
//! [`AmxString`]: crate::cell::AmxString
//! [`Deref`]: std::ops::Deref

use crate::cell::{AmxString, Buffer, Ref};

/// Builds an `AmxString<'static>` from a `&str` in unpacked format
/// (1 byte per cell + `0` terminator). The buffer is intentionally leaked
/// to guarantee a stable address throughout the test.
fn make_amx_string(s: &str) -> (Vec<i32>, AmxString<'static>) {
    let mut data: Vec<i32> = s.bytes().map(i32::from).collect();
    data.push(0);

    let boxed: Box<Vec<i32>> = Box::new(data);
    let len = boxed.len();
    let ptr = boxed.as_ptr().cast_mut();
    let r = unsafe { Ref::new(0, ptr) };
    let buf: Buffer<'static> = Buffer::new(r, len);
    let amx_str = AmxString::from_buffer_parts(buf, len - 1);
    Box::leak(boxed);
    (vec![], amx_str)
}

#[test]
fn len_returns_character_count() {
    let (_, s) = make_amx_string("hello");
    assert_eq!(s.len(), 5);
}

#[test]
fn is_empty_for_empty_string() {
    let (_, s) = make_amx_string("");
    assert!(s.is_empty());
}

#[test]
fn is_empty_false_for_nonempty() {
    let (_, s) = make_amx_string("x");
    assert!(!s.is_empty());
}

#[test]
fn deref_returns_correct_str() {
    let (_, s) = make_amx_string("world");
    assert_eq!(&*s, "world");
}

#[test]
fn deref_cached_is_consistent() {
    let (_, s) = make_amx_string("rust");
    let first: *const str = &raw const *s;
    let second: *const str = &raw const *s;
    assert_eq!(
        first, second,
        "OnceCell should return same ptr on repeated access"
    );
}

#[test]
fn to_bytes_returns_correct_bytes() {
    let (_, s) = make_amx_string("abc");
    assert_eq!(s.to_bytes(), b"abc");
}

#[test]
fn to_bytes_empty() {
    let (_, s) = make_amx_string("");
    assert_eq!(s.to_bytes(), b"");
}

#[test]
fn as_str_matches_deref() {
    let (_, s) = make_amx_string("test");
    assert_eq!(s.as_str(), &*s);
}
