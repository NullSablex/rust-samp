//! AMX strings: cell vector with `0` terminator.
//!
//! Pawn supports two binary representations:
//!
//! - **Unpacked**: 1 character per cell (4x memory usage, default).
//! - **Packed**: 4 characters packed into each i32 cell (bits 31..24,
//!   23..16, 15..8, 7..0). The first cell signals the mode if its value
//!   exceeds [`MAX_UNPACKED`]; the SDK detects it automatically in [`to_bytes`].
//!
//! [`to_bytes`]: AmxString::to_bytes

use std::cell::OnceCell;
use std::fmt;
use std::ops::Deref;

use super::{AmxCell, Buffer, UnsizedBuffer};
use crate::amx::Amx;
#[cfg(feature = "encoding")]
use crate::encoding;
use crate::error::AmxResult;

/// Upper bound for the first cell of an unpacked string.
///
/// Values above this indicate a packed string (4 chars/cell).
const MAX_UNPACKED: i32 = 0x00FF_FFFF;

/// Native Pawn string — packed or unpacked.
///
/// Implements [`Deref<Target = str>`], so `&str` methods are available
/// directly, without `.to_string()`:
///
/// ```no_run
/// # use samp_sdk::cell::AmxString;
/// # use samp_sdk::amx::Amx;
/// # use samp_sdk::error::AmxResult;
/// # struct Plugin;
/// # impl Plugin {
/// fn greet(&self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
///     if name.starts_with("Admin") {
///         println!("Welcome, {}!", &*name);
///     }
///     Ok(true)
/// }
/// # }
/// ```
///
/// The decoded version (UTF-8 or Windows-1251 via the `encoding` feature) is
/// computed on the first `Deref` call and cached — subsequent accesses
/// return the `&str` without allocation.
pub struct AmxString<'amx> {
    inner: Buffer<'amx>,
    len: usize,
    decoded: OnceCell<String>,
}

impl<'amx> AmxString<'amx> {
    /// Creates an `AmxString` from an allocated buffer and copies `bytes` (1 byte
    /// per cell) with a trailing `0` terminator.
    ///
    /// # Safety
    /// `buffer` must have at least `bytes.len() + 1` cells and remain
    /// alive for `'amx`.
    #[must_use]
    pub unsafe fn new(mut buffer: Buffer<'amx>, bytes: &[u8]) -> AmxString<'amx> {
        buffer.as_mut_slice()[..bytes.len()]
            .iter_mut()
            .zip(bytes)
            .for_each(|(cell, &byte)| *cell = i32::from(byte));
        buffer[bytes.len()] = 0;

        AmxString {
            len: bytes.len(),
            inner: buffer,
            decoded: OnceCell::new(),
        }
    }

    /// Constructor for tests/benchmarks — assumes `inner` is already populated.
    /// Not part of the stable API.
    #[doc(hidden)]
    #[must_use]
    pub fn from_buffer_parts(inner: Buffer<'amx>, len: usize) -> AmxString<'amx> {
        AmxString {
            inner,
            len,
            decoded: OnceCell::new(),
        }
    }

    /// Decodes the cells back into a `Vec<u8>`.
    ///
    /// Automatically detects packed (4 chars/cell) or unpacked (1 char/cell)
    /// from the value of the first cell. Caps the read at 1 MiB to avoid
    /// uncontrolled allocation if `len` is corrupted.
    pub fn to_bytes(&self) -> Vec<u8> {
        const MAX_STRING_LEN: usize = 1024 * 1024;
        let len = self.len.min(MAX_STRING_LEN);
        let mut vec = Vec::with_capacity(len);

        // packed string
        if self.inner[0] > MAX_UNPACKED {
            let cells = self.inner.as_slice();
            let max_cells = cells.len();
            let mut cell_idx = 0usize;
            let mut mark = 3usize;
            for _ in 0..len {
                if cell_idx >= max_cells {
                    break;
                }
                // Byte extraction from a packed i32 cell — truncation is intentional.
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let ch = (cells[cell_idx] >> (mark * 8)) as u8;
                if ch == b'\0' {
                    break;
                }
                vec.push(ch);
                mark = (mark + 3) % 4;
                if mark == 3 {
                    cell_idx += 1;
                }
            }
        } else {
            for item in self.inner.iter().take(len) {
                // An unpacked cell holds a single byte; truncation is intentional.
                #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
                let byte = *item as u8;
                vec.push(byte);
            }
        }

        vec
    }

    /// String length in characters (excluding the `0` terminator).
    pub fn len(&self) -> usize {
        self.len
    }

    /// `true` if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Size of the underlying buffer in cells — always `>= len + 1`.
    pub fn bytes_len(&self) -> usize {
        self.inner.len()
    }

    /// Explicit form of the `Deref` to `&str`.
    ///
    /// Useful when type inference does not trigger auto-deref (e.g. a generic
    /// context with `T: AsRef<str>`).
    pub fn as_str(&self) -> &str {
        self
    }
}

/// Decodes the raw bytes using the configured encoding (UTF-8 by default;
/// Windows-1251 etc. via the `encoding` feature).
fn decode_bytes(bytes: &[u8]) -> String {
    #[cfg(feature = "encoding")]
    return encoding::get().decode(bytes).0.into_owned();

    #[cfg(not(feature = "encoding"))]
    return String::from_utf8_lossy(bytes).into_owned();
}

impl<'amx> AmxCell<'amx> for AmxString<'amx> {
    fn from_raw(amx: &'amx Amx, cell: i32) -> AmxResult<AmxString<'amx>> {
        let buffer = UnsizedBuffer::from_raw(amx, cell)?;
        let ptr = buffer.as_ptr();
        let str_len = amx.strlen(ptr)?;
        let buf_len = str_len + 1;

        Ok(AmxString {
            inner: buffer.into_sized_buffer(buf_len),
            len: str_len,
            decoded: OnceCell::new(),
        })
    }

    fn as_cell(&self) -> i32 {
        self.inner.as_cell()
    }
}

impl Deref for AmxString<'_> {
    type Target = str;

    /// Decodes on the first call and caches in [`OnceCell`] — subsequent
    /// accesses return the same `&str` without allocation.
    fn deref(&self) -> &str {
        self.decoded.get_or_init(|| decode_bytes(&self.to_bytes()))
    }
}

impl fmt::Display for AmxString<'_> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.write_str(self)
    }
}

