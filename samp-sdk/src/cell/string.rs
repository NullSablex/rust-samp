//! String interperation inside an AMX.
use std::cell::OnceCell;
use std::fmt;
use std::ops::Deref;

use super::{AmxCell, Buffer, UnsizedBuffer};
use crate::amx::Amx;
#[cfg(feature = "encoding")]
use crate::encoding;
use crate::error::AmxResult;

const MAX_UNPACKED: i32 = 0x00FF_FFFF;

/// A wrapper around an AMX string (packed or unpacked).
///
/// Implements [`Deref<Target = str>`] so you can use string methods directly
/// without an explicit `.to_string()`:
///
/// ```no_run
/// # use samp_sdk::cell::AmxString;
/// # use samp_sdk::amx::Amx;
/// # use samp_sdk::error::AmxResult;
/// # struct Plugin;
/// # impl Plugin {
/// fn greet(&self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
///     // Métodos de &str disponíveis diretamente via Deref
///     if name.starts_with("Admin") {
///         println!("Welcome, {}!", &*name);
///     }
///     Ok(true)
/// }
/// # }
/// ```
///
/// The decoded string is computed lazily on first access and cached —
/// repeated access has zero allocation cost.
pub struct AmxString<'amx> {
    inner: Buffer<'amx>,
    /// Real length of the string (character count, without null terminator).
    len: usize,
    /// Lazily decoded string — computed on first Deref access.
    decoded: OnceCell<String>,
}

impl<'amx> AmxString<'amx> {
    /// Create a new `AmxString` from an allocated buffer and fill it with raw bytes.
    ///
    /// # Safety
    /// `buffer` must be a valid allocation from the AMX heap with at least
    /// `bytes.len() + 1` cells, and must remain valid for `'amx`.
    pub unsafe fn new(mut buffer: Buffer<'amx>, bytes: &[u8]) -> AmxString<'amx> {
        for (idx, byte) in bytes.iter().enumerate() {
            buffer[idx] = i32::from(*byte);
        }
        buffer[bytes.len()] = 0;

        AmxString {
            len: bytes.len(),
            inner: buffer,
            decoded: OnceCell::new(),
        }
    }

    /// Internal constructor for tests and benchmarks — not a stable API.
    ///
    /// Creates an `AmxString` from a pre-filled buffer without writing bytes.
    /// Useful for testing packed string parsing where the buffer is manually crafted.
    #[doc(hidden)]
    pub fn from_buffer_parts(inner: Buffer<'amx>, len: usize) -> AmxString<'amx> {
        AmxString {
            inner,
            len,
            decoded: OnceCell::new(),
        }
    }

    /// Convert the AMX string to a `Vec<u8>`.
    pub fn to_bytes(&self) -> Vec<u8> {
        const MAX_STRING_LEN: usize = 1024 * 1024;
        let len = self.len.min(MAX_STRING_LEN);
        let mut vec = Vec::with_capacity(len);

        // packed string
        if self.inner[0] > MAX_UNPACKED {
            let base = self.inner.as_ptr();
            let max_cells = self.inner.len();
            let mut ptr = base;
            let mut mark = 3;
            for _ in 0..len {
                let offset = unsafe { ptr.offset_from(base) } as usize;
                if offset >= max_cells {
                    break;
                }
                let ch = (unsafe { *ptr } >> (mark * 8)) as u8;
                if ch == b'\0' {
                    break;
                }
                vec.push(ch);
                mark = (mark + 3) % 4;
                if mark == 3 {
                    ptr = unsafe { ptr.add(1) };
                    let new_offset = unsafe { ptr.offset_from(base) } as usize;
                    if new_offset >= max_cells {
                        break;
                    }
                }
            }
        } else {
            for item in self.inner.iter().take(len) {
                vec.push(*item as u8);
            }
        }

        vec
    }

    /// Return the length of the string in characters (without null terminator).
    pub fn len(&self) -> usize {
        self.len
    }

    /// Return `true` if the string is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Return the length of the underlying buffer in cells.
    pub fn bytes_len(&self) -> usize {
        self.inner.len()
    }

    /// Returns a `&str` view of this string.
    ///
    /// Equivalent to `&*self` via [`Deref`], but more readable — especially
    /// when passing to functions that expect `&str` and auto-deref doesn't
    /// trigger due to a generic bound (e.g. `T: AsRef<str>`).
    ///
    /// # Example
    /// ```rust,no_run
    /// # use samp_sdk::cell::AmxString;
    /// # fn connect(_addr: &str) {}
    /// fn example(addr: AmxString) {
    ///     connect(addr.as_str());          // explicit
    ///     connect(&addr);                  // equivalent — auto-deref
    /// }
    /// ```
    pub fn as_str(&self) -> &str {
        self.deref()
    }
}

/// Decode raw bytes to a `String` using the configured encoding.
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

    /// Returns the decoded string. Computed lazily on first access and cached.
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
    /// Compare directly with a `&str` literal — no allocation needed.
    ///
    /// ```rust,no_run
    /// # use samp_sdk::cell::AmxString;
    /// # use samp_sdk::amx::Amx;
    /// # use samp_sdk::error::AmxResult;
    /// # struct P; impl P {
    /// fn check(&self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
    ///     Ok(name == "Admin")
    /// }
    /// # }
    /// ```
    fn eq(&self, other: &str) -> bool {
        self.deref() == other
    }
}

impl PartialEq<&str> for AmxString<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.deref() == *other
    }
}

