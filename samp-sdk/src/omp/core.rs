//! Bindings for the `ILogger` interface of `ICore` (Open Multiplayer).
//!
//! `ICore` inherits from `IExtensible` and `ILogger` (`ICore : public IExtensible, public ILogger`).
//! As multiple inheritance, `ILogger` is a secondary base class with its own vtable,
//! located after the `IExtensible` subobject.
//!
//! ## Offsets (both confirmed via disasm of `Console.dll` / `Console.so`)
//!
//! - **MSVC i686** (`Console.dll`):
//!   `lea edx, [core+0x38]; mov ecx, [edx]; call [ecx+8]` -> `ILogger` at offset **56**
//! - **Linux GCC i686 / Itanium** (`Console.so`):
//!   `add edi, 0x28; mov ebx, [edi]; call [ebx+8]` -> `ILogger` at offset **40**
//!
//! In both, `slot[2]` is `logLn` — matches the order declared in `core.hpp:151-184`.
//!
//! ## `ILogger` vtable (order defined in `core.hpp:151-184`)
//!
//! ```text
//! [0] printLn(fmt, ...)         — print without level
//! [1] vprintLn(fmt, va_list)
//! [2] logLn(level, fmt, ...)    — print with LogLevel
//! [3] vlogLn(level, fmt, va_list)
//! [4] printLnU8(fmt, ...)       — UTF-8 variants
//! [5] vprintLnU8(fmt, va_list)
//! [6] logLnU8(level, fmt, ...)
//! [7] vlogLnU8(level, fmt, va_list)
//! ```
//!
//! ## Calling convention
//!
//! Variadic virtual methods on x86 use **`__cdecl`** on both MSVC and Itanium
//! (thiscall does not support varargs). `this` is the **first arg pushed on the stack**;
//! the caller is responsible for cleaning the stack.
//!
//! Since stable Rust does not support `extern "C"` variadic (the `c_variadic`
//! feature is nightly), we declare the functions with a fixed arity of 1 arg and
//! use the format `"%s"`: the caller formats the message in Rust (`format!`) and
//! passes the resulting `CString` as the single variadic argument. The ABI is
//! identical to that of the C variadic function — `printf("%s", msg)` is
//! equivalent to `printf(msg)` for the calling convention.

use super::component::ICore;
use std::ffi::CString;
use std::os::raw::{c_char, c_int};

/// Offset of the `ILogger` subobject inside `ICore`.
#[cfg(target_env = "msvc")]
const ILOGGER_OFFSET: isize = 56;

#[cfg(not(target_env = "msvc"))]
const ILOGGER_OFFSET: isize = 40;

/// Slot of the `printLn(fmt, ...)` function in the `ILogger` vtable.
const SLOT_PRINTLN: usize = 0;

/// Slot of the `logLn(level, fmt, ...)` function in the `ILogger` vtable.
const SLOT_LOGLN: usize = 2;

/// Slot of the `printLnU8(fmt, ...)` function in the `ILogger` vtable.
const SLOT_PRINTLN_U8: usize = 4;

/// Slot of the `logLnU8(level, fmt, ...)` function in the `ILogger` vtable.
const SLOT_LOGLN_U8: usize = 6;

/// Open Multiplayer log level (corresponds to `LogLevel` in `core.hpp`).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug = 0,
    Message = 1,
    Warning = 2,
    Error = 3,
}

/// Type of the `printLn(this, fmt, arg)` function.
///
/// Declared with fixed arity instead of variadic — uses `fmt = "%s"` and a
/// single `arg` (already-formatted message). ABI-compatible with the original
/// variadic function.
type PrintLnFn = unsafe extern "C" fn(this: *mut u8, fmt: *const c_char, arg: *const c_char);

/// Type of the `logLn(this, level, fmt, arg)` function.
type LogLnFn =
    unsafe extern "C" fn(this: *mut u8, level: c_int, fmt: *const c_char, arg: *const c_char);

