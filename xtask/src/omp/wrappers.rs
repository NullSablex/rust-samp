//! One module per interface: its slots, its wrappers, and what was left out.

use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{Context, Result};
use clang::Entity;

use super::ast::{self, Method, UidKind};
use super::generator::{Generator, Returns, Skip};
use super::mirror::snake;
use super::spec::Interface;
use super::vtables::{SlotTable, key_of};

/// Base classes whose methods are not the interface's own business:
/// extension plumbing, identity and pooling are wrapped once, by hand.
const NOT_OURS: [&str; 9] = [
    "IExtensible",
    "IExtension",
    "IIDProvider",
    "IEntity",
    "IUIDProvider",
    "IComponent",
    "IPoolComponent",
    "IPool",
    "IReadOnlyPool",
];

const RUST_KEYWORDS: [&str; 51] = [
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen",
];

pub fn is_keyword(name: &str) -> bool {
    RUST_KEYWORDS.contains(&name)
}

/// A Rust parameter name for a C++ one, clear of keywords and of the handle:
/// `attachToObject(IObject& object)` on an `object` handle gives `other_object`.
fn param_name(cpp: &str, this: &str) -> String {
    let name = snake(cpp);
    if name == this {
        format!("other_{name}")
    } else if is_keyword(&name) {
        format!("{name}_value")
    } else {
        name
    }
}

/// `getModel` -> `object_model`, `setModel` -> `object_set_model`: getters
/// drop `get`, as `player_health` does for `getHealth`.
fn rust_name(prefix: &str, method: &str) -> String {
    let stripped = method
        .strip_prefix("get")
        .filter(|rest| rest.starts_with(|c: char| c.is_ascii_uppercase()))
        .unwrap_or(method);
    format!("{prefix}_{}", snake(stripped))
}

/// One line when it fits in 100 columns, one parameter per line otherwise —
/// rustfmt does not reach inside a macro, so the generator lays it out.
fn wrap_params(head: &str, params: &[String], tail: &str) -> String {
    let indent = "    ";
    let one = format!("{head}({}){tail}", params.join(", "));
    if indent.len() + one.chars().count() <= 100 {
        return format!("{indent}{one}");
    }
    let mut out = format!("{indent}{head}(\n");
    for param in params {
        let _ = writeln!(out, "{indent}    {param},");
    }
    out.push_str(indent);
    out.push(')');
    out.push_str(tail);
    out
}

/// Where each ABI puts every method, and where `IComponent` sits in the
/// component interfaces.
pub struct Layouts {
    pub itanium: SlotTable,
    pub msvc: SlotTable,
    pub component_itanium: HashMap<String, usize>,
    pub component_msvc: HashMap<String, usize>,
}

/// A wrapper written into the module.
enum Wrapper {
    /// An entry inside `virtual_fns!`.
    Macro(String),
    /// A plain function, for a small struct returned through a hidden pointer.
    Plain(String),
}

