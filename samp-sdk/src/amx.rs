//! Safe API for a live AMX VM instance.
//!
//! Wraps the `*mut AMX` received from the server + the `amx_Exports` table.
//! Each method here resolves the corresponding `amx_*` function on demand (via
//! [`crate::exports`]) and invokes it with idiomatic Rust error handling.

use crate::cell::{AmxCell, AmxPrimitive, AmxString, Buffer, Ref};
use crate::consts::{AmxExecIdx, AmxFlags};
use crate::error::{AmxError, AmxResult};
// Intentional wildcard: brings in the 40+ marker types of the exported AMX
// functions (`Register`, `Allot`, `Exec`, ...). Listing each one would be
// noisy and fragile when a new function is added to the table.
#[allow(clippy::wildcard_imports)]
use crate::exports::*;
use crate::raw::types::{AMX, AMX_HEADER, AMX_NATIVE_INFO};

#[cfg(feature = "encoding")]
use crate::encoding;

use std::borrow::Cow;
use std::ffi::CString;
use std::ptr::NonNull;

macro_rules! amx_try {
    ($call:expr) => {
        let result = $call;

        if result > 0 {
            return Err(result.into());
        }
    };
}

/// Wrapper over the raw `*mut AMX` and the exported function table.
#[derive(Debug)]
pub struct Amx {
    ptr: *mut AMX,
    fn_table: usize,
}

impl Amx {
    /// Builds the wrapper.
    ///
    /// `ptr` is the pointer received in callbacks such as `AmxLoad`; `fn_table`
    /// is the address resolved during plugin initialization (typically stored
    /// in a global [`AtomicUsize`] read in `Load()` from
    /// [`crate::consts::ServerData::AmxExports`]).
    ///
    /// [`AtomicUsize`]: std::sync::atomic::AtomicUsize
    pub fn new(ptr: *mut AMX, fn_table: usize) -> Amx {
        Amx { ptr, fn_table }
    }

    /// Registers plugin natives in the VM via `amx_Register`.
    ///
    /// Generally called in `AmxLoad` — the `#[native]` macro + `initialize_plugin!`
    /// build the list automatically; only call manually from `raw` code.
    ///
    /// # Errors
    /// Propagates any [`AmxError`] returned by `amx_Register` — typically
    /// `AmxError::NotFound` if a listed native is not declared in the script,
    /// or VM state errors if called outside the load cycle.
    pub fn register(&self, natives: &[AMX_NATIVE_INFO]) -> AmxResult<()> {
        let register = Register::from_table(self.fn_table);
        // `usize` -> `i32`: the `amx_Register` ABI takes the count as `int`.
        // Practical truncation would require >2 billion natives — impossible.
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let len = natives.len() as i32;
        let ptr = natives.as_ptr();

        amx_try!(register(self.ptr, ptr, len));

        Ok(())
    }

    pub(crate) fn allot<T: Sized + AmxPrimitive>(&self, cells: usize) -> AmxResult<Ref<'_, T>> {
        if cells > i32::MAX as usize {
            return Err(AmxError::Memory);
        }

        let allot = Allot::from_table(self.fn_table);

        let mut amx_addr = 0;
        let mut phys_addr = 0;

        // `cells` was validated above as `<= i32::MAX`; cast is safe.
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let cells_i32 = cells as i32;
        amx_try!(allot(
            self.ptr,
            cells_i32,
            &raw mut amx_addr,
            &raw mut phys_addr
        ));

        if phys_addr == 0 {
            return Err(AmxError::Memory);
        }

