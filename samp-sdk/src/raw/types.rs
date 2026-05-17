//! Direct translation of the `amx.h` header structs into Rust.
//!
//! All are `#[repr(C, packed)]` — the AMX assumes a layout identical to the
//! original Pawn interpreter. Do not add, remove, or reorder fields without
//! reviewing the corresponding C header, or the VM will start reading/writing
//! at the wrong offsets.

use super::functions::{AmxCallback, AmxDebug, AmxNative};
use std::os::raw::{c_char, c_int, c_long, c_uchar, c_void};

/// AMX VM instance. Each AMX loaded by the server (gamemode + filterscripts)
/// is a pointer to this struct.
#[repr(C, packed)]
pub struct AMX {
    pub base: *mut c_uchar,
    pub data: *mut c_uchar,
    pub callback: AmxCallback,
    pub debug: AmxDebug,
    pub cip: i32,
    pub frm: i32,
    pub hea: i32,
    pub hlw: i32,
    pub stk: i32,
    pub stp: i32,
    pub flags: c_int,
    pub usertags: [c_long; 4usize],
    pub userdata: [*mut c_void; 4usize],
    pub error: c_int,
    pub paramcount: c_int,
    pub pri: i32,
    pub alt: i32,
    pub reset_stk: i32,
    pub reset_hea: i32,
    pub sysreq_d: i32,
}

/// Entry of the natives table registered via `amx_Register` — a name+pointer pair.
#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct AMX_NATIVE_INFO {
    pub name: *const c_char,
    pub func: AmxNative,
}

/// Public/native function stub inside the `.amx`. Layout with inline name (20 bytes).
#[repr(C, packed)]
pub struct AMX_FUNCSTUB {
    pub address: u32,
    pub name: [c_char; 20usize],
}

/// Function stub in `.amx` compiled with `defsize=8` — the name is resolved by
/// an offset in the nametable (not inline).
///
/// Name inherited from the original C header (`AMX_FUNCSTUBNT` — the leading
/// "ANX" is a historic typo kept for binary compatibility).
#[repr(C, packed)]
pub struct ANX_FUNCSTUBNT {
    pub address: u32,
    pub nameofs: u32,
}

/// Header of the `.amx` file loaded in memory. Offsets in this struct point
/// to sections inside the file itself.
#[repr(C, packed)]
pub struct AMX_HEADER {
    pub size: i32,
    pub magic: u16,
    pub file_version: c_char,
    pub amx_version: c_char,
    pub flags: i16,
    pub defsize: i16,
    pub cod: i32,
    pub dat: i32,
    pub hea: i32,
    pub stp: i32,
    pub cip: i32,
    pub publics: i32,
    pub natives: i32,
    pub libraries: i32,
    pub pubvars: i32,
    pub tags: i32,
    pub nametable: i32,
}
