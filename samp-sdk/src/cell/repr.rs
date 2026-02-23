//! A module to discribe how AMX cells work.
use crate::amx::Amx;
use crate::error::{AmxError, AmxResult};

/// AmxCell trait is a core trait of whole SDK.
/// It shows that value can be borrowed (or copied if it's a primitive) from AMX and passed to it.
pub trait AmxCell<'amx>
where
    Self: Sized,
{
    fn from_raw(_amx: &'amx Amx, _cell: i32) -> AmxResult<Self>
    where
        Self: 'amx,
    {
        Err(AmxError::General)
    }

    /// Get a value which can be passed to AMX.
    fn as_cell(&self) -> i32;
}

/// A marker showing that a value can be stored directly on a stack or a heap of an AMX.
///
/// Types: i8, u8, i16, u16, i32, u32, usize, isize, f32, bool
///
/// There is no values that's bigger than 4 bytes, because size of an AMX cell is 32 bits.
///
/// # Safety
/// Must only be implemented for types that fit within a single 32-bit AMX cell.
pub unsafe trait AmxPrimitive
where
    Self: Sized,
{
}

impl<'a, T: AmxCell<'a>> AmxCell<'a> for &'a T {
    fn as_cell(&self) -> i32 {
        (**self).as_cell()
    }
}

impl<'a, T: AmxCell<'a>> AmxCell<'a> for &'a mut T {
    fn as_cell(&self) -> i32 {
        (**self).as_cell()
    }
}

macro_rules! impl_for_primitive {
    ($type:ty) => {
        impl AmxCell<'_> for $type {
            fn from_raw(_amx: &Amx, cell: i32) -> AmxResult<Self> {
                Ok(cell as Self)
            }

            fn as_cell(&self) -> i32 {
                *self as i32
            }
        }

        unsafe impl AmxPrimitive for $type {}
    };
}

impl_for_primitive!(i8);
impl_for_primitive!(u8);
impl_for_primitive!(i16);
impl_for_primitive!(u16);
impl_for_primitive!(i32);
impl_for_primitive!(u32);
impl_for_primitive!(usize);
impl_for_primitive!(isize);

impl AmxCell<'_> for f32 {
    fn from_raw(_amx: &Amx, cell: i32) -> AmxResult<f32> {
        Ok(f32::from_bits(cell as u32))
    }

    fn as_cell(&self) -> i32 {
        f32::to_bits(*self).cast_signed()
    }
}

impl AmxCell<'_> for bool {
    fn from_raw(_amx: &Amx, cell: i32) -> AmxResult<bool> {
        // just to be sure that boolean value will be correct I don't use there `std::mem::transmute` or `as` keyword.
        Ok(cell != 0)
    }

    fn as_cell(&self) -> i32 {
        i32::from(*self)
    }
}

unsafe impl AmxPrimitive for f32 {}
unsafe impl AmxPrimitive for bool {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn i32_as_cell_identity() {
        for v in [0, 1, -1, 42, i32::MAX, i32::MIN] {
            assert_eq!(v.as_cell(), v);
        }
    }

    #[test]
    fn f32_as_cell_preserves_bits() {
        for v in [0.0f32, 1.0, -1.0, 42.5, f32::MAX, f32::MIN, f32::EPSILON] {
            let cell = v.as_cell();
            let recovered = f32::from_bits(cell as u32);
            assert_eq!(v, recovered, "f32 {v} não preservou bits");
        }
    }

    #[test]
    fn bool_as_cell() {
        assert_eq!(true.as_cell(), 1);
        assert_eq!(false.as_cell(), 0);
    }

    #[test]
    fn u8_as_cell() {
        assert_eq!(0u8.as_cell(), 0);
        assert_eq!(255u8.as_cell(), 255);
    }

    #[test]
    fn i8_as_cell() {
        assert_eq!(0i8.as_cell(), 0);
        assert_eq!((-1i8).as_cell(), -1);
        assert_eq!(127i8.as_cell(), 127);
    }

    #[test]
    fn u16_as_cell() {
        assert_eq!(0u16.as_cell(), 0);
        assert_eq!(65535u16.as_cell(), 65535);
    }

    #[test]
    fn i16_as_cell() {
        assert_eq!(0i16.as_cell(), 0);
        assert_eq!((-1i16).as_cell(), -1);
    }

    #[test]
    fn ref_delegates_to_inner() {
        let val = 42i32;
        let r = &val;
        assert_eq!(r.as_cell(), 42);
    }

    #[test]
    fn mut_ref_delegates_to_inner() {
        let mut val = 42i32;
        let r = &mut val;
        assert_eq!(r.as_cell(), 42);
    }
}
