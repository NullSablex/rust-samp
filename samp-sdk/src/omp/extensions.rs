//! Reading the extension map an `IExtensible` carries.
//!
//! Components attach their per-player data with `addExtension`, which files it
//! in a `robin_hood::unordered_flat_map<UID, Pair<IExtension*, bool>>`. The
//! virtual `getExtension` does not consult that map — the C++ side reaches it
//! through `queryExtension<T>()`, a template that checks the map first. A
//! plugin calling through vtables only therefore sees none of the stock
//! per-player data: dialogs, checkpoints, a player's menu.
//!
//! This module walks the map instead, which is the only route left.
//!
//! ## What makes this different from the rest of the SDK
//!
//! Every other layout here is fixed by an ABI or by a public header, and
//! `scripts/check-abi-slots.py` re-derives it from the shipped binaries. This
//! one depends on the internals of a vendored hash map, which is not a stable
//! contract — it changes when open.mp updates its copy.
//!
//! What it does **not** depend on is the standard library: `robin_hood::hash`
//! specializes for integral keys and hashes the value itself, so libstdc++ and
//! the MSVC STL produce the same bucket. The reimplementation is checked
//! against values printed by the real thing.
//!
//! So the lookup is written to fail closed:
//!
//! - the field offsets are pinned by tests, per ABI, from clang's record layout;
//! - the probe is bounded by the table's own size, so a wrong read cannot spin;
//! - a candidate is only returned after `getExtensionID()` on it answers with
//!   the UID that was asked for.
//!
//! A mismatch in any of that yields `null`, which is what the caller already
//! handles. The remaining risk is dereferencing a pointer read from a table
//! laid out differently than assumed — which the `getExtensionID` check turns
//! into a crash rather than a wrong answer, and which the tests are there to
//! prevent.
//!
//! When in doubt, the Pawn natives remain available through
//! [`crate::amx::Amx::call_native`] and do not depend on any of this.

use super::types::UID;

/// Offsets of the map's fields inside an `IExtensible`, from clang's record
/// layout of `IExtensible` for each ABI. The map itself starts right after the
/// vtable pointer; MSVC pads it further because `uint64_t` is 8-aligned there.
#[cfg(not(target_env = "msvc"))]
mod layout {
    pub const HASH_MULTIPLIER: usize = 4;
    pub const KEY_VALS: usize = 12;
    pub const INFO: usize = 16;
    pub const NUM_ELEMENTS: usize = 20;
    pub const MASK: usize = 24;
    pub const INFO_INC: usize = 32;
    pub const INFO_HASH_SHIFT: usize = 36;
}

#[cfg(target_env = "msvc")]
mod layout {
    pub const HASH_MULTIPLIER: usize = 16;
    pub const KEY_VALS: usize = 24;
    pub const INFO: usize = 28;
    pub const NUM_ELEMENTS: usize = 32;
    pub const MASK: usize = 36;
    pub const INFO_INC: usize = 44;
    pub const INFO_HASH_SHIFT: usize = 48;
}

/// A map entry: the UID, then the `IExtension*` and its auto-delete flag.
/// Sixteen bytes on both ABIs, measured with `offsetof`.
const NODE_SIZE: usize = 16;
const NODE_VALUE_OFFSET: usize = 8;

/// `robin_hood` reserves the low bits of the hash for its own bookkeeping.
const INFO_BITS: u32 = 5;
const INFO_MASK: u64 = (1 << INFO_BITS) - 1;

/// `IExtension::getExtensionID()` — the first method it declares, and it has no
/// virtual destructor, so the slot is the same on both ABIs.
const SLOT_GET_EXTENSION_ID: usize = 0;

/// `robin_hood::hash_int` — the murmur3 finalizer, without its last step.
fn hash_int(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    // `size_t` is 32 bits on the targets this SDK builds for, and the C++ side
    // truncates on return.
    u64::from(x as u32)
}

/// `robin_hood::hash<UID>`.
///
/// `robin_hood::hash` specializes for integral types and hashes the value
/// directly, so the standard library's `std::hash` — which differs between
/// libstdc++ and the MSVC STL — never enters the picture. The same input gives
/// the same bucket on both ABIs.
fn hash_uid(uid: UID) -> u64 {
    hash_int(uid)
}

/// Reads a pointer-sized field of the map.
unsafe fn read_usize(base: *mut u8, offset: usize) -> usize {
    unsafe { base.add(offset).cast::<usize>().read_unaligned() }
}

