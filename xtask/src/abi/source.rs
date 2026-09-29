//! What the SDK's source claims: slot constants, and the slots and sizes the
//! generator wrote. Read with `syn`; `slots!` bodies are walked as tokens.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::visit::Visit;

/// A constant's value for each ABI.
#[derive(Clone, Copy, Default)]
struct PerAbi {
    itanium: Option<usize>,
    msvc: Option<usize>,
}

/// Every slot constant a source file declares.
pub struct Constants(HashMap<String, PerAbi>);

impl Constants {
    pub fn read(path: &Path) -> Result<Self> {
        let file = parse(path)?;
        let mut visitor = ConstVisitor::default();
        visitor.visit_file(&file);
        Ok(Self(visitor.found))
    }

    /// The constant's value for one ABI. An ungated constant answers for both.
    pub fn get(&self, name: &str, msvc: bool) -> Option<usize> {
        let value = self.0.get(name)?;
        if msvc { value.msvc } else { value.itanium }
    }
}

fn parse(path: &Path) -> Result<syn::File> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    syn::parse_file(&text).with_context(|| format!("parsing {}", path.display()))
}

#[derive(Default)]
struct ConstVisitor {
    found: HashMap<String, PerAbi>,
}

/// Which ABI a `#[cfg(...)]` selects: `Some(true)` for MSVC, `Some(false)`
/// for everything else, `None` for an attribute that says neither.
fn cfg_abi(attrs: &[syn::Attribute]) -> Option<bool> {
    attrs
        .iter()
        .filter(|a| a.path().is_ident("cfg"))
        .find_map(|attr| {
            let text = attr.meta.require_list().ok()?.tokens.to_string();
            if !text.contains("target_env = \"msvc\"") {
                return None;
            }
            Some(!text.trim_start().starts_with("not"))
        })
}

impl<'ast> Visit<'ast> for ConstVisitor {
    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Int(value),
            ..
        }) = &*item.expr
            && let Ok(value) = value.base10_parse::<usize>()
        {
            let entry = self.found.entry(item.ident.to_string()).or_default();
            match cfg_abi(&item.attrs) {
                Some(true) => entry.msvc = Some(value),
                Some(false) => entry.itanium = Some(value),
                None => {
                    entry.itanium.get_or_insert(value);
                    entry.msvc.get_or_insert(value);
                }
            }
        }
        syn::visit::visit_item_const(self, item);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac.path.is_ident("slots") {
            for entry in slot_entries(mac.tokens.clone()) {
                self.found.insert(
                    entry.name,
                    PerAbi {
                        itanium: Some(entry.itanium),
                        msvc: Some(entry.msvc),
                    },
                );
            }
        }
    }
}

/// One `slots!` line: `/// doc` `NAME: usize = itanium, msvc;`.
pub struct SlotEntry {
    pub doc: String,
    pub name: String,
    pub itanium: usize,
    pub msvc: usize,
}

/// The entries of a `slots!` body.
fn slot_entries(tokens: TokenStream) -> Vec<SlotEntry> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut entries = Vec::new();
    let mut doc = String::new();
    let mut i = 0;
    while i < trees.len() {
        // `/// text` arrives as `#` `[doc = " text"]`.
        if let (TokenTree::Punct(hash), Some(TokenTree::Group(group))) =
            (&trees[i], trees.get(i + 1))
            && hash.as_char() == '#'
            && group.delimiter() == Delimiter::Bracket
        {
            if let Ok(syn::Meta::NameValue(meta)) = syn::parse2::<syn::Meta>(group.stream())
                && let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(text),
                    ..
                }) = meta.value
            {
                doc = text.value();
            }
            i += 2;
            continue;
        }
        // Up to the `;`: the name is the ident before the `:`, the values the
        // two integers after the `=`.
        let end = trees[i..]
            .iter()
            .position(|t| matches!(t, TokenTree::Punct(p) if p.as_char() == ';'))
            .map_or(trees.len(), |p| i + p);
        let line = &trees[i..end];
        let name = line.windows(2).find_map(|w| match w {
            [TokenTree::Ident(name), TokenTree::Punct(colon)] if colon.as_char() == ':' => {
                Some(name.to_string())
            }
            _ => None,
        });
        let values: Vec<usize> = line
            .iter()
            .skip_while(|t| !matches!(t, TokenTree::Punct(p) if p.as_char() == '='))
            .filter_map(|t| match t {
                TokenTree::Literal(lit) => lit.to_string().parse().ok(),
                _ => None,
            })
            .collect();
        if let (Some(name), [itanium, msvc]) = (name, values.as_slice()) {
            entries.push(SlotEntry {
                doc: std::mem::take(&mut doc),
                name,
                itanium: *itanium,
                msvc: *msvc,
            });
        }
        i = end + 1;
    }
    entries
}

