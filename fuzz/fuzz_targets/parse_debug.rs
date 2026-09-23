//! `samp_sdk::debug::parse` against arbitrary bytes.
//!
//! The debug block comes from a `.amx` file on the server's disk — a file the
//! plugin did not produce and a tool like a debugger reads on demand. So the
//! parser's contract is that **no input may panic or hang it**: a malformed
//! block must come back as `DbgError`, never as an out-of-bounds read, an
//! arithmetic overflow, or an allocation sized by a count the file claims.
//!
//! Run with:
//!
//! ```sh
//! cargo +nightly fuzz run parse_debug
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The result is deliberately ignored: what is under test is that the call
    // returns at all, with either variant.
    let _ = samp_sdk::debug::AmxDbg::parse(data);
});