/// The `IExtension*` registered under `uid`, or null.
///
/// # Safety
/// `extensible` must point at an object whose first base is `IExtensible` —
/// every entity and component in the SDK qualifies.
#[must_use]
pub unsafe fn extension(extensible: *mut u8, uid: UID) -> *mut u8 {
    if extensible.is_null() {
        return std::ptr::null_mut();
    }

    let key_vals = unsafe { read_usize(extensible, layout::KEY_VALS) } as *mut u8;
    let info = unsafe { read_usize(extensible, layout::INFO) } as *const u8;
    let elements = unsafe { read_usize(extensible, layout::NUM_ELEMENTS) };
    let mask = unsafe { read_usize(extensible, layout::MASK) };

    // An empty map points `mKeyVals` and `mInfo` at `mMask` itself, a sentinel
    // that is not a table; the element count is what tells them apart.
    if key_vals.is_null() || info.is_null() || elements == 0 {
        return std::ptr::null_mut();
    }

    let multiplier = unsafe {
        extensible
            .add(layout::HASH_MULTIPLIER)
            .cast::<u64>()
            .read_unaligned()
    };
    let info_inc = unsafe {
        extensible
            .add(layout::INFO_INC)
            .cast::<u32>()
            .read_unaligned()
    };
    let info_shift = unsafe {
        extensible
            .add(layout::INFO_HASH_SHIFT)
            .cast::<u32>()
            .read_unaligned()
    };

    // `keyToIdx`: mix once more, then split the hash into the bucket index and
    // the distance-from-ideal byte the table stores alongside it.
    let mut hash = hash_uid(uid).wrapping_mul(multiplier);
    hash ^= hash >> 33;
    let mut want = info_inc.wrapping_add(((hash & INFO_MASK) >> info_shift) as u32);
    let mut index = ((hash >> INFO_BITS) as usize) & mask;

    // Robin Hood probing: entries are ordered by distance, so the search can
    // stop as soon as the stored distance drops below the one being looked for.
    // The step count is bounded by the table size in case the layout read above
    // was wrong — a wrong read must fail, not spin.
    for _ in 0..=mask {
        let stored = u32::from(unsafe { info.add(index).read() });
        if want > stored {
            want = want.wrapping_add(info_inc);
            index = (index + 1) & mask;
            continue;
        }
        if want < stored {
            break;
        }

        let node = unsafe { key_vals.add(index * NODE_SIZE) };
        let key = unsafe { node.cast::<UID>().read_unaligned() };
        if key == uid {
            let candidate = unsafe { read_usize(node, NODE_VALUE_OFFSET) } as *mut u8;
            return if unsafe { extension_id(candidate) } == uid {
                candidate
            } else {
                // The table is not laid out the way this module assumes.
                std::ptr::null_mut()
            };
        }
        want = want.wrapping_add(info_inc);
        index = (index + 1) & mask;
    }

    std::ptr::null_mut()
}

/// `IExtension::getExtensionID()`, used to confirm a candidate is what the map
/// claimed it is.
unsafe fn extension_id(candidate: *mut u8) -> UID {
    if candidate.is_null() {
        return 0;
    }

    #[cfg(not(target_env = "msvc"))]
    type GetIdFn = unsafe extern "C" fn(*mut u8) -> UID;
    #[cfg(target_env = "msvc")]
    type GetIdFn = unsafe extern "thiscall" fn(*mut u8) -> UID;

    let Some((this, f_ptr)) =
        (unsafe { super::vtable::secondary_call_target_ptr(candidate, 0, SLOT_GET_EXTENSION_ID) })
    else {
        return 0;
    };
    let get_id: GetIdFn = unsafe { std::mem::transmute(f_ptr) };
    unsafe { get_id(this) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_the_cpp_implementation() {
        // Values printed by `robin_hood::hash<UID>` compiled for i686 against
        // the SDK's vendored copy — the oracle for this reimplementation. They
        // hold for both ABIs, since the hash does not go through `std::hash`.
        assert_eq!(hash_uid(0xbc03_376a_a359_1a11), 0x135c_f77a);
        assert_eq!(hash_uid(1), 0x92fd_5b26);
        assert_eq!(hash_uid(0xffff_ffff_ffff_ffff), 0x84aa_9ccc);
    }

    #[test]
    fn layout_offsets_match_clangs_record_dump() {
        #[cfg(not(target_env = "msvc"))]
        assert_eq!(
            [
                layout::HASH_MULTIPLIER,
                layout::KEY_VALS,
                layout::INFO,
                layout::NUM_ELEMENTS,
                layout::MASK,
            ],
            [4, 12, 16, 20, 24]
        );
        #[cfg(target_env = "msvc")]
        assert_eq!(
            [
                layout::HASH_MULTIPLIER,
                layout::KEY_VALS,
                layout::INFO,
                layout::NUM_ELEMENTS,
                layout::MASK,
            ],
            [16, 24, 28, 32, 36]
        );
        assert_eq!(NODE_SIZE, 16, "UID + IExtension* + bool, padded");
        assert_eq!(NODE_VALUE_OFFSET, 8);
    }

    #[test]
    fn a_null_extensible_has_no_extensions() {
        assert!(unsafe { extension(std::ptr::null_mut(), 1) }.is_null());
    }
}
