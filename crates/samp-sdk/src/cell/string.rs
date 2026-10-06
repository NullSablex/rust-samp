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
use crate::error::{AmxError, AmxResult};

/// Upper bound for the first cell of an unpacked string.
///
/// Values above this indicate a packed string (4 chars/cell). The comparison
/// is unsigned, as in the server (`(ucell)*cstr > UNPACKEDMAX`): a packed
/// string whose first byte is 0x80 or above has a negative first cell.
const MAX_UNPACKED: u32 = 0x00FF_FFFF;

/// Longest string read from a script, in cells.
const MAX_STRING_CELLS: usize = 1024 * 1024;

fn is_packed(first: i32) -> bool {
    first.cast_unsigned() > MAX_UNPACKED
}

/// Length of the string in `cells` (bytes when packed, cells when not), or
/// `None` when no terminator comes before the end of `cells`.
///
/// Replaces the server's `amx_StrLen`, which has no bound: given an address
/// near the top of the stack it walks past the end of the AMX memory.
fn bounded_strlen(cells: &[i32]) -> Option<usize> {
    if is_packed(*cells.first()?) {
        let at = find_cell(cells, has_zero_byte)?;
        let lead = cells[at].to_be_bytes().iter().position(|&byte| byte == 0)?;
        Some(at * 4 + lead)
    } else {
        find_cell(cells, |cell| cell == 0)
    }
}

/// Index of the first cell for which `ends` holds.
///
/// Whole blocks are tested without an early exit, which the compiler turns
/// into vector code; only the block holding the match is searched cell by
/// cell. A cell-by-cell search from the start cannot be vectorized and costs
/// several times more on a long string.
fn find_cell(cells: &[i32], ends: impl Fn(i32) -> bool + Copy) -> Option<usize> {
    const BLOCK: usize = 16;
    let block = cells
        .chunks(BLOCK)
        .position(|block| block.iter().fold(false, |found, &cell| found | ends(cell)))?;
    let start = block * BLOCK;
    Some(start + cells[start..].iter().position(|&cell| ends(cell))?)
}

/// Whether any of the four bytes of `cell` is zero, without looking at them
/// one by one.
fn has_zero_byte(cell: i32) -> bool {
    let bits = cell.cast_unsigned();
    bits.wrapping_sub(0x0101_0101) & !bits & 0x8080_8080 != 0
}

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
        // An empty backing buffer has no first cell to probe for the
        // packed/unpacked marker — return early instead of indexing `[0]`
        // (which would panic). Reachable only via a corrupted length.
        if self.inner.is_empty() {
            return Vec::new();
        }
        let len = self.len.min(MAX_STRING_LEN);
        let cells = self.inner.as_slice();

        if is_packed(cells[0]) {
            // Four bytes per cell, the first in the high byte. A cell with no
            // zero byte goes in whole; the one holding the terminator, or
            // reaching `len`, byte by byte.
            let mut vec = Vec::with_capacity(len);
            for &cell in cells {
                let bytes = cell.to_be_bytes();
                if !has_zero_byte(cell) && vec.len() + 4 <= len {
                    vec.extend_from_slice(&bytes);
                    continue;
                }
                let room = len - vec.len();
                vec.extend(bytes.into_iter().take(room).take_while(|&byte| byte != 0));
                break;
            }
            vec
        } else {
            // One byte per cell. A single pass the compiler vectorizes.
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            cells[..len.min(cells.len())]
                .iter()
                .map(|&cell| cell as u8)
                .collect()
        }
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
///
/// Takes the bytes by value: when they are already the UTF-8 the result needs
/// — ASCII, or valid UTF-8 — the `String` reuses their allocation instead of
/// copying them.
fn decode_bytes(bytes: Vec<u8>) -> String {
    // Without BOM sniffing: the bytes are a Pawn string, often typed by a
    // player, and one starting with FF FE must not switch to UTF-16.
    #[cfg(feature = "encoding")]
    if let std::borrow::Cow::Owned(text) = encoding::get().decode_without_bom_handling(&bytes).0 {
        return text;
    }
    // Valid UTF-8 as it stands (always so when the encoding borrowed it).
    String::from_utf8(bytes)
        .unwrap_or_else(|invalid| String::from_utf8_lossy(invalid.as_bytes()).into_owned())
}

