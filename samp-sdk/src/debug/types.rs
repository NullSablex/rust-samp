//! Structures of the `AMX_DBG` block, already decoded (owning their data).
//!
//! Unlike C (`amxdbg.h`), names are `String` and tables are `Vec`, with no
//! pointers nor `name[1]`. The AMX `ucell`/`cell` are 32-bit.

/// Parse error of the debug block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DbgError {
    /// Not enough bytes to read what the header announces.
    Truncated,
    /// `magic` differs from [`super::AMX_DBG_MAGIC`].
    BadMagic(u16),
    /// A zero-terminated name does not close inside the block.
    UnterminatedName,
}

impl std::fmt::Display for DbgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated debug block"),
            Self::BadMagic(m) => write!(f, "invalid magic: {m:#06x}"),
            Self::UnterminatedName => write!(f, "unterminated name in debug block"),
        }
    }
}

impl std::error::Error for DbgError {}

/// Symbol kind (`AMX_DBG_SYMBOL.ident`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ident {
    /// `iVARIABLE` — a cell with an address, read directly (lvalue).
    Variable,
    /// `iREFERENCE` — like `Variable`, but must be dereferenced.
    Reference,
    /// `iARRAY`.
    Array,
    /// `iREFARRAY` — array passed by reference (pointer).
    RefArray,
    /// `iFUNCTN` — function.
    Function,
    /// Unknown value (preserves the original byte).
    Other(u8),
}

impl Ident {
    pub(crate) fn from_byte(b: u8) -> Self {
        match b {
            1 => Self::Variable,
            2 => Self::Reference,
            3 => Self::Array,
            4 => Self::RefArray,
            9 => Self::Function,
            other => Self::Other(other),
        }
    }
}

/// Symbol class (`AMX_DBG_SYMBOL.vclass`): the scope it lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VClass {
    /// Global — `address` is absolute in the data segment.
    Global,
    /// Local — `address` is relative to the frame (`frm`).
    Local,
    /// Function argument — also relative to the frame.
    Argument,
    /// Unknown value (preserves the original byte).
    Other(u8),
}

impl VClass {
    pub(crate) fn from_byte(b: u8) -> Self {
        match b {
            0 => Self::Global,
            1 => Self::Local,
            2 => Self::Argument,
            other => Self::Other(other),
        }
    }
}

/// File-table entry: where, in the code segment, the code of a source file
/// begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbgFile {
    pub address: u32,
    pub name: String,
}

/// Line-table entry: `address` (code segment) → `line` (1-based after parsing;
/// the compiler stores it zero-based). The basis of the address↔line mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbgLine {
    pub address: u32,
    pub line: i32,
}

/// Array dimension (`AMX_DBG_SYMDIM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DbgSymDim {
    pub tag: i16,
    pub size: u32,
}

/// Symbol-table entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbgSymbol {
    /// Data-segment address (global) or frame-relative (local/arg).
    pub address: u32,
    pub tag: i16,
    /// Start of the code range where the symbol is in scope.
    pub codestart: u32,
    /// End (exclusive) of the scope range.
    pub codeend: u32,
    pub ident: Ident,
    pub vclass: VClass,
    pub name: String,
    /// Dimensions, when an array (`dim` entries).
    pub dims: Vec<DbgSymDim>,
}

/// Tag-table entry (`AMX_DBG_TAG`): id ↔ tag name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbgTag {
    pub tag: i16,
    pub name: String,
}

/// Decoded debug block. Tables keep the original file order (lines and files
/// already come sorted by ascending address).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AmxDbg {
    pub file_version: u8,
    pub amx_version: u8,
    pub files: Vec<DbgFile>,
    pub lines: Vec<DbgLine>,
    pub symbols: Vec<DbgSymbol>,
    pub tags: Vec<DbgTag>,
}
