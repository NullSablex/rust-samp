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
//! Slots are read and returned as `*const ()`, never as `usize`: a function
//! pointer rebuilt from an integer carries no provenance, and calling it is
//! undefined behavior under Rust's memory model.
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
//!     vtable::secondary_call_target_ptr(core, 56, 2)?
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
#[deprecated(
    since = "3.5.0",
    note = "returns the address without provenance; use `vtable_slot_ptr`"
)]
#[inline]
pub unsafe fn vtable_slot(subobject: *mut u8, slot: usize) -> Option<usize> {
    unsafe { vtable_slot_ptr(subobject, slot) }.map(|f| f.addr())
}

/// Reads the slot `slot` function pointer from the vtable pointed to by `subobject`.
///
/// Returns `None` if `subobject` is null, the vtable is null, or the slot
/// is null (defensive against uninitialized or corrupted vtables). The pointer
/// keeps its provenance, so the caller may `transmute` it to a function type.
///
/// # Safety
/// `subobject` must point to a valid C++ object whose first member is the vptr.
/// `slot` must be within the valid range of the vtable.
#[inline]
pub unsafe fn vtable_slot_ptr(subobject: *mut u8, slot: usize) -> Option<*const ()> {
    if subobject.is_null() {
        return None;
    }
    // FFI: the first field of any C++ object with a virtual method is the
    // vtable pointer, always pointer-aligned by the ABI (Itanium and MSVC).
    #[allow(clippy::cast_ptr_alignment)]
    let vtable = unsafe { *(subobject as *const *const *const ()) };
    if vtable.is_null() {
        return None;
    }
    let f_ptr = unsafe { *vtable.add(slot) };
    if f_ptr.is_null() {
        return None;
    }
    Some(f_ptr)
}

/// Combines [`subobject_ptr`] + [`vtable_slot_ptr`] in a single helper.
///
/// Returns `(this, f_ptr)`: the `this` adjusted for the subobject (the first
/// arg of virtual method calls on that subobject) and the function pointer at
/// the slot. The caller does the `transmute` to the correct function type and
/// invokes it.
///
/// Returns `None` on any failure (`obj` null, vtable null, slot null).
///
/// # Safety
/// See [`subobject_ptr`] and [`vtable_slot_ptr`].
#[inline]
pub unsafe fn secondary_call_target_ptr(
    obj: *mut u8,
    offset: isize,
    slot: usize,
) -> Option<(*mut u8, *const ())> {
    let this = unsafe { subobject_ptr(obj, offset)? };
    let f_ptr = unsafe { vtable_slot_ptr(this, slot)? };
    Some((this, f_ptr))
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
#[deprecated(
    since = "3.5.0",
    note = "returns the address without provenance; use `secondary_call_target_ptr`"
)]
#[inline]
pub unsafe fn secondary_call_target(
    obj: *mut u8,
    offset: isize,
    slot: usize,
) -> Option<(*mut u8, usize)> {
    unsafe { secondary_call_target_ptr(obj, offset, slot) }.map(|(this, f)| (this, f.addr()))
}

/// How a value comes back from a C++ virtual call, and what to declare the
/// foreign function as returning to receive it safely.
///
/// A C++ function returning `bool`, `uint8_t` or a 16-bit integer sets only the
/// low part of `EAX`; the rest of the register is left as it was. The official
/// `IVehicle::isOccupied()` on Windows ORs two pointers into `EAX` and then
/// `setne %al` — `true` comes back as `0x????..01`. Rust, told the function
/// returns `bool`, assumes the register holds exactly 0 or 1, and a `bool` with
/// any other bit pattern is undefined behaviour: a comparison may read the
/// whole register and answer wrongly, with nothing to show for it.
///
/// So narrow types are received as the full register (`Raw`) and narrowed in
/// Rust, where truncation is defined. Everything else comes back as it is.
pub trait VirtualReturn: Sized {
    /// The type the foreign function is declared to return.
    type Raw;
    /// The value the caller sees.
    fn from_raw(raw: Self::Raw) -> Self;
}

impl VirtualReturn for bool {
    type Raw = u32;
    fn from_raw(raw: u32) -> bool {
        // Only `AL` is defined; C++ puts 0 or 1 there.
        raw & 0xff != 0
    }
}

