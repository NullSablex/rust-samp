//! The generated files that are not one interface's: the mirrored structs,
//! the handlers, and the index.

use super::generator::Generator;
use super::wrappers::is_keyword;

fn ident(field: &str) -> String {
    if is_keyword(field) {
        format!("r#{field}")
    } else {
        field.to_owned()
    }
}

impl Generator<'_> {
    pub fn structs(&self) -> String {
        let mut out: Vec<String> = [
            "//! Structs the open.mp headers pass by value or by reference — generated",
            "//! by `cargo xtask gen-omp`. Do not edit.",
            "//!",
            "//! Each mirrors its C++ struct field for field. The layout is clang's for",
            "//! both ABIs, and the assertions below each struct fail the build if the",
            "//! Rust one differs from it. An anonymous union becomes a named Rust",
            "//! union field (`u0`, ...); a run of bit-fields becomes one integer",
            "//! (`bits_*`), first declared in the lowest bits.",
            "",
            "#![allow(unused_imports, clippy::struct_field_names)]",
            "",
            "use crate::omp::types::{Colour, GTAQuat, Hours, Microseconds, Milliseconds, Minutes, Seconds, TimePoint, Vector2, Vector3, Vector4, WorldTimePoint};",
            "use crate::omp::containers::{FlatSet, HybridString, Pair, Span};",
            "use crate::omp::vtable::VirtualReturn;",
            "use crate::omp::*;",
            "",
        ]
        .map(str::to_owned)
        .to_vec();

        for (name, union) in &self.unions {
            out.extend(union_items(name, union));
        }
        for (name, info) in &self.mirrored {
            out.extend(self.struct_items(name, info));
        }
        if !self.not_mirrored.is_empty() {
            out.push("// What the generator did not mirror, and why.".into());
            out.extend(
                self.not_mirrored
                    .iter()
                    .map(|(name, why)| format!("// skipped: `{name}` — {why}")),
            );
            out.push(String::new());
        }
        out.join("\n")
    }

    /// One mirrored struct: the type, its `Default`, its layout assertions for
    /// each ABI, and its `VirtualReturn`.
    fn struct_items(&self, name: &str, info: &super::mirror::Mirrored) -> Vec<String> {
        let mut out = Vec::new();
        let derives = if info.ref_only {
            None
        } else if self.is_plain(name) {
            Some("Debug, Clone, Copy, PartialEq")
        } else {
            Some("Clone, Copy")
        };
        out.push(format!("/// `{name}` in `{}`.", info.header));
        if info.ref_only {
            out.extend([
                "///".into(),
                "/// The C++ side copies it with a constructor, so it travels by".into(),
                "/// reference only: build one here and pass `&`, or read the server's.".into(),
            ]);
        }
        out.push("#[repr(C)]".into());
        if let Some(derives) = derives {
            out.push(format!("#[derive({derives})]"));
        }
        out.push(format!("pub struct {name} {{"));
        for field in &info.fields {
            match &field.bits {
                Some(bits) => {
                    let spread: Vec<String> =
                        bits.iter().map(|(n, w)| format!("`{n}`: {w}")).collect();
                    out.push(format!(
                        "    /// Bit-fields, from bit 0: {}.",
                        spread.join(", ")
                    ));
                }
                None => out.push(format!("    /// `{}`.", field.cpp)),
            }
            out.push(format!("    pub {}: {},", ident(&field.rust), field.ty));
        }
        out.extend(["}".into(), String::new()]);
        if let Some(defaults) = &info.default {
            out.extend([
                format!("impl Default for {name} {{"),
                "    /// The C++ struct's own defaults.".into(),
                "    fn default() -> Self {".into(),
                "        Self {".into(),
            ]);
            for (field, value) in info.fields.iter().zip(defaults) {
                out.push(format!("            {}: {value},", ident(&field.rust)));
            }
            out.extend([
                "        }".into(),
                "    }".into(),
                "}".into(),
                String::new(),
            ]);
        }
        for (cfg, layout) in [
            ("target_os = \"linux\"", &info.itanium),
            ("target_env = \"msvc\"", &info.msvc),
        ] {
            out.extend([
                format!("#[cfg(all(target_arch = \"x86\", {cfg}))]"),
                "const _: () = {".into(),
                format!(
                    "    assert!(std::mem::size_of::<{name}>() == {});",
                    layout.size / 8
                ),
            ]);
            for (field, offset) in info.fields.iter().zip(&layout.offsets) {
                if let Some(bit) = offset {
                    out.push(format!(
                        "    assert!(std::mem::offset_of!({name}, {}) == {});",
                        ident(&field.rust),
                        bit / 8
                    ));
                }
            }
            out.extend(["};".into(), String::new()]);
        }
        if !info.ref_only {
            out.extend([
                format!("impl VirtualReturn for {name} {{"),
                "    type Raw = Self;".into(),
                "    fn from_raw(raw: Self) -> Self {".into(),
                "        raw".into(),
                "    }".into(),
                "}".into(),
                String::new(),
            ]);
        }
        out
    }

    /// Whether a struct can derive `Debug` and `PartialEq`: no unions, no
    /// bit-fields, nothing copied with a constructor, all the way down.
    fn is_plain(&self, name: &str) -> bool {
        let Some(info) = self.mirrored.get(name) else {
            return true;
        };
        info.fields.iter().all(|field| {
            let ty = field.ty.to_string();
            field.bits.is_none()
                && !self.unions.contains_key(&ty)
                && !field.ty.is_hybrid()
                && !self.is_ref_only(&field.ty)
                && !matches!(field.ty, super::types::Rust::FlatSet(_))
                && (!self.mirrored.contains_key(&ty) || self.is_plain(&ty))
        })
    }

    pub fn handlers_file(&self) -> String {
        let mut out: Vec<String> = [
            "//! Event handlers of the groups the SDK does not write by hand — generated",
            "//! by `cargo xtask gen-omp` from the open.mp headers. Do not edit.",
            "//!",
            "//! Each handler's vtable follows its C++ declaration order, which is the",
            "//! slot order on both ABIs: none of these declares a destructor or an",
            "//! overload. `DEFAULT` does what each C++ body does. Narrow integer",
            "//! arguments arrive as a whole word; the value is its low bits.",
            "",
            "#![allow(unused_imports)]",
            "",
            "use crate::omp::dispatch::event_handler;",
            "use crate::omp::types::{Colour, GTAQuat, Hours, Microseconds, Milliseconds, Minutes, Seconds, StringView, TimePoint, Vector2, Vector3, Vector4};",
            "use crate::omp::containers::{FlatSet, HybridString, Pair, Span};",
            "use crate::omp::*;",
            "",
        ]
        .map(str::to_owned)
        .to_vec();
        for (cpp, handler) in &self.handlers {
            out.extend([
                "event_handler! {".into(),
                format!("    /// `{cpp}` in `{}`.", handler.header),
                format!("    {0}VTable for {0} {{", handler.rust),
            ]);
            for entry in &handler.entries {
                let tail = entry.ret.map(|r| format!(" -> {r}")).unwrap_or_default();
                let given = entry
                    .default
                    .as_ref()
                    .map(|d| format!(" = {d}"))
                    .unwrap_or_default();
                out.push(format!("        /// `{}`.", entry.signature));
                out.push(format!(
                    "        {}: fn({}){tail}{given},",
                    entry.field,
                    entry.args.join(", ")
                ));
            }
            out.extend(["    }".into(), "}".into(), String::new()]);
        }
        if !self.not_handlers.is_empty() {
            out.push("// What the generator did not write, and why.".into());
            out.extend(
                self.not_handlers
                    .iter()
                    .map(|(name, why)| format!("// skipped: `{name}` — {why}")),
            );
            out.push(String::new());
        }
        out.join("\n")
    }
}

