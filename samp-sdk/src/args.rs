//! Workaround to parse input of natives functions.
use crate::amx::Amx;
use crate::cell::AmxCell;

/// A wrapper of a list of arguments of a native function.
pub struct Args<'a> {
    amx: &'a Amx,
    args: *const i32,
    offset: usize,
}

impl<'a> Args<'a> {
    /// Creates a list from [`Amx`] and arguments.
    ///
    /// # Example
    /// ```
    /// use samp_sdk::args::Args;
    /// use samp_sdk::amx::Amx;
    /// use samp_sdk::cell::AmxString;
    /// # use samp_sdk::raw::types::AMX;
    ///
    /// // native: RawNative(const say_that[]);
    /// extern "C" fn raw_native(amx: *mut AMX, args: *mut i32) -> i32 {
    ///     # let amx_exports = 0;
    ///     // let amx_exports = ...;
    ///     let amx = Amx::new(amx, amx_exports);
    ///     let mut args = Args::new(&amx, args);
    ///
    ///     let say_what = match args.next_arg::<AmxString>() {
    ///         Some(string) => string.to_string(),
    ///         None => {
    ///             println!("RawNative error: no argument");
    ///             return 0;
    ///         }
    ///     };
    ///
    ///     println!("RawNative: {}", say_what);
    ///
    ///     return 1;
    /// }
    /// ```
    ///
    /// [`Amx`]: ../amx/struct.Amx.html
    pub fn new(amx: &'a Amx, args: *const i32) -> Args<'a> {
        Args {
            amx,
            args,
            offset: 0,
        }
    }

    /// Return the next argument in the list.
    ///
    /// When there is no arguments left returns `None`.
    pub fn next_arg<T: AmxCell<'a> + 'a>(&mut self) -> Option<T> {
        let result = self.get(self.offset);
        self.offset += 1;

        result
    }

    /// Get an argument by position, if there is no argument in given location, returns `None`.
    ///
    /// # Example
    /// ```
    /// use samp_sdk::args::Args;
    /// use samp_sdk::amx::Amx;
    /// use samp_sdk::cell::Ref;
    /// # use samp_sdk::raw::types::AMX;
    ///
    /// // native: NativeFn(player_id, &Float:health, &Float:armor);
    /// extern "C" fn raw_native(amx: *mut AMX, args: *mut i32) -> i32 {
    ///     # let amx_exports = 0;
    ///     // let amx_exports = ...;
    ///     let amx = Amx::new(amx, amx_exports);
    ///     let args = Args::new(&amx, args);
    ///
    ///     // change only armor
    ///     args.get::<Ref<f32>>(2)
    ///         .map(|mut armor| *armor = 255.0);
    ///
    ///     return 1;
    /// }
    /// ```
    pub fn get<T: AmxCell<'a> + 'a>(&self, offset: usize) -> Option<T> {
        if offset >= self.count() {
            return None;
        }

        unsafe { T::from_raw(self.amx, self.args.add(offset + 1).read()).ok() }
    }

    /// Reset a read offset for [`next()`] method.
    ///
    /// [`next()`]: #method.next
    pub fn reset(&mut self) {
        self.offset = 0;
    }

    /// Get count of arguments in the list.
    pub fn count(&self) -> usize {
        let raw = unsafe { self.args.read() };
        if raw <= 0 {
            return 0;
        }
        (raw / 4) as usize
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
        // 3 argumentos = 12 bytes (3 * 4)
        let data: [i32; 4] = [12, 100, 200, 300];
        let amx = Amx::new(std::ptr::null_mut(), 0);
        let args = Args::new(&amx, data.as_ptr());
        assert_eq!(args.count(), 3);
    }

    #[test]
    fn get_out_of_bounds_returns_none() {
        let data: [i32; 2] = [4, 42]; // 1 argumento
        let amx = Amx::new(std::ptr::null_mut(), 0);
        let args = Args::new(&amx, data.as_ptr());
        // offset == count deve retornar None
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
