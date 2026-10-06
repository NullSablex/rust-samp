//! `cargo xtask roundtrip`: set/get round trips over the generated open.mp
//! wrappers, for `examples/omp-showcase`.
//!
//! A wrapper can be wrong in ways no unit test sees: the right name on the
//! wrong slot, a `float` read as an `int`, a stack the callee cleans by the
//! wrong amount. A running server sees all of them. For every generated
//! setter with one argument whose getter returns the same type, this writes a
//! check that sets a value different from the current one, reads it back and
//! restores the original. It reads what `cargo xtask gen-omp` wrote.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::{Context, Result};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{Token, Type};

/// Types a round trip can compare, with an obvious "other value".
const COMPARABLE: [&str; 12] = [
    "i32", "u32", "i16", "u16", "i8", "u8", "i64", "u64", "f32", "bool", "Vector3", "Vector4",
];

/// Types a small-struct getter hands back as an `Option`: the setter's type,
/// and what the getter returns for it.
fn wrapped(setter: &str) -> Option<&'static str> {
    Some(match setter {
        "StringView" => "String",
        "Colour" => "Colour",
        "Milliseconds" => "Milliseconds",
        "Seconds" => "Seconds",
        "Minutes" => "Minutes",
        "Hours" => "Hours",
        _ => return None,
    })
}

/// A generated wrapper, as far as a round trip cares.
struct Wrapper {
    handle: String,
    params: Vec<Type>,
    returns: Option<Type>,
}

/// One entry of `virtual_fns!`: `pub fn name(this: Handle, args) -> Ret = [..] or ..;`.
struct Entry {
    name: String,
    handle: String,
    params: Vec<Type>,
    returns: Option<Type>,
}

impl Parse for Entry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        input.call(syn::Attribute::parse_outer)?;
        input.parse::<syn::Visibility>()?;
        input.parse::<Token![fn]>()?;
        let name: syn::Ident = input.parse()?;
        let content;
        syn::parenthesized!(content in input);
        let args: Punctuated<syn::PatType, Token![,]> = content.parse_terminated(
            |stream| {
                Ok(syn::PatType {
                    attrs: Vec::new(),
                    pat: Box::new(syn::Pat::Ident(syn::PatIdent {
                        attrs: Vec::new(),
                        by_ref: None,
                        mutability: None,
                        ident: stream.parse()?,
                        subpat: None,
                    })),
                    colon_token: stream.parse()?,
                    ty: stream.parse()?,
                })
            },
            Token![,],
        )?;
        let returns = match input.parse::<syn::ReturnType>()? {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => Some(*ty),
        };
        input.parse::<Token![=]>()?;
        // `[offset, slot]`: not needed here, but read through.
        let slot;
        syn::bracketed!(slot in input);
        slot.parse::<proc_macro2::TokenStream>()?;
        if input.peek(syn::Ident) {
            input.parse::<syn::Ident>()?; // `or`
            input.parse::<syn::Expr>()?;
        }
        input.parse::<Token![;]>()?;
        let mut types = args.into_iter().map(|arg| *arg.ty);
        let handle = types.next().map(|ty| type_name(&ty)).unwrap_or_default();
        Ok(Self {
            name: name.to_string(),
            handle,
            params: types.collect(),
            returns,
        })
    }
}

/// The last path segment's name: `Vector3` for `crate::omp::Vector3`.
fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// `T` for `Option<T>`.
fn option_of(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else { return None };
    let last = path.path.segments.last()?;
    if last.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    match args.args.first()? {
        syn::GenericArgument::Type(inner) => Some(type_name(inner)),
        _ => None,
    }
}

/// The wrappers the generated modules declare, and any `virtual_fns!` body
/// that could not be read — a failure, never a silently smaller result.
#[derive(Default)]
struct Collect {
    wrappers: BTreeMap<String, Wrapper>,
    errors: Vec<String>,
}