macro_rules! narrowed {
    ($($ty:ty => $raw:ty),* $(,)?) => {$(
        impl VirtualReturn for $ty {
            type Raw = $raw;
            #[allow(clippy::cast_possible_truncation)]
            fn from_raw(raw: $raw) -> $ty {
                // Truncation keeps the defined low bits and drops the rest.
                raw as $ty
            }
        }
    )*};
}

narrowed!(u8 => u32, i8 => i32, u16 => u32, i16 => i32);

macro_rules! as_returned {
    ($($ty:ty),* $(,)?) => {$(
        impl VirtualReturn for $ty {
            type Raw = $ty;
            fn from_raw(raw: $ty) -> $ty {
                raw
            }
        }
    )*};
}

as_returned!(
    (),
    i32,
    u32,
    i64,
    u64,
    f32,
    f64,
    usize,
    isize,
    super::types::Vector3,
    super::types::Vector4,
);

impl<T> VirtualReturn for *mut T {
    type Raw = *mut T;
    fn from_raw(raw: *mut T) -> *mut T {
        raw
    }
}

impl<T> VirtualReturn for *const T {
    type Raw = *const T;
    fn from_raw(raw: *const T) -> *const T {
        raw
    }
}

/// Calls a virtual method through a server object's vtable.
///
/// Every wrapper in this module family repeats the same four steps: name the
/// function type for the target's calling convention, adjust `this` to the
/// right subobject, read the slot, and give up gracefully when either pointer
/// is missing. The macro is that sequence written once.
///
/// ```ignore
/// // bool IPlayer::isBot() const, slot 8, primary vtable
/// call_vtable!(player.cast::<u8>(), 0, SLOT_IS_BOT, () -> bool, (), false)
///
/// // void IPlayer::setHealth(float)
/// call_vtable!(player.cast::<u8>(), 0, SLOT_SET_HEALTH, (f32) -> (), (health), ())
/// ```
///
/// The last argument is what to return when the object, its vtable or the slot
/// is null — the "fails closed" behaviour the null-safety tests check. Methods
/// whose return type crosses the ABI differently (a struct through a hidden
/// pointer, say) are written out by hand instead.
macro_rules! call_vtable {
    (
        $ptr:expr, $offset:expr, $slot:expr,
        ($($arg_ty:ty),* $(,)?) -> $ret:ty,
        ($($arg:expr),* $(,)?),
        $absent:expr
    ) => {{
        type Raw = <$ret as $crate::omp::vtable::VirtualReturn>::Raw;
        #[cfg(not(target_env = "msvc"))]
        type VirtualFn = unsafe extern "C" fn(*mut u8 $(, $arg_ty)*) -> Raw;
        #[cfg(target_env = "msvc")]
        type VirtualFn = unsafe extern "thiscall" fn(*mut u8 $(, $arg_ty)*) -> Raw;

        match unsafe { $crate::omp::vtable::secondary_call_target_ptr($ptr, $offset, $slot) } {
            Some((this, f_ptr)) => {
                let call: VirtualFn = unsafe { std::mem::transmute(f_ptr) };
                <$ret as $crate::omp::vtable::VirtualReturn>::from_raw(unsafe { call(this $(, $arg)*) })
            }
            None => $absent,
        }
    }};
}

pub(crate) use call_vtable;

/// Calls a no-argument virtual method returning a small struct — at most eight
/// bytes, trivially copyable: a `StringView`, a `SemanticVersion`.
///
/// The two ABIs disagree on where such a value comes back. Itanium returns it in
/// `EAX:EDX`; MSVC writes it through a hidden pointer the caller passes after
/// `this`, and returns that pointer. Declaring it the wrong way round reads
/// whatever the registers held — or crashes the server, as it did when
/// `IPlayer::getName` was first written. This is that rule, written once.
///
/// `$empty` is the value the MSVC out-parameter starts as. `None` when the
/// object, its vtable or the slot is null.
///
/// A struct larger than eight bytes (`Vector3`) comes back through a hidden
/// pointer on both ABIs, which plain [`call_vtable!`] already handles by
/// declaring the return type.
macro_rules! call_vtable_small_struct {
    ($ptr:expr, $offset:expr, $slot:expr, $ret:ty, $empty:expr) => {{
        #[cfg(not(target_env = "msvc"))]
        type VirtualFn = unsafe extern "C" fn(*mut u8) -> $ret;
        #[cfg(target_env = "msvc")]
        type VirtualFn = unsafe extern "thiscall" fn(*mut u8, *mut $ret) -> *mut $ret;

        match unsafe { $crate::omp::vtable::secondary_call_target_ptr($ptr, $offset, $slot) } {
            Some((this, f_ptr)) => {
                let call: VirtualFn = unsafe { std::mem::transmute(f_ptr) };
                #[cfg(not(target_env = "msvc"))]
                let value = unsafe { call(this) };
                #[cfg(target_env = "msvc")]
                let value = {
                    let mut out: $ret = $empty;
                    unsafe { call(this, &raw mut out) };
                    out
                };
                Some(value)
            }
            None => None,
        }
    }};
}

