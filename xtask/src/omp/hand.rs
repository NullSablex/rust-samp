//! What the hand-written SDK already defines, so the generated code
//! complements it instead of shadowing it.
//!
//! Read with `syn`. Much of the SDK is written inside its own macros
//! (`virtual_fns!`, `opaque!`, `handler_vtable!`), whose bodies `syn` sees as
//! tokens; those are walked token by token, which leaves comments and doc
//! text out.

use std::collections::HashSet;
use std::path::Path;

use anyhow::{Context, Result};
use proc_macro2::{TokenStream, TokenTree};
use syn::visit::Visit;

#[derive(Default)]
pub struct Hand {
    /// Every function name.
    pub functions: HashSet<String>,
    /// `pub const` names, and `impl:X` for each `impl ComponentInterface for X`.
    pub consts: HashSet<String>,
    /// Interface handles declared with `opaque!`.
    pub handles: HashSet<String>,
    /// Every type: structs, opaque handles, handlers.
    pub types: HashSet<String>,
}

impl Hand {
    /// Reads every `.rs` file directly in `dir` — the generated modules live in
    /// a subdirectory and are not the SDK's own.
    pub fn scan(dir: &Path) -> Result<Self> {
        let mut hand = Hand::default();
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .with_context(|| format!("reading {}", dir.display()))?
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.extension().is_some_and(|e| e == "rs"))
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let file =
                syn::parse_file(&text).with_context(|| format!("parsing {}", path.display()))?;
            hand.visit_file(&file);
        }
        Ok(hand)
    }

    fn tokens(&mut self, macro_name: &str, tokens: TokenStream) {
        let trees: Vec<TokenTree> = flatten(tokens);
        for window in trees.windows(3) {
            match window {
                [TokenTree::Ident(keyword), TokenTree::Ident(name), _] if keyword == "fn" => {
                    self.functions.insert(name.to_string());
                }
                [
                    TokenTree::Ident(vtable),
                    TokenTree::Ident(keyword),
                    TokenTree::Ident(handler),
                ] if keyword == "for" && macro_name.ends_with("_vtable") => {
                    self.types.insert(vtable.to_string());
                    self.types.insert(handler.to_string());
                }
                [
                    TokenTree::Ident(keyword),
                    TokenTree::Ident(name),
                    TokenTree::Punct(semi),
                ] if keyword == "pub" && semi.as_char() == ';' && macro_name == "opaque" => {
                    let name = name.to_string();
                    if name.starts_with('I') {
                        self.handles.insert(name.clone());
                    }
                    self.types.insert(name);
                }
                _ => {}
            }
        }
        // A two-token tail: `fn name` at the very end of a stream.
        if let [.., TokenTree::Ident(keyword), TokenTree::Ident(name)] = trees.as_slice()
            && keyword == "fn"
        {
            self.functions.insert(name.to_string());
        }
    }
}

/// Every token, groups opened in place.
fn flatten(tokens: TokenStream) -> Vec<TokenTree> {
    let mut out = Vec::new();
    for tree in tokens {
        match tree {
            TokenTree::Group(group) => out.extend(flatten(group.stream())),
            other => out.push(other),
        }
    }
    out
}

impl<'ast> Visit<'ast> for Hand {
    fn visit_signature(&mut self, signature: &'ast syn::Signature) {
        self.functions.insert(signature.ident.to_string());
        syn::visit::visit_signature(self, signature);
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.consts.insert(item.ident.to_string());
        }
        syn::visit::visit_item_const(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if let (Some((_, path, _)), syn::Type::Path(target)) = (&item.trait_, &*item.self_ty) {
            let is_component = path
                .segments
                .last()
                .is_some_and(|s| s.ident == "ComponentInterface");
            if let (true, Some(name)) = (is_component, target.path.segments.last()) {
                self.consts.insert(format!("impl:{}", name.ident));
            }
        }
        syn::visit::visit_item_impl(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            self.types.insert(item.ident.to_string());
        }
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let name = mac
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        self.tokens(&name, mac.tokens.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(source: &str) -> Hand {
        let mut hand = Hand::default();
        hand.visit_file(&syn::parse_file(source).unwrap());
        hand
    }

    #[test]
    fn finds_what_the_macros_declare() {
        let hand = scan(
            r"
            opaque! {
                /// Opaque handle for `IPlayer*` — not a `fn fake` in a doc.
                pub IPlayer;
                pub PlayerBulletData;
            }
            virtual_fns! {
                pub fn player_score(player: IPlayer) -> i32 = [0, SLOT] or 0;
            }
            handler_vtable! {
                PlayerSpawnHandlerVTable for PlayerSpawnHandler { on_spawn: fn(*mut IPlayer) }
            }
            pub struct GangZonePos { pub min: i32 }
            pub const OBJECTS_COMPONENT_UID: u64 = 1;
            impl ComponentInterface for IObjectsComponent {}
            fn helper() {}
            ",
        );
        assert!(hand.functions.contains("player_score"));
        assert!(hand.functions.contains("helper"));
        assert!(!hand.functions.contains("fake"));
        assert!(hand.handles.contains("IPlayer"));
        assert!(!hand.handles.contains("PlayerBulletData"));
        assert!(hand.types.contains("PlayerBulletData"));
        assert!(hand.types.contains("PlayerSpawnHandler"));
        assert!(hand.types.contains("GangZonePos"));
        assert!(hand.consts.contains("OBJECTS_COMPONENT_UID"));
        assert!(hand.consts.contains("impl:IObjectsComponent"));
    }
}