impl<'ast> Visit<'ast> for Collect {
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if !mac.path.is_ident("virtual_fns") {
            return;
        }
        let parser = |input: ParseStream<'_>| {
            let mut entries = Vec::new();
            while !input.is_empty() {
                entries.push(input.parse::<Entry>()?);
            }
            Ok(entries)
        };
        match mac.parse_body_with(parser) {
            Ok(entries) => {
                for entry in entries {
                    self.wrappers.insert(
                        entry.name,
                        Wrapper {
                            handle: entry.handle,
                            params: entry.params,
                            returns: entry.returns,
                        },
                    );
                }
            }
            Err(error) => self.errors.push(error.to_string()),
        }
    }

    /// A small-struct getter: `pub unsafe fn name(this: *mut Handle) -> Option<T>`.
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let signature = &item.sig;
        let (Some(syn::FnArg::Typed(this)), 1) = (signature.inputs.first(), signature.inputs.len())
        else {
            return;
        };
        let Type::Ptr(pointer) = &*this.ty else {
            return;
        };
        let syn::ReturnType::Type(_, returns) = &signature.output else {
            return;
        };
        if option_of(returns).is_some() {
            self.wrappers.insert(
                signature.ident.to_string(),
                Wrapper {
                    handle: type_name(&pointer.elem),
                    params: Vec::new(),
                    returns: Some((**returns).clone()),
                },
            );
        }
    }
}

/// A round trip: getter, setter, the type compared, and whether the setter
/// takes it by reference.
type Pair = (String, String, String, bool);

/// Setter and getter pairs that agree, by handle.
fn pairs(found: &BTreeMap<String, Wrapper>) -> BTreeMap<String, Vec<Pair>> {
    let mut out: BTreeMap<String, Vec<Pair>> = BTreeMap::new();
    for (setter, wrapper) in found {
        let Some((prefix, rest)) = setter.split_once("_set_") else {
            continue;
        };
        let [param] = wrapper.params.as_slice() else {
            continue;
        };
        // A `const T &` parameter is a Rust reference; the value compared is `T`.
        let (ty, by_ref) = match param {
            Type::Reference(reference) if reference.mutability.is_none() => {
                (type_name(&reference.elem), true)
            }
            other => (type_name(other), false),
        };
        let getter_name = format!("{prefix}_{rest}");
        let Some(getter) = found.get(&getter_name) else {
            continue;
        };
        if getter.handle != wrapper.handle || !getter.params.is_empty() {
            continue;
        }
        let Some(returns) = &getter.returns else {
            continue;
        };
        let direct = COMPARABLE.contains(&ty.as_str())
            && type_name(returns) == ty
            && option_of(returns).is_none();
        let through_option =
            wrapped(&ty).is_some_and(|inner| option_of(returns).as_deref() == Some(inner));
        if direct || through_option {
            out.entry(wrapper.handle.clone()).or_default().push((
                getter_name,
                setter.clone(),
                ty,
                by_ref,
            ));
        }
    }
    out
}

/// `IPlayerTextDraw` -> `player_text_draw`.
fn function_for(handle: &str) -> String {
    let name = handle.strip_prefix('I').unwrap_or(handle);
    let mut out = String::new();
    let mut previous: Option<char> = None;
    for c in name.chars() {
        if c.is_ascii_uppercase()
            && previous.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit())
        {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
        previous = Some(c);
    }
    format!("round_trips_{out}")
}

