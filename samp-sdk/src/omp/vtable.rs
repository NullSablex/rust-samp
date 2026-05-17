//! Helpers for accessing vtables (primary and secondary) of server-owned C++ objects.
//!
//! In C++ with multiple inheritance, each base class with virtuals results in a
//! distinct vtable. The primary lies at offset 0 of the object; secondaries at
//! offsets that depend on the `sizeof` of the preceding bases. The offsets are
//! fixed per class and known at compile time (after layout analysis via disasm).
//!
//! This module centralizes the repeated pattern of:
//!
//! 1. Adjust the object pointer to point to a subobject (`obj + offset`).
//! 2. Read the secondary vtable (`*subobject`).
//! 3. Load the slot N pointer (`*(vtable + N * sizeof(usize))`).
//!
//! Each specific caller still performs the final `transmute` to the correct
//! function type, because the calling convention varies (`extern "C"`,
//! `extern "thiscall"`, variadic vs fixed arity).
//!
//! ## Example usage
//!
//! ```rust,no_run
//! # use samp_sdk::omp::vtable;
//! # use std::os::raw::{c_char, c_int};
//! # type LogLnFn = unsafe extern "C" fn(*mut u8, c_int, *const c_char, *const c_char);
//! # fn example(core: *mut u8, level: c_int, fmt: *const c_char, arg: *const c_char) -> Option<()> {
//! // ILogger at offset 56 inside ICore; logLn at slot [2].
//! let (this, f_ptr) = unsafe {
//!     vtable::secondary_call_target(core, 56, 2)?
//! };
//! let f: LogLnFn = unsafe { std::mem::transmute(f_ptr) };
//! unsafe { f(this, level, fmt, arg) };
//! # Some(()) }
//! ```

/// Returns the subobject pointer at `offset` bytes from `obj`.
///
/// For the primary base class (at offset 0), `offset = 0`. For secondary bases,
/// the offset is determined by the `sizeof` of the preceding bases in C++.
///
/// Returns `None` if `obj` is null.
///
/// # Safety
/// `obj` must be a valid pointer (or null). `offset` must be the correct offset
/// of the subobject — passing the wrong offset produces an invalid pointer.
#[inline]
pub unsafe fn subobject_ptr(obj: *mut u8, offset: isize) -> Option<*mut u8> {
    if obj.is_null() {
        return None;
    }
    Some(unsafe { obj.offset(offset) })
}

/// Reads the slot `slot` pointer from the vtable pointed to by `subobject`.
///
/// Returns `None` if `subobject` is null, the vtable is null, or the slot
/// contains zero (defensive against uninitialized or corrupted vtables).
///
/// # Safety
/// `subobject` must point to a valid C++ object whose first member is the vptr.
/// `slot` must be within the valid range of the vtable — reading a non-existent
/// slot yields an undefined value (but not aliasing UB).
#[inline]
pub unsafe fn vtable_slot(subobject: *mut u8, slot: usize) -> Option<usize> {
    if subobject.is_null() {
        return None;
    }
    // FFI: the first field of any C++ object with a virtual method is the
    // vtable pointer, always pointer-aligned by the ABI (Itanium and MSVC).
    #[allow(clippy::cast_ptr_alignment)]
    let vtable = unsafe { *(subobject as *const *const usize) };
    if vtable.is_null() {
        return None;
    }
    let f_ptr = unsafe { *vtable.add(slot) };
    if f_ptr == 0 {
        return None;
    }
    Some(f_ptr)
}

/// Combines [`subobject_ptr`] + [`vtable_slot`] in a single helper.
///
/// Returns `(this, f_ptr)`: the `this` adjusted for the subobject (the first
/// arg of virtual method calls on that subobject) and the function pointer at
/// the slot. The caller does the `transmute` to the correct function type and
/// invokes it.
///
/// Returns `None` on any failure (`obj` null, vtable null, slot zero).
///
/// # Safety
/// See [`subobject_ptr`] and [`vtable_slot`].
#[inline]
pub unsafe fn secondary_call_target(
    obj: *mut u8,
    offset: isize,
    slot: usize,
) -> Option<(*mut u8, usize)> {
    let this = unsafe { subobject_ptr(obj, offset)? };
    let f_ptr = unsafe { vtable_slot(this, slot)? };
    Some((this, f_ptr))
}

