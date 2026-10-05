//! AMX strings against arbitrary cells and sizes.
//!
//! The cells of a string come from the script, and so does the size of the
//! array a native writes into: a script can declare a native with any
//! signature and pass whatever it likes. So decoding must survive any cell
//! values and any claimed length, and writing must stay inside the buffer it
//! was given — and what is written must read back as the same text whenever
//! the encoding can represent it.
//!
//! Run with:
//!
//! ```sh
//! cargo +nightly fuzz run amx_string
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use samp_sdk::cell::{AmxString, Buffer, Ref};

const ENCODINGS: [&str; 3] = ["utf-8", "windows-1251", "windows-1252"];

fn buffer(cells: &mut [i32]) -> Buffer<'_> {
    let len = cells.len();
    // SAFETY: the pointer covers `len` live cells for the borrow of `cells`.
    Buffer::new(unsafe { Ref::new(0, cells.as_mut_ptr()) }, len)
}

fuzz_target!(|data: &[u8]| {
    let Some((&selector, rest)) = data.split_first() else {
        return;
    };
    let Some((claimed, rest)) = rest.split_first_chunk::<4>() else {
        return;
    };
    samp_sdk::encoding::set_default_encoding_by_label(
        ENCODINGS[usize::from(selector) % ENCODINGS.len()],
    );

    // Decode: whatever the cells hold and however long the string claims to be.
    let mut cells: Vec<i32> = rest
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    let claimed = u32::from_le_bytes(*claimed) as usize;
    let capacity = cells.len() * 4;
    let string = AmxString::from_buffer_parts(buffer(&mut cells), claimed);
    assert!(string.to_bytes().len() <= capacity);
    let _ = string.as_str();
    drop(string);

    // Write, into a buffer sized by the input, then read it back.
    let text = String::from_utf8_lossy(rest);
    let size = usize::from(selector) % 64;
    let mut out = vec![0x7FFF_FFFF_i32; size + 1];
    let guard = out[size];
    if buffer(&mut out[..size]).write_str(&text).is_ok() {
        let (encoded, lost) = samp_sdk::encoding::encode_checked(&text);
        let len = encoded.len();
        let read = AmxString::from_buffer_parts(buffer(&mut out[..size]), len);
        if !lost && !text.contains('\0') {
            assert_eq!(read.as_str(), text, "a written string read back differently");
        }
    }
    assert_eq!(out[size], guard, "write_str went past the end of its buffer");
});