impl<'tu> Generator<'tu> {
    /// The module for one interface.
    pub fn module(&mut self, entry: &Interface, layouts: &Layouts) -> Result<String> {
        let (leaf, handle) = (&entry.class, &entry.handle);
        let chain: Vec<Entity<'tu>> = ast::primary_chain(&self.records, leaf)
            .into_iter()
            .filter(|record| {
                record
                    .get_name()
                    .is_some_and(|n| !NOT_OURS.contains(&n.as_str()))
            })
            .collect();

        // Overloads anywhere in the chain need a name each, given in the spec.
        let mut counts: HashMap<String, usize> = HashMap::new();
        for record in &chain {
            for method in ast::pure_methods(*record) {
                *counts.entry(method.name).or_default() += 1;
            }
        }

        let mut slots = Vec::new();
        let mut wrappers = Vec::new();
        let mut notes = Vec::new();
        for record in chain.iter().rev() {
            let class = record.get_name().unwrap_or_default();
            for method in ast::pure_methods(*record) {
                let signature = signature(&class, &method);
                match self.wrapper(
                    entry,
                    layouts,
                    &chain,
                    &class,
                    &method,
                    &signature,
                    counts[&method.name],
                ) {
                    Ok((constant, slot_i, slot_m, wrapper)) => {
                        slots.push(format!("    /// `{signature}`"));
                        slots.push(format!("    {constant}: usize = {slot_i}, {slot_m};"));
                        wrappers.push(wrapper);
                    }
                    Err(Skip(why)) => notes.push(format!("// skipped: `{signature}` — {why}")),
                }
            }
        }

        let mut out = vec![
            format!("//! `{leaf}` — generated by `cargo xtask gen-omp` from `{}`.", entry.header),
            format!("//! Implemented by `{}` in the server's `{}`.", entry.implementor, entry.library),
            "//!".into(),
            "//! Do not edit: change `xtask/omp-wrappers.toml` or the generator and".into(),
            "//! regenerate. Slots are clang's layout of the header for each ABI, checked".into(),
            "//! against the official binaries by `cargo xtask check-abi --generated`.".into(),
            String::new(),
            "#![allow(unused_imports)]".into(),
            String::new(),
            "use crate::omp::types::{Colour, GTAQuat, Hours, Microseconds, Milliseconds, Minutes, Seconds, StringView, TimePoint, UID, Vector2, Vector3, Vector4, WorldTimePoint};".into(),
            "use crate::omp::containers::{FlatSet, HybridString, Pair, Span};".into(),
            "use crate::omp::vtable::{call_vtable_small_struct, opaque, slots, virtual_fns};".into(),
            "use crate::omp::*;".into(),
            String::new(),
        ];
        // The handle, when no other module declares it.
        if self.declared.insert(handle.clone()) {
            out.extend([
                "opaque! {".into(),
                format!("    /// Opaque handle for `{handle}*`."),
                format!("    pub {handle};"),
                "}".into(),
                String::new(),
            ]);
        }
        out.extend(self.uid_items(entry, layouts, &mut notes)?);
        if !slots.is_empty() {
            out.push("slots! {".into());
            out.extend(slots);
            out.extend(["}".into(), String::new()]);
        }
        let macros: Vec<&str> = wrappers
            .iter()
            .filter_map(|w| {
                if let Wrapper::Macro(body) = w {
                    Some(body.as_str())
                } else {
                    None
                }
            })
            .collect();
        if !macros.is_empty() {
            out.extend([
                "virtual_fns! {".into(),
                macros.join("\n\n"),
                "}".into(),
                String::new(),
            ]);
        }
        for wrapper in &wrappers {
            if let Wrapper::Plain(body) = wrapper {
                out.extend([body.clone(), String::new()]);
            }
        }
        if !notes.is_empty() {
            out.push("// What the generator left out, and why.".into());
            out.extend(notes);
            out.push(String::new());
        }
        Ok(out.join("\n"))
    }

