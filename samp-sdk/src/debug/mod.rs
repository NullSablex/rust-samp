//! `AMX_DBG` debug-info parser (behind the `debug` feature).
//!
//! Pure logic: bytes → structures. Testable without a server. Maps a code
//! address ↔ line ↔ symbol, mirroring the queries of `amxdbg.c`.
//!
//! The debug block is what `pawncc -d2`/`-d3` appends to the `.amx`. Layout
//! (little-endian, *packed*): a fixed header followed by the tables, in this
//! order — files, lines, symbols, tags, automatons, states. The file/symbol/
//! tag/automaton/state entries end with a zero-terminated variable-length ASCII
//! name; the line entries are fixed size.
//!
//! Pair this with the VM register/cell accessors on [`crate::amx::Amx`]
//! (`cip`, `frame`, `read_cell`/`write_cell`, `install_debug_hook`) to build a
//! debugger on top of the SDK.

mod parse;
mod query;
mod types;

pub use types::{AmxDbg, DbgError, DbgFile, DbgLine, DbgSymDim, DbgSymbol, DbgTag, Ident, VClass};

/// Signature of the debug block (`AMX_DBG_HDR.magic`).
pub const AMX_DBG_MAGIC: u16 = 0xf1ef;

#[cfg(test)]
mod tests {
    use super::{AMX_DBG_MAGIC, AmxDbg, Ident, VClass};

    /// Builds a minimal debug block: 2 files, 4 lines, 3 symbols (1 function,
    /// 1 global, 1 local), 1 tag. Little-endian packed layout, like `pawncc`.
    fn sample_block() -> Vec<u8> {
        let mut b = Vec::new();

        // Tables (built first so we know the total size).
        let mut tables = Vec::new();

        // --- files: 2 ---
        push_u32(&mut tables, 0); // a.pwn starts at 0
        push_cstr(&mut tables, "a.pwn");
        push_u32(&mut tables, 100); // b.inc starts at 100
        push_cstr(&mut tables, "b.inc");

        // --- lines: 4 (sorted by address) ---
        for (addr, line) in [(0u32, 1i32), (8, 2), (20, 3), (104, 10)] {
            push_u32(&mut tables, addr);
            push_i32(&mut tables, line);
        }

        // --- symbols: 3 ---
        // function "main": iFUNCTN, scope [0, 40)
        push_symbol(&mut tables, 0, 0, 0, 40, 9, 0, "main", &[]);
        // global "g_score": iVARIABLE global, address 200
        push_symbol(&mut tables, 200, 1, 0, 0, 1, 0, "g_score", &[]);
        // local "tmp": iVARIABLE local, scope [8, 40), relative address -4
        push_symbol(
            &mut tables,
            (-4i32).cast_unsigned(),
            0,
            8,
            40,
            1,
            1,
            "tmp",
            &[],
        );

        // --- tags: 1 ---
        push_i16(&mut tables, 1);
        push_cstr(&mut tables, "Float");

        // --- header (22 bytes) ---
        let total = 22 + tables.len();
        push_i32(&mut b, i32::try_from(total).unwrap()); // size
        push_u16(&mut b, AMX_DBG_MAGIC); // magic
        b.push(1); // file_version
        b.push(1); // amx_version
        push_i16(&mut b, 0); // flags
        push_i16(&mut b, 2); // files
        push_i16(&mut b, 4); // lines
        push_i16(&mut b, 3); // symbols
        push_i16(&mut b, 1); // tags
        push_i16(&mut b, 0); // automatons
        push_i16(&mut b, 0); // states
        assert_eq!(b.len(), 22);

        b.extend_from_slice(&tables);
        b
    }

    fn push_u16(v: &mut Vec<u8>, x: u16) {
        v.extend_from_slice(&x.to_le_bytes());
    }
    fn push_i16(v: &mut Vec<u8>, x: i16) {
        v.extend_from_slice(&x.to_le_bytes());
    }
    fn push_u32(v: &mut Vec<u8>, x: u32) {
        v.extend_from_slice(&x.to_le_bytes());
    }
    fn push_i32(v: &mut Vec<u8>, x: i32) {
        v.extend_from_slice(&x.to_le_bytes());
    }
    fn push_cstr(v: &mut Vec<u8>, s: &str) {
        v.extend_from_slice(s.as_bytes());
        v.push(0);
    }

