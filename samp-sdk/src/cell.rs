//! Smart pointers for accessing AMX VM cells from safe Rust code.
//!
//! A "cell" is the native Pawn VM type: a 32-bit integer (`i32`) that can
//! represent `int`, `float` (via bit reinterpretation) or a pointer relative
//! to the AMX heap/data. The types in this module wrap those cells with
//! Rust semantics:
//!
//! - [`Ref<T>`]: typed pointer to a cell (by-reference output of natives).
//! - [`Buffer`] / [`UnsizedBuffer`]: array of contiguous cells.
//! - [`AmxString`]: native Pawn string (cell vector with `0` terminator).
//! - [`AmxCell`], [`AmxPrimitive`], [`CellConvert`]: conversion traits.

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use crate::amx::Amx;
use crate::error::AmxResult;

pub mod buffer;
pub mod repr;
pub mod string;

pub use buffer::{Buffer, UnsizedBuffer};
pub use repr::{AmxCell, AmxPrimitive, CellConvert};
pub use string::AmxString;

/// Typed pointer to a live cell in the AMX heap/data.
///
/// Implements [`Deref`] and [`DerefMut`] for `T`, allowing reading and writing
/// the cell directly (`*r`). The `'amx` lifetime ensures the `Ref` does not
/// outlive its `Amx`, avoiding dangling pointers.
pub struct Ref<'amx, T: Sized + AmxPrimitive> {
    amx_addr: i32,
    phys_addr: *mut T,
    marker: PhantomData<&'amx Amx>,
}

impl<'amx, T: Sized + AmxPrimitive> Ref<'amx, T> {
    /// Creates a `Ref` from the (AMX address, physical address) pair, already resolved.
    ///
    /// Prefer obtaining `Ref` via [`Amx::get_ref`] or via automatic parsing of
    /// native arguments — this direct API is the low-level path used
    /// internally by the SDK.
    ///
    /// # Safety
    /// `phys_addr` must point to a live `T` cell for as long as this
    /// `Ref` exists and must be aligned for `T` (debug asserts validate both
    /// conditions in builds with `debug_assertions`).
    ///
    /// [`Amx::get_ref`]: crate::amx::Amx::get_ref
    pub unsafe fn new(amx_addr: i32, phys_addr: *mut T) -> Ref<'amx, T> {
        debug_assert!(!phys_addr.is_null(), "Ref::new() received null pointer");
        debug_assert!(
            (phys_addr as usize).is_multiple_of(std::mem::align_of::<T>()),
            "Ref::new() received misaligned pointer for {}",
            std::any::type_name::<T>()
        );
        Ref {
            amx_addr,
            phys_addr,
            marker: PhantomData,
        }
    }

    /// Address of the cell in the AMX address space (not the physical pointer).
    ///
    /// This is the value the VM sees — useful when passing the cell back into
    /// calls to other AMX functions.
    #[inline]
    #[must_use]
    pub fn address(&self) -> i32 {
        self.amx_addr
    }

    /// Physical (host) pointer to the cell.
    #[inline]
    #[must_use]
    pub fn as_ptr(&self) -> *const T {
        self.phys_addr
    }

    /// Mutable physical pointer to the cell.
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.phys_addr
    }
}

impl<T: Sized + AmxPrimitive> Deref for Ref<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.phys_addr }
    }
}

impl<T: Sized + AmxPrimitive> DerefMut for Ref<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.phys_addr }
    }
}

impl<'amx, T: Sized + AmxPrimitive> AmxCell<'amx> for Ref<'amx, T> {
    fn from_raw(amx: &'amx Amx, cell: i32) -> AmxResult<Ref<'amx, T>> {
        amx.get_ref(cell)
    }

    fn as_cell(&self) -> i32 {
        self.address()
    }
}
