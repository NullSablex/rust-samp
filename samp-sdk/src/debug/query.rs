//! Queries over a decoded [`AmxDbg`] — mirror the `dbg_*` functions of
//! `amxdbg.c`. These are the reason the module exists: translating what the VM
//! reports (addresses) into what the developer sees (file, line, symbol).

use super::types::{AmxDbg, DbgError, DbgSymbol, Ident, VClass};

impl AmxDbg {
    /// Decodes a debug block from the bytes that start at the `AMX_DBG_HDR`
    /// (what `pawncc -d2`/`-d3` appends to the `.amx`).
    ///
    /// # Errors
    /// [`DbgError`] if the block is truncated, the `magic` is invalid, or a
    /// name does not terminate.
    pub fn parse(data: &[u8]) -> Result<Self, DbgError> {
        super::parse::parse(data)
    }

    /// Extracts and decodes the debug block from a whole `.amx`.
    ///
    /// The `AMX_DBG` block starts at offset `AMX_HEADER.size` (the first field
    /// of the header, `i32` little-endian) — see `dbg_LoadInfo` in `amxdbg.c`.
    /// The `.amx` must have been compiled with `-d2`/`-d3`.
    ///
    /// # Errors
    /// [`DbgError::Truncated`] if the file is smaller than the header or the
    /// announced offset; other errors come from [`parse`](Self::parse).
    pub fn from_amx(amx: &[u8]) -> Result<Self, DbgError> {
        let size_bytes = amx.get(0..4).ok_or(DbgError::Truncated)?;
        let offset =
            u32::from_le_bytes([size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3]])
                as usize;
        let block = amx.get(offset..).ok_or(DbgError::Truncated)?;
        Self::parse(block)
    }

    /// Source line of a code address (`dbg_LookupLine`): the last entry with
    /// `address <= addr`. `None` if none applies.
    #[must_use]
    pub fn lookup_line(&self, addr: u32) -> Option<i32> {
        self.lines
            .iter()
            .take_while(|l| l.address <= addr)
            .last()
            .map(|l| l.line)
    }

    /// Source file of a code address (`dbg_LookupFile`): the last entry with
    /// `address <= addr`.
    #[must_use]
    pub fn lookup_file(&self, addr: u32) -> Option<&str> {
        self.files
            .iter()
            .take_while(|f| f.address <= addr)
            .last()
            .map(|f| f.name.as_str())
    }

    /// Function containing a code address (`dbg_LookupFunction`): an `iFUNCTN`
    /// symbol with `codestart <= addr < codeend`, ignoring internal names
    /// (those starting with `@`). Useful for the call stack.
    #[must_use]
    pub fn lookup_function(&self, addr: u32) -> Option<&str> {
        self.symbols
            .iter()
            .find(|s| {
                s.ident == Ident::Function
                    && s.codestart <= addr
                    && s.codeend > addr
                    && !s.name.starts_with('@')
            })
            .map(|s| s.name.as_str())
    }

    /// Code address of a source line (`dbg_GetLineAddress`), to set a
    /// breakpoint by line. Moves to the next "breakable" line if the exact one
    /// does not exist; use [`Self::lookup_line`] to learn which line it landed
    /// on.
    ///
    /// Faithfully reproduces `dbg_GetLineAddress` from `amxdbg.c`: a file may
    /// appear MULTIPLE times in the file table (e.g. the gamemode re-included),
    /// each instance covering a `[bottomaddr, topaddr)` range. We look for the
    /// first line `>= line` WITHIN each instance's range, in line-table order —
    /// not the globally smallest line, which would match the wrong instance.
    #[must_use]
    pub fn line_to_address(&self, line: i32, file: Option<&str>) -> Option<u32> {
        // No file: first entry with `line >= line` in line-table order.
        let Some(want) = file else {
            return self
                .lines
                .iter()
                .find(|l| l.line >= line)
                .map(|l| l.address);
        };

        // Iterate the file's instances in the file table (like the C `for file`).
        for (i, f) in self.files.iter().enumerate() {
            if !file_name_matches(&f.name, want) {
                continue;
            }
            let bottom = f.address;
            // topaddr = address of the NEXT file-table entry, or +inf.
            let top = self.files.get(i + 1).map_or(u32::MAX, |n| n.address);

            // Walk the line table within the file's range and look for
            // `line >= line` without passing `top`. The first match is the
            // address.
            if let Some(l) = self
                .lines
                .iter()
                .filter(|l| l.address >= bottom && l.address < top)
                .find(|l| l.line >= line)
            {
                return Some(l.address);
            }
            // Not found in this instance: try the next one (same name).
        }
        None
    }

    /// Symbols in scope at a code address: globals + locals/args whose
    /// `codestart <= addr < codeend`. Basis of variable inspection.
    #[must_use]
    pub fn symbols_in_scope(&self, addr: u32) -> Vec<&DbgSymbol> {
        self.symbols
            .iter()
            .filter(|s| {
                s.ident != Ident::Function
                    && match s.vclass {
                        VClass::Global => true,
                        _ => s.codestart <= addr && s.codeend > addr,
                    }
            })
            .collect()
    }

    /// Name of a tag by id (`dbg_GetTagName`).
    #[must_use]
    pub fn tag_name(&self, tag: i16) -> Option<&str> {
        self.tags
            .iter()
            .find(|t| t.tag == tag)
            .map(|t| t.name.as_str())
    }
}

/// Matches the file-table name (`tbl`, e.g. `gamemodes/molde.pwn`) against the
/// path the editor requested (`want`, which may be relative `molde.pwn` or
/// absolute `/.../gamemodes/molde.pwn`). The C `dbg_GetLineAddress` uses an
/// exact `strcmp` because it assumes names identical to the filetbl; here we
/// must tolerate the forms the editor sends, so we match when one path is a
/// suffix of the other at component boundaries (separator `/` or `\`).
fn file_name_matches(tbl: &str, want: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/");
    let (tbl, want) = (norm(tbl), norm(want));
    if tbl == want {
        return true;
    }
    // `a` is a suffix of `b` at a component boundary (or `b` ends with `/a`).
    let suffix_at_boundary = |a: &str, b: &str| {
        b.strip_suffix(a)
            .is_some_and(|head| head.is_empty() || head.ends_with('/'))
    };
    suffix_at_boundary(&tbl, &want) || suffix_at_boundary(&want, &tbl)
}
