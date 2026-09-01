//! Call-stack walking over the AMX frame chain.
//!
//! Pure logic: the caller supplies the cell reader, so the walk is testable
//! with a fake memory map and works both inside a debug hook (feeding it
//! [`Amx::read_cell`](crate::amx::Amx::read_cell)) and host-side.
//!
//! # AMX frame layout
//!
//! The AMX stack grows **downwards** (smaller address = more recent), so a
//! caller's frame sits at a **higher** address. The `OP_PROC` prologue pushes
//! the previous `FRM` and points `FRM` at the top; `OP_CALL` had already pushed
//! the return address. Relative to the current `frm`:
//!
//! ```text
//! [frm]        = the caller's FRM (saved by PROC)
//! [frm + CELL] = the return address inside the caller (pushed by CALL)
//! ```
//!
//! `amx_Exec` pushes a return address of `0` before entering the public it was
//! asked to run, so `[frm + CELL] == 0` marks the bottom of the chain.

/// Size of an AMX cell (32-bit VM).
const CELL: i32 = 4;

/// Depth ceiling for [`walk`] — a guard against a corrupted stack (a frame that
/// does not ascend, a cycle) so a debug hook cannot spin forever.
pub const MAX_DEPTH: usize = 128;

/// Walks the frame chain from the top frame `(top_cip, top_frm)` and returns
/// the `(cip, frm)` of each frame, from the top (index 0, where the VM stopped)
/// down to the entry public.
///
/// `stp` is the top of the stack ([`Amx::stp`](crate::amx::Amx::stp)), the upper
/// bound of a valid data address; `read_cell` reads one cell of the data
/// segment, returning `None` when the address is inaccessible.
///
/// For each caller, `cip` is the saved return address — a code offset inside
/// the calling function, which maps to the line of the call site.
///
/// The walk stops early, keeping what it has, when the chain leaves the stack,
/// stops ascending, cannot be read, or reaches [`MAX_DEPTH`]. It never returns
/// an empty vector: the top frame is always present.
///
/// ```
/// use samp_sdk::debug::stack;
///
/// // Two frames: the top at frm=1000 returns to code 40 in a caller at 1100,
/// // whose own return address is 0 (the entry public).
/// let mem = |addr: i32| match addr {
///     1000 => Some(1100), // caller's FRM
///     1004 => Some(40),   // return address
///     1100 => Some(0),
///     1104 => Some(0),    // return 0 → bottom of the chain
///     _ => None,
/// };
///
/// let frames = stack::walk(8, 1000, 2000, mem);
/// assert_eq!(frames, vec![(8, 1000), (40, 1100)]);
/// ```
#[must_use]
pub fn walk(
    top_cip: u32,
    top_frm: i32,
    stp: i32,
    read_cell: impl Fn(i32) -> Option<i32>,
) -> Vec<(u32, i32)> {
    let mut frames = vec![(top_cip, top_frm)];
    let mut frm = top_frm;

    for _ in 0..MAX_DEPTH {
        // The frame header (two cells) must fit inside the stack.
        if frm <= 0 || frm + CELL >= stp {
            break;
        }
        let (Some(ret), Some(prev)) = (read_cell(frm + CELL), read_cell(frm)) else {
            break;
        };
        // `amx_Exec` pushes a return address of 0 before the entry public.
        if ret <= 0 {
            break;
        }
        frames.push((ret.cast_unsigned(), prev));
        // The caller's frame must be ABOVE (higher address) and inside the
        // stack; otherwise the chain is invalid and we stop after recording it.
        if prev <= frm || prev >= stp {
            break;
        }
        frm = prev;
    }

    frames
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Builds a fake cell reader from `(address, value)` pairs.
    fn mem(pairs: &[(i32, i32)]) -> impl Fn(i32) -> Option<i32> {
        let map: HashMap<i32, i32> = pairs.iter().copied().collect();
        move |addr| map.get(&addr).copied()
    }

    #[test]
    fn single_frame_when_return_is_zero() {
        // Entry public: [frm+4] = 0 (the sentinel `amx_Exec` pushes).
        let read = mem(&[(1000, 0), (1004, 0)]);
        assert_eq!(walk(40, 1000, 2000, read), vec![(40, 1000)]);
    }

    #[test]
    fn walks_two_levels() {
        let read = mem(&[
            (1000, 1100), // top frame: caller's FRM
            (1004, 40),   // top frame: return address
            (1100, 0),
            (1104, 0), // caller is the entry public
        ]);
        assert_eq!(walk(8, 1000, 2000, read), vec![(8, 1000), (40, 1100)]);
    }

    #[test]
    fn stops_when_frame_leaves_the_stack() {
        // The caller's FRM (9000) is past `stp`: record the frame, then stop.
        let read = mem(&[(1000, 9000), (1004, 40)]);
        assert_eq!(walk(8, 1000, 2000, read), vec![(8, 1000), (40, 9000)]);
    }

    #[test]
    fn stops_when_the_chain_does_not_ascend() {
        // A frame pointing at itself would loop forever without the check.
        let read = mem(&[(1000, 1000), (1004, 40)]);
        assert_eq!(walk(8, 1000, 2000, read), vec![(8, 1000), (40, 1000)]);
    }

    #[test]
    fn stops_when_memory_is_unreadable() {
        // Nothing readable at the frame header: only the top frame survives.
        let read = mem(&[]);
        assert_eq!(walk(8, 1000, 2000, read), vec![(8, 1000)]);
    }

    #[test]
    fn depth_is_capped() {
        // Every frame ascends by 8 bytes with a non-zero return address, so the
        // chain only ends at MAX_DEPTH.
        let read = |addr: i32| {
            if addr % 8 == 0 {
                Some(addr + 8) // caller's FRM, always ascending
            } else {
                Some(40) // return address
            }
        };
        assert_eq!(walk(8, 1000, i32::MAX, read).len(), MAX_DEPTH + 1);
    }
}
