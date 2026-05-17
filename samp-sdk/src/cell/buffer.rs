//! AMX cell vectors (Pawn arrays) — `Buffer` (sized) and
//! `UnsizedBuffer` (unsized, received as a native argument).

use std::ops::{Deref, DerefMut};

use super::{AmxCell, Ref};
use crate::amx::Amx;
use crate::cell::repr::CellConvert;
use crate::cell::string;
use crate::error::AmxResult;

/// AMX cell array with a known size.
///
/// Implements [`Deref<Target = [i32]>`], so the full `&[i32]` API is
/// available (`iter`, `len`, indexing, etc). For non-`i32` types (`f32`,
/// `bool`, etc), use [`iter_as`], [`get_as`], [`set_as`].
///
/// # Example
/// ```
/// use samp_sdk::cell::{UnsizedBuffer, Buffer};
/// # use samp_sdk::amx::Amx;
/// fn double_all(amx: &Amx, buffer: UnsizedBuffer, size: usize) {
///     let mut buffer: Buffer = buffer.into_sized_buffer(size);
///     buffer.iter_mut().for_each(|cell| *cell *= 2);
/// }
/// ```
///
/// [`iter_as`]: Buffer::iter_as
/// [`get_as`]: Buffer::get_as
/// [`set_as`]: Buffer::set_as
pub struct Buffer<'amx> {
    inner: Ref<'amx, i32>,
    len: usize,
}

impl<'amx> Buffer<'amx> {
    /// Builds a `Buffer` from the `Ref` to the first cell and its size.
    #[must_use]
    pub fn new(reference: Ref<'amx, i32>, len: usize) -> Buffer<'amx> {
        Buffer {
            inner: reference,
            len,
        }
    }

    /// Number of cells in the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the buffer has no cells.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Read-only slice covering every cell.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[i32] {
        unsafe { std::slice::from_raw_parts(self.inner.as_ptr(), self.len) }
    }

    /// Mutable slice covering every cell.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [i32] {
        unsafe { std::slice::from_raw_parts_mut(self.inner.as_mut_ptr(), self.len) }
    }

    /// Iterator that converts each cell to `T` via [`CellConvert`].
    ///
    /// Idiomatic ergonomics for arrays of `f32`, `bool` etc. — combine with
    /// iterator adapters (`sum`, `filter_map`, ...).
    ///
    /// ```rust,no_run
    /// # use samp_sdk::cell::Buffer;
    /// fn sum_floats(buf: &Buffer) -> f32 { buf.iter_as::<f32>().sum() }
    /// ```
    pub fn iter_as<T: CellConvert>(&self) -> impl Iterator<Item = T> + '_ {
        self.as_slice().iter().map(|&raw| T::from_cell(raw))
    }

    /// Reads the cell at `index`, converting to `T`. `None` if out of bounds.
    #[must_use]
    pub fn get_as<T: CellConvert>(&self, index: usize) -> Option<T> {
        self.as_slice().get(index).map(|&raw| T::from_cell(raw))
    }

    /// Converts `value` to a raw cell and writes it at `index`.
    ///
    /// Returns `true` if the write happened, `false` if `index` was out of bounds.
    pub fn set_as<T: CellConvert>(&mut self, index: usize, value: T) -> bool {
        if let Some(cell) = self.as_mut_slice().get_mut(index) {
            *cell = value.into_cell();
            true
        } else {
            false
        }
    }

    /// Writes a Rust string into the buffer (unpacked format, `0` terminator).
    ///
    /// Requires `s.len() + 1` cells of space.
    ///
    /// # Errors
    /// `AmxError::General` if the encoded string is >= the buffer size.
    pub fn write_str(&mut self, s: &str) -> AmxResult<()> {
        string::put_in_buffer(self, s)
    }
}

// `Buffer` cannot be parsed directly from a cell — use `UnsizedBuffer`
// as the native argument and then `.into_sized_buffer(len)`.
impl<'amx> AmxCell<'amx> for Buffer<'amx> {
    #[inline]
    fn as_cell(&self) -> i32 {
        self.inner.as_cell()
    }
}

impl Deref for Buffer<'_> {
    type Target = [i32];

    fn deref(&self) -> &[i32] {
        self.as_slice()
    }
}

impl DerefMut for Buffer<'_> {
    fn deref_mut(&mut self) -> &mut [i32] {
        self.as_mut_slice()
    }
}

impl std::fmt::Debug for Buffer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.as_slice())
    }
}

