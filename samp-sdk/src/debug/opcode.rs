//! AMX opcode numbers, operand sizes and the dispatch-table decoder.
//!
//! The opcode numbering is the order of the opcode enum in `amx.c`, and it is
//! the same for SA-MP and open.mp because both embed the same AMX VM. Reading
//! the code segment gives a raw cell that is only the opcode number on a
//! non-relocated image; see [`OpcodeMap`] for the computed-goto case.
//!
//! Pure logic, no FFI: usable both inside a plugin and from a host-side tool
//! that depends on the SDK with `default-features = false, features =
//! ["debug"]`.

use std::collections::HashMap;

/// Number of opcodes in the VM (`OP_NUM_OPCODES`) — the length of
/// `amx_opcodelist`, and the `count` to pass to
/// [`Amx::opcode_table`](crate::amx::Amx::opcode_table).
pub const OP_NUM_OPCODES: usize = 158;

/// Marks a variable-length instruction in [`OP_PARAMS`] (`casetbl`/`switch`,
/// or an invalid opcode): the operand count is not known statically, so a
/// scanner cannot compute where the next instruction starts.
pub const OP_VARIABLE_LENGTH: u8 = 99;

/// Inline operand cells per opcode, derived from `amx_BrowseRelocate` in
/// `amx.c`. Index by opcode number; [`OP_VARIABLE_LENGTH`] means the size is
/// not static. Prefer [`operand_cells`], which bounds-checks the index.
#[rustfmt::skip]
pub const OP_PARAMS: [u8; OP_NUM_OPCODES] = [
    99,1,1,1,1,1,1,1,1,0,1,1,1,1,1,1,1,1,1,1,1,1,1,0,1,0,1,0,1,1,1,1,1,0,0,0,
    0,0,1,1,1,1,0,0,1,1,0,0,0,1,0,1,1,1,1,1,1,1,1,1,1,1,1,1,1,0,0,0,1,1,1,1,
    0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1,1,0,0,1,1,0,0,0,0,0,0,0,0,0,0,0,0,1,1,0,
    0,1,1,0,0,0,1,1,0,1,1,1,1,1,0,1,0,0,0,0,0,99,99,0,0,1,0,2,1,0,2,2,2,2,3,
    3,3,3,4,4,4,4,5,5,5,5,2,2,2,2,
];

/// Inline operand cells of `opcode`, or `None` when the opcode is out of range
/// or has a variable length ([`OP_VARIABLE_LENGTH`]).
///
/// An instruction occupies `1 + operand_cells(op)` cells, so a scanner walking
/// a source line advances by that much — and must stop when this returns
/// `None`, since it cannot know where the next instruction begins.
#[must_use]
pub fn operand_cells(opcode: i32) -> Option<u8> {
    let params = *OP_PARAMS.get(usize::try_from(opcode).ok()?)?;
    (params != OP_VARIABLE_LENGTH).then_some(params)
}

/// Stack/heap safety margin of `amx.c` (`STKMARGIN`, 16 cells in bytes). The VM
/// raises `AMX_ERR_STACKERR` when `hea + STK_MARGIN > stk`.
pub const STK_MARGIN: i32 = 16 * 4;

// Load/store and address arithmetic.
/// `pri = data[offs]`
pub const OP_LOAD_PRI: i32 = 1;
/// `alt = data[offs]`
pub const OP_LOAD_ALT: i32 = 2;
/// `pri = data[frm + offs]`
pub const OP_LOAD_S_PRI: i32 = 3;
/// `alt = data[frm + offs]`
pub const OP_LOAD_S_ALT: i32 = 4;
/// `pri = data[pri]` — indirect load, checked by `VERIFYADDRESS`.
pub const OP_LOAD_I: i32 = 9;
/// `pri = data[pri]` (byte/word) — indirect load, checked by `VERIFYADDRESS`.
pub const OP_LODB_I: i32 = 10;
/// `pri = constant`
pub const OP_CONST_PRI: i32 = 11;
/// `alt = constant`
pub const OP_CONST_ALT: i32 = 12;
/// `pri = frm + offs`
pub const OP_ADDR_PRI: i32 = 13;
/// `alt = frm + offs`
pub const OP_ADDR_ALT: i32 = 14;
/// `data[alt] = pri` — indirect store, checked by `VERIFYADDRESS`.
pub const OP_STOR_I: i32 = 23;
/// `data[alt] = pri` (byte/word) — indirect store, checked by `VERIFYADDRESS`.
pub const OP_STRB_I: i32 = 24;
/// `pri = data[pri * 4 + alt]` — indexed load, checked by `VERIFYADDRESS`.
pub const OP_LIDX: i32 = 25;
/// `pri = data[(pri << n) + alt]` — indexed load, checked by `VERIFYADDRESS`.
pub const OP_LIDX_B: i32 = 26;
/// `pri = pri * 4 + alt`
pub const OP_IDXADDR: i32 = 27;
/// `pri = (pri << n) + alt`
pub const OP_IDXADDR_B: i32 = 28;

