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
use crate::raw::functions::AmxNative;
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

/// Reads a field of the `#[repr(C, packed)]` `AMX` via `read_unaligned` (taking
/// a reference to a packed field is unsound). `None` when the pointer is null.
macro_rules! read_reg {
    ($self:ident . $field:ident) => {
        NonNull::new($self.ptr)
            .map(|amx| unsafe { std::ptr::addr_of!((*amx.as_ptr()).$field).read_unaligned() })
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

    /// Calls a native registered by **another plugin** in the same AMX.
    ///
    /// SA-MP plugins inject their natives into every loaded AMX via
    /// `amx_Register`, which writes a host function pointer into the
    /// native's entry inside the `AMX_HEADER` natives table. This helper
    /// resolves the name through `amx_FindNative`, reads that function
    /// pointer back, builds the `params` block in the AMX convention
    /// (first cell = `argc * sizeof(cell)`, then the arguments), and
    /// invokes the native.
    ///
    /// Integer arguments are passed as their `i32` value. Floats are
    /// passed bit-cast to `i32` (use [`f32::to_bits`] then
    /// [`i32::from_ne_bytes`] on `to_ne_bytes`, or `f32::to_bits() as i32`).
    /// String and array arguments are AMX cell addresses returned by
    /// [`Allocator::allot_string`]/[`Allocator::allot_buffer`] — same
    /// marshalling as for [`exec_public`](crate::exec_public).
    ///
    /// # Example
    /// ```rust,ignore
    /// // Calling Streamer_CreateDynamicObject from a Rust plugin
    /// fn on_amx_load(&mut self, amx: &Amx) -> AmxResult<()> {
    ///     let model_id: i32 = 1337;
    ///     #[allow(clippy::cast_possible_wrap)]
    ///     let x = 100.0_f32.to_bits() as i32;
    ///     let y = 200.0_f32.to_bits() as i32;
    ///     let z =  10.0_f32.to_bits() as i32;
    ///     let object_id = amx.call_native(
    ///         "Streamer_CreateDynamicObject",
    ///         &[model_id, x, y, z, 0, 0, 0],
    ///     )?;
    ///     log::info!("created dynamic object id={object_id}");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    /// - [`AmxError::NotFound`] if `name` contains an interior NUL byte,
    ///   the native is not registered, or its address is still zero
    ///   (registered name but no host pointer attached).
    /// - [`AmxError::MemoryAccess`] if the AMX header cannot be read.
    /// - [`AmxError::Index`] if the resolved index is out of range for
    ///   the natives table reported by the AMX header.
    /// - Any [`AmxError`] propagated from the called native via
    ///   `amx.error` (re-raised by the caller through `amx_try!`).
    pub fn call_native(&self, name: &str, params: &[i32]) -> AmxResult<i32> {
        let index = self.find_native(name)?;
        if index < 0 {
            return Err(AmxError::NotFound);
        }

        let header_ptr = self.header().ok_or(AmxError::MemoryAccess)?;
        // SAFETY: `header()` returned NonNull, and the AMX is alive for
        // the duration of `&self`.
        let (natives_off, libraries_off, defsize) = unsafe {
            let h = header_ptr.as_ptr();
            (
                std::ptr::read_unaligned(&raw const (*h).natives),
                std::ptr::read_unaligned(&raw const (*h).libraries),
                std::ptr::read_unaligned(&raw const (*h).defsize),
            )
        };

        if defsize <= 0 || libraries_off < natives_off {
            return Err(AmxError::MemoryAccess);
        }
        let defsize_i32 = i32::from(defsize);
        let table_bytes = libraries_off - natives_off;
        let num_natives = table_bytes / defsize_i32;
        if index >= num_natives {
            return Err(AmxError::Index);
        }

        let amx_ptr = self.amx().ok_or(AmxError::MemoryAccess)?;
        // SAFETY: `amx_ptr` is NonNull and points to the live AMX.
        let base = unsafe { (*amx_ptr.as_ptr()).base };
        if base.is_null() {
            return Err(AmxError::MemoryAccess);
        }

        let entry_off = natives_off + index * defsize_i32;
        // SAFETY: `entry_off` is within the natives table bounded by
        // (libraries - natives), which the header advertises as part of
        // the AMX-mapped region pointed to by `base`.
        let entry_ptr = unsafe { base.offset(entry_off as isize) };

        // First 4 bytes of each entry — both `AMX_FUNCSTUB` and
        // `ANX_FUNCSTUBNT` start with `u32 address`, the host function
        // pointer written by `amx_Register`.
        let address = unsafe { std::ptr::read_unaligned(entry_ptr.cast::<u32>()) };
        if address == 0 {
            return Err(AmxError::NotFound);
        }

        // SAFETY: SA-MP / open.mp are 32-bit; the AMX cell width and host
        // function pointer width are both 4 bytes. `address` came from
        // `amx_Register`, which writes a valid `AmxNative` pointer.
        let native: AmxNative = unsafe { std::mem::transmute(address as usize) };

        // Build the params block: `[argc * sizeof(cell), arg0, arg1, ...]`.
        // Bytes, not cells — matches the convention every AMX native
        // implementation reads (`params[0] / sizeof(cell)` to recover argc).
        let mut buf: Vec<i32> = Vec::with_capacity(params.len() + 1);
        // `params.len()` bounded by `i32::MAX` in practice; the AMX
        // would have failed long before reaching 2 billion args.
        #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
        let argc_bytes = (params.len() as i32) * 4;
        buf.push(argc_bytes);
        buf.extend_from_slice(params);

        let retval = native(self.ptr, buf.as_mut_ptr());
        // Surface VM-side errors set by the native into `amx.error`.
        // SAFETY: `amx_ptr` already validated above.
        let err = unsafe { (*amx_ptr.as_ptr()).error };
        if err > 0 {
            return Err(err.into());
        }
        Ok(retval)
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

    // ---- VM register accessors (all `None` when the pointer is null) ----

    /// Current instruction pointer (`cip`) — a code-segment offset in a debug
    /// hook. Read as `u32`.
    #[must_use]
    pub fn cip(&self) -> Option<u32> {
        read_reg!(self.cip).map(i32::cast_unsigned)
    }

    /// Current frame pointer (`frm`); local/argument symbols are addressed
    /// relative to it.
    #[must_use]
    pub fn frame(&self) -> Option<i32> {
        read_reg!(self.frm)
    }

    /// Current stack pointer (`stk`).
    #[must_use]
    pub fn stack(&self) -> Option<i32> {
        read_reg!(self.stk)
    }

    /// Current heap pointer (`hea`).
    #[must_use]
    pub fn heap(&self) -> Option<i32> {
        read_reg!(self.hea)
    }

    /// Top of the stack (`stp`) — the upper bound of the data address space.
    #[must_use]
    pub fn stp(&self) -> Option<i32> {
        read_reg!(self.stp)
    }

    /// Resolves a data-segment address to a raw pointer with the same bounds
    /// checking as `amx_GetAddr`, without going through the exported function
    /// table. Returns `None` when the address falls in the free region between
    /// heap and stack, is negative, or is past the top of the stack.
    ///
    /// Unlike [`get_ref`](Self::get_ref), this works inside a debug hook, where
    /// no native call context is available. It is the building block for
    /// [`read_cell`](Self::read_cell)/[`write_cell`](Self::write_cell).
    fn data_ptr(&self, addr: i32) -> Option<*mut u8> {
        let amx = NonNull::new(self.ptr)?.as_ptr();
        let base = unsafe { std::ptr::addr_of!((*amx).base).read_unaligned() };
        if base.is_null() {
            return None;
        }
        let data_field = unsafe { std::ptr::addr_of!((*amx).data).read_unaligned() };
        let hea = unsafe { std::ptr::addr_of!((*amx).hea).read_unaligned() };
        let stk = unsafe { std::ptr::addr_of!((*amx).stk).read_unaligned() };
        let stp = unsafe { std::ptr::addr_of!((*amx).stp).read_unaligned() };

        // `data` is `amx->data` when set, otherwise `amx->base + header->dat`.
        let data = if data_field.is_null() {
            let hdr = base.cast::<AMX_HEADER>();
            let dat = unsafe { std::ptr::addr_of!((*hdr).dat).read_unaligned() };
            unsafe { base.add(usize::try_from(dat).ok()?) }
        } else {
            data_field
        };

        // Same valid region as `amx_GetAddr`: reject the active heap/stack gap
        // and anything outside `[0, stp)`.
        if (addr >= hea && addr < stk) || addr < 0 || addr >= stp {
            return None;
        }
        Some(unsafe { data.add(usize::try_from(addr).ok()?) })
    }

    /// Reads a 32-bit cell from the data segment at `addr`, validating bounds
    /// like `amx_GetAddr`. Returns `None` if the address is inaccessible.
    ///
    /// Reads byte-wise (no alignment assumption). Usable from a debug hook.
    #[must_use]
    pub fn read_cell(&self, addr: i32) -> Option<i32> {
        let ptr = self.data_ptr(addr)?;
        let mut buf = [0u8; 4];
        unsafe { std::ptr::copy_nonoverlapping(ptr, buf.as_mut_ptr(), 4) };
        Some(i32::from_ne_bytes(buf))
    }

    /// Writes a 32-bit cell to the data segment at `addr`, validating bounds
    /// like `amx_GetAddr`. Returns `false` if the address is inaccessible.
    ///
    /// Writes byte-wise (no alignment assumption). Usable from a debug hook to
    /// edit a variable while the VM is paused.
    pub fn write_cell(&self, addr: i32, value: i32) -> bool {
        let Some(ptr) = self.data_ptr(addr) else {
            return false;
        };
        let buf = value.to_ne_bytes();
        unsafe { std::ptr::copy_nonoverlapping(buf.as_ptr(), ptr, 4) };
        true
    }

    /// Installs a debug hook callback into this VM (`amx->debug = cb`), the
    /// equivalent of `amx_SetDebugHook`. The VM then calls `cb` on every line,
    /// provided the `.amx` was compiled with `-d2`/`-d3`.
    ///
    /// The callback runs on the VM thread and crosses the FFI boundary, so it
    /// must never unwind (no panics).
    pub fn install_debug_hook(&self, cb: crate::raw::functions::AmxDebug) {
        if let Some(amx) = NonNull::new(self.ptr) {
            unsafe { std::ptr::addr_of_mut!((*amx.as_ptr()).debug).write_unaligned(cb) };
        }
    }

    /// Removes a previously installed debug hook, restoring `amx->debug` to a
    /// no-op callback that returns `AMX_ERR_NONE`.
    pub fn remove_debug_hook(&self) {
        extern "C" fn noop(_amx: *mut AMX) -> i32 {
            0
        }
        self.install_debug_hook(noop);
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

#[cfg(test)]
mod vm_tests {
    use super::Amx;
    use crate::raw::types::AMX;
    use std::mem::MaybeUninit;

    /// Builds a synthetic `AMX` over `data` and runs `f` with an `Amx` wrapping
    /// it. Only the fields the VM accessors read are initialized (`base`/`data`/
    /// register fields); `data` non-null means `data_ptr` uses it directly,
    /// without needing a real `AMX_HEADER`.
    ///
    /// Region layout: valid data is `[0, stp)` minus the active heap/stack gap
    /// `[hea, stk)` — mirroring `amx_GetAddr`. Here `stp = data.len()`.
    fn with_amx(data: &mut [u8], cip: i32, frm: i32, hea: i32, stk: i32, f: impl FnOnce(&Amx)) {
        let stp = i32::try_from(data.len()).unwrap();
        let mut raw = MaybeUninit::<AMX>::uninit();
        let p = raw.as_mut_ptr();
        unsafe {
            let base = data.as_mut_ptr();
            std::ptr::addr_of_mut!((*p).base).write_unaligned(base);
            std::ptr::addr_of_mut!((*p).data).write_unaligned(base);
            std::ptr::addr_of_mut!((*p).cip).write_unaligned(cip);
            std::ptr::addr_of_mut!((*p).frm).write_unaligned(frm);
            std::ptr::addr_of_mut!((*p).hea).write_unaligned(hea);
            std::ptr::addr_of_mut!((*p).stk).write_unaligned(stk);
            std::ptr::addr_of_mut!((*p).stp).write_unaligned(stp);
        }
        let amx = Amx::new(p, 0);
        f(&amx);
    }

    #[test]
    fn registers_read_back() {
        let mut data = vec![0u8; 256];
        with_amx(&mut data, 40, 100, 64, 192, |amx| {
            assert_eq!(amx.cip(), Some(40));
            assert_eq!(amx.frame(), Some(100));
            assert_eq!(amx.heap(), Some(64));
            assert_eq!(amx.stack(), Some(192));
            assert_eq!(amx.stp(), Some(256));
        });
    }

    #[test]
    fn read_write_cell_roundtrip_and_bounds() {
        let mut data = vec![0u8; 256];
        // Seed a global at addr 0 (below the heap/stack gap [64,192)).
        data[0..4].copy_from_slice(&7i32.to_ne_bytes());
        with_amx(&mut data, 40, 100, 64, 192, |amx| {
            // Valid below the gap.
            assert_eq!(amx.read_cell(0), Some(7));
            // Valid above the stack pointer (addr 200 in [192,256)).
            assert!(amx.write_cell(200, 0x1234_5678));
            assert_eq!(amx.read_cell(200), Some(0x1234_5678));
            // Inside the active heap/stack gap: rejected like amx_GetAddr.
            assert_eq!(amx.read_cell(100), None);
            assert!(!amx.write_cell(100, 1));
            // Negative and past the top of the stack: rejected.
            assert_eq!(amx.read_cell(-4), None);
            assert_eq!(amx.read_cell(256), None);
            assert_eq!(amx.read_cell(260), None);
        });
    }

    #[test]
    fn null_amx_is_safe() {
        let amx = Amx::new(std::ptr::null_mut(), 0);
        assert_eq!(amx.cip(), None);
        assert_eq!(amx.frame(), None);
        assert_eq!(amx.stp(), None);
        assert_eq!(amx.read_cell(0), None);
        assert!(!amx.write_cell(0, 1));
        // Installing/removing a hook on a null AMX must not crash.
        amx.remove_debug_hook();
    }
}
