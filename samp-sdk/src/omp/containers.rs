//! The generic containers the open.mp headers pass across the ABI: `Pair`,
//! `Span`, `HybridString`, and the `robin_hood` sets the pools and entities
//! hand out.
//!
//! Each is laid out as the C++ type is, from clang's record layout for both
//! ABIs; the tests pin the numbers.

use std::marker::PhantomData;

use super::vtable::VirtualReturn;

/// `Pair<A, B>` — `std::pair`, two fields in order. The same on both ABIs:
/// `std::pair` of trivially copyable members travels by value like a struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct Pair<A, B> {
    pub first: A,
    pub second: B,
}

impl<A, B> VirtualReturn for Pair<A, B> {
    type Raw = Self;
    fn from_raw(raw: Self) -> Self {
        raw
    }
}

/// `Span<T>` — `nonstd::span`, a pointer and a count, by value. The server
/// reads or fills `size` elements at `data`, so both must describe memory the
/// caller owns for the call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Span<T> {
    pub data: *mut T,
    pub size: usize,
}

impl<T> Span<T> {
    /// A span over `items`, for one call.
    #[must_use]
    pub fn of(items: &mut [T]) -> Self {
        Self {
            data: items.as_mut_ptr(),
            size: items.len(),
        }
    }
}

/// `HybridStringDynamicStorage` — where a [`HybridString`] too long for its
/// inline buffer keeps its bytes, and how the owner frees them.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct HybridStringDynamicStorage {
    pub ptr: *mut u8,
    pub free: Option<unsafe extern "C" fn(*mut std::ffi::c_void)>,
}

/// The storage of a [`HybridString`]: inline, or a pointer elsewhere.
#[derive(Clone, Copy)]
#[repr(C)]
pub union HybridStringStorage<const N: usize> {
    pub dynamic: HybridStringDynamicStorage,
    pub inline: [u8; N],
}

/// `HybridString<N>` — open.mp's ABI-stable string: `N` bytes inline
/// (terminator included), or a heap pointer past that. `len_dynamic` holds the
/// length shifted left by one, and whether the string is on the heap in bit 0.
///
/// Not `Copy`: a heap string belongs to whoever built it and is freed through
/// the pointer it carries. Structs holding one travel by reference only.
#[repr(C)]
pub struct HybridString<const N: usize> {
    pub len_dynamic: usize,
    pub storage: HybridStringStorage<N>,
}

impl<const N: usize> HybridString<N> {
    /// An inline copy of `text`, or `None` when it does not fit in `N - 1`
    /// bytes. A string built here never owns heap memory, so it needs no drop.
    #[must_use]
    pub fn new(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        if bytes.len() >= N {
            return None;
        }
        let mut inline = [0u8; N];
        inline[..bytes.len()].copy_from_slice(bytes);
        Some(Self {
            len_dynamic: bytes.len() << 1,
            storage: HybridStringStorage { inline },
        })
    }

    /// Whether the bytes live on the heap.
    #[must_use]
    pub fn is_dynamic(&self) -> bool {
        self.len_dynamic & 1 != 0
    }

    /// The length in bytes, terminator excluded.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len_dynamic >> 1
    }

    /// Whether the string is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The bytes, wherever they live.
    ///
    /// # Safety
    /// A heap string's pointer must still be live — true of one the server
    /// hands over by reference, for as long as the object that holds it.
    #[must_use]
    pub unsafe fn as_bytes(&self) -> &[u8] {
        let len = self.len();
        if self.is_dynamic() {
            let ptr = unsafe { self.storage.dynamic.ptr };
            if ptr.is_null() {
                return &[];
            }
            unsafe { std::slice::from_raw_parts(ptr, len) }
        } else {
            let inline = unsafe { &self.storage.inline };
            &inline[..len.min(N)]
        }
    }

    /// The text, with invalid UTF-8 replaced.
    ///
    /// # Safety
    /// As for [`as_bytes`](Self::as_bytes).
    #[must_use]
    pub unsafe fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(unsafe { self.as_bytes() }).into_owned()
    }
}