fn render(by_handle: &BTreeMap<String, Vec<Pair>>) -> String {
    let mut out = String::from(
        "//! Set/get round trips over the generated wrappers — generated by\n//! `cargo xtask roundtrip`. Do not edit.\n\nuse samp::omp::*;\n\nuse crate::report::{Other, Report};\n\n",
    );
    for (handle, pairs) in by_handle {
        let mut pairs = pairs.clone();
        pairs.sort();
        let _ = write!(
            out,
            "/// Every setter of `{handle}` that has a matching getter.\n///\n/// # Safety\n/// `handle` must be a live `{handle}`.\npub unsafe fn {}(handle: *mut {handle}, report: &mut Report) {{\n",
            function_for(handle)
        );
        for (getter, setter, ty, by_ref) in pairs {
            let amp = if by_ref { "&" } else { "" };
            let _ = writeln!(out, "    report.begin(\"{getter}\");");
            let body = if ty == "StringView" {
                // Text goes in as a view and comes back as an owned copy.
                format!(
                    "        let before = {getter}(handle).unwrap_or_default();\n        let want = before.other();\n        {setter}(handle, StringView::of(&want));\n        let got = {getter}(handle).unwrap_or_default();\n        report.round_trip(\"{getter}\", before.clone(), want, got);\n        {setter}(handle, StringView::of(&before));\n"
                )
            } else if wrapped(&ty).is_some() {
                format!(
                    "        let before = {getter}(handle).unwrap_or_default();\n        let want = before.other();\n        {setter}(handle, {amp}want);\n        report.round_trip(\"{getter}\", before, want, {getter}(handle).unwrap_or_default());\n        {setter}(handle, {amp}before);\n"
                )
            } else {
                format!(
                    "        let before: {ty} = {getter}(handle);\n        let want = before.other();\n        {setter}(handle, {amp}want);\n        report.round_trip(\"{getter}\", before, want, {getter}(handle));\n        {setter}(handle, {amp}before);\n"
                )
            };
            let _ = write!(out, "    unsafe {{\n{body}    }}\n");
        }
        out.push_str("}\n\n");
    }
    out
}

/// Writes `examples/omp-showcase/src/round_trips.rs`, or with `check`
/// compares it; `Ok(false)` when it is stale.
pub fn run(check: bool) -> Result<bool> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("the xtask crate has no parent")?;
    let generated = repo.join("crates/samp-sdk/src/omp/generated");
    let target = repo.join("examples/omp-showcase/src/round_trips.rs");

    let mut paths: Vec<_> = std::fs::read_dir(&generated)
        .with_context(|| format!("reading {}", generated.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    paths.sort();
    let mut collect = Collect::default();
    for path in paths {
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let file = syn::parse_file(&text).with_context(|| format!("parsing {}", path.display()))?;
        let before = collect.errors.len();
        collect.visit_file(&file);
        for error in &mut collect.errors[before..] {
            *error = format!("{}: {error}", path.display());
        }
    }
    if !collect.errors.is_empty() {
        anyhow::bail!(
            "could not read the generated wrappers:\n  {}",
            collect.errors.join("\n  ")
        );
    }
    let by_handle = pairs(&collect.wrappers);
    let text = crate::omp::output::rustfmt(&render(&by_handle))?;
    if check {
        let current = std::fs::read_to_string(&target).ok().as_deref() == Some(text.as_str());
        println!(
            "{}",
            if current {
                "round trips are current"
            } else {
                "round_trips.rs is stale"
            }
        );
        return Ok(current);
    }
    std::fs::write(&target, text).with_context(|| format!("writing {}", target.display()))?;
    let total: usize = by_handle.values().map(Vec::len).sum();
    println!(
        "wrote {}: {total} round trips over {} handles",
        target.display(),
        by_handle.len()
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn functions_are_named_after_the_handle() {
        assert_eq!(
            function_for("IPlayerTextDraw"),
            "round_trips_player_text_draw"
        );
        assert_eq!(function_for("INPC"), "round_trips_npc");
    }

    #[test]
    fn entries_parse_with_or_without_a_result() {
        let entries: Vec<Entry> = syn::parse::Parser::parse_str(
            |input: ParseStream<'_>| {
                let mut out = Vec::new();
                while !input.is_empty() {
                    out.push(input.parse::<Entry>()?);
                }
                Ok(out)
            },
            "/// doc\n#[must_use]\npub fn actor_skin(actor: IActor) -> i32 = [0, SLOT_GET_SKIN] or 0;\npub fn actor_set_skin(actor: IActor, id: i32) = [0, SLOT_SET_SKIN];",
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].handle, "IActor");
        assert_eq!(entries[1].params.len(), 1);
    }
}
