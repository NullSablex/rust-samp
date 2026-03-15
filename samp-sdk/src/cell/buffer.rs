//! Contains types to interact with AMX arrays.
use std::ops::{Deref, DerefMut};

use super::{AmxCell, Ref};
use crate::amx::Amx;
use crate::cell::repr::CellConvert;
use crate::cell::string;
use crate::error::AmxResult;

/// Contains a pointer to a sequence of `Amx` cells.
///
/// Can be dereferenced to a [`slice`].
///
/// # Example
/// ```
/// use samp_sdk::cell::{UnsizedBuffer, Buffer};
/// # use samp_sdk::amx::Amx;
///
/// // native: IGiveYouABuffer(buffer[]);
/// fn it_gave_me_a_buffer(amx: &Amx, buffer: UnsizedBuffer, size: usize) {
///     let mut buffer: Buffer = buffer.into_sized_buffer(size);
///     println!("Got {:?}", buffer);
///     buffer.iter_mut().for_each(|elem| *elem *= 2);
///     println!("Changed to {:?}", buffer);
/// }
/// ```
///
/// [`slice`]: https://doc.rust-lang.org/std/primitive.slice.html
pub struct Buffer<'amx> {
    inner: Ref<'amx, i32>,
    len: usize,
}

impl<'amx> Buffer<'amx> {
    /// Create a buffer from a reference to its first element.
    pub fn new(reference: Ref<'amx, i32>, len: usize) -> Buffer<'amx> {
        Buffer {
            inner: reference,
            len,
        }
    }

    /// Return the number of cells in the buffer.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return `true` if the buffer has zero cells.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Extracts a slice containing the entire buffer.
    #[inline]
    pub fn as_slice(&self) -> &[i32] {
        unsafe { std::slice::from_raw_parts(self.inner.as_ptr(), self.len) }
    }

    /// Extracts a mutable slice of the entire buffer.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [i32] {
        unsafe { std::slice::from_raw_parts_mut(self.inner.as_mut_ptr(), self.len) }
    }

    /// Returns an iterator that converts each cell to type `T`.
    ///
    /// This is the idiomatic way to process typed arrays from Pawn —
    /// use it with standard iterator adapters like `sum`, `filter_map`, `map`, etc.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use samp_sdk::cell::Buffer;
    /// fn sum_floats(buf: &Buffer) -> f32 {
    ///     buf.iter_as::<f32>().sum()
    /// }
    ///
    /// fn any_flag_set(buf: &Buffer) -> bool {
    ///     buf.iter_as::<bool>().any(|v| v)
    /// }
    /// ```
    pub fn iter_as<T: CellConvert>(&self) -> impl Iterator<Item = T> + '_ {
        self.as_slice().iter().map(|&raw| T::from_cell(raw))
    }

    /// Read a cell at `index` and convert it to type `T`.
    ///
    /// Returns `None` if `index` is out of bounds.
    ///
    /// This is the ergonomic way to read typed values (e.g. `f32`, `bool`) from
    /// a Pawn array without manual bit manipulation.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use samp_sdk::cell::{Buffer, CellConvert};
    /// fn sum_floats(buf: &Buffer) -> f32 {
    ///     (0..buf.len())
    ///         .filter_map(|i| buf.get_as::<f32>(i))
    ///         .sum()
    /// }
    /// ```
    pub fn get_as<T: CellConvert>(&self, index: usize) -> Option<T> {
        self.as_slice().get(index).map(|&raw| T::from_cell(raw))
    }

    /// Convert `value` to a raw cell and write it at `index`.
    ///
    /// Returns `true` if `index` is within bounds and the write succeeded,
    /// `false` otherwise.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use samp_sdk::cell::{Buffer, CellConvert};
    /// fn fill_bools(buf: &mut Buffer, flag: bool) {
    ///     for i in 0..buf.len() {
    ///         buf.set_as(i, flag);
    ///     }
    /// }
    /// ```
    pub fn set_as<T: CellConvert>(&mut self, index: usize, value: T) -> bool {
        if let Some(cell) = self.as_mut_slice().get_mut(index) {
            *cell = value.into_cell();
            true
        } else {
            false
        }
    }

    /// Write a string into this buffer.
    ///
    /// Encodes `s` and stores it cell-by-cell (unpacked format).
    /// The buffer must have at least `s.len() + 1` cells.
    ///
    /// # Errors
    /// Returns `AmxError::General` if the string is too long for the buffer.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use samp_sdk::amx::Amx;
    /// # use samp_sdk::error::AmxResult;
    /// # fn example(amx: &Amx) -> AmxResult<()> {
    /// let allocator = amx.allocator();
    /// let mut buf = allocator.allot_buffer(32)?;
    /// buf.write_str("Hello, SA-MP!")?;
    /// # Ok(()) }
    /// ```
    pub fn write_str(&mut self, s: &str) -> AmxResult<()> {
        string::put_in_buffer(self, s)
    }
}

// Buffer cannot be parsed from a cell directly — it must be sized first via UnsizedBuffer.
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