/// `mod.rs`: every module, and a re-export of each that defines anything — a
/// module whose every method was skipped still exists, for its list of what
/// was left out.
pub fn index(modules: &[(String, bool)]) -> String {
    let mut out = vec![
        "//! Wrappers generated by `cargo xtask gen-omp` — one module per interface".to_owned(),
        "//! listed in `xtask/omp-wrappers.toml`. Do not edit.".to_owned(),
        String::new(),
    ];
    out.extend(modules.iter().map(|(name, _)| format!("pub mod {name};")));
    out.push(String::new());
    out.extend(
        modules
            .iter()
            .filter(|(_, defines)| *defines)
            .map(|(name, _)| format!("pub use {name}::*;")),
    );
    out.push(String::new());
    out.join("\n")
}

/// Whether generated text defines anything a `pub use` would bring in.
pub fn defines_anything(text: &str) -> bool {
    text.lines().any(|line| {
        ["pub ", "impl ", "opaque!", "virtual_fns!", "event_handler!"]
            .iter()
            .any(|start| line.starts_with(start))
    })
}

/// An anonymous union — or a struct inside one — as a Rust type.
fn union_items(name: &str, union: &super::mirror::Union) -> Vec<String> {
    let mut out = Vec::new();
    let kind = if union.is_union { "union" } else { "struct" };
    out.extend([
        format!("/// An anonymous {kind} inside a mirrored struct."),
        "#[repr(C)]".into(),
        "#[derive(Clone, Copy)]".into(),
        format!("pub {kind} {name} {{"),
    ]);
    for (cpp, field, rust) in &union.members {
        out.extend([
            format!("    /// `{cpp}`."),
            format!("    pub {}: {rust},", ident(field)),
        ]);
    }
    out.extend(["}".into(), String::new()]);
    out
}