/// Array with unknown size — received as a native argument when the
/// Pawn signature is `array[]` without a fixed dimension.
///
/// The actual size usually comes as another parameter (`sizeof(array)`). Use
/// [`into_sized_buffer`] to convert into [`Buffer`] before iterating.
///
/// [`into_sized_buffer`]: UnsizedBuffer::into_sized_buffer
pub struct UnsizedBuffer<'amx> {
    inner: Ref<'amx, i32>,
}

impl<'amx> UnsizedBuffer<'amx> {
    /// Converts into `Buffer` by declaring the size.
    ///
    /// `len` must be <= the actual number of allocated cells — larger values
    /// cause UB when accessing cells outside the Pawn array. The SDK caps it
    /// at 1 MiB as a defense against a corrupted `len` from the script.
    #[must_use]
    pub fn into_sized_buffer(self, len: usize) -> Buffer<'amx> {
        const MAX_BUFFER_CELLS: usize = 1024 * 1024;
        debug_assert!(
            len <= MAX_BUFFER_CELLS,
            "into_sized_buffer() received len={len} above the {MAX_BUFFER_CELLS} limit"
        );
        let len = len.min(MAX_BUFFER_CELLS);
        Buffer::new(self.inner, len)
    }

    /// Pointer to the first cell.
    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const i32 {
        self.inner.as_ptr()
    }

    /// Mutable pointer to the first cell.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut i32 {
        self.inner.as_mut_ptr()
    }

    /// Constructor for tests/benchmarks — not part of the stable API.
    #[doc(hidden)]
    #[must_use]
    pub fn from_raw_parts(inner: Ref<'amx, i32>) -> Self {
        UnsizedBuffer { inner }
    }

    /// Sizes the buffer to `max_len` and writes `s` in one call.
    ///
    /// Equivalent to `into_sized_buffer(max_len).write_str(s)`. This is the
    /// recommended way to fill an output string in natives.
    ///
    /// # Errors
    /// `AmxError::General` if the encoded `s` is >= `max_len` (no room for
    /// the `0` terminator).
    pub fn write_str(self, max_len: usize, s: &str) -> AmxResult<()> {
        let mut buf = self.into_sized_buffer(max_len);
        string::put_in_buffer(&mut buf, s)
    }
}

