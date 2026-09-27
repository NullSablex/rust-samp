//! `samp::pawn_include` against arbitrary text.
//!
//! The parser and the template renderer read files from outside the plugin —
//! an include named by `SAMP_PAWN_INCLUDE_CHECK`, a template named by
//! `SAMP_PAWN_INCLUDE_TEMPLATE` — and they run inside the server, at load. A
//! panic there ends the server's process, so none may panic on any input.

#![no_main]

use libfuzzer_sys::fuzz_target;
use samp::pawn_include::{
    Template, compare_declarations, compare_with, missing_forwards, parse, parse_forwards,
    pawndoc, registered,
};

/// Declarations shaped like what `#[native]` renders: plain, tagged, by
/// reference, varargs, and a `raw` one commented out.
const DECLS: &[&str] = &[
    "native Counter_Increment();",
    "native bool:Counter_Get(&out);",
    "native Float:Counter_Scale(Float:x, const name[] = \"\", dest[], len = sizeof(dest));",
    "native Counter_Printf(const format[], {Float,_}:...);",
    "// native bool:Counter_Raw(...); // raw native — fill in the arguments",
];

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };

    let declared = parse(text);
    let _ = parse_forwards(text);
    let _ = compare_declarations(&declared, &registered(DECLS));
    // The input as the plugin's side, too: a hostile `#[native]` shape.
    let _ = compare_declarations(&parse(DECLS[1]), &registered(&[text]));
    let _ = compare_with(text, DECLS);
    let _ = missing_forwards(text, &["OnThing(a, b)", text]);
    let _ = pawndoc(text);

    let _ = Template::new(text, DECLS)
        .var("VERSION", "1.0.0")
        .var("NAME", text)
        .docs(&[text, "", "@param x y", "<summary>s</summary>", ""])
        .with_docs()
        .callbacks(&[text, "OnX(a)"])
        .plugin_name(text)
        .render();
});
