//! Roundtrips and edge cases of the [`CellConvert`] trait across every primitive
//! type supported by the AMX VM. Verifies bit preservation (including NaN for f32)
//! and the `bool ↔ i32 (non-zero is true)` mapping.
//!
//! [`CellConvert`]: crate::cell::repr::CellConvert

use crate::cell::repr::CellConvert;

#[test]
fn i32_roundtrip() {
    assert_eq!(i32::from_cell(42_i32.into_cell()), 42);
    assert_eq!(i32::from_cell((-1_i32).into_cell()), -1);
    assert_eq!(i32::from_cell(0_i32.into_cell()), 0);
    assert_eq!(i32::from_cell(i32::MAX.into_cell()), i32::MAX);
    assert_eq!(i32::from_cell(i32::MIN.into_cell()), i32::MIN);
}

#[test]
fn f32_roundtrip() {
    let cases: &[f32] = &[
        0.0,
        1.0,
        -1.0,
        f32::INFINITY,
        f32::NEG_INFINITY,
        std::f32::consts::PI,
    ];
    for &v in cases {
        assert_eq!(f32::from_cell(v.into_cell()).to_bits(), v.to_bits());
    }
}

#[test]
fn f32_nan_roundtrip() {
    let raw = f32::NAN.into_cell();
    assert!(f32::from_cell(raw).is_nan());
}

#[test]
fn bool_false_is_zero() {
    assert_eq!(false.into_cell(), 0);
}

#[test]
fn bool_true_is_one() {
    assert_eq!(true.into_cell(), 1);
}

#[test]
fn bool_from_cell_zero_is_false() {
    assert!(!bool::from_cell(0));
}

#[test]
fn bool_from_cell_nonzero_is_true() {
    for v in [1, -1, 100, i32::MAX, i32::MIN] {
        assert!(bool::from_cell(v), "expected true for raw={v}");
    }
}

#[test]
fn u8_roundtrip() {
    assert_eq!(u8::from_cell(255_u8.into_cell()), 255);
    assert_eq!(u8::from_cell(0_u8.into_cell()), 0);
}

#[test]
fn i16_roundtrip() {
    assert_eq!(i16::from_cell(i16::MIN.into_cell()), i16::MIN);
    assert_eq!(i16::from_cell(i16::MAX.into_cell()), i16::MAX);
}

#[test]
fn u32_roundtrip() {
    assert_eq!(u32::from_cell(u32::MAX.into_cell()), u32::MAX);
    assert_eq!(u32::from_cell(0_u32.into_cell()), 0);
}