impl<'amx> AmxCell<'amx> for UnsizedBuffer<'amx> {
    fn from_raw(amx: &'amx Amx, cell: i32) -> AmxResult<UnsizedBuffer<'amx>> {
        Ok(UnsizedBuffer {
            inner: amx.get_ref(cell)?,
        })
    }

    #[inline]
    fn as_cell(&self) -> i32 {
        self.inner.as_cell()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Ref;
    use crate::cell::repr::CellConvert;

    fn make_ref(data: &mut Vec<i32>) -> Ref<'_, i32> {
        unsafe { Ref::new(0, data.as_mut_ptr()) }
    }

    fn make_buffer(data: &mut Vec<i32>) -> Buffer<'_> {
        let len = data.len();
        let r = make_ref(data);
        Buffer::new(r, len)
    }

    fn make_unsized(data: &mut Vec<i32>) -> UnsizedBuffer<'_> {
        UnsizedBuffer {
            inner: make_ref(data),
        }
    }

    // --- Buffer ---

    #[test]
    fn buffer_len_and_is_empty() {
        let mut data = vec![0i32; 4];
        let buf = make_buffer(&mut data);
        assert_eq!(buf.len(), 4);
        assert!(!buf.is_empty());

        let mut empty = vec![];
        let empty_buf = make_buffer(&mut empty);
        assert_eq!(empty_buf.len(), 0);
        assert!(empty_buf.is_empty());
    }

    #[test]
    fn buffer_deref_reads_values() {
        let mut data = vec![10i32, 20, 30];
        let buf = make_buffer(&mut data);
        assert_eq!(&buf[..], &[10, 20, 30]);
        assert_eq!(buf[0], 10);
        assert_eq!(buf[2], 30);
    }

    #[test]
    fn buffer_deref_mut_writes_values() {
        let mut data = vec![0i32; 3];
        let mut buf = make_buffer(&mut data);
        buf[0] = 100;
        buf[1] = 200;
        buf[2] = 300;
        assert_eq!(&data, &[100, 200, 300]);
    }

    #[test]
    fn buffer_iter_works() {
        let mut data = vec![1i32, 2, 3, 4];
        let buf = make_buffer(&mut data);
        let sum: i32 = buf.iter().sum();
        assert_eq!(sum, 10);
    }

    #[test]
    fn buffer_iter_mut_modifies_in_place() {
        let mut data = vec![1i32, 2, 3];
        let mut buf = make_buffer(&mut data);
        buf.iter_mut().for_each(|x| *x *= 2);
        assert_eq!(&data, &[2, 4, 6]);
    }

    #[test]
    fn buffer_debug_format() {
        let mut data = vec![1i32, 2, 3];
        let buf = make_buffer(&mut data);
        assert_eq!(format!("{buf:?}"), "[1, 2, 3]");
    }

    #[test]
    fn buffer_as_cell_returns_amx_addr() {
        let mut data = vec![0i32; 4];
        let buf = make_buffer(&mut data);
        // as_cell() returns the AMX address of the inner Ref (0 in our helper)
        assert_eq!(buf.as_cell(), 0);
    }

    // --- UnsizedBuffer ---

    #[test]
    fn unsized_into_sized_normal_len() {
        let mut data = vec![1i32, 2, 3, 4, 5];
        let ub = make_unsized(&mut data);
        let buf = ub.into_sized_buffer(3);
        assert_eq!(buf.len(), 3);
        assert_eq!(buf[0], 1);
        assert_eq!(buf[2], 3);
    }

    /// In debug, `debug_assert!` fires for values above the limit.
    /// In release, the value is silently clamped.
    #[test]
    #[cfg_attr(
        debug_assertions,
        should_panic(expected = "into_sized_buffer() received len=")
    )]
    fn unsized_into_sized_clamps_to_max_in_release() {
        let mut data = vec![0i32; 8];
        let ub = make_unsized(&mut data);
        let buf = ub.into_sized_buffer(1024 * 1024 + 1);
        // Only reaches here in release — verifies the clamp
        assert_eq!(buf.len(), 1024 * 1024);
    }

    #[test]
    fn unsized_into_sized_at_exact_max() {
        let mut data = vec![0i32; 8];
        let ub = make_unsized(&mut data);
        let buf = ub.into_sized_buffer(1024 * 1024);
        assert_eq!(buf.len(), 1024 * 1024);
    }

    #[test]
    fn unsized_as_ptr_not_null() {
        let mut data = vec![42i32];
        let ub = make_unsized(&mut data);
        assert!(!ub.as_ptr().is_null());
    }

    #[test]
    fn unsized_as_cell_returns_amx_addr() {
        let mut data = vec![0i32];
        let ub = make_unsized(&mut data);
        assert_eq!(ub.as_cell(), 0);
    }

    // --- Buffer::get_as / set_as ---

    #[test]
    fn get_as_i32_reads_value() {
        let mut data = vec![10i32, 20, 30];
        let buf = make_buffer(&mut data);
        assert_eq!(buf.get_as::<i32>(0), Some(10));
        assert_eq!(buf.get_as::<i32>(2), Some(30));
    }

    #[test]
    fn get_as_out_of_bounds_returns_none() {
        let mut data = vec![1i32, 2];
        let buf = make_buffer(&mut data);
        assert_eq!(buf.get_as::<i32>(2), None);
        assert_eq!(buf.get_as::<i32>(99), None);
    }

    #[test]
    fn set_as_i32_writes_value() {
        let mut data = vec![0i32; 3];
        let mut buf = make_buffer(&mut data);
        assert!(buf.set_as(1, 42i32));
        assert_eq!(data[1], 42);
    }

    #[test]
    fn set_as_out_of_bounds_returns_false() {
        let mut data = vec![0i32; 2];
        let mut buf = make_buffer(&mut data);
        assert!(!buf.set_as(5, 99i32));
    }

    #[test]
    fn get_as_f32_roundtrip() {
        let value = 1.5f32; // exact IEEE-754 value, no approx_constant risk
        let mut data = vec![value.into_cell()];
        let buf = make_buffer(&mut data);
        let recovered: f32 = buf.get_as::<f32>(0).unwrap();
        assert!(
            (recovered - value).abs() < f32::EPSILON,
            "f32 roundtrip failed: {recovered} != {value}"
        );
    }

    #[test]
    fn set_as_f32_stores_bits_correctly() {
        let mut data = vec![0i32];
        let mut buf = make_buffer(&mut data);
        buf.set_as(0, 1.5f32);
        assert_eq!(data[0].cast_unsigned(), 1.5f32.to_bits());
    }

    #[test]
    fn get_as_bool_true_and_false() {
        let mut data = vec![1i32, 0, 42];
        let buf = make_buffer(&mut data);
        assert_eq!(buf.get_as::<bool>(0), Some(true));
        assert_eq!(buf.get_as::<bool>(1), Some(false));
        // any non-zero value is true
        assert_eq!(buf.get_as::<bool>(2), Some(true));
    }

    #[test]
    fn set_as_bool_writes_zero_and_one() {
        let mut data = vec![0i32; 2];
        let mut buf = make_buffer(&mut data);
        buf.set_as(0, true);
        buf.set_as(1, false);
        assert_eq!(data[0], 1);
        assert_eq!(data[1], 0);
    }

    #[test]
    fn get_as_u8_reads_byte() {
        let mut data = vec![255i32];
        let buf = make_buffer(&mut data);
        assert_eq!(buf.get_as::<u8>(0), Some(255u8));
    }

    // --- Buffer::iter_as ---

    #[test]
    fn iter_as_i32_collects_all() {
        let mut data = vec![1i32, 2, 3, 4];
        let buf = make_buffer(&mut data);
        let vals: Vec<i32> = buf.iter_as::<i32>().collect();
        assert_eq!(vals, vec![1, 2, 3, 4]);
    }

    #[test]
    fn iter_as_i32_sum() {
        let mut data = vec![10i32, 20, 30];
        let buf = make_buffer(&mut data);
        let sum: i32 = buf.iter_as::<i32>().sum();
        assert_eq!(sum, 60);
    }

    #[test]
    fn iter_as_f32_roundtrip() {
        let values = [1.0f32, 2.5, 1.25];
        let mut data: Vec<i32> = values.iter().map(|&v| v.into_cell()).collect();
        let buf = make_buffer(&mut data);
        let recovered: Vec<f32> = buf.iter_as::<f32>().collect();
        for (orig, got) in values.iter().zip(recovered.iter()) {
            assert!((orig - got).abs() < f32::EPSILON, "{orig} != {got}");
        }
    }

    #[test]
    fn iter_as_bool_any_and_all() {
        let mut data = vec![1i32, 0, 1, 1];
        let buf = make_buffer(&mut data);
        assert!(buf.iter_as::<bool>().any(|v| !v));
        assert!(!buf.iter_as::<bool>().all(|v| v));
    }

    #[test]
    fn iter_as_empty_buffer() {
        let mut data: Vec<i32> = vec![];
        let buf = make_buffer(&mut data);
        assert_eq!(buf.iter_as::<i32>().count(), 0);
    }

    #[test]
    fn iter_as_matches_get_as_loop() {
        let mut data = vec![10i32, 20, 30, 40];
        let buf = make_buffer(&mut data);
        let via_iter: Vec<i32> = buf.iter_as::<i32>().collect();
        let via_loop: Vec<i32> = (0..buf.len())
            .filter_map(|i| buf.get_as::<i32>(i))
            .collect();
        assert_eq!(via_iter, via_loop);
    }

    // --- Buffer::write_str ---

    #[test]
    fn write_str_encodes_string_into_cells() {
        // "hi" -> cells [104, 105, 0] (h=104, i=105, nul=0)
        let mut data = vec![0i32; 3];
        let mut buf = make_buffer(&mut data);
        assert!(buf.write_str("hi").is_ok());
        assert_eq!(data[0], i32::from(b'h'));
        assert_eq!(data[1], i32::from(b'i'));
        assert_eq!(data[2], 0); // null terminator
    }

    #[test]
    fn write_str_empty_string_writes_null_terminator() {
        let mut data = vec![99i32; 2];
        let mut buf = make_buffer(&mut data);
        assert!(buf.write_str("").is_ok());
        assert_eq!(data[0], 0);
    }

    #[test]
    fn write_str_exact_fit_fails() {
        // A 3-cell buffer cannot hold "abc" (it would need 4: a, b, c, nul)
        let mut data = vec![0i32; 3];
        let mut buf = make_buffer(&mut data);
        assert!(buf.write_str("abc").is_err());
    }

    // --- UnsizedBuffer::write_str ---

    #[test]
    fn unsized_write_str_sizes_and_writes() {
        let mut data = vec![0i32; 5];
        let ub = make_unsized(&mut data);
        assert!(ub.write_str(5, "hi").is_ok());
        assert_eq!(data[0], i32::from(b'h'));
        assert_eq!(data[1], i32::from(b'i'));
        assert_eq!(data[2], 0);
    }

    #[test]
    fn unsized_write_str_too_long_returns_err() {
        let mut data = vec![0i32; 3];
        let ub = make_unsized(&mut data);
        assert!(ub.write_str(3, "abc").is_err());
    }
}
