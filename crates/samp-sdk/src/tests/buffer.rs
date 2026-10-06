//! Testes de [`Buffer`]/[`UnsizedBuffer`]: leitura/escrita tipada via
//! [`CellConvert`] (`get_as`/`set_as`/`iter_as`), `write_str` em formato
//! unpacked, comportamento defensivo de `into_sized_buffer`.
//!
//! [`Buffer`]: crate::cell::Buffer
//! [`UnsizedBuffer`]: crate::cell::UnsizedBuffer
//! [`CellConvert`]: crate::cell::repr::CellConvert

use crate::cell::repr::CellConvert;
use crate::cell::{Buffer, Ref, UnsizedBuffer};
use crate::error::AmxError;

fn make_buffer(data: &mut Vec<i32>) -> Buffer<'_> {
    let len = data.len();
    let r = unsafe { Ref::new(0, data.as_mut_ptr()) };
    Buffer::new(r, len)
}

fn make_unsized(data: &mut Vec<i32>) -> UnsizedBuffer<'_> {
    let r = unsafe { Ref::new(0, data.as_mut_ptr()) };
    UnsizedBuffer::from_raw_parts(r)
}

// --- get_as ---

#[test]
fn get_as_f32_reads_bits() {
    let val: f32 = 1.5;
    let mut data = vec![val.into_cell()];
    let buf = make_buffer(&mut data);
    let got = buf.get_as::<f32>(0).unwrap();
    assert_eq!(got.to_bits(), val.to_bits());
}

#[test]
fn get_as_bool_nonzero_is_true() {
    let mut data = vec![7i32, 0i32];
    let buf = make_buffer(&mut data);
    assert!(buf.get_as::<bool>(0).unwrap());
    assert!(!buf.get_as::<bool>(1).unwrap());
}

#[test]
fn get_as_out_of_bounds_is_none() {
    let mut data = vec![1i32];
    let buf = make_buffer(&mut data);
    assert!(buf.get_as::<i32>(1).is_none());
}

// --- set_as ---

#[test]
fn set_as_bool_writes_zero_one() {
    let mut data = vec![99i32, 99i32];
    let mut buf = make_buffer(&mut data);
    assert!(buf.set_as(0, false));
    assert!(buf.set_as(1, true));
    assert_eq!(data[0], 0);
    assert_eq!(data[1], 1);
}

#[test]
fn set_as_f32_roundtrip() {
    let val: f32 = std::f32::consts::PI;
    let mut data = vec![0i32];
    let mut buf = make_buffer(&mut data);
    assert!(buf.set_as(0, val));
    let got = buf.get_as::<f32>(0).unwrap();
    assert_eq!(got.to_bits(), val.to_bits());
}

#[test]
fn set_as_out_of_bounds_returns_false() {
    let mut data = vec![0i32];
    let mut buf = make_buffer(&mut data);
    assert!(!buf.set_as(1, 42i32));
}

// --- iter_as ---

#[test]
fn iter_as_i32_sums_correctly() {
    let mut data = vec![1i32, 2, 3, 4];
    let buf = make_buffer(&mut data);
    let sum: i32 = buf.iter_as::<i32>().sum();
    assert_eq!(sum, 10);
}

#[test]
fn iter_as_bool_counts_true() {
    let mut data = vec![0i32, 1, 0, 5, 0];
    let buf = make_buffer(&mut data);
    let count = buf.iter_as::<bool>().filter(|&v| v).count();
    assert_eq!(count, 2);
}

// --- write_str ---

#[test]
fn write_str_exact_fit() {
    // "hello" needs 6 cells: 5 chars + null terminator
    let mut data = vec![0i32; 6];
    let mut buf = make_buffer(&mut data);
    buf.write_str("hello").unwrap();
    assert_eq!(data[0], i32::from(b'h'));
    assert_eq!(data[4], i32::from(b'o'));
    assert_eq!(data[5], 0); // null terminator
}

#[test]
fn write_str_shorter_than_buffer() {
    let mut data = vec![99i32; 10];
    let mut buf = make_buffer(&mut data);
    buf.write_str("hi").unwrap();
    assert_eq!(data[0], i32::from(b'h'));
    assert_eq!(data[1], i32::from(b'i'));
    assert_eq!(data[2], 0);
}

#[test]
fn write_str_too_long_returns_error() {
    // buffer of 3 cells can hold at most 2 chars + null
    let mut data = vec![0i32; 3];
    let mut buf = make_buffer(&mut data);
    let result = buf.write_str("toolong");
    assert!(matches!(result, Err(AmxError::General)));
}

// --- UnsizedBuffer::write_str ---

#[test]
fn unsized_buffer_write_str_roundtrip() {
    let mut data = vec![0i32; 8];
    let ubuf = make_unsized(&mut data);
    ubuf.write_str(8, "world").unwrap();
    assert_eq!(data[0], i32::from(b'w'));
    assert_eq!(data[4], i32::from(b'd'));
    assert_eq!(data[5], 0);
}