/// A buffer whose length is not yet known — comes directly from an AMX native call.
///
/// Must be converted to a [`Buffer`] via [`into_sized_buffer`] before use.
///
/// # Example
/// ```
/// use samp_sdk::cell::UnsizedBuffer;
/// # use samp_sdk::amx::Amx;
/// # use samp_sdk::error::AmxResult;
///
/// fn zero_array(amx: &Amx, array: UnsizedBuffer, length: usize) -> AmxResult<u32> {
///     let mut array = array.into_sized_buffer(length);
///     array.iter_mut().for_each(|cell| *cell = 0);
///     Ok(1)
/// }
/// ```
///
/// [`into_sized_buffer`]: UnsizedBuffer::into_sized_buffer
pub struct UnsizedBuffer<'amx> {
    inner: Ref<'amx, i32>,
}

impl<'amx> UnsizedBuffer<'amx> {
    /// Convert `UnsizedBuffer` into a `Buffer` with the given length.
    ///
    /// `len` must not exceed the actual number of cells allocated in the AMX heap.
    /// Passing a larger value causes undefined behavior. The maximum allowed is 1MB.
    ///
    /// # Example
    /// ```
    /// use samp_sdk::cell::UnsizedBuffer;
    /// # use samp_sdk::amx::Amx;
    ///
    /// fn push_ones(amx: &Amx, array: UnsizedBuffer, length: usize) {
    ///     let mut buffer = array.into_sized_buffer(length);
    ///     buffer.iter_mut().for_each(|item| *item = 1);
    /// }
    /// ```
    pub fn into_sized_buffer(self, len: usize) -> Buffer<'amx> {
        const MAX_BUFFER_CELLS: usize = 1024 * 1024;
        debug_assert!(
            len <= MAX_BUFFER_CELLS,
            "into_sized_buffer() recebeu len={} acima do limite de {}",
            len,
            MAX_BUFFER_CELLS
        );
        let len = len.min(MAX_BUFFER_CELLS);
        Buffer::new(self.inner, len)
    }

    /// Return a raw pointer to the first cell.
    #[inline]
    pub fn as_ptr(&self) -> *const i32 {
        self.inner.as_ptr()
    }

    /// Return a mutable raw pointer to the first cell.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut i32 {
        self.inner.as_mut_ptr()
    }

    /// Internal helper for tests and benchmarks — not a stable API.
    #[doc(hidden)]
    pub fn from_raw_parts(inner: Ref<'amx, i32>) -> Self {
        UnsizedBuffer { inner }
    }

    /// Write a string into this buffer after sizing it to `max_len` cells.
    ///
    /// Combines [`into_sized_buffer`] and [`Buffer::write_str`] in one call.
    /// This is the idiomatic way to write an output string in a native function.
    ///
    /// # Errors
    /// Returns `AmxError::General` if `s` is too long for `max_len`.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use samp_sdk::amx::Amx;
    /// # use samp_sdk::cell::UnsizedBuffer;
    /// # use samp_sdk::error::AmxResult;
    ///
    /// // native: GetValue(buffer[], max_len);
    /// fn get_value(_amx: &Amx, buffer: UnsizedBuffer, max_len: usize) -> AmxResult<bool> {
    ///     buffer.write_str(max_len, "my value")?;
    ///     Ok(true)
    /// }
    /// ```
    ///
    /// [`into_sized_buffer`]: UnsizedBuffer::into_sized_buffer
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
    use crate::cell::repr::CellConvert;
    use crate::cell::Ref;

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
        // as_cell() devolve o endereço AMX do Ref interno (0 no nosso helper)
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

    /// Em debug, `debug_assert!` dispara para valores acima do limite.
    /// Em release, o valor é silenciosamente clampeado.
    #[test]
    #[cfg_attr(debug_assertions, should_panic)]
    fn unsized_into_sized_clamps_to_max_in_release() {
        let mut data = vec![0i32; 8];
        let ub = make_unsized(&mut data);
        let buf = ub.into_sized_buffer(1024 * 1024 + 1);
        // Só chega aqui em release — verifica o clamp
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
        let value = 1.5f32; // valor exato em IEEE-754, sem risco de approx_constant
        let mut data = vec![value.into_cell()];
        let buf = make_buffer(&mut data);
        let recovered: f32 = buf.get_as::<f32>(0).unwrap();
        assert!((recovered - value).abs() < f32::EPSILON, "f32 roundtrip falhou: {recovered} != {value}");
    }

    #[test]
    fn set_as_f32_stores_bits_correctly() {
        let mut data = vec![0i32];
        let mut buf = make_buffer(&mut data);
        buf.set_as(0, 1.5f32);
        assert_eq!(f32::from_bits(data[0] as u32), 1.5f32);
    }

    #[test]
    fn get_as_bool_true_and_false() {
        let mut data = vec![1i32, 0, 42];
        let buf = make_buffer(&mut data);
        assert_eq!(buf.get_as::<bool>(0), Some(true));
        assert_eq!(buf.get_as::<bool>(1), Some(false));
        // qualquer valor não-zero é true
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
        let via_loop: Vec<i32> = (0..buf.len()).filter_map(|i| buf.get_as::<i32>(i)).collect();
        assert_eq!(via_iter, via_loop);
    }
}