// Register moves.
/// `pri = alt`
pub const OP_MOVE_PRI: i32 = 33;
/// `alt = pri`
pub const OP_MOVE_ALT: i32 = 34;
/// Swaps `pri` and `alt`.
pub const OP_XCHG: i32 = 35;

// Stack and heap.
/// Pushes `pri`.
pub const OP_PUSH_PRI: i32 = 36;
/// Pushes `alt`.
pub const OP_PUSH_ALT: i32 = 37;
/// Pushes `pri` `offs` times.
pub const OP_PUSH_R: i32 = 38;
/// Pushes a constant.
pub const OP_PUSH_C: i32 = 39;
/// Pushes `data[offs]`.
pub const OP_PUSH: i32 = 40;
/// Pushes `data[frm + offs]`.
pub const OP_PUSH_S: i32 = 41;
/// Pops into `pri`.
pub const OP_POP_PRI: i32 = 42;
/// Pops into `alt`.
pub const OP_POP_ALT: i32 = 43;
/// `alt = stk; stk += offs` — runs `CHKMARGIN`.
pub const OP_STACK: i32 = 44;
/// `alt = hea; hea += offs` — runs `CHKMARGIN` and `CHKHEAP`.
pub const OP_HEAP: i32 = 45;
/// Function prologue: pushes `frm`, then `frm = stk` — runs `CHKMARGIN`.
pub const OP_PROC: i32 = 46;
/// Pushes the return address and jumps.
pub const OP_CALL: i32 = 49;
/// Like [`OP_CALL`], with the target in `pri`.
pub const OP_CALL_PRI: i32 = 50;
/// Pushes `frm + offs`.
pub const OP_PUSH_ADR: i32 = 133;
/// First of the composite push opcodes (`push2.c`…`push5.adr`), which push 2 to
/// 5 values in one instruction.
pub const OP_PUSH2_C: i32 = 138;
/// Last of the composite push opcodes — see [`OP_PUSH2_C`].
pub const OP_PUSH5_ADR: i32 = 153;

// Arithmetic that can abort, and the checks.
/// Signed division; the divisor is in `alt`.
pub const OP_SDIV: i32 = 73;
/// Signed division; the divisor is in `pri`.
pub const OP_SDIV_ALT: i32 = 74;
/// Unsigned division; the divisor is in `alt`.
pub const OP_UDIV: i32 = 76;
/// Unsigned division; the divisor is in `pri`.
pub const OP_UDIV_ALT: i32 = 77;
/// `pri = 0`
pub const OP_ZERO_PRI: i32 = 89;
/// `alt = 0`
pub const OP_ZERO_ALT: i32 = 90;
/// Array bounds check: aborts when `(unsigned) pri > limit`.
pub const OP_BOUNDS: i32 = 121;
/// Debug break — emitted at the start of each source line by `pawncc -d2`/`-d3`.
pub const OP_BREAK: i32 = 137;