pub(crate) use call_vtable_small_struct;

/// Declares constants whose value depends on the C++ ABI, one line each.
///
/// Slot indices and subobject offsets differ between Itanium (Linux) and MSVC
/// (Windows), so every one of them used to be a pair of `#[cfg]`-gated
/// declarations. This writes the pair from a single line, Itanium first:
///
/// ```ignore
/// slots! {
///     /// `IPlayer::kick()`.
///     SLOT_PLAYER_KICK: usize = 6, 5;
///     pub(crate) ENTITY_OFFSET: isize = 40, 56;
/// }
/// ```
macro_rules! slots {
    ($(
        $(#[$meta:meta])*
        $vis:vis $name:ident: $ty:ty = $itanium:expr, $msvc:expr;
    )*) => {$(
        $(#[$meta])*
        #[cfg(not(target_env = "msvc"))]
        $vis const $name: $ty = $itanium;
        $(#[$meta])*
        #[cfg(target_env = "msvc")]
        $vis const $name: $ty = $msvc;
    )*};
}

pub(crate) use slots;

/// Declares opaque handles for server interfaces the SDK only ever holds by
/// pointer.
///
/// ```ignore
/// opaque! {
///     /// Opaque handle for `IPlayerPool*`.
///     pub IPlayerPool;
/// }
/// ```
macro_rules! opaque {
    ($(
        $(#[$meta:meta])*
        $vis:vis $name:ident;
    )*) => {$(
        $(#[$meta])*
        #[repr(C)]
        $vis struct $name {
            _opaque: [u8; 0],
        }
    )*};
}

pub(crate) use opaque;

/// Declares typed wrappers for virtual methods, one entry each.
///
/// Most of the SDK's surface is a thin, typed door onto one vtable slot: take
/// the handle, call the slot, return what the server returns — or a neutral
/// value when the handle, its vtable or the slot is null. Written out, each of
/// those was a function signature around a single [`call_vtable!`]. This keeps
/// the part that carries information — which slot, on which subobject, with
/// which types, answering what when absent:
///
/// ```ignore
/// virtual_fns! {
///     /// `IPlayer::getHealth()`.
///     #[must_use]
///     pub fn player_health(player: IPlayer) -> f32 = [0, SLOT_PLAYER_GET_HEALTH] or 0.0;
///
///     /// `IPlayer::setHealth(float)`.
///     pub fn player_set_health(player: IPlayer, health: f32) = [0, SLOT_PLAYER_SET_HEALTH];
/// }
/// ```
///
/// `[offset, slot]` is the subobject offset and the slot inside that
/// subobject's vtable. A method with no return type needs no `or`. Every
/// generated function is `unsafe`: the handle must be live, which only the
/// caller can know.
macro_rules! virtual_fns {
    ($(
        $(#[$meta:meta])*
        $vis:vis fn $name:ident($this:ident: $handle:ty $(, $arg:ident: $arg_ty:ty)* $(,)?)
            $(-> $ret:ty)? = [$offset:expr, $slot:expr] $(or $absent:expr)?;
    )*) => {$(
        $(#[$meta])*
        // A wrapper takes what the C++ method takes, argument for argument:
        // grouping them would read better alone and worse next to the header,
        // which is what the wrapper has to be checked against.
        #[allow(clippy::too_many_arguments)]
        $vis unsafe fn $name($this: *mut $handle $(, $arg: $arg_ty)*) $(-> $ret)? {
            $crate::omp::vtable::call_vtable!(
                $this.cast::<u8>(),
                $offset,
                $slot,
                ($($arg_ty),*) -> $crate::omp::vtable::virtual_fns!(@ret $($ret)?),
                ($($arg),*),
                $crate::omp::vtable::virtual_fns!(@absent $($absent)?)
            )
        }
    )*};
    (@ret) => { () };
    (@ret $ret:ty) => { $ret };
    (@absent) => { () };
    (@absent $absent:expr) => { $absent };
}

pub(crate) use virtual_fns;

/// Function-pointer table for unit-test mocks. Raw pointers are not `Sync`,
/// so the wrapper lets a mock vtable live in a `static`.
#[cfg(test)]
pub(crate) struct MockTable<const N: usize>(pub [*const (); N]);

// SAFETY: the table is written once at init and only read afterwards.
#[cfg(test)]
unsafe impl<const N: usize> Sync for MockTable<N> {}
#[cfg(test)]
unsafe impl<const N: usize> Send for MockTable<N> {}

#[cfg(test)]
mod tests {
    use super::*;

    const DUMMY: [u8; 8] = [0; 8];

    /// Fake function pointers into `DUMMY`: never called, only compared.
    fn fake(i: usize) -> *const () {
        DUMMY.as_ptr().wrapping_add(i).cast()
    }

    /// Creates a 32-pointer buffer; at `byte_offset` it installs the vptr for `table`.
    fn make_obj_with_secondary_vtable(byte_offset: isize, table: &[*const ()]) -> [*const (); 32] {
        let mut buf = [std::ptr::null::<()>(); 32];
        let idx = usize::try_from(byte_offset).expect("byte_offset must be >= 0")
            / std::mem::size_of::<*const ()>();
        buf[idx] = table.as_ptr().cast();
        buf
    }

    #[test]
    fn subobject_ptr_returns_none_for_null() {
        assert!(unsafe { subobject_ptr(std::ptr::null_mut(), 56) }.is_none());
    }

    #[test]
    fn subobject_ptr_adds_offset_correctly() {
        let mut buf = [0u8; 64];
        let base = buf.as_mut_ptr();
        let sub = unsafe { subobject_ptr(base, 56) }.unwrap();
        assert_eq!(sub, base.wrapping_add(56));
    }

    #[test]
    fn vtable_slot_ptr_returns_none_for_null_subobject() {
        assert!(unsafe { vtable_slot_ptr(std::ptr::null_mut(), 0) }.is_none());
    }

    #[test]
    fn vtable_slot_ptr_null_slot_returns_none() {
        let table = [fake(0), fake(1), std::ptr::null()];
        let mut buf = make_obj_with_secondary_vtable(0, &table);
        let buf_u8 = buf.as_mut_ptr().cast::<u8>();
        assert!(unsafe { vtable_slot_ptr(buf_u8, 2) }.is_none());
        assert_eq!(unsafe { vtable_slot_ptr(buf_u8, 0) }, Some(fake(0)));
    }

    #[test]
    fn secondary_call_target_ptr_combines_both() {
        let table: Vec<*const ()> = (0..8).map(fake).collect();
        let mut buf = make_obj_with_secondary_vtable(56, &table);
        let buf_u8 = buf.as_mut_ptr().cast::<u8>();
        let (this, f_ptr) = unsafe { secondary_call_target_ptr(buf_u8, 56, 3).unwrap() };
        assert_eq!(this, buf_u8.wrapping_add(56));
        assert_eq!(f_ptr, fake(3));
    }

    #[test]
    fn secondary_call_target_ptr_null_obj_returns_none() {
        assert!(unsafe { secondary_call_target_ptr(std::ptr::null_mut(), 56, 0) }.is_none());
    }

    #[test]
    fn secondary_call_target_ptr_null_slot_returns_none() {
        let table = [std::ptr::null::<()>()];
        let mut buf = make_obj_with_secondary_vtable(8, &table);
        let buf_u8 = buf.as_mut_ptr().cast::<u8>();
        assert!(unsafe { secondary_call_target_ptr(buf_u8, 8, 0) }.is_none());
    }

    #[test]
    #[allow(deprecated)]
    fn deprecated_wrappers_return_the_same_address() {
        let table: Vec<*const ()> = (0..8).map(fake).collect();
        let mut buf = make_obj_with_secondary_vtable(56, &table);
        let buf_u8 = buf.as_mut_ptr().cast::<u8>();
        let (_, f) = unsafe { secondary_call_target(buf_u8, 56, 3).unwrap() };
        assert_eq!(f, fake(3).addr());
        let sub = buf_u8.wrapping_add(56);
        assert_eq!(unsafe { vtable_slot(sub, 5) }, Some(fake(5).addr()));
    }
}