impl PartialEq<String> for AmxString<'_> {
    fn eq(&self, other: &String) -> bool {
        self.deref() == other.as_str()
    }
}

/// Fill a buffer with a given string.
///
/// Prefer [`Buffer::write_str`] for a more ergonomic API.
///
/// # Example
/// ```rust,no_run
/// use samp_sdk::cell::Buffer;
/// use samp_sdk::cell::string;
/// # use samp_sdk::error::AmxResult;
/// # use samp_sdk::amx::Amx;
///
/// # fn main() -> AmxResult<()> {
/// # let amx = Amx::new(std::ptr::null_mut(), 0);
/// let allocator = amx.allocator();
/// let mut buffer = allocator.allot_buffer(25)?;
/// string::put_in_buffer(&mut buffer, "Hello, world!")?;
/// #   Ok(())
/// # }
/// ```
/// # Errors
/// Returns `AmxError::General` when the string is longer than the buffer.
pub fn put_in_buffer(buffer: &mut Buffer, string: &str) -> AmxResult<()> {
    #[cfg(feature = "encoding")]
    let bytes = encoding::get().encode(string).0;

    #[cfg(not(feature = "encoding"))]
    let bytes = std::borrow::Cow::from(string.as_bytes());

    let bytes = bytes.as_ref();

    if bytes.len() >= buffer.len() {
        return Err(crate::error::AmxError::General);
    }

    for (idx, byte) in bytes.iter().enumerate() {
        buffer[idx] = i32::from(*byte);
    }

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

    // --- Strings unpacked (um byte por célula) ---

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
        // Métodos de &str sem .to_string()
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
        // OnceCell não foi inicializada ainda
        assert!(s.decoded.get().is_none());
        // Primeiro acesso via Deref → inicializa
        let _ = &*s;
        assert!(s.decoded.get().is_some());
        // Segundo acesso → mesmo ponteiro (cache hit)
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
        let mut data: Vec<i32> = text.iter().map(|&b| b as i32).chain(std::iter::once(0)).collect();
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

    // --- Strings packed (4 bytes por célula) ---
    //
    // Bytes lidos de cada cell: bits[31..24], [23..16], [15..8], [7..0].
    // "ABCD" → cell = 0x41424344, próxima cell = 0x00000000 (null)

    #[test]
    fn packed_four_chars_one_cell() {
        let mut data = vec![0x41424344i32, 0x00000000i32];
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 4);
        assert_eq!(s.to_bytes(), b"ABCD");
        assert_eq!(&*s, "ABCD");
    }

    #[test]
    fn packed_five_chars_two_cells() {
        // "ABCDE": 4 chars em cell[0], 1 em cell[1]
        let mut data = vec![0x41424344i32, 0x45000000i32, 0x00000000i32];
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 5);
        assert_eq!(s.to_bytes(), b"ABCDE");
        assert_eq!(&*s, "ABCDE");
    }

    #[test]
    fn packed_truncates_at_len() {
        let mut data = vec![0x41424344i32, 0x00000000i32];
        let buf = make_buffer(&mut data);
        let s = AmxString::from_buffer_parts(buf, 2);
        assert_eq!(s.to_bytes(), b"AB");
    }

    #[test]
    fn packed_stops_at_null_byte() {
        // "AB\0D" → para em \0, retorna "AB"
        let mut data = vec![0x41420044i32, 0x00000000i32];
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
        // Ambos acionam o mesmo OnceCell — mesmo ponteiro de &str
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
        assert_eq!(buf[0], b'h' as i32);
        assert_eq!(buf[4], b'o' as i32);
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