    #[allow(clippy::too_many_arguments)]
    fn push_symbol(
        v: &mut Vec<u8>,
        address: u32,
        tag: i16,
        codestart: u32,
        codeend: u32,
        ident: u8,
        vclass: u8,
        name: &str,
        dims: &[(i16, u32)],
    ) {
        push_u32(v, address);
        push_i16(v, tag);
        push_u32(v, codestart);
        push_u32(v, codeend);
        v.push(ident);
        v.push(vclass);
        push_i16(v, i16::try_from(dims.len()).unwrap());
        push_cstr(v, name);
        for &(dtag, size) in dims {
            push_i16(v, dtag);
            push_u32(v, size);
        }
    }

    #[test]
    fn parses_header_and_tables() {
        let dbg = AmxDbg::parse(&sample_block()).expect("parse");
        assert_eq!(dbg.files.len(), 2);
        assert_eq!(dbg.lines.len(), 4);
        assert_eq!(dbg.symbols.len(), 3);
        assert_eq!(dbg.tags.len(), 1);
        assert_eq!(dbg.files[1].name, "b.inc");
        assert_eq!(dbg.files[1].address, 100);
        assert_eq!(dbg.tag_name(1), Some("Float"));
    }

    #[test]
    fn lookup_line_and_file() {
        let dbg = AmxDbg::parse(&sample_block()).unwrap();
        // Lines in the block are ZERO-BASED (as the compiler writes them); the
        // lib re-bases to 1-based. The block has lines 1,2,3,10 → become 2,3,4,11.
        assert_eq!(dbg.lookup_line(0), Some(2));
        assert_eq!(dbg.lookup_line(10), Some(3));
        assert_eq!(dbg.lookup_line(25), Some(4));
        assert_eq!(dbg.lookup_line(104), Some(11));
        assert_eq!(dbg.lookup_file(5), Some("a.pwn"));
        assert_eq!(dbg.lookup_file(150), Some("b.inc"));
    }

    #[test]
    fn lookup_function_by_address() {
        let dbg = AmxDbg::parse(&sample_block()).unwrap();
        assert_eq!(dbg.lookup_function(10), Some("main")); // inside [0,40)
        assert_eq!(dbg.lookup_function(50), None); // outside any function
    }

    #[test]
    fn line_to_address_for_breakpoint() {
        let dbg = AmxDbg::parse(&sample_block()).unwrap();
        // 1-based (the lib adds +1 to the file's zero-based): line 4 in a.pwn → 20.
        assert_eq!(dbg.line_to_address(4, Some("a.pwn")), Some(20));
        // line 11 only exists in b.inc.
        assert_eq!(dbg.line_to_address(11, Some("b.inc")), Some(104));
    }

    #[test]
    fn symbols_in_scope_filters_by_address() {
        let dbg = AmxDbg::parse(&sample_block()).unwrap();
        // At addr 10: global g_score always visible; local tmp in scope [8,40).
        let names: Vec<&str> = dbg
            .symbols_in_scope(10)
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(names.contains(&"g_score"));
        assert!(names.contains(&"tmp"));
        // At addr 5 (before tmp's scope): only the global.
        let names: Vec<&str> = dbg
            .symbols_in_scope(5)
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert!(names.contains(&"g_score"));
        assert!(!names.contains(&"tmp"));
    }

    #[test]
    fn symbol_classes_decoded() {
        let dbg = AmxDbg::parse(&sample_block()).unwrap();
        let g = dbg.symbols.iter().find(|s| s.name == "g_score").unwrap();
        assert_eq!(g.ident, Ident::Variable);
        assert_eq!(g.vclass, VClass::Global);
        let f = dbg.symbols.iter().find(|s| s.name == "main").unwrap();
        assert_eq!(f.ident, Ident::Function);
    }

