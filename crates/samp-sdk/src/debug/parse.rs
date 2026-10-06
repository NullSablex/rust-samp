//! Decodes the `AMX_DBG` block from bytes (little-endian, packed).
//!
//! Mirrors `dbg_LoadInfo` from `amxdbg.c`, but reads only the debug block
//! itself — not the `.amx` header. The caller passes the block starting at
//! `AMX_DBG_HDR`.

use super::AMX_DBG_MAGIC;
use super::types::{
    AmxDbg, DbgError, DbgFile, DbgLine, DbgSymDim, DbgSymbol, DbgTag, Ident, VClass,
};

/// Fixed header (`AMX_DBG_HDR`): 22 packed bytes.
const HDR_SIZE: usize = 22;

/// Fixed size of a line-table entry: address(u32) + line(i32).
const LINE_SIZE: usize = 8;

/// Little-endian read cursor over a slice, with bounds checking.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    fn u8(&mut self) -> Result<u8, DbgError> {
        let b = *self.buf.get(self.pos).ok_or(DbgError::Truncated)?;
        self.pos += 1;
        Ok(b)
    }

    fn u16(&mut self) -> Result<u16, DbgError> {
        let s = self.take(2)?;
        Ok(u16::from_le_bytes([s[0], s[1]]))
    }

    fn i16(&mut self) -> Result<i16, DbgError> {
        let s = self.take(2)?;
        Ok(i16::from_le_bytes([s[0], s[1]]))
    }

    /// Table count. The format declares `i16`, but counts are never negative:
    /// what looks negative is overflow of the 16-bit field (e.g. a large
    /// gamemode with more than 32767 lines). Reinterpreted as `u16`, so a
    /// `-5986` becomes `59550`. For tables that may exceed 65535, see the
    /// overflow handling in the line-table reader.
    fn count(&mut self) -> Result<usize, DbgError> {
        Ok(usize::from(self.u16()?))
    }

    fn u32(&mut self) -> Result<u32, DbgError> {
        let s = self.take(4)?;
        Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    /// Reads a `u32` at the current position WITHOUT advancing the cursor.
    /// `None` if there are not enough bytes.
    fn peek_u32(&self) -> Option<u32> {
        let s = self.buf.get(self.pos..self.pos + 4)?;
        Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    /// Bytes remaining from the current position.
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    fn i32(&mut self) -> Result<i32, DbgError> {
        let s = self.take(4)?;
        Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DbgError> {
        let end = self.pos.checked_add(n).ok_or(DbgError::Truncated)?;
        let s = self.buf.get(self.pos..end).ok_or(DbgError::Truncated)?;
        self.pos = end;
        Ok(s)
    }

    /// Zero-terminated ASCII name; consumes the `\0`. Latin-1 → `String` (each
    /// byte becomes a char), preserving high bytes without failing.
    fn cstr(&mut self) -> Result<String, DbgError> {
        let start = self.pos;
        while *self.buf.get(self.pos).ok_or(DbgError::UnterminatedName)? != 0 {
            self.pos += 1;
        }
        let name = self.buf[start..self.pos]
            .iter()
            .map(|&b| b as char)
            .collect();
        self.pos += 1; // skip the '\0'
        Ok(name)
    }
}

/// Decodes the debug block. `data` must start at `AMX_DBG_HDR`.
///
/// # Errors
/// [`DbgError`] if the block is truncated, the magic is invalid, or a name is
/// not terminated.
pub fn parse(data: &[u8]) -> Result<AmxDbg, DbgError> {
    let mut r = Reader::new(data);

    // --- Header (AMX_DBG_HDR) ---
    let _size = r.i32()?;
    let magic = r.u16()?;
    if magic != AMX_DBG_MAGIC {
        return Err(DbgError::BadMagic(magic));
    }
    let file_version = r.u8()?;
    let amx_version = r.u8()?;
    let _flags = r.i16()?;
    let files = r.count()?;
    let lines = r.count()?;
    let symbols = r.count()?;
    let tags = r.count()?;
    let automatons = r.count()?;
    let states = r.count()?;
    debug_assert_eq!(r.pos, HDR_SIZE);

    // --- File table: address(u32) + name (>=1 byte: the `\0`) ---
    let mut file_tbl = Vec::with_capacity(safe_cap(files, 5, r.remaining()));
    for _ in 0..files {
        let address = r.u32()?;
        let name = r.cstr()?;
        file_tbl.push(DbgFile { address, name });
    }

    // --- Line table (see `read_line_table` for the overflow handling) ---
    let line_tbl = read_line_table(&mut r, lines)?;

    // --- Symbol table: fixed fields (>=18 bytes) + name + dim x SYMDIM ---
    let mut sym_tbl = Vec::with_capacity(safe_cap(symbols, 19, r.remaining()));
    for _ in 0..symbols {
        let address = r.u32()?;
        let tag = r.i16()?;
        let codestart = r.u32()?;
        let codeend = r.u32()?;
        let ident = Ident::from_byte(r.u8()?);
        let vclass = VClass::from_byte(r.u8()?);
        let dim = r.count()?;
        let name = r.cstr()?;
        let mut dims = Vec::with_capacity(safe_cap(dim, 6, r.remaining()));
        for _ in 0..dim {
            let dtag = r.i16()?;
            let size = r.u32()?;
            dims.push(DbgSymDim { tag: dtag, size });
        }
        sym_tbl.push(DbgSymbol {
            address,
            tag,
            codestart,
            codeend,
            ident,
            vclass,
            name,
            dims,
        });
    }

    // --- Tag table: tag(i16) + name ---
    let mut tag_tbl = Vec::with_capacity(safe_cap(tags, 3, r.remaining()));
    for _ in 0..tags {
        let tag = r.i16()?;
        let name = r.cstr()?;
        tag_tbl.push(DbgTag { tag, name });
    }

    // --- Automatons and states: read to advance the cursor; unused in v1. ---
    for _ in 0..automatons {
        let _automaton = r.i16()?;
        let _address = r.u32()?;
        let _name = r.cstr()?;
    }
    for _ in 0..states {
        let _state = r.i16()?;
        let _automaton = r.i16()?;
        let _name = r.cstr()?;
    }

    Ok(AmxDbg {
        file_version,
        amx_version,
        files: file_tbl,
        lines: line_tbl,
        symbols: sym_tbl,
        tags: tag_tbl,
    })
}

/// Safe pre-allocation capacity: never more entries than would fit in the
/// `remaining` bytes (`entry_min` = minimum size of an entry in bytes).
/// Guards against corrupted counts that would request huge allocations.
fn safe_cap(count: usize, entry_min: usize, remaining: usize) -> usize {
    count.min(remaining / entry_min.max(1))
}

/// Reads the line table: `address(u32) + line(i32)`, 8 bytes per entry.
///
/// The count field is 16-bit and can OVERFLOW in a large gamemode (the
/// compiler writes EVERY `L:` entry, but the count saturates at 16 bits).
/// Overflow signal: the value read as `i16` went negative — i.e. as `u16` it
/// passed 32767. Only then do we look for extra 65536 blocks, detecting them
/// by address monotonicity. Without this guard, a small legitimate `lines`
/// would trigger a false overflow by "seeing" the following symbol table. The
/// remaining-bytes ceiling avoids an infinite loop on adversarial data.
fn read_line_table(r: &mut Reader, lines: usize) -> Result<Vec<DbgLine>, DbgError> {
    let overflow_possible = lines > usize::try_from(i16::MAX).unwrap_or(0);
    let max_lines = r.remaining() / LINE_SIZE;
    let mut line_tbl = Vec::with_capacity(safe_cap(lines, LINE_SIZE, r.remaining()));
    let mut to_read = lines;
    let mut last_addr: Option<u32> = None;
    loop {
        for _ in 0..to_read {
            if line_tbl.len() >= max_lines {
                break;
            }
            let address = r.u32()?;
            // The compiler writes ZERO-BASED lines (`insert_dbgline` in
            // sclist.c does `linenr--`). The editor/DAP uses 1-based, so we
            // compensate here: the whole lib works in 1-based.
            let line = r.i32()?.saturating_add(1);
            line_tbl.push(DbgLine { address, line });
            last_addr = Some(address);
        }
        if !overflow_possible {
            break; // count fit in 16 bits: the table ended.
        }
        // Peek the next entry: if the address keeps growing, it is another
        // 65536-line block; otherwise we reached the symbol table.
        let Some(prev) = last_addr else { break };
        if line_tbl.len() >= max_lines {
            break;
        }
        let Some(next_addr) = r.peek_u32() else { break };
        if next_addr > prev {
            to_read = 1usize << 16;
        } else {
            break;
        }
    }
    Ok(line_tbl)
}