impl PartialEq<str> for AmxString<'_> {
    /// Direct comparison with `&str` (`name == "Admin"`) — no extra allocation.
    fn eq(&self, other: &str) -> bool {
        &**self == other
    }
}

impl PartialEq<&str> for AmxString<'_> {
    fn eq(&self, other: &&str) -> bool {
        &**self == *other
    }
}

impl PartialEq<String> for AmxString<'_> {
    fn eq(&self, other: &String) -> bool {
        &**self == other.as_str()
    }
}

/// Copies a Rust string into an AMX `Buffer` (1 byte per cell, `0`
/// terminator at the end).
///
/// Internal implementation shared by [`Buffer::write_str`] and
/// [`UnsizedBuffer::write_str`] — the public API goes through them.
///
/// [`Buffer::write_str`]: crate::cell::buffer::Buffer::write_str
/// [`UnsizedBuffer::write_str`]: crate::cell::buffer::UnsizedBuffer::write_str
///
/// # Errors
/// `AmxError::General` if `string` (after encoding) is >= the buffer size.
pub(crate) fn put_in_buffer(buffer: &mut Buffer, string: &str) -> AmxResult<()> {
    #[cfg(feature = "encoding")]
    let bytes = encoding::get().encode(string).0;

    #[cfg(not(feature = "encoding"))]
    let bytes = std::borrow::Cow::from(string.as_bytes());

    let bytes = bytes.as_ref();

    if bytes.len() >= buffer.len() {
        return Err(crate::error::AmxError::General);
    }

    buffer.as_mut_slice()[..bytes.len()]
        .iter_mut()
        .zip(bytes)
        .for_each(|(cell, &byte)| *cell = i32::from(byte));

    buffer[bytes.len()] = 0;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Ref;

    fn make_buffer(data: &mut Vec<i32>) -> Buffer<'_> {
        let len = data.len();
        let r = unsafe { Ref::new(0, data.as_mut_ptr()) };
        Buffer::new(r, len)
    }

    // --- Unpacked strings (one byte per cell) ---

    #[test]
    fn new_empty_string() {
        let mut data = vec![0i32; 4];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"") };
        assert!(s.is_empty());
        assert_eq!(s.len(), 0);
        assert_eq!(&*s, "");
        assert_eq!(s.to_bytes(), b"");
    }

    #[test]
    fn new_ascii_string() {
        let mut data = vec![0i32; 16];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"hello") };
        assert_eq!(s.len(), 5);
        assert_eq!(&*s, "hello");
        assert_eq!(s.to_bytes(), b"hello");
        assert!(!s.is_empty());
    }

    #[test]
    fn deref_str_enables_string_methods() {
        let mut data = vec![0i32; 32];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"hello world") };
        // &str methods without .to_string()
        assert!(s.contains("world"));
        assert!(s.starts_with("hello"));
        assert!(s.ends_with("world"));
        assert_eq!(s.to_uppercase(), "HELLO WORLD");
        assert_eq!(s.split_once(' ').unwrap(), ("hello", "world"));
    }

    #[test]
    fn deref_is_lazy_and_cached() {
        let mut data = vec![0i32; 16];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"world") };
        // OnceCell has not been initialized yet
        assert!(s.decoded.get().is_none());
        // First access via Deref -> initializes
        let _ = &*s;
        assert!(s.decoded.get().is_some());
        // Second access -> same pointer (cache hit)
        let a = s.decoded.get().unwrap().as_ptr();
        let _ = &*s;
        let b = s.decoded.get().unwrap().as_ptr();
        assert_eq!(a, b);
    }

    #[test]
    fn display_and_deref_are_consistent() {
        let mut data = vec![0i32; 16];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"world") };
        assert_eq!(s.to_string(), "world");
        assert_eq!(&*s, "world");
        assert_eq!(format!("{s}"), "world");
    }

    #[test]
    fn bytes_len_reflects_buffer_size() {
        let mut data = vec![0i32; 8];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"abc") };
        assert_eq!(s.bytes_len(), 8);
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn unpacked_to_bytes_ascii() {
        let text = b"SA-MP Plugin";
        let mut data: Vec<i32> = text
            .iter()
            .map(|&b| i32::from(b))
            .chain(std::iter::once(0))
            .collect();
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, text) };
        assert_eq!(s.to_bytes(), text);
    }

    #[test]
    fn unpacked_single_char() {
        let mut data = vec![0x41i32, 0];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"A") };
        assert_eq!(s.len(), 1);
        assert_eq!(&*s, "A");
    }

    // --- Packed strings (4 bytes per cell) ---
    //
    // Bytes read from each cell: bits[31..24], [23..16], [15..8], [7..0].
    // "ABCD" -> cell = 0x41424344, next cell = 0x00000000 (null)

    #[test]
    fn packed_four_chars_one_cell() {
        let mut data = vec![0x4142_4344i32, 0x0000_0000i32];
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 4);
        assert_eq!(s.to_bytes(), b"ABCD");
        assert_eq!(&*s, "ABCD");
    }

    #[test]
    fn packed_five_chars_two_cells() {
        // "ABCDE": 4 chars in cell[0], 1 in cell[1]
        let mut data = vec![0x4142_4344i32, 0x4500_0000i32, 0x0000_0000i32];
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 5);
        assert_eq!(s.to_bytes(), b"ABCDE");
        assert_eq!(&*s, "ABCDE");
    }

    #[test]
    fn packed_truncates_at_len() {
        let mut data = vec![0x4142_4344i32, 0x0000_0000i32];
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 2);
        assert_eq!(s.to_bytes(), b"AB");
    }

    #[test]
    fn packed_stops_at_null_byte() {
        // "AB\0D" -> stops at \0, returns "AB"
        let mut data = vec![0x4142_0044i32, 0x0000_0000i32];
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 4);
        assert_eq!(s.to_bytes(), b"AB");
    }

    // --- as_str ---

    #[test]
    fn as_str_returns_decoded() {
        let mut data = vec![0i32; 16];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"hello") };
        assert_eq!(s.as_str(), "hello");
    }

    #[test]
    fn as_str_and_deref_are_same_pointer() {
        let mut data = vec![0i32; 16];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"rust") };
        // Both trigger the same OnceCell — same &str pointer
        let a: &str = s.as_str();
        let b: &str = &s;
        assert_eq!(a.as_ptr(), b.as_ptr());
    }

    // --- PartialEq ---

    #[test]
    fn partial_eq_str_literal() {
        let mut data = vec![0i32; 16];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"Admin") };
        assert!(s == "Admin");
        assert!(s != "admin");
    }

    #[test]
    fn partial_eq_ref_str() {
        let mut data = vec![0i32; 16];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"samp") };
        let key: &str = "samp";
        assert!(s == key);
    }

    #[test]
    fn partial_eq_string() {
        let mut data = vec![0i32; 16];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"plugin") };
        let owned_match: String = "plugin".to_string();
        let owned_other: String = "other".to_string();
        assert!(s == owned_match);
        assert!(s != owned_other);
    }

    #[test]
    fn partial_eq_empty() {
        let mut data = vec![0i32; 4];
        let buf = make_buffer(&mut data);
        let s = unsafe { AmxString::new(buf, b"") };
        assert!(s.is_empty());
        assert!(s != "x");
    }

    // --- put_in_buffer ---

    #[test]
    fn put_in_buffer_writes_correctly() {
        let mut data = vec![0i32; 16];
        let mut buf = make_buffer(&mut data);
        put_in_buffer(&mut buf, "hello").unwrap();
        assert_eq!(buf[0], i32::from(b'h'));
        assert_eq!(buf[4], i32::from(b'o'));
        assert_eq!(buf[5], 0);
    }

    #[test]
    fn put_in_buffer_exact_fit_fails() {
        let mut data = vec![0i32; 5];
        let mut buf = make_buffer(&mut data);
        assert!(put_in_buffer(&mut buf, "hello").is_err());
    }

    #[test]
    fn put_in_buffer_empty_string() {
        let mut data = vec![0i32; 4];
        let mut buf = make_buffer(&mut data);
        put_in_buffer(&mut buf, "").unwrap();
        assert_eq!(buf[0], 0);
    }
}