    #[test]
    fn effective_address_global_and_local() {
        let dbg = AmxDbg::parse(&sample_block()).unwrap();
        let g = dbg.symbols.iter().find(|s| s.name == "g_score").unwrap();
        let tmp = dbg.symbols.iter().find(|s| s.name == "tmp").unwrap();
        // Global: absolute address, frame ignored.
        assert_eq!(g.effective_address(9_999), 200);
        // Local: frame-relative — frm(100) + (-4) = 96.
        assert_eq!(tmp.effective_address(100), 96);
        assert!(!g.is_array());
    }

    #[test]
    fn rejects_bad_magic() {
        let mut bad = sample_block();
        bad[4] = 0x00; // corrupt the magic
        bad[5] = 0x00;
        assert!(AmxDbg::parse(&bad).is_err());
    }

    #[test]
    fn rejects_truncated() {
        let block = sample_block();
        assert!(AmxDbg::parse(&block[..10]).is_err());
    }

    /// Large gamemodes have more than 65535 lines; the count field is 16-bit
    /// and "overflows". The parser detects the overflow when `lines` read as
    /// `i16` goes negative (u16 > 32767) and then reads the extra 65536 blocks
    /// by address monotonicity. Here we simulate 98304 lines: the low 16 bits =
    /// 32768, which as `i16` is negative — the overflow trigger.
    #[test]
    fn handles_line_count_overflow() {
        const TOTAL: u32 = 98304; // 65536 + 32768; low 16 bits = 0x8000 (i16 < 0)
        let mut tables = Vec::new();
        // 1 file
        push_u32(&mut tables, 0);
        push_cstr(&mut tables, "big.pwn");
        // TOTAL lines with strictly increasing address (4-byte step).
        for i in 0..TOTAL {
            push_u32(&mut tables, i * 4);
            push_i32(&mut tables, i.cast_signed() + 1);
        }
        // no symbols/tags/etc.

        let mut b = Vec::new();
        push_i32(&mut b, i32::try_from(22 + tables.len()).unwrap_or(i32::MAX));
        push_u16(&mut b, AMX_DBG_MAGIC);
        b.push(1);
        b.push(1);
        push_i16(&mut b, 0); // flags
        push_i16(&mut b, 1); // files
        // lines = TOTAL & 0xFFFF (the low 16 bits): 98304 & 0xFFFF = 32768 = 0x8000
        push_u16(&mut b, (TOTAL & 0xFFFF) as u16);
        push_i16(&mut b, 0); // symbols
        push_i16(&mut b, 0); // tags
        push_i16(&mut b, 0); // automatons
        push_i16(&mut b, 0); // states
        b.extend_from_slice(&tables);

        let dbg = AmxDbg::parse(&b).expect("parse with line overflow");
        assert_eq!(dbg.lines.len(), TOTAL as usize, "read the real line total");
        // Lookup at the end of the table. Lines in the block are zero-based; the
        // lib adds +1. The last entry has line = (TOTAL-1)+1 (stored) → +1 from
        // the lib = TOTAL+1.
        assert_eq!(
            dbg.lookup_line((TOTAL - 1) * 4),
            Some(TOTAL.cast_signed() + 1)
        );
    }

    /// A corrupted count that would request a huge allocation must not hang nor
    /// blow up memory: the sanity ceiling caps by the buffer size and the parse
    /// fails gracefully (no panic).
    #[test]
    fn corrupt_count_does_not_oom() {
        let mut b = Vec::new();
        push_i32(&mut b, 22); // size
        push_u16(&mut b, AMX_DBG_MAGIC);
        b.push(1);
        b.push(1);
        push_i16(&mut b, 0); // flags
        push_i16(&mut b, 0); // files
        push_u16(&mut b, 0xFFFF); // lines: 65535 (but no data) → must not OOM
        push_i16(&mut b, 0x7FFF); // symbols: 32767 with no data
        push_i16(&mut b, 0);
        push_i16(&mut b, 0);
        push_i16(&mut b, 0);
        // No table body: the parse fails somewhere, but WITHOUT hanging/allocating
        // billions (the ceiling uses the remaining bytes).
        let _ = AmxDbg::parse(&b); // must not panic
    }
}