/// Every `slots!` entry a generated module wrote.
pub fn generated_slots(path: &Path) -> Result<Vec<SlotEntry>> {
    struct Slots(Vec<SlotEntry>);
    impl<'ast> Visit<'ast> for Slots {
        fn visit_macro(&mut self, mac: &'ast syn::Macro) {
            if mac.path.is_ident("slots") {
                self.0.extend(slot_entries(mac.tokens.clone()));
            }
        }
    }
    let mut slots = Slots(Vec::new());
    slots.visit_file(&parse(path)?);
    Ok(slots.0)
}

/// MSVC sizes of the mirrored structs, from the layout assertions the
/// generator wrote into `structs.rs`: `size_of::<Name>() == N` under
/// `target_env = "msvc"`.
pub fn mirrored_sizes(path: &Path) -> Result<HashMap<String, usize>> {
    #[derive(Default)]
    struct Sizes(HashMap<String, usize>);
    impl<'ast> Visit<'ast> for Sizes {
        fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
            if cfg_abi(&item.attrs) == Some(true) {
                let mut asserts = Asserts(&mut self.0);
                asserts.visit_expr(&item.expr);
            }
        }
    }
    struct Asserts<'a>(&'a mut HashMap<String, usize>);
    impl<'ast> Visit<'ast> for Asserts<'_> {
        fn visit_macro(&mut self, mac: &'ast syn::Macro) {
            let Ok(syn::Expr::Binary(check)) = mac.parse_body::<syn::Expr>() else {
                return;
            };
            let (
                syn::Expr::Call(call),
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(size),
                    ..
                }),
            ) = (&*check.left, &*check.right)
            else {
                return;
            };
            let syn::Expr::Path(function) = &*call.func else {
                return;
            };
            let Some(last) = function.path.segments.last() else {
                return;
            };
            if last.ident != "size_of" {
                return;
            }
            let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
                return;
            };
            if let (Some(syn::GenericArgument::Type(syn::Type::Path(ty))), Ok(size)) =
                (args.args.first(), size.base10_parse())
                && let Some(name) = ty.path.segments.last()
            {
                self.0.insert(name.ident.to_string(), size);
            }
        }
    }
    let mut sizes = Sizes::default();
    sizes.visit_file(&parse(path)?);
    Ok(sizes.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_slots_and_gated_constants() {
        let file: syn::File = syn::parse_str(
            r#"
            slots! {
                /// `void IActor::setSkin(int)`
                SLOT_SET_SKIN: usize = 22, 21;
                pub(crate) ENTITY_OFFSET: isize = 40, 56;
            }
            #[cfg(not(target_env = "msvc"))]
            const SLOT_KILL: usize = 11;
            #[cfg(target_env = "msvc")]
            const SLOT_KILL: usize = 10;
            const SLOT_GET_AMX: usize = 57;
            "#,
        )
        .unwrap();
        let mut visitor = ConstVisitor::default();
        visitor.visit_file(&file);
        let constants = Constants(visitor.found);
        assert_eq!(constants.get("SLOT_SET_SKIN", false), Some(22));
        assert_eq!(constants.get("SLOT_SET_SKIN", true), Some(21));
        assert_eq!(constants.get("ENTITY_OFFSET", true), Some(56));
        assert_eq!(constants.get("SLOT_KILL", false), Some(11));
        assert_eq!(constants.get("SLOT_KILL", true), Some(10));
        assert_eq!(constants.get("SLOT_GET_AMX", true), Some(57));

        let tokens: TokenStream = "/// `void IActor::setSkin(int)`\nSLOT_SET_SKIN: usize = 22, 21;"
            .parse()
            .unwrap();
        let entries = slot_entries(tokens);
        assert_eq!(entries[0].doc.trim(), "`void IActor::setSkin(int)`");
    }
}