impl<'amx> AmxCell<'amx> for AmxString<'amx> {
    fn from_raw(amx: &'amx Amx, cell: i32) -> AmxResult<AmxString<'amx>> {
        let buffer = UnsizedBuffer::from_raw(amx, cell)?;
        let str_len = match buffer.max_cells() {
            // Null VM (no `stp` to bound by): only the server can tell.
            usize::MAX => amx.strlen(buffer.as_ptr())?,
            max_cells => {
                // SAFETY: `[cell, stp)` is AMX memory, alive for `'amx`.
                let cells = unsafe {
                    std::slice::from_raw_parts(buffer.as_ptr(), max_cells.min(MAX_STRING_CELLS))
                };
                bounded_strlen(cells).ok_or(AmxError::MemoryAccess)?
            }
        };
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
        self.decoded.get_or_init(|| decode_bytes(self.to_bytes()))
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
    put_in_buffer_checked(buffer, string).map(|_| ())
}

/// Same as [`put_in_buffer`], reporting whether the encoding had to substitute
/// characters it could not represent.
pub(crate) fn put_in_buffer_checked(buffer: &mut Buffer, string: &str) -> AmxResult<bool> {
    #[cfg(feature = "encoding")]
    let (bytes, had_unmappable) = encoding::encode_checked(string);

    // Without the feature the bytes are the string's own UTF-8: nothing to map,
    // so nothing can be lost.
    #[cfg(not(feature = "encoding"))]
    let (bytes, had_unmappable) = (std::borrow::Cow::from(string.as_bytes()), false);

    let bytes = bytes.as_ref();

    if bytes.len() >= buffer.len() {
        return Err(crate::error::AmxError::General);
    }

    buffer.as_mut_slice()[..bytes.len()]
        .iter_mut()
        .zip(bytes)
        .for_each(|(cell, &byte)| *cell = i32::from(byte));

    buffer[bytes.len()] = 0;

    Ok(had_unmappable)
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

    // --- Length without the server's `amx_StrLen` ---

    #[test]
    fn bounded_strlen_stops_at_the_terminator() {
        assert_eq!(bounded_strlen(&[0x41, 0x42, 0, 0x43]), Some(2));
        assert_eq!(bounded_strlen(&[0x4142_4300]), Some(3));
        assert_eq!(bounded_strlen(&[0x4142_4344, 0x4500_0000]), Some(5));
        assert_eq!(bounded_strlen(&[0]), Some(0));
        // Past the first block of cells, and a terminator on a block edge.
        let mut long = vec![0x41; 40];
        long[37] = 0;
        assert_eq!(bounded_strlen(&long), Some(37));
        long[16] = 0;
        assert_eq!(bounded_strlen(&long), Some(16));
        let mut packed = vec![0x4142_4344; 40];
        packed[20] = 0x4142_0044;
        assert_eq!(bounded_strlen(&packed), Some(20 * 4 + 2));
    }

    #[test]
    fn bounded_strlen_agrees_with_a_byte_by_byte_search() {
        fn naive(cells: &[i32]) -> Option<usize> {
            let first = *cells.first()?;
            if first.cast_unsigned() > 0x00FF_FFFF {
                let bytes = cells.iter().flat_map(|cell| cell.to_be_bytes());
                bytes
                    .enumerate()
                    .find(|&(_, byte)| byte == 0)
                    .map(|(at, _)| at)
            } else {
                cells.iter().position(|&cell| cell == 0)
            }
        }
        // A small linear congruential generator: deterministic, no dependency.
        let mut state = 0x2545_f491_u32;
        let mut next = move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state
        };
        for _ in 0..5000 {
            let len = (next() % 70) as usize;
            let packed = next() % 2 == 0;
            let mut cells: Vec<i32> = (0..len)
                .map(|_| {
                    let value = next() | 0x0101_0101;
                    if packed {
                        value.cast_signed()
                    } else {
                        (value & 0xFF).cast_signed()
                    }
                })
                .collect();
            if len > 0 && next() % 4 != 0 {
                let at = (next() as usize) % len;
                if packed {
                    let byte = (next() % 4) as usize;
                    let mut bytes = cells[at].to_be_bytes();
                    bytes[byte] = 0;
                    cells[at] = i32::from_be_bytes(bytes);
                } else {
                    cells[at] = 0;
                }
            }
            assert_eq!(bounded_strlen(&cells), naive(&cells), "{cells:x?}");
        }
    }

    #[test]
    fn zero_bytes_are_found_in_any_position() {
        for cell in [
            0x0041_4243,
            0x4100_4243,
            0x4142_0043,
            0x4142_4300,
            0x0000_0000,
        ] {
            assert!(has_zero_byte(cell), "{cell:#x}");
        }
        for cell in [0x4142_4344, 0x0101_0101, 0x8080_8080_u32.cast_signed(), -1] {
            assert!(!has_zero_byte(cell), "{cell:#x}");
        }
    }

    #[test]
    fn bounded_strlen_without_terminator_is_none() {
        // A string running to the end of the AMX memory: the server's
        // `amx_StrLen` would keep reading past it.
        assert_eq!(bounded_strlen(&[0x41, 0x42]), None);
        assert_eq!(bounded_strlen(&[0x4142_4344]), None);
        assert_eq!(bounded_strlen(&[]), None);
    }

    #[cfg(feature = "encoding")]
    #[test]
    fn a_leading_bom_does_not_change_the_encoding() {
        let _g = crate::encoding::tests_lock();
        // "\u{ff}\u{fe}ab" in Windows-1252 (the default), which also starts
        // like a UTF-16LE BOM.
        let mut data = vec![0xFF, 0xFE, 0x61, 0x62, 0];
        let s = AmxString::from_buffer_parts(make_buffer(&mut data), 4);
        assert_eq!(&*s, "\u{ff}\u{fe}ab");
    }

    #[test]
    fn packed_with_a_high_first_byte_is_packed() {
        // `!"\233xyz"`: 0xE9 makes the first cell negative. The server
        // compares unsigned, so this is packed, not one cell per byte.
        let mut data = vec![0xE978_797Au32.cast_signed(), 0];
        assert_eq!(bounded_strlen(&data), Some(4));
        let s = AmxString::from_buffer_parts(make_buffer(&mut data), 4);
        assert_eq!(s.to_bytes(), [0xE9, b'x', b'y', b'z']);
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

    // --- Adversarial / property tests: decoding must never panic or overrun,
    //     whatever garbage (or a corrupted length) the script hands over. ---

    /// Tiny deterministic LCG — dependency-free pseudo-randomness for fuzzing.
    fn lcg(seed: &mut u64) -> u32 {
        *seed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (*seed >> 33) as u32
    }

    #[test]
    fn to_bytes_declared_len_larger_than_buffer_is_bounded() {
        // A corrupted length far beyond the backing cells must read only what
        // exists, never past the slice.
        let mut data = vec![0x41i32, 0x42, 0x43]; // "ABC", no terminator
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 9999);
        let bytes = s.to_bytes();
        assert!(bytes.len() <= 3, "read past the backing buffer: {bytes:?}");
    }

    #[test]
    fn to_bytes_non_utf8_decodes_lossy_without_panic() {
        // 0xFF is not valid UTF-8; decoding must produce replacement chars,
        // never panic.
        let mut data = vec![0xFFi32, 0xFE, 0x41, 0];
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 3);
        let _ = &*s; // triggers decode
        assert!(!s.is_empty());
    }

    #[test]
    fn fuzz_decode_never_panics() {
        // Random cell contents + a possibly-corrupted declared length, for both
        // packed and unpacked interpretations. The contract: decoding is total
        // (no panic, no overrun) no matter what the VM memory holds.
        let mut seed = 0x0BAD_F00D_DEAD_BEEFu64;
        for _ in 0..4000 {
            let cells = (lcg(&mut seed) % 12) as usize + 1; // 1..=12 cells
            let mut data: Vec<i32> = (0..cells).map(|_| lcg(&mut seed) as i32).collect();
            // Declared length may be anything, including far beyond `cells`.
            let declared = (lcg(&mut seed) % 64) as usize;
            let buf = make_buffer(&mut data);
            let s = AmxString::from_buffer_parts(buf, declared);

            let bytes = s.to_bytes();
            assert!(bytes.len() <= 1024 * 1024);
            // Deref decodes and caches — must also be total.
            let decoded = &*s;
            assert!(decoded.len() <= bytes.len().max(4 * bytes.len() + 4));
        }
    }
}