    /// The wrapper for one method: its slot constant, both slots, the code.
    #[allow(clippy::too_many_arguments)]
    fn wrapper(
        &mut self,
        entry: &Interface,
        layouts: &Layouts,
        chain: &[Entity<'tu>],
        class: &str,
        method: &Method<'tu>,
        signature: &str,
        overloads: usize,
    ) -> Result<(String, usize, usize, Wrapper), Skip> {
        let (handle, prefix, this) = (&entry.handle, &entry.prefix, entry.this());
        let key = key_of(signature);
        let mut name = rust_name(prefix, &method.name);
        let mut constant = format!("SLOT_{}", snake(&method.name).to_uppercase());
        let hand_covers = entry.skip.contains(&method.name);
        if method.variadic {
            return Err(Skip::new(
                "variadic — a C-style `...` cannot go through a typed wrapper",
            ));
        }
        if hand_covers && !entry.overloads.contains_key(&key) {
            return Err(Skip::new(
                "covered by a hand-written wrapper under another name",
            ));
        }
        if overloads > 1 {
            // Named one by one in the spec; clang's layout places each
            // overload, MSVC's reversed order included.
            let Some(own) = entry.overloads.get(&key) else {
                return Err(Skip::new("overloaded"));
            };
            name = format!("{prefix}_{own}");
            constant = format!("SLOT_{}", own.to_uppercase());
        }
        if self.taken.contains(&name) {
            return Err(Skip::new(format!("`{name}` is written by hand")));
        }
        let slot_i = layouts
            .itanium
            .get(class)
            .and_then(|t| t.get(&key))
            .copied();
        let slot_m = layouts.msvc.get(class).and_then(|t| t.get(&key)).copied();
        let (Some(slot_i), Some(slot_m)) = (slot_i, slot_m) else {
            return Err(Skip::new("clang reported no slot"));
        };
        let params = method
            .params
            .iter()
            .map(|(cpp, ty)| Ok((param_name(cpp, this), self.param_type(*ty)?)))
            .collect::<Result<Vec<_>, Skip>>()?;
        let returns = self.returns(method.result, chain)?;

        let call = Call {
            name: &name,
            this,
            handle,
            constant: &constant,
            params: &params,
            signature,
        };
        let wrapper = match returns {
            Returns::Small(rust) => {
                let empty = if rust.to_string() == "StringView" {
                    "StringView::EMPTY".to_owned()
                } else {
                    self.default_of(&rust)
                };
                small_wrapper(&call, &rust.to_string(), &empty)
            }
            Returns::Plain { rust, neutral } => {
                macro_wrapper(&call, Some((&rust.to_string(), &neutral)))
            }
            Returns::Nothing => macro_wrapper(&call, None),
        };
        self.taken.insert(name);
        Ok((constant, slot_i, slot_m, wrapper))
    }

    /// The UID that finds the interface, and what comes with it: a component's
    /// `ComponentInterface` impl, or a per-player extension's accessor.
    fn uid_items(
        &mut self,
        entry: &Interface,
        layouts: &Layouts,
        notes: &mut Vec<String>,
    ) -> Result<Vec<String>> {
        let (leaf, handle, prefix) = (&entry.class, &entry.handle, &entry.prefix);
        let Some(record) = self.records.get(leaf) else {
            return Ok(Vec::new());
        };
        let Some(uid) = ast::uid_of(record) else {
            return Ok(Vec::new());
        };
        // Named after the prefix rather than the header's constant: the
        // headers are not consistent (`SomePlayerData_UID` for the vehicles'
        // player data), and the prefix rule reproduces the names the SDK
        // already had (`OBJECTS_COMPONENT_UID`), so an existing constant is
        // recognised instead of duplicated under a second name.
        let constant = match uid.kind {
            UidKind::Component => format!("{}_COMPONENT_UID", prefix.to_uppercase()),
            UidKind::Extension => format!("{}_UID", prefix.to_uppercase()),
        };
        let mut out = Vec::new();
        if self.consts.insert(constant.clone()) {
            out.extend([
                format!("/// `{}` in `{}`.", uid.constant, entry.header),
                format!("pub const {constant}: UID = {:#018x};", uid.value),
                String::new(),
            ]);
        }
        match uid.kind {
            UidKind::Component if self.consts.insert(format!("impl:{handle}")) => {
                let offset = |table: &HashMap<String, usize>| {
                    table
                        .get(leaf)
                        .copied()
                        .with_context(|| format!("no IComponent offset for {leaf}"))
                };
                let (itanium, msvc) = (
                    offset(&layouts.component_itanium)?,
                    offset(&layouts.component_msvc)?,
                );
                out.push(format!("impl ComponentInterface for {handle} {{"));
                out.push(format!("    const UID: UID = {constant};"));
                if itanium != 0 || msvc != 0 {
                    out.extend([
                        "    // `IComponent` is not this interface's first base.".into(),
                        "    #[cfg(not(target_env = \"msvc\"))]".into(),
                        format!("    const COMPONENT_OFFSET: isize = {itanium};"),
                        "    #[cfg(target_env = \"msvc\")]".into(),
                        format!("    const COMPONENT_OFFSET: isize = {msvc};"),
                    ]);
                }
                out.extend(["}".into(), String::new()]);
            }
            UidKind::Extension => {
                let first_base = ast::bases(record)
                    .first()
                    .and_then(clang::Entity::get_type)
                    .and_then(|t| t.get_canonical_type().get_declaration())
                    .and_then(|d| d.get_name());
                if first_base.as_deref() == Some("IExtension") && !self.taken.contains(prefix) {
                    out.extend([
                        format!("/// The player's `{leaf}`, or null when the component providing it is not loaded."),
                        "///".into(),
                        format!("/// Found by walking the player's extension map for `{constant}` — the"),
                        "/// virtual `getExtension` does not consult that map. `IExtension`".into(),
                        format!("/// is `{leaf}`'s first base, so the extension pointer is the interface pointer."),
                        "///".into(),
                        "/// # Safety".into(),
                        "/// `player` must be a live `IPlayer`.".into(),
                        "#[must_use]".into(),
                        format!("pub unsafe fn {prefix}(player: *mut IPlayer) -> *mut {handle} {{"),
                        format!("    unsafe {{ extension(player.cast::<u8>(), {constant}) }}.cast::<{handle}>()"),
                        "}".into(),
                        String::new(),
                    ]);
                    self.taken.insert(prefix.clone());
                } else {
                    notes.push(format!("// skipped: a `player` accessor — `IExtension` is not `{leaf}`'s first base"));
                }
            }
            UidKind::Component => {}
        }
        Ok(out)
    }
}

/// What a wrapper is made of.
struct Call<'a> {
    name: &'a str,
    /// The handle argument's name.
    this: &'a str,
    handle: &'a str,
    constant: &'a str,
    /// Rust name and type of each argument after the handle.
    params: &'a [(String, String)],
    signature: &'a str,
}

