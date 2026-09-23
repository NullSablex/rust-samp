//! The AMX functions that exist only on Open Multiplayer.
//!
//! `getAmxFunctions()` returns 52 entries; the table SA-MP hands a plugin stops
//! at 44. The eight extra ones are wrapped here, behind a check of which table
//! is actually in use — resolving them against a SA-MP table would read past
//! its end and call whatever happened to follow it in memory.
//!
//! So every method returns [`AmxError::NotFound`] unless the plugin is running
//! as a native Open Multiplayer component, where the SDK obtained the table
//! from `getAmxFunctions()` itself. A legacy plugin loaded by open.mp also gets
//! `NotFound`: it receives the table through SA-MP's `Load()`, and the SDK has
//! no way to prove how many entries that one has.
//!
//! ```rust,no_run
//! use samp::prelude::*;
//! use samp::omp_amx::AmxOmpExt;
//! # fn example(amx: &Amx) -> samp::error::AmxResult<()> {
//! match amx.native_by_index(0) {
//!     Ok(info) => { /* ... */ }
//!     Err(_) => { /* not running as an open.mp component */ }
//! }
//! # Ok(()) }
//! ```

use samp_sdk::amx::Amx;
use samp_sdk::error::{AmxError, AmxResult};
use samp_sdk::exports::{
    Export, Exports, GetNativeByIndex, MakeAddr, StrSize, Swap16, Swap32, Swap64,
};
use samp_sdk::raw::types::AMX_NATIVE_INFO;

use crate::runtime::Runtime;

/// The extended table is only known to be present when the SDK itself read it
/// from `IPawnComponent::getAmxFunctions()`.
#[cfg(not(feature = "samp-only"))]
fn extended_table() -> AmxResult<usize> {
    let rt = Runtime::get();
    if rt.omp_has_amx_exports() {
        Ok(rt.amx_exports())
    } else {
        Err(AmxError::NotFound)
    }
}

#[cfg(feature = "samp-only")]
fn extended_table() -> AmxResult<usize> {
    Err(AmxError::NotFound)
}

/// Whether the extended (52-entry) AMX function table is available.
///
/// True only when running as a native Open Multiplayer component, after
/// `on_ready`.
#[must_use]
pub fn extended_table_available() -> bool {
    extended_table().is_ok()
}

/// Open Multiplayer additions to [`Amx`].
///
/// Each method fails with [`AmxError::NotFound`] when the extended table is not
/// available, which [`extended_table_available`] reports up front.
pub trait AmxOmpExt {
    /// `amx_GetNativeByIndex` — the name and address of a registered native.
    ///
    /// Walks the natives a script registered, which `amx_NumNatives` counts.
    /// Useful for tooling: listing what a script can call, or checking that a
    /// plugin's natives really landed.
    ///
    /// # Errors
    /// [`AmxError::NotFound`] without the extended table; [`AmxError::Index`]
    /// from the VM for an index out of range.
    fn native_by_index(&self, index: i32) -> AmxResult<AMX_NATIVE_INFO>;

    /// `amx_MakeAddr` — the AMX address of a physical pointer into VM memory.
    ///
    /// The inverse of `amx_GetAddr`: turns a pointer the plugin holds back into
    /// the cell address a script understands.
    ///
    /// # Errors
    /// [`AmxError::NotFound`] without the extended table; the VM's error when
    /// the pointer lies outside its memory.
    fn make_addr(&self, phys_addr: *mut i32) -> AmxResult<i32>;

    /// `amx_StrSize` — size in **bytes** an AMX string occupies, terminator
    /// included.
    ///
    /// `amx_StrLen` counts characters; this counts storage, which is what a
    /// buffer allocation needs.
    ///
    /// # Safety
    /// `cstr` must point to a NUL-terminated AMX string inside the VM.
    ///
    /// # Errors
    /// [`AmxError::NotFound`] without the extended table.
    unsafe fn str_size(&self, cstr: *const i32) -> AmxResult<usize>;

    /// `amx_Swap16` — reverses the byte order of a 16-bit value in place.
    ///
    /// # Errors
    /// [`AmxError::NotFound`] without the extended table.
    fn swap16(&self, value: &mut u16) -> AmxResult<()>;

    /// `amx_Swap32` — reverses the byte order of a 32-bit value in place.
    ///
    /// # Errors
    /// [`AmxError::NotFound`] without the extended table.
    fn swap32(&self, value: &mut u32) -> AmxResult<()>;

    /// `amx_Swap64` — reverses the byte order of a 64-bit value in place.
    ///
    /// # Errors
    /// [`AmxError::NotFound`] without the extended table.
    fn swap64(&self, value: &mut u64) -> AmxResult<()>;
}

impl AmxOmpExt for Amx {
    fn native_by_index(&self, index: i32) -> AmxResult<AMX_NATIVE_INFO> {
        let table = extended_table()?;
        let amx = self.amx().ok_or(AmxError::NotFound)?;
        let get = GetNativeByIndex::from_table(table);

        let mut info = AMX_NATIVE_INFO {
            name: std::ptr::null_mut(),
            func: dummy_native,
        };
        let code = get(amx.as_ptr(), index, &raw mut info);
        if code > 0 {
            return Err(AmxError::from(code));
        }
        Ok(info)
    }

    fn make_addr(&self, phys_addr: *mut i32) -> AmxResult<i32> {
        let table = extended_table()?;
        let amx = self.amx().ok_or(AmxError::NotFound)?;
        let make = MakeAddr::from_table(table);

        let mut address = 0;
        let code = make(amx.as_ptr(), phys_addr, &raw mut address);
        if code > 0 {
            return Err(AmxError::from(code));
        }
        Ok(address)
    }

    unsafe fn str_size(&self, cstr: *const i32) -> AmxResult<usize> {
        let table = extended_table()?;
        let size = StrSize::from_table(table);

        let mut length = 0;
        let code = size(cstr, &raw mut length);
        if code > 0 {
            return Err(AmxError::from(code));
        }
        Ok(usize::try_from(length).unwrap_or(0))
    }

    fn swap16(&self, value: &mut u16) -> AmxResult<()> {
        let table = extended_table()?;
        let code = Swap16::from_table(table)(&raw mut *value);
        if code > 0 {
            return Err(AmxError::from(code));
        }
        Ok(())
    }

    fn swap32(&self, value: &mut u32) -> AmxResult<()> {
        let table = extended_table()?;
        let code = Swap32::from_table(table)(&raw mut *value);
        if code > 0 {
            return Err(AmxError::from(code));
        }
        Ok(())
    }

    fn swap64(&self, value: &mut u64) -> AmxResult<()> {
        let table = extended_table()?;
        let code = Swap64::from_table(table)(&raw mut *value);
        if code > 0 {
            return Err(AmxError::from(code));
        }
        Ok(())
    }
}

/// Placeholder for the `func` field before the VM fills the struct in.
extern "C" fn dummy_native(_amx: *mut samp_sdk::raw::types::AMX, _params: *mut i32) -> i32 {
    0
}

/// `Exports` indices the extended table adds, for a plugin resolving one by
/// hand instead of through the trait.
#[must_use]
pub fn extended_indices() -> [Exports; 8] {
    [
        Exports::PushStringLen,
        Exports::SetStringLen,
        Exports::Swap16,
        Exports::Swap32,
        Exports::Swap64,
        Exports::GetNativeByIndex,
        Exports::MakeAddr,
        Exports::StrSize,
    ]
}
