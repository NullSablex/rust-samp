//! Conversion between Rust types and AMX cells (raw i32).

use crate::amx::Amx;
use crate::error::{AmxError, AmxResult};

/// Conversion to/from an AMX cell in the context of a live VM.
///
/// This is the centerpiece of natives: `T: AmxCell` arguments are parsed via
/// [`from_raw`] from the parameter array, and return values serialized
/// via [`as_cell`] to the output slot. Complex types (strings, buffers, refs)
/// rely on `&Amx` to resolve relative addresses.
///
/// [`from_raw`]: AmxCell::from_raw
/// [`as_cell`]: AmxCell::as_cell
pub trait AmxCell<'amx>
where
    Self: Sized,
{
    /// Reconstructs the value from a raw AMX cell.
    ///
    /// # Errors
    /// `AmxError::General` in the default implementation (use the concrete
    /// impls); specific implementations may return `AmxError::MemoryAccess`
    /// if the address is invalid, or other variants for decode failures.
    fn from_raw(_amx: &'amx Amx, _cell: i32) -> AmxResult<Self>
    where
        Self: 'amx,
    {
        Err(AmxError::General)
    }

    fn as_cell(&self) -> i32;
}

/// Marker: the value fits in a single AMX cell (32 bits) and can be
/// stored directly in the VM stack/heap without indirection.
///
/// Implemented for `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `usize`, `isize`,
/// `f32` and `bool`.
///
/// # Safety
/// Only implement for types that fit in 32 bits — the SDK assumes this when
/// copying bytes from the cell. Larger types corrupt the VM memory.
pub unsafe trait AmxPrimitive
where
    Self: Sized,
{
}

/// Conversion between Rust and an AMX cell (`i32`) without needing `&Amx`.
///
/// Difference vs [`AmxCell`]: this trait operates on standalone values, ideal
/// for bulk operations over [`Buffer`] (each element is an independent cell).
///
/// | Trait | When to use | Needs `&Amx`? |
/// |-------|-------------|--------------------|
/// | [`AmxCell`] | Argument of a `#[native]` | Yes (for complex types) |
/// | `CellConvert` | Element of a [`Buffer`] | No |
///
/// Rarely needs to be imported/implemented directly — [`Buffer::get_as`] and
/// [`Buffer::set_as`] already consume this trait.
///
/// # Example
/// ```rust,no_run
/// # use samp_sdk::cell::Buffer;
/// fn scale_floats(buf: &mut Buffer, factor: f32) {
///     for i in 0..buf.len() {
///         if let Some(v) = buf.get_as::<f32>(i) {
///             buf.set_as(i, v * factor);
///         }
///     }
/// }
/// ```
///
/// [`Buffer::get_as`]: crate::cell::Buffer::get_as
/// [`Buffer::set_as`]: crate::cell::Buffer::set_as
/// [`Buffer`]: crate::cell::Buffer
pub trait CellConvert: Sized {
    /// Decodes a raw `i32` into the Rust type.
    fn from_cell(raw: i32) -> Self;

    /// Encodes the value as a raw cell.
    fn into_cell(self) -> i32;
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

// The `as` cast is intentional: the AMX VM uses `i32` cells for every
// primitive type, so truncation/sign-extension/lossless casts follow the
// Pawn ABI (e.g. a Pawn `byte` lives in an i32 cell and truncates to u8 on read).
// `try_into` would add error handling in a hot path with no semantic gain.
macro_rules! impl_for_primitive {
    ($type:ty) => {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss,
            clippy::cast_lossless
        )]
        impl AmxCell<'_> for $type {
            fn from_raw(_amx: &Amx, cell: i32) -> AmxResult<Self> {
                Ok(cell as Self)
            }

            fn as_cell(&self) -> i32 {
                *self as i32
            }
        }

        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss,
            clippy::cast_lossless
        )]
        impl CellConvert for $type {
            #[inline]
            fn from_cell(raw: i32) -> Self {
                raw as Self
            }

            #[inline]
            fn into_cell(self) -> i32 {
                self as i32
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

// `cell as u32`: bit-for-bit reinterpretation of the i32 coming from the AMX
// VM, required by `f32::from_bits`. Sign-loss here is the goal (i32 and u32
// share the same bit pattern for the same cell content).
impl AmxCell<'_> for f32 {
    fn from_raw(_amx: &Amx, cell: i32) -> AmxResult<f32> {
        #[allow(clippy::cast_sign_loss)]
        let bits = cell as u32;
        Ok(f32::from_bits(bits))
    }

    fn as_cell(&self) -> i32 {
        f32::to_bits(*self).cast_signed()
    }
}

impl CellConvert for f32 {
    #[inline]
    fn from_cell(raw: i32) -> Self {
        #[allow(clippy::cast_sign_loss)]
        let bits = raw as u32;
        f32::from_bits(bits)
    }

    #[inline]
    fn into_cell(self) -> i32 {
        f32::to_bits(self).cast_signed()
    }
}

impl AmxCell<'_> for bool {
    fn from_raw(_amx: &Amx, cell: i32) -> AmxResult<bool> {
        // Explicit comparison instead of `transmute` or `as bool`: any non-zero
        // value counts as `true`, mirroring the `if (val)` behavior in C.
        Ok(cell != 0)
    }

    fn as_cell(&self) -> i32 {
        i32::from(*self)
    }
}

impl CellConvert for bool {
    #[inline]
    fn from_cell(raw: i32) -> Self {
        raw != 0
    }

    #[inline]
    fn into_cell(self) -> i32 {
        i32::from(self)
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
            let recovered = f32::from_bits(cell.cast_unsigned());
            assert_eq!(
                v.to_bits(),
                recovered.to_bits(),
                "f32 {v} did not preserve bits"
            );
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
