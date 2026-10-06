//! Typed calls into Pawn publics — [`Amx::call_public`].
//!
//! [`exec_public!`] covers the same ground, but a macro cannot be passed
//! around, stored, or written generically, and it makes the caller mark which
//! arguments need heap allocation (`expr => string`). This module carries that
//! decision in the type instead: a `&str` allocates and copies, an `i32` goes
//! straight into a cell, and the call site says only what it means.
//!
//! ```rust,no_run
//! # use samp_sdk::amx::Amx;
//! # fn example(amx: &Amx) -> samp_sdk::error::AmxResult<()> {
//! amx.call_public("OnPlayerScored", (7, "headshot", 1.5))?;
//! amx.call_public("OnRoundEnd", ())?;
//! # Ok(()) }
//! ```
//!
//! Pawn reads the arguments in declaration order, which means the VM wants them
//! pushed backwards; the tuple implementations do that, so the call site lists
//! them the way the Pawn signature does.
//!
//! Types accepted as arguments: the integer primitives, `f32` (as `Float:`),
//! `bool`, `&str` and `String` (allocated on the AMX heap and passed as
//! `const arg[]`), `&[i32]` and `&Vec<i32>` (likewise, as `arg[]`), and the
//! cell types `AmxString`, `Buffer` and `Ref<T>` that a native already receives.
//! Anything else — a custom [`AmxCell`] — keeps using [`exec_public!`].
//!
//! [`exec_public!`]: crate::exec_public
//! [`AmxCell`]: crate::cell::AmxCell

use crate::amx::{Allocator, Amx};
use crate::cell::{AmxPrimitive, AmxString, Buffer, Ref};
use crate::error::AmxResult;

/// A value that can be passed to a Pawn public by [`Amx::call_public`].
///
/// Implemented for the types listed in the [module docs](self); the `allocator`
/// is there for the ones that need memory inside the VM, and is ignored by the
/// ones that fit in a cell.
pub trait PublicArg<'amx> {
    /// Pushes `self` onto the AMX stack.
    ///
    /// # Errors
    /// Propagates the VM's error when the push fails, or when the allocation an
    /// argument needs does not fit in the AMX heap.
    fn push_to(self, amx: &'amx Amx, allocator: &Allocator<'amx>) -> AmxResult<()>;
}

/// Cell-sized values: pushed as they are.
macro_rules! impl_cell_arg {
    ($($type:ty),* $(,)?) => {
        $(
            impl<'amx> PublicArg<'amx> for $type {
                #[inline]
                fn push_to(self, amx: &'amx Amx, _allocator: &Allocator<'amx>) -> AmxResult<()> {
                    amx.push(self)
                }
            }
        )*
    };
}

impl_cell_arg!(i8, u8, i16, u16, i32, u32, usize, isize, f32, bool);

impl<'amx> PublicArg<'amx> for AmxString<'amx> {
    #[inline]
    fn push_to(self, amx: &'amx Amx, _allocator: &Allocator<'amx>) -> AmxResult<()> {
        amx.push(self)
    }
}

impl<'amx> PublicArg<'amx> for Buffer<'amx> {
    #[inline]
    fn push_to(self, amx: &'amx Amx, _allocator: &Allocator<'amx>) -> AmxResult<()> {
        amx.push(self)
    }
}

impl<'amx, T: AmxPrimitive> PublicArg<'amx> for Ref<'amx, T> {
    #[inline]
    fn push_to(self, amx: &'amx Amx, _allocator: &Allocator<'amx>) -> AmxResult<()> {
        amx.push(self)
    }
}

/// Rust strings are copied into the AMX heap and passed as `const arg[]`.
impl<'amx> PublicArg<'amx> for &str {
    #[inline]
    fn push_to(self, amx: &'amx Amx, allocator: &Allocator<'amx>) -> AmxResult<()> {
        let string = allocator.allot_string(self)?;
        amx.push(string)
    }
}

impl<'amx> PublicArg<'amx> for &String {
    #[inline]
    fn push_to(self, amx: &'amx Amx, allocator: &Allocator<'amx>) -> AmxResult<()> {
        self.as_str().push_to(amx, allocator)
    }
}

/// Cell slices are copied into the AMX heap and passed as `arg[]`.
impl<'amx> PublicArg<'amx> for &[i32] {
    #[inline]
    fn push_to(self, amx: &'amx Amx, allocator: &Allocator<'amx>) -> AmxResult<()> {
        let array = allocator.allot_array(self)?;
        amx.push(array)
    }
}

impl<'amx> PublicArg<'amx> for &Vec<i32> {
    #[inline]
    fn push_to(self, amx: &'amx Amx, allocator: &Allocator<'amx>) -> AmxResult<()> {
        self.as_slice().push_to(amx, allocator)
    }
}

/// An argument list for [`Amx::call_public`] — a tuple of [`PublicArg`]s, up to
/// twelve of them, or `()` for a public that takes none.
pub trait PublicArgs<'amx> {
    /// Pushes every argument, last one first, as the VM expects.
    ///
    /// # Errors
    /// Propagates the first failure from [`PublicArg::push_to`].
    fn push_all(self, amx: &'amx Amx, allocator: &Allocator<'amx>) -> AmxResult<()>;
}

impl PublicArgs<'_> for () {
    #[inline]
    fn push_all(self, _amx: &Amx, _allocator: &Allocator<'_>) -> AmxResult<()> {
        Ok(())
    }
}

/// Generates the tuple implementations. Each one pushes in reverse: the VM pops
/// the first declared argument last.
macro_rules! impl_args_tuple {
    ($(($($name:ident),+)),* $(,)?) => {
        $(
            #[allow(non_snake_case)]
            impl<'amx, $($name: PublicArg<'amx>),+> PublicArgs<'amx> for ($($name,)+) {
                #[inline]
                fn push_all(self, amx: &'amx Amx, allocator: &Allocator<'amx>) -> AmxResult<()> {
                    let ($($name,)+) = self;
                    impl_args_tuple!(@push amx, allocator, $($name),+);
                    Ok(())
                }
            }
        )*
    };
    (@push $amx:ident, $al:ident, $head:ident) => {
        $head.push_to($amx, $al)?;
    };
    (@push $amx:ident, $al:ident, $head:ident, $($tail:ident),+) => {
        impl_args_tuple!(@push $amx, $al, $($tail),+);
        $head.push_to($amx, $al)?;
    };
}

impl_args_tuple!(
    (A),
    (A, B),
    (A, B, C),
    (A, B, C, D),
    (A, B, C, D, E),
    (A, B, C, D, E, F),
    (A, B, C, D, E, F, G),
    (A, B, C, D, E, F, G, H),
    (A, B, C, D, E, F, G, H, I),
    (A, B, C, D, E, F, G, H, I, J),
    (A, B, C, D, E, F, G, H, I, J, K),
    (A, B, C, D, E, F, G, H, I, J, K, L),
);
