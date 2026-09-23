//! Derivation of the Pawn declaration of a native from its Rust signature.
//!
//! `#[native(name = "Foo")] fn foo(&mut self, _amx: &Amx, text: &AmxString,
//! out: Ref<f32>) -> AmxResult<bool>` yields:
//!
//! ```text
//! native bool:Foo(const text[], &Float:out);
//! ```
//!
//! The mapping is syntactic — the macro sees the written type, not the resolved
//! one. A type it does not recognize becomes a plain cell argument, which is
//! what the AMX ABI passes anyway; only the Pawn tag is lost.

use syn::{FnArg, ItemFn, Pat, ReturnType, Type};

/// Pawn tag for a Rust type used as a by-value argument or as a return type.
fn tag_of(ty: &str) -> &'static str {
    match ty {
        "f32" => "Float:",
        "bool" => "bool:",
        _ => "",
    }
}

/// Last path segment of a type, without generics: `samp::cell::Ref<f32>` -> `Ref`.
fn type_name(ty: &Type) -> Option<String> {
    match ty {
        Type::Reference(r) => type_name(&r.elem),
        Type::Path(p) => p.path.segments.last().map(|s| s.ident.to_string()),
        _ => None,
    }
}

/// First generic argument of a type: `Ref<f32>` -> `f32`.
fn first_generic(ty: &Type) -> Option<String> {
    let Type::Path(p) = strip_ref(ty) else {
        return None;
    };
    let last = p.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    args.args.iter().find_map(|a| match a {
        syn::GenericArgument::Type(t) => type_name(t),
        _ => None,
    })
}

fn strip_ref(ty: &Type) -> &Type {
    match ty {
        Type::Reference(r) => strip_ref(&r.elem),
        other => other,
    }
}

/// Renders one argument as Pawn declares it.
fn arg_decl(ty: &Type, name: &str) -> String {
    match type_name(ty).as_deref() {
        // A string always reaches the native as a read-only cell array.
        Some("AmxString") => format!("const {name}[]"),
        // `Ref<T>` is Pawn's `&arg`: the script passes the address of a cell.
        Some("Ref") => {
            let tag = first_generic(ty).map_or("", |g| tag_of(&g));
            format!("&{tag}{name}")
        }
        // Buffers are arrays; the size convention is the plugin's own, so no
        // dimension is emitted.
        Some("Buffer" | "UnsizedBuffer") => format!("{name}[]"),
        Some(other) => format!("{}{name}", tag_of(other)),
        None => name.to_string(),
    }
}

/// Unwraps `AmxResult<T>` / `Result<T, E>` down to `T`.
fn unwrap_result(ty: &Type) -> Option<String> {
    match type_name(ty).as_deref() {
        Some("AmxResult" | "Result") => first_generic(ty),
        other => other.map(str::to_string),
    }
}

/// Builds the `native ...;` line for a function marked with `#[native]`.
///
/// `raw` natives take the argument list untyped, so their arity cannot be
/// derived; they come back commented out, for the author to fill in.
pub fn native_decl(origin_fn: &ItemFn, amx_name: &str, raw: bool, skip_count: usize) -> String {
    let ret_tag = match &origin_fn.sig.output {
        ReturnType::Default => "",
        ReturnType::Type(_, ty) => unwrap_result(ty).map_or("", |t| tag_of(&t)),
    };

    if raw {
        return format!(
            "// native {ret_tag}{amx_name}(...); // raw native — fill in the arguments"
        );
    }

    let args: Vec<String> = origin_fn
        .sig
        .inputs
        .iter()
        .skip(skip_count)
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => {
                let Pat::Ident(pat_ident) = &*pat_type.pat else {
                    return None;
                };
                let name = pat_ident.ident.to_string();
                let name = name.trim_start_matches('_');
                Some(arg_decl(&pat_type.ty, name))
            }
            FnArg::Receiver(_) => None,
        })
        .collect();

    format!("native {ret_tag}{amx_name}({});", args.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    fn decl(f: ItemFn, name: &str) -> String {
        native_decl(&f, name, false, 2)
    }

    #[test]
    fn primitives_and_tags() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, count: i32, speed: f32, on: bool) -> AmxResult<i32> { }
        };
        assert_eq!(decl(f, "Foo"), "native Foo(count, Float:speed, bool:on);");
    }

    #[test]
    fn string_is_a_const_array() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, text: &AmxString) -> AmxResult<i32> { }
        };
        assert_eq!(decl(f, "Say"), "native Say(const text[]);");
    }

    #[test]
    fn ref_becomes_by_reference_and_keeps_the_tag() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, out: Ref<f32>, n: Ref<i32>) -> AmxResult<bool> { }
        };
        assert_eq!(decl(f, "Get"), "native bool:Get(&Float:out, &n);");
    }

    #[test]
    fn buffers_are_arrays() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, dest: UnsizedBuffer, size: usize) -> AmxResult<i32> { }
        };
        assert_eq!(decl(f, "Read"), "native Read(dest[], size);");
    }

    #[test]
    fn leading_underscore_is_dropped_from_the_argument_name() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, _unused: i32) -> AmxResult<i32> { }
        };
        assert_eq!(decl(f, "Foo"), "native Foo(unused);");
    }

    #[test]
    fn associated_function_without_self() {
        let f: ItemFn = parse_quote! {
            fn f(_amx: &Amx, id: i32) -> AmxResult<i32> { }
        };
        assert_eq!(native_decl(&f, "Ping", false, 1), "native Ping(id);");
    }

    #[test]
    fn plain_return_type_without_result() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, a: i32) -> bool { }
        };
        assert_eq!(decl(f, "Is"), "native bool:Is(a);");
    }

    #[test]
    fn fully_qualified_paths_resolve_by_last_segment() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, s: &samp::cell::AmxString, r: samp::cell::Ref<f32>) -> AmxResult<i32> { }
        };
        assert_eq!(decl(f, "Q"), "native Q(const s[], &Float:r);");
    }

    #[test]
    fn raw_native_comes_back_commented_out() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, args: Args) -> AmxResult<i32> { }
        };
        assert!(native_decl(&f, "Raw", true, 2).starts_with("// native Raw(...);"));
    }

    #[test]
    fn unknown_type_falls_back_to_a_plain_cell() {
        let f: ItemFn = parse_quote! {
            fn f(&mut self, _amx: &Amx, thing: MyCustomCell) -> AmxResult<i32> { }
        };
        assert_eq!(decl(f, "Foo"), "native Foo(thing);");
    }
}