/// Loads a slot from the `ILogger` vtable given the `ICore` pointer.
///
/// Thin wrapper over [`vtable::secondary_call_target`] with the offset
/// pre-resolved for the `ILogger` subobject.
///
/// # Safety
/// `core` must point to a valid `ICore` (alive, with the secondary vtable initialized).
unsafe fn logger_slot(core: *mut ICore, slot: usize) -> Option<(*mut u8, usize)> {
    unsafe { super::vtable::secondary_call_target(core.cast::<u8>(), ILOGGER_OFFSET, slot) }
}

/// `ICore::printLn(message)` — writes a line to the server log.
///
/// The message is passed as a `%s` arg, avoiding interpretation of `%` in the content.
/// Returns `false` if `core` is null or the vtable is corrupted (nothing is printed).
///
/// # Safety
/// `core` must point to a valid `ICore` received in `on_load`.
pub unsafe fn core_print_ln(core: *mut ICore, message: &str) -> bool {
    let Some((this, slot)) = (unsafe { logger_slot(core, SLOT_PRINTLN) }) else {
        return false;
    };
    let Ok(msg) = CString::new(message) else {
        return false;
    };
    let fmt = c"%s";
    let f: PrintLnFn = unsafe { std::mem::transmute(slot) };
    unsafe { f(this, fmt.as_ptr(), msg.as_ptr()) };
    true
}

/// `ICore::logLn(level, message)` — writes a line with a log level.
///
/// The Open Multiplayer server prepends `[Info]`/`[Warning]`/`[Error]`/`[Debug]` and a
/// timestamp to the message, exactly as it does for its own logs.
///
/// # Safety
/// `core` must point to a valid `ICore` received in `on_load`.
pub unsafe fn core_log_ln(core: *mut ICore, level: LogLevel, message: &str) -> bool {
    let Some((this, slot)) = (unsafe { logger_slot(core, SLOT_LOGLN) }) else {
        return false;
    };
    let Ok(msg) = CString::new(message) else {
        return false;
    };
    let fmt = c"%s";
    let f: LogLnFn = unsafe { std::mem::transmute(slot) };
    unsafe { f(this, level as c_int, fmt.as_ptr(), msg.as_ptr()) };
    true
}

/// `ICore::printLnU8(message)` — UTF-8 variant of `printLn`.
///
/// Uses the Open Multiplayer server's UTF-8 pipeline, which preserves accented characters
/// regardless of the console locale (important on Windows, where the default code
/// page can corrupt non-ASCII bytes if passed through the regular `printLn`).
///
/// # Safety
/// `core` must point to a valid `ICore` received in `on_load`.
pub unsafe fn core_print_ln_u8(core: *mut ICore, message: &str) -> bool {
    let Some((this, slot)) = (unsafe { logger_slot(core, SLOT_PRINTLN_U8) }) else {
        return false;
    };
    let Ok(msg) = CString::new(message) else {
        return false;
    };
    let fmt = c"%s";
    let f: PrintLnFn = unsafe { std::mem::transmute(slot) };
    unsafe { f(this, fmt.as_ptr(), msg.as_ptr()) };
    true
}

/// `ICore::logLnU8(level, message)` — UTF-8 variant of `logLn`.
///
/// Combines the server's UTF-8 pipeline with a log level. Recommended as the
/// default for any message that may contain accented characters or non-ASCII
/// symbols.
///
/// # Safety
/// `core` must point to a valid `ICore` received in `on_load`.
pub unsafe fn core_log_ln_u8(core: *mut ICore, level: LogLevel, message: &str) -> bool {
    let Some((this, slot)) = (unsafe { logger_slot(core, SLOT_LOGLN_U8) }) else {
        return false;
    };
    let Ok(msg) = CString::new(message) else {
        return false;
    };
    let fmt = c"%s";
    let f: LogLnFn = unsafe { std::mem::transmute(slot) };
    unsafe { f(this, level as c_int, fmt.as_ptr(), msg.as_ptr()) };
    true
}

#[cfg(test)]
mod tests {
    //! Smoke tests for the 4 log functions of `ICore`.
    //!
    //! Each test sets up a fake `ICore` in a buffer and installs a mock vtable
    //! that captures `(slot, level, fmt, message)`. It validates that each
    //! `core_*_ln*` calls the correct slot of the `ILogger` secondary vtable at
    //! the correct offset.
    //!
    //! Runs serially via `TEST_LOCK` because the captured state is global.