impl Call<'_> {
    /// The doc comment every wrapper carries, at `indent`.
    fn doc(&self, indent: &str) -> String {
        format!(
            "{indent}/// `{}`.\n{indent}///\n{indent}/// # Safety\n{indent}/// `{}` must be a live `{}`.",
            self.signature, self.this, self.handle
        )
    }

    /// `this: <handle type>` and the arguments, as a parameter list.
    fn args(&self, handle_type: &str) -> Vec<String> {
        std::iter::once(format!("{}: {handle_type}", self.this))
            .chain(self.params.iter().map(|(name, ty)| format!("{name}: {ty}")))
            .collect()
    }
}

/// A plain function for a small struct returned through MSVC's hidden
/// pointer; a `StringView` is copied out into a `String`.
fn small_wrapper(call: &Call<'_>, rust: &str, empty: &str) -> Wrapper {
    let is_view = rust == "StringView";
    let body_ret = if is_view {
        "Option<String>".to_owned()
    } else {
        format!("Option<{rust}>")
    };
    let extra = if call.params.is_empty() {
        String::new()
    } else {
        let types: Vec<&str> = call.params.iter().map(|(_, t)| t.as_str()).collect();
        let names: Vec<&str> = call.params.iter().map(|(n, _)| n.as_str()).collect();
        format!(", ({}) ({})", types.join(", "), names.join(", "))
    };
    let macro_call = format!(
        "call_vtable_small_struct!({}.cast::<u8>(), 0, {}, {rust}, {empty}{extra})",
        call.this, call.constant
    );
    // The macro carries its own `unsafe` blocks; only the copy out of the
    // server's memory needs one here.
    let body = if is_view {
        format!("    let view = {macro_call}?;\n    unsafe {{ view.to_owned_string() }}")
    } else {
        format!("    {macro_call}")
    };
    Wrapper::Plain(format!(
        "{}\n#[must_use]\npub unsafe fn {}({}) -> {body_ret} {{\n{body}\n}}",
        call.doc(""),
        call.name,
        call.args(&format!("*mut {}", call.handle)).join(", ")
    ))
}

/// An entry of `virtual_fns!`, with the result type and the neutral value a
/// null object answers with, if the method returns anything.
fn macro_wrapper(call: &Call<'_>, returns: Option<(&str, &str)>) -> Wrapper {
    let mut tail = String::new();
    if let Some((ret, _)) = returns {
        let _ = write!(tail, " -> {ret}");
    }
    let _ = write!(tail, " = [0, {}]", call.constant);
    if let Some((_, neutral)) = returns {
        let _ = write!(tail, " or {neutral}");
    }
    tail.push(';');
    let mut lines = vec![call.doc("    ")];
    if returns.is_some() {
        lines.push("    #[must_use]".into());
    }
    lines.push(wrap_params(
        &format!("pub fn {}", call.name),
        &call.args(call.handle),
        &tail,
    ));
    Wrapper::Macro(lines.join("\n"))
}

/// `void IPlayer::setTime(Hours, Minutes)`: the result and parameters as the
/// header spells them.
fn signature(class: &str, method: &Method<'_>) -> String {
    let mut shown: Vec<String> = method
        .params
        .iter()
        .map(|(_, ty)| ty.get_display_name())
        .collect();
    if method.variadic {
        shown.push("...".into());
    }
    format!(
        "{} {class}::{}({})",
        method.result.get_display_name(),
        method.name,
        shown.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn getters_drop_get() {
        assert_eq!(rust_name("object", "getModel"), "object_model");
        assert_eq!(rust_name("object", "setModel"), "object_set_model");
        assert_eq!(rust_name("player", "getaway"), "player_getaway");
    }

    #[test]
    fn parameters_avoid_keywords_and_the_handle() {
        assert_eq!(param_name("object", "object"), "other_object");
        assert_eq!(param_name("type", "pickup"), "type_value");
        assert_eq!(param_name("modelID", "pickup"), "model_id");
    }
}