        unsafe { Ok(Ref::new(amx_addr, phys_addr as *mut T)) }
    }

    /// Executes the public function identified by `index` in the VM.
    ///
    /// Returns the Pawn return value (`i32`). Arguments must have been pushed
    /// via [`push`] (in reverse order) and [`Allocator`] (for strings/arrays)
    /// before this call.
    ///
    /// [`push`]: Amx::push
    ///
    /// # Errors
    /// Propagates any [`AmxError`] from script execution — notably
    /// `Exit`/`Assert` (Pawn aborted), `StackError`/`StackLow`/`HeapLow`
    /// (stack or heap overflow), `Divide`, `Native` (a called native
    /// returned an error) or `Index` if `index` does not match a valid function.
    pub fn exec(&self, index: AmxExecIdx) -> AmxResult<i32> {
        let exec = Exec::from_table(self.fn_table);
        let mut retval = 0;

        amx_try!(exec(self.ptr, &raw mut retval, index.into()));

        Ok(retval)
    }

    /// Index of a native by name (resolved via `amx_FindNative`).
    ///
    /// # Errors
    /// `AmxError::NotFound` if `name` contains an interior NUL byte or if the
    /// native is not registered in the VM.
    pub fn find_native(&self, name: &str) -> AmxResult<i32> {
        let find_native = FindNative::from_table(self.fn_table);
        let c_str = CString::new(name).map_err(|_| AmxError::NotFound)?;
        let mut index = -1;

        amx_try!(find_native(self.ptr, c_str.as_ptr(), &raw mut index));

        Ok(index)
    }

    /// Index of a public function by name — pass the result to [`exec`].
    ///
    /// ```
    /// use samp_sdk::amx::Amx;
    /// use samp_sdk::error::AmxResult;
    /// fn has_on_player_connect(amx: &Amx) -> AmxResult<bool> {
    ///     let idx = amx.find_public("OnPlayerConnect")?;
    ///     Ok(i32::from(idx) >= 0)
    /// }
    /// ```
    ///
    /// [`exec`]: Amx::exec
    ///
    /// # Errors
    /// `AmxError::NotFound` if `name` contains an interior NUL byte or if the
    /// public function is not declared in the Pawn script.
    pub fn find_public(&self, name: &str) -> AmxResult<AmxExecIdx> {
        let find_public = FindPublic::from_table(self.fn_table);
        let c_str = CString::new(name).map_err(|_| AmxError::NotFound)?;
        let mut index = -1;

        amx_try!(find_public(self.ptr, c_str.as_ptr(), &raw mut index));

        Ok(AmxExecIdx::from(index))
    }

    /// `Ref<T>` pointing to a public variable declared in the Pawn script.
    ///
    /// ```rust,no_run
    /// # use samp_sdk::amx::Amx;
    /// # use samp_sdk::error::AmxResult;
    /// # fn check(amx: &Amx) -> AmxResult<()> {
    /// let version = amx.find_pubvar::<f32>("my_plugin_version")?;
    /// // outdated
    /// if *version < 1.0 { }
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// `AmxError::NotFound` if `name` contains an interior NUL byte or if the
    /// pubvar is not declared. `AmxError::MemoryAccess` if the address returned
    /// by the VM is invalid.
    pub fn find_pubvar<T: Sized + AmxPrimitive>(&self, name: &str) -> AmxResult<Ref<'_, T>> {
        let find_pubvar = FindPubVar::from_table(self.fn_table);
        let c_str = CString::new(name).map_err(|_| AmxError::NotFound)?;
        let mut cell_ptr = 0;

        amx_try!(find_pubvar(self.ptr, c_str.as_ptr(), &raw mut cell_ptr));

        self.get_ref(cell_ptr)
    }

    /// Flags of the loaded `.amx`.
    ///
    /// # Errors
    /// Propagates any [`AmxError`] returned by `amx_Flags` — in practice, it
    /// only fails if the internal `AMX*` is corrupted or null.
    pub fn flags(&self) -> AmxResult<AmxFlags> {
        let flags = Flags::from_table(self.fn_table);
        let mut value: u16 = 0;

        amx_try!(flags(self.ptr, &raw mut value));

        Ok(AmxFlags::from_bits_truncate(value))
    }

    /// Resolves an AMX cell (relative address) to a typed [`Ref<T>`].
    ///
    /// # Errors
    /// `AmxError::MemoryAccess` if `address` does not correspond to a valid
    /// cell in the Pawn script address space.
    pub fn get_ref<T: Sized + AmxPrimitive>(&self, address: i32) -> AmxResult<Ref<'_, T>> {
        let get_addr = GetAddr::from_table(self.fn_table);
        let mut dest = 0;
        let mut dest_addr = std::ptr::addr_of_mut!(dest);

        amx_try!(get_addr(self.ptr, address, &raw mut dest_addr));

        if dest_addr.is_null() {
            return Err(AmxError::MemoryAccess);
        }

        unsafe { Ok(Ref::new(address, dest_addr.cast::<T>())) }
    }

    #[inline]
    pub(crate) fn release(&self, address: i32) {
        if let Some(mut amx) = self.amx() {
            let amx = unsafe { amx.as_mut() };
            if address >= 0 && amx.hea > address {
                amx.hea = address;
            }
        }
    }

    /// Pushes an `AmxCell` value onto the VM stack. Use **in reverse order**
    /// of the public function's arguments before calling [`exec`].
    ///
    /// [`exec`]: Amx::exec
    ///
    /// # Errors
    /// Propagates any [`AmxError`] from `amx_Push` — typically
    /// `AmxError::StackError`/`StackLow` if the stack is full.
    pub fn push<'a, T: AmxCell<'a>>(&'a self, value: T) -> AmxResult<()> {
        let push = Push::from_table(self.fn_table);

        amx_try!(push(self.ptr, value.as_cell()));

        Ok(())
    }

    /// Length in characters of an AMX string at address `value`.
    ///
    /// # Errors
    /// `AmxError::MemoryAccess` if `value` does not point to valid memory in
    /// the script space. Other [`AmxError`] are propagated from `amx_StrLen`.
    pub fn strlen(&self, value: *const i32) -> AmxResult<usize> {
        let strlen = StrLen::from_table(self.fn_table);
        let mut len = 0;
        amx_try!(strlen(value, &raw mut len));
        // `len` returned by `amx_StrLen` is always >= 0 (a negative value
        // would become an error via `amx_try!`).
        #[allow(clippy::cast_sign_loss)]
        Ok(len as usize)
    }

    /// Creates an [`Allocator`] bound to this `Amx`.
    ///
    /// All memory allocated via [`Allocator::allot`]/[`Allocator::allot_buffer`]/
    /// [`Allocator::allot_string`] is released automatically when the
    /// `Allocator` goes out of scope (`Drop`). Keep it alive while using the
    /// returned references.
    #[must_use]
    pub fn allocator(&self) -> Allocator<'_> {
        Allocator::new(self)
    }

    /// Raw pointer to the `AMX` (non-null) or `None` if constructed with null.
    #[must_use]
    pub fn amx(&self) -> Option<NonNull<AMX>> {
        NonNull::new(self.ptr)
    }

    /// Raw pointer to the `AMX_HEADER` of the loaded `.amx`.
    #[must_use]
    pub fn header(&self) -> Option<NonNull<AMX_HEADER>> {
        let amx = NonNull::new(self.ptr)?;
        NonNull::new(unsafe { (*amx.as_ptr()).base.cast::<AMX_HEADER>() })
    }
}