    use super::*;
    use std::ffi::CStr;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[derive(Default, Clone)]
    struct Captured {
        slot: Option<usize>,
        level: Option<c_int>,
        fmt: Option<String>,
        message: Option<String>,
    }

    static CAPTURED: Mutex<Option<Captured>> = Mutex::new(None);

    fn reset_captures() {
        *CAPTURED.lock().unwrap() = Some(Captured::default());
    }

    fn last_capture() -> Captured {
        CAPTURED.lock().unwrap().clone().unwrap_or_default()
    }

    fn cstr_to_string(ptr: *const c_char) -> Option<String> {
        if ptr.is_null() {
            return None;
        }
        unsafe { CStr::from_ptr(ptr) }
            .to_str()
            .ok()
            .map(String::from)
    }

    unsafe extern "C" fn mock_print_ln(_this: *mut u8, fmt: *const c_char, arg: *const c_char) {
        let mut guard = CAPTURED.lock().unwrap();
        let c = guard.as_mut().unwrap();
        c.slot = Some(SLOT_PRINTLN);
        c.fmt = cstr_to_string(fmt);
        c.message = cstr_to_string(arg);
    }

    unsafe extern "C" fn mock_log_ln(
        _this: *mut u8,
        level: c_int,
        fmt: *const c_char,
        arg: *const c_char,
    ) {
        let mut guard = CAPTURED.lock().unwrap();
        let c = guard.as_mut().unwrap();
        c.slot = Some(SLOT_LOGLN);
        c.level = Some(level);
        c.fmt = cstr_to_string(fmt);
        c.message = cstr_to_string(arg);
    }

    unsafe extern "C" fn mock_print_ln_u8(_this: *mut u8, fmt: *const c_char, arg: *const c_char) {
        let mut guard = CAPTURED.lock().unwrap();
        let c = guard.as_mut().unwrap();
        c.slot = Some(SLOT_PRINTLN_U8);
        c.fmt = cstr_to_string(fmt);
        c.message = cstr_to_string(arg);
    }

    unsafe extern "C" fn mock_log_ln_u8(
        _this: *mut u8,
        level: c_int,
        fmt: *const c_char,
        arg: *const c_char,
    ) {
        let mut guard = CAPTURED.lock().unwrap();
        let c = guard.as_mut().unwrap();
        c.slot = Some(SLOT_LOGLN_U8);
        c.level = Some(level);
        c.fmt = cstr_to_string(fmt);
        c.message = cstr_to_string(arg);
    }

    unsafe extern "C" fn unused_slot() {}

    /// Mock vtable — initialized at runtime via `OnceLock` because `fn as usize`
    /// is not const-evaluable. 10 slots = 8 of the `ILogger` header + 2 spare.
    static MOCK_VTABLE: std::sync::OnceLock<[usize; 10]> = std::sync::OnceLock::new();