impl<const N: usize> Default for HybridString<N> {
    fn default() -> Self {
        Self {
            len_dynamic: 0,
            storage: HybridStringStorage { inline: [0; N] },
        }
    }
}

/// A `robin_hood` flat set of pointers — `FlatPtrHashSet<T>` and
/// `FlatHashSet<T*>` in the headers. Held by pointer only; read it with
/// [`flat_set_entries`].
#[repr(C)]
pub struct FlatSet<T> {
    _opaque: [u8; 0],
    _item: PhantomData<*mut T>,
}

/// Field offsets inside the set, from clang's layout of
/// `robin_hood::detail::Table<true, 80, T*, ...>` for each ABI. MSVC aligns the
/// `uint64_t` multiplier to 8, which moves every field after it.
#[cfg(not(target_env = "msvc"))]
mod set_layout {
    pub const KEY_VALS: usize = 8;
    pub const INFO: usize = 12;
    pub const NUM_ELEMENTS: usize = 16;
    pub const MASK: usize = 20;
}

#[cfg(target_env = "msvc")]
mod set_layout {
    pub const KEY_VALS: usize = 16;
    pub const INFO: usize = 20;
    pub const NUM_ELEMENTS: usize = 24;
    pub const MASK: usize = 28;
}

/// Buckets a `robin_hood` table allocates for `mask + 1` slots:
/// `calcNumElementsWithBuffer` — the slots, plus an overflow area of 80% of
/// them, capped at 255. The info array is that long (and a sentinel more), so
/// a walk bounded by it never reads outside the allocation.
fn buckets(mask: usize) -> Option<usize> {
    let slots = mask.checked_add(1)?;
    let allowed = if slots <= usize::MAX / 100 {
        slots * 80 / 100
    } else {
        (slots / 100) * 80
    };
    slots.checked_add(allowed.min(0xff))
}