/// Translates a raw code-segment cell into the real opcode number.
///
/// On a server built with computed-goto threading (GCC/Clang — the SA-MP and
/// open.mp builds), the loader rewrites every opcode in the code segment into
/// the **address** of its handler label, so
/// [`Amx::read_code`](crate::amx::Amx::read_code) yields a pointer rather than
/// an opcode. This inverts the VM's dispatch table (address → opcode) to undo
/// that.
///
/// Build it once per VM, from
/// [`Amx::opcode_table`](crate::amx::Amx::opcode_table) — or straight from an
/// `Amx` with [`Amx::opcode_map`](crate::amx::Amx::opcode_map).
///
/// ```
/// use samp_sdk::debug::OpcodeMap;
/// use samp_sdk::debug::opcode::OP_BOUNDS;
///
/// // Fake dispatch table: opcode 121 (`bounds`) handled at address 0xdead.
/// let mut table = vec![0usize; 158];
/// table[OP_BOUNDS as usize] = 0xdead;
///
/// let map = OpcodeMap::new(Some(table));
/// assert_eq!(map.decode(0xdead), Some(OP_BOUNDS));
/// ```
pub struct OpcodeMap {
    /// `label address → opcode number`. Empty when the image is not relocated.
    inverse: HashMap<usize, i32>,
}

impl OpcodeMap {
    /// Builds the map from the VM's dispatch table (`amx_opcodelist`).
    ///
    /// `None` — the table could not be fetched — yields an empty map, which
    /// [`decode`](Self::decode) treats as a non-relocated image.
    #[must_use]
    pub fn new(opcode_table: Option<Vec<usize>>) -> Self {
        let inverse = opcode_table
            .map(|table| {
                table
                    .into_iter()
                    .enumerate()
                    .map(|(op, addr)| (addr, i32::try_from(op).unwrap_or(-1)))
                    .collect()
            })
            .unwrap_or_default();
        Self { inverse }
    }

    /// `true` when the map is empty, i.e. the code segment holds plain opcode
    /// numbers rather than label addresses.
    #[must_use]
    pub fn is_identity(&self) -> bool {
        self.inverse.is_empty()
    }

    /// Real opcode behind a raw `code[cip]` value: resolves a computed-goto
    /// label address, or accepts a plain opcode number on a non-relocated
    /// image. `None` when the value is neither.
    #[must_use]
    pub fn decode(&self, raw: i32) -> Option<i32> {
        if self.inverse.is_empty() {
            return Some(raw);
        }
        if let Some(&op) = self
            .inverse
            .get(&usize::try_from(raw.cast_unsigned()).ok()?)
        {
            return Some(op);
        }
        (0..i32::try_from(OP_NUM_OPCODES).ok()?)
            .contains(&raw)
            .then_some(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operand_cells_known_opcodes() {
        // `load.pri` takes one inline operand, `move.pri` none.
        assert_eq!(operand_cells(OP_LOAD_PRI), Some(1));
        assert_eq!(operand_cells(OP_MOVE_PRI), Some(0));
        // `push5.adr` pushes five values, so five inline operands.
        assert_eq!(operand_cells(OP_PUSH5_ADR), Some(5));
    }

    #[test]
    fn operand_cells_rejects_variable_and_out_of_range() {
        // Opcode 0 is `casetbl`-like: variable length.
        assert_eq!(operand_cells(0), None);
        assert_eq!(operand_cells(-1), None);
        assert_eq!(operand_cells(i32::try_from(OP_NUM_OPCODES).unwrap()), None);
    }

    #[test]
    fn decode_resolves_relocated_addresses() {
        let mut table = vec![0usize; OP_NUM_OPCODES];
        table[OP_SDIV as usize] = 0x1000;
        table[OP_BREAK as usize] = 0x2000;
        let map = OpcodeMap::new(Some(table));

        assert!(!map.is_identity());
        assert_eq!(map.decode(0x1000), Some(OP_SDIV));
        assert_eq!(map.decode(0x2000), Some(OP_BREAK));
    }

    #[test]
    fn decode_accepts_plain_opcode_on_relocated_image() {
        let mut table = vec![0usize; OP_NUM_OPCODES];
        table[OP_SDIV as usize] = 0x1000;
        let map = OpcodeMap::new(Some(table));

        // Not a label address, but a valid opcode number: accepted.
        assert_eq!(map.decode(OP_BOUNDS), Some(OP_BOUNDS));
        // Neither a label address nor a valid opcode.
        assert_eq!(map.decode(0x7fff_0000), None);
    }

    #[test]
    fn decode_is_identity_without_a_table() {
        let map = OpcodeMap::new(None);
        assert!(map.is_identity());
        // Non-relocated image: the raw value already is the opcode.
        assert_eq!(map.decode(OP_BOUNDS), Some(OP_BOUNDS));
        assert_eq!(map.decode(0x7fff_0000), Some(0x7fff_0000));
    }
}