#[cfg(test)]
mod tests {
    use super::*;

    static MOCK_VTABLE: std::sync::OnceLock<[usize; 8]> = std::sync::OnceLock::new();

    fn mock_vtable() -> &'static [usize; 8] {
        MOCK_VTABLE.get_or_init(|| {
            [
                0xDEAD_0000,
                0xDEAD_0001,
                0xDEAD_0002,
                0xDEAD_0003,
                0xDEAD_0004,
                0xDEAD_0005,
                0xDEAD_0006,
                0xDEAD_0007,
            ]
        })
    }

    /// Creates a 128-byte buffer (32x`usize` on i686) naturally aligned;
    /// at `byte_offset` it installs the vptr for `mock_vtable`.
    fn make_obj_with_secondary_vtable(byte_offset: isize) -> [usize; 32] {
        let mut buf = [0usize; 32];
        let vptr = mock_vtable().as_ptr() as usize;
        let idx = usize::try_from(byte_offset).expect("byte_offset must be >= 0")
            / std::mem::size_of::<usize>();
        buf[idx] = vptr;
        buf
    }

    #[test]
    fn subobject_ptr_returns_none_for_null() {
        assert!(unsafe { subobject_ptr(std::ptr::null_mut(), 56) }.is_none());
    }

    #[test]
    fn subobject_ptr_adds_offset_correctly() {
        let base = 0x1000 as *mut u8;
        let sub = unsafe { subobject_ptr(base, 56) }.unwrap();
        assert_eq!(sub as usize, 0x1000 + 56);
    }

    #[test]
    fn vtable_slot_returns_none_for_null_subobject() {
        assert!(unsafe { vtable_slot(std::ptr::null_mut(), 0) }.is_none());
    }

    #[test]
    fn vtable_slot_returns_zero_check() {
        // Buffer with a vtable containing 0 at slot 2
        let zero_table: [usize; 3] = [0xDEAD, 0xDEAD, 0];
        let mut buf = [0usize; 8];
        buf[0] = zero_table.as_ptr() as usize;
        let buf_u8 = buf.as_mut_ptr().cast::<u8>();
        // Slot 2 is zero — must return None
        assert!(unsafe { vtable_slot(buf_u8, 2) }.is_none());
        // Slot 0 is non-zero
        assert_eq!(unsafe { vtable_slot(buf_u8, 0) }, Some(0xDEAD));
    }

    #[test]
    fn secondary_call_target_combines_both() {
        let mut buf = make_obj_with_secondary_vtable(56);
        let buf_u8 = buf.as_mut_ptr().cast::<u8>();
        let (this, f_ptr) = unsafe { secondary_call_target(buf_u8, 56, 3).unwrap() };
        assert_eq!(this as usize, buf_u8 as usize + 56);
        assert_eq!(f_ptr, 0xDEAD_0003);
    }

    #[test]
    fn secondary_call_target_null_obj_returns_none() {
        assert!(unsafe { secondary_call_target(std::ptr::null_mut(), 56, 0) }.is_none());
    }

    #[test]
    fn secondary_call_target_zero_slot_returns_none() {
        // Buffer with a 1-slot zeroed vtable
        let zero_table: [usize; 1] = [0];
        let mut buf = [0usize; 16];
        // byte offset 8 = index 2 on i686 (usize = 4 bytes)
        buf[2] = zero_table.as_ptr() as usize;
        let buf_u8 = buf.as_mut_ptr().cast::<u8>();
        assert!(unsafe { secondary_call_target(buf_u8, 8, 0) }.is_none());
    }
}