/// Every pointer the set holds, in table order.
///
/// Like the extension-map walk in [`super::extensions`], this reads the
/// internals of a vendored hash table, not an ABI — so it fails closed. The
/// walk stops once it has found as many entries as the table says it holds,
/// never goes past the table's own bound, and an empty `Vec` comes back if the
/// numbers disagree.
///
/// # Safety
/// `set` must be null or a set the server handed out, still alive.
#[must_use]
pub unsafe fn flat_set_entries<T>(set: *const FlatSet<T>) -> Vec<*mut T> {
    if set.is_null() {
        return Vec::new();
    }
    let base = set.cast::<u8>();
    let read_usize = |offset: usize| unsafe { base.add(offset).cast::<usize>().read_unaligned() };
    let read_ptr = |offset: usize| unsafe { base.add(offset).cast::<*const u8>().read_unaligned() };

    let count = read_usize(set_layout::NUM_ELEMENTS);
    let mask = read_usize(set_layout::MASK);
    let key_vals = read_ptr(set_layout::KEY_VALS).cast::<*mut T>();
    let info = read_ptr(set_layout::INFO);
    if count == 0 || key_vals.is_null() || info.is_null() {
        return Vec::new();
    }
    let Some(bound) = buckets(mask) else {
        return Vec::new();
    };
    if count > bound {
        return Vec::new();
    }

    let mut found = Vec::with_capacity(count);
    for i in 0..bound {
        if found.len() == count {
            break;
        }
        if unsafe { info.add(i).read() } != 0 {
            found.push(unsafe { key_vals.add(i).read() });
        }
    }
    if found.len() == count {
        found
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The layouts clang gives are i686 ones: pointers are four bytes there.
    #[cfg(target_arch = "x86")]
    #[test]
    fn layouts_match_clang() {
        use std::mem::{offset_of, size_of};
        assert_eq!(size_of::<Span<u8>>(), 8);
        assert_eq!(offset_of!(Span<u8>, size), 4);
        assert_eq!(size_of::<Pair<i32, i32>>(), 8);
        assert_eq!(size_of::<Pair<bool, crate::omp::StringView>>(), 12);
        assert_eq!(offset_of!(Pair<bool, crate::omp::StringView>, second), 4);
        assert_eq!(size_of::<HybridString<16>>(), 20);
        assert_eq!(size_of::<HybridString<46>>(), 52);
        assert_eq!(offset_of!(HybridString<16>, storage), 4);
    }

    #[test]
    fn hybrid_string_round_trips_inline() {
        let s = HybridString::<16>::new("hello").unwrap();
        assert!(!s.is_dynamic());
        assert_eq!(s.len(), 5);
        assert_eq!(unsafe { s.to_string_lossy() }, "hello");
        // Fifteen bytes and the terminator fill it; sixteen do not fit.
        assert!(HybridString::<16>::new("123456789012345").is_some());
        assert!(HybridString::<16>::new("1234567890123456").is_none());
    }

    #[test]
    fn hybrid_string_reads_a_heap_copy() {
        let mut bytes = *b"far too long for sixteen";
        let s = HybridString::<16> {
            len_dynamic: (bytes.len() << 1) | 1,
            storage: HybridStringStorage {
                dynamic: HybridStringDynamicStorage {
                    ptr: bytes.as_mut_ptr(),
                    free: None,
                },
            },
        };
        assert_eq!(unsafe { s.to_string_lossy() }, "far too long for sixteen");
    }

    /// A table laid out as `robin_hood` does, with three entries spread over
    /// eight buckets.
    /// Pointer-typed words, so that moving it keeps the pointers' provenance.
    #[repr(C, align(8))]
    struct FakeSet([*const u8; 16]);

    fn fake(count: usize, mask: usize, keys: &[*mut i32], info: &[u8]) -> FakeSet {
        let mut raw = FakeSet([std::ptr::null(); 16]);
        let base = raw.0.as_mut_ptr().cast::<u8>();
        // Pointers go in as pointers, so they keep their provenance.
        unsafe {
            base.add(set_layout::KEY_VALS)
                .cast::<*const *mut i32>()
                .write_unaligned(keys.as_ptr());
            base.add(set_layout::INFO)
                .cast::<*const u8>()
                .write_unaligned(info.as_ptr());
            base.add(set_layout::NUM_ELEMENTS)
                .cast::<usize>()
                .write_unaligned(count);
            base.add(set_layout::MASK)
                .cast::<usize>()
                .write_unaligned(mask);
        }
        raw
    }

    // The field offsets are the i686 table's.
    #[cfg(target_arch = "x86")]
    #[test]
    fn walks_the_occupied_buckets() {
        let (mut a, mut b, mut c) = (1, 2, 3);
        let n = std::ptr::null_mut();
        let keys = [n, &raw mut a, n, n, &raw mut b, n, &raw mut c, n, n];
        let info = [0u8, 1, 0, 0, 1, 0, 2, 0, 1];
        let raw = fake(3, 7, &keys, &info);
        let got = unsafe { flat_set_entries((&raw const raw).cast::<FlatSet<i32>>()) };
        assert_eq!(got, vec![&raw mut a, &raw mut b, &raw mut c]);
    }

    #[test]
    fn fails_closed_when_the_count_disagrees() {
        let mut a = 1;
        let keys = [&raw mut a, std::ptr::null_mut()];
        let info = [1u8, 0];
        // Claims five entries in a one-slot table: more than it can hold, so
        // nothing is read at all.
        let raw = fake(5, 0, &keys, &info);
        let got = unsafe { flat_set_entries((&raw const raw).cast::<FlatSet<i32>>()) };
        assert!(got.is_empty());
    }

    #[test]
    fn bucket_count_follows_robin_hood() {
        assert_eq!(buckets(0), Some(1));
        assert_eq!(buckets(7), Some(14));
        assert_eq!(buckets(1023), Some(1024 + 255));
    }

    #[test]
    fn a_null_set_is_empty() {
        assert!(unsafe { flat_set_entries(std::ptr::null::<FlatSet<i32>>()) }.is_empty());
    }
}