/// AMX heap allocator with automatic release (RAII).
///
/// Captures the value of `amx.hea` at creation time and restores it on `Drop`,
/// freeing everything allocated by the `Allocator` in a single operation.
/// Do not use multiple nested `Allocator`s — each one restores to a different
/// heap point.
pub struct Allocator<'amx> {
    amx: &'amx Amx,
    release_addr: i32,
}

impl<'amx> Allocator<'amx> {
    pub(crate) fn new(amx: &'amx Amx) -> Allocator<'amx> {
        let amx_ptr = amx
            .amx()
            .expect("Allocator::new() received Amx with null pointer")
            .as_ptr();
        let release_addr = unsafe { (*amx_ptr).hea };

        Allocator { amx, release_addr }
    }

    /// Allocates a single cell on the heap and initializes it with `init_value`.
    ///
    /// # Errors
    /// `AmxError::Memory` if the VM heap is exhausted.
    pub fn allot<T: Sized + AmxPrimitive>(&self, init_value: T) -> AmxResult<Ref<'_, T>> {
        let mut cell = self.amx.allot(1)?;
        *cell = init_value;

        Ok(cell)
    }

    /// Allocates `size` cells on the heap and returns a [`Buffer`] covering that region.
    ///
    /// # Errors
    /// `AmxError::Memory` if the VM heap is exhausted or if `size` exceeds
    /// `i32::MAX`.
    pub fn allot_buffer(&self, size: usize) -> AmxResult<Buffer<'_>> {
        let buffer = self.amx.allot(size)?;

        Ok(Buffer::new(buffer, size))
    }

    /// Allocates space for `array.len()` cells and copies the content (`AmxCell::as_cell`).
    ///
    /// # Errors
    /// `AmxError::Memory` if the VM heap is exhausted.
    pub fn allot_array<T>(&self, array: &[T]) -> AmxResult<Buffer<'_>>
    where
        T: AmxCell<'amx> + AmxPrimitive,
    {
        let mut buffer = self.allot_buffer(array.len())?;

        let slice = buffer.as_mut_slice();

        for (idx, item) in array.iter().enumerate() {
            slice[idx] = item.as_cell();
        }

        Ok(buffer)
    }

    /// Allocates space for a string and copies `string` (configured encoding),
    /// adding the `0` terminator at the end.
    ///
    /// # Errors
    /// `AmxError::Memory` if the VM heap is exhausted.
    pub fn allot_string(&self, string: &str) -> AmxResult<AmxString<'_>> {
        let bytes = Allocator::string_bytes(string);
        let buffer = self.allot_buffer(bytes.len() + 1)?;

        Ok(unsafe { AmxString::new(buffer, bytes.as_ref()) })
    }

    fn string_bytes(string: &str) -> Cow<'_, [u8]> {
        #[cfg(feature = "encoding")]
        return encoding::get().encode(string).0;

        #[cfg(not(feature = "encoding"))]
        return Cow::from(string.as_bytes());
    }
}

impl Drop for Allocator<'_> {
    fn drop(&mut self) {
        // AMX::release never fails
        self.amx.release(self.release_addr);
    }
}
