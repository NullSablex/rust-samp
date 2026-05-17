//! Parsing of arguments from a Pawn native call.
//!
//! The AMX delivers arguments in a `*mut i32` pointer where:
//! - `args[0]` = number of bytes used by the arguments (not the count)
//! - `args[1..]` = the cells with each argument, in signature order
//!
//! This module wraps that indirection and converts each cell to the correct
//! Rust type via [`AmxCell`].

use crate::amx::Amx;
use crate::cell::AmxCell;

/// Typed list of arguments for a native function.
///
/// Generally the `#[native]` proc macro builds and consumes an `Args` automatically
/// — call manually only in `raw` natives that receive `(amx, args)` directly.
pub struct Args<'a> {
    amx: &'a Amx,
    params: *const i32,
    offset: usize,
}

impl<'a> Args<'a> {
    /// Builds from the `Amx` and the `args` pointer received by the native.
    ///
    /// # Example
    /// ```
    /// use samp_sdk::args::Args;
    /// use samp_sdk::amx::Amx;
    /// use samp_sdk::cell::AmxString;
    /// # use samp_sdk::raw::types::AMX;
    ///
    /// // native RawNative(const say_that[]);
    /// extern "C" fn raw_native(amx: *mut AMX, args: *mut i32) -> i32 {
    ///     # let amx_exports = 0;
    ///     let amx = Amx::new(amx, amx_exports);
    ///     let mut args = Args::new(&amx, args);
    ///     let Some(text) = args.next_arg::<AmxString>() else { return 0 };
    ///     println!("RawNative: {}", &*text);
    ///     1
    /// }
    /// ```
    #[must_use]
    pub fn new(amx: &'a Amx, params: *const i32) -> Args<'a> {
        Args {
            amx,
            params,
            offset: 0,
        }
    }

    /// Next argument in signature order. `None` at the end of the list.
    pub fn next_arg<T: AmxCell<'a> + 'a>(&mut self) -> Option<T> {
        let result = self.get(self.offset);
        self.offset += 1;

        result
    }

    /// Argument at position `offset` (zero-indexed). `None` if out of bounds.
    #[must_use]
    pub fn get<T: AmxCell<'a> + 'a>(&self, offset: usize) -> Option<T> {
        if offset >= self.count() {
            return None;
        }

        unsafe { T::from_raw(self.amx, self.params.add(offset + 1).read()).ok() }
    }

    /// Resets the [`next_arg`] cursor back to the start of the list.
    ///
    /// [`next_arg`]: Args::next_arg
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// How many arguments were received.
    ///
    /// Reads from `args[0]` (total bytes) and divides by 4 (cell size).
    /// Negative or zero values return `0` — defense against dirty pointers.
    #[must_use]
    pub fn count(&self) -> usize {
        let raw = unsafe { self.params.read() };
        if raw <= 0 {
            return 0;
        }
        // `raw > 0` was validated above; `/ 4` keeps it positive.
        #[allow(clippy::cast_sign_loss)]
        let count = (raw / 4) as usize;
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_with_zero_returns_zero() {
        let data: [i32; 1] = [0];
        let amx = Amx::new(std::ptr::null_mut(), 0);
        let args = Args::new(&amx, data.as_ptr());
        assert_eq!(args.count(), 0);
    }

    #[test]
    fn count_with_negative_returns_zero() {
        let data: [i32; 1] = [-8];
        let amx = Amx::new(std::ptr::null_mut(), 0);
        let args = Args::new(&amx, data.as_ptr());
        assert_eq!(args.count(), 0);
    }

    #[test]
    fn count_with_valid_args() {
        // 3 arguments = 12 bytes (3 * 4)
        let data: [i32; 4] = [12, 100, 200, 300];
        let amx = Amx::new(std::ptr::null_mut(), 0);
        let args = Args::new(&amx, data.as_ptr());
        assert_eq!(args.count(), 3);
    }

    #[test]
    fn get_out_of_bounds_returns_none() {
        let data: [i32; 2] = [4, 42]; // 1 argument
        let amx = Amx::new(std::ptr::null_mut(), 0);
        let args = Args::new(&amx, data.as_ptr());
        // offset == count should return None
        assert!(args.get::<crate::cell::Ref<i32>>(1).is_none());
    }

    #[test]
    fn reset_resets_offset() {
        let data: [i32; 1] = [0];
        let amx = Amx::new(std::ptr::null_mut(), 0);
        let mut args = Args::new(&amx, data.as_ptr());
        args.offset = 5;
        args.reset();
        assert_eq!(args.offset, 0);
    }
}