    fn mock_vtable() -> &'static [usize; 10] {
        MOCK_VTABLE.get_or_init(|| {
            [
                mock_print_ln as *const () as usize,    // [0] printLn
                unused_slot as *const () as usize,      // [1] vprintLn
                mock_log_ln as *const () as usize,      // [2] logLn
                unused_slot as *const () as usize,      // [3] vlogLn
                mock_print_ln_u8 as *const () as usize, // [4] printLnU8
                unused_slot as *const () as usize,      // [5] vprintLnU8
                mock_log_ln_u8 as *const () as usize,   // [6] logLnU8
                unused_slot as *const () as usize,      // [7] vlogLnU8
                0,
                0,
            ]
        })
    }

    /// Builds a buffer simulating the `ICore` layout:
    /// `[0..ILOGGER_OFFSET]` represent the `IExtensible` subobject (zeroed garbage);
    /// `[ILOGGER_OFFSET..ILOGGER_OFFSET+4]` is the vptr to our mock vtable.
    ///
    /// Size 32x`usize` = 128 bytes on i686 (target); `usize` ensures natural
    /// alignment for the `*mut usize` cast at the vptr slot.
    fn make_mock_core() -> [usize; 32] {
        let mut buf = [0usize; 32];
        let vptr = mock_vtable().as_ptr() as usize;
        // ILOGGER_OFFSET in bytes; on i686 each `usize` = 4 bytes.
        let idx = usize::try_from(ILOGGER_OFFSET).expect("ILOGGER_OFFSET must be >= 0")
            / std::mem::size_of::<usize>();
        buf[idx] = vptr;
        buf
    }

    #[test]
    fn core_print_ln_calls_slot_0_at_logger_offset() {
        let _g = TEST_LOCK.lock().unwrap();
        reset_captures();
        let mut core = make_mock_core();
        let core_ptr = core.as_mut_ptr().cast::<ICore>();

        let ok = unsafe { core_print_ln(core_ptr, "hello") };
        assert!(ok, "core_print_ln must return true with a valid mock");

        let c = last_capture();
        assert_eq!(c.slot, Some(SLOT_PRINTLN));
        assert_eq!(c.fmt.as_deref(), Some("%s"));
        assert_eq!(c.message.as_deref(), Some("hello"));
        assert_eq!(c.level, None, "printLn does not take a LogLevel");
    }

    #[test]
    fn core_log_ln_calls_slot_2_with_level() {
        let _g = TEST_LOCK.lock().unwrap();
        reset_captures();
        let mut core = make_mock_core();
        let core_ptr = core.as_mut_ptr().cast::<ICore>();

        let ok = unsafe { core_log_ln(core_ptr, LogLevel::Warning, "alert") };
        assert!(ok);

        let c = last_capture();
        assert_eq!(c.slot, Some(SLOT_LOGLN));
        assert_eq!(c.level, Some(LogLevel::Warning as c_int));
        assert_eq!(c.fmt.as_deref(), Some("%s"));
        assert_eq!(c.message.as_deref(), Some("alert"));
    }

    #[test]
    fn core_print_ln_u8_calls_slot_4() {
        let _g = TEST_LOCK.lock().unwrap();
        reset_captures();
        let mut core = make_mock_core();
        let core_ptr = core.as_mut_ptr().cast::<ICore>();

        let ok = unsafe { core_print_ln_u8(core_ptr, "hi") };
        assert!(ok);

        let c = last_capture();
        assert_eq!(c.slot, Some(SLOT_PRINTLN_U8));
        assert_eq!(c.message.as_deref(), Some("hi"));
    }

    #[test]
    fn core_log_ln_u8_calls_slot_6_with_level() {
        let _g = TEST_LOCK.lock().unwrap();
        reset_captures();
        let mut core = make_mock_core();
        let core_ptr = core.as_mut_ptr().cast::<ICore>();

        let ok = unsafe { core_log_ln_u8(core_ptr, LogLevel::Error, "critical failure") };
        assert!(ok);

        let c = last_capture();
        assert_eq!(c.slot, Some(SLOT_LOGLN_U8));
        assert_eq!(c.level, Some(LogLevel::Error as c_int));
        assert_eq!(c.message.as_deref(), Some("critical failure"));
    }

    #[test]
    fn all_log_fns_return_false_for_null_core() {
        let _g = TEST_LOCK.lock().unwrap();
        let nul = std::ptr::null_mut();
        assert!(!unsafe { core_print_ln(nul, "x") });
        assert!(!unsafe { core_log_ln(nul, LogLevel::Message, "x") });
        assert!(!unsafe { core_print_ln_u8(nul, "x") });
        assert!(!unsafe { core_log_ln_u8(nul, LogLevel::Message, "x") });
    }

    #[test]
    fn log_fns_reject_message_with_interior_nul() {
        let _g = TEST_LOCK.lock().unwrap();
        let mut core = make_mock_core();
        let core_ptr = core.as_mut_ptr().cast::<ICore>();
        // CString::new fails on an interior NUL -> log_fn returns false silently
        assert!(!unsafe { core_print_ln(core_ptr, "a\0b") });
        assert!(!unsafe { core_log_ln(core_ptr, LogLevel::Message, "a\0b") });
    }
}
