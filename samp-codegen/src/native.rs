//! Implementation of the `#[native]` proc macro.
//!
//! For each marked method, generates:
//! - **`extern "C"` wrapper function** with prefix `__samp_native_` that parses
//!   arguments (via `samp::args::Args`), calls the original method and converts
//!   the return value into an AMX cell.
//! - **Registration function** with prefix `__samp_reg_` that produces an
//!   `AMX_NATIVE_INFO` (name as a C-string + wrapper pointer) consumed by
//!   `initialize_plugin!`.
//!
//! `raw` mode skips parsing and hands `Args` directly to the method — useful for
//! variadic natives or those that need to validate arguments manually.

use proc_macro::TokenStream;
use quote::{quote, quote_spanned};

use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    Error, FnArg, Ident, ItemFn, LitStr, Pat, Result as SynResult, ReturnType, Token, Type,
    parse_macro_input,
};

use crate::DOC_PREFIX;
use crate::INC_PREFIX;
use crate::NATIVE_PREFIX;
use crate::REG_PREFIX;
use crate::pawn_decl::{Shape, argument_names, native_decl};

/// The text of a function's `///` doc comment, lines joined by newlines.
///
/// Empty when undocumented. Leading spaces are dropped, since `///` leaves one.
fn doc_comment(origin_fn: &ItemFn) -> String {
    let mut lines: Vec<String> = Vec::new();

    for attr in &origin_fn.attrs {
        if !attr.path().is_ident("doc") {
            continue;
        }
        if let syn::Meta::NameValue(nv) = &attr.meta
            && let syn::Expr::Lit(lit) = &nv.value
            && let syn::Lit::Str(text) = &lit.lit
        {
            lines.push(
                text.value()
                    .strip_prefix(' ')
                    .unwrap_or(&text.value())
                    .to_string(),
            );
        }
    }

    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

/// One `arg = value` inside `default(...)` or `sizeof(...)`.
///
/// The value is written as Pawn will read it: a literal goes through as it is,
/// and a string literal is taken verbatim, which is how a Pawn string default
/// (`greeting = ""`) or an expression is expressed.
fn parse_pair(input: ParseStream) -> SynResult<(String, String)> {
    let arg: Ident = input.parse()?;
    let _: Token![=] = input.parse()?;

    let value = if input.peek(LitStr) {
        input.parse::<LitStr>()?.value()
    } else if input.peek(syn::Lit) {
        let lit: syn::Lit = input.parse()?;
        match lit {
            syn::Lit::Int(v) => v.base10_digits().to_string(),
            syn::Lit::Float(v) => v.base10_digits().to_string(),
            syn::Lit::Bool(v) => v.value.to_string(),
            other => {
                return Err(Error::new(
                    other.span(),
                    "expected a number, a bool or a string with the Pawn text",
                ));
            }
        }
    } else {
        // An identifier, which is what `sizeof(len = dest)` uses.
        input.parse::<Ident>()?.to_string()
    };

    Ok((arg.to_string(), value))
}

/// Args of `#[native(...)]`: `name = "..."` (Pawn name) and optional `raw`.
struct NativeName {
    pub name: String,
    pub raw: bool,
    /// What the Rust signature cannot say about the Pawn declaration.
    pub shape: Shape,
}

impl Parse for NativeName {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let mut name = String::new();
        let mut raw = false;
        let mut shape = Shape::default();

        while !input.is_empty() {
            let ident: Ident = input.parse()?;

            if ident == "name" {
                let _: Token![=] = input.parse()?;
                let native_name: LitStr = input.parse()?;
                let value = native_name.value();
                // Validate at proc-macro time: the native name becomes a `CString`
                // at runtime — an internal NUL byte would make `CString::new` panic.
                // Emitting a compile error here is more useful.
                if value.contains('\0') {
                    return Err(Error::new(
                        native_name.span(),
                        "native name cannot contain null bytes ('\\0')",
                    ));
                }
                name = value;
            } else if ident == "raw" {
                raw = true;
            } else if ident == "varargs" {
                shape.varargs = true;
            } else if ident == "args" {
                let _: Token![=] = input.parse()?;
                let list: LitStr = input.parse()?;
                shape.arguments = Some(list.value());
            } else if ident == "default" || ident == "sizeof" {
                // `default(account = 0, greeting = "\"\"")`, `sizeof(len = dest)`
                let body;
                syn::parenthesized!(body in input);
                let pairs = body.parse_terminated(parse_pair, Token![,])?;
                for (arg, value) in pairs {
                    if ident == "default" {
                        shape.defaults.push((arg, value));
                    } else {
                        shape.sizeofs.push((arg, value));
                    }
                }
            } else {
                return Err(Error::new(
                    ident.span(),
                    "Unexpected argument name. Currently supports only \"name\" and \"raw\".",
                ));
            }

            let _: Option<Token![,]> = input.parse()?;
        }

        Ok(NativeName { name, raw, shape })
    }
}

/// Entry point of `#[native]`. Currently requires the function to be a method
/// of a struct that implements `SampPlugin` — usage on free functions is not
/// supported.
pub fn create_native(args: TokenStream, input: TokenStream) -> TokenStream {
    let native = parse_macro_input!(args as NativeName);
    let origin_fn = parse_macro_input!(input as ItemFn);

    let vis = &origin_fn.vis;
    let origin_name = &origin_fn.sig.ident;
    let native_name = prepend(&origin_fn.sig.ident, NATIVE_PREFIX);
    let reg_name = prepend(&origin_fn.sig.ident, REG_PREFIX);
    let inc_name = prepend(&origin_fn.sig.ident, INC_PREFIX);
    let doc_name = prepend(&origin_fn.sig.ident, DOC_PREFIX);
    let amx_name = &native.name;

    // `#[native]` accepts both methods (`fn foo(&mut self, _amx: &Amx, ...)`)
    // and associated functions (`fn foo(_amx: &Amx, ...)`) — stateless natives
    // look cleaner without the ceremonial `self`.
    let has_self = matches!(origin_fn.sig.inputs.first(), Some(FnArg::Receiver(_)));
    let skip_count = if has_self { 2 } else { 1 };

    let fn_input_idents = gen_fn_input_idents(&origin_fn, skip_count);
    let args_parsing = gen_args_parsing(&origin_fn, skip_count, native.raw, amx_name);
    let plugin_binding = gen_plugin_binding(has_self);
    let call_origin = gen_call_origin(origin_name, has_self, native.raw, &fn_input_idents);
    let invocation = gen_invocation(&origin_fn, &call_origin, amx_name);

    let native_generated = quote! {
        #vis extern "C" fn #native_name(amx: *mut samp::raw::types::AMX, args: *mut i32) -> i32 {
            // Resolved without panicking: this runs before `catch_unwind`, where
            // a panic would cross into the server and end its process.
            let Some(amx) = samp::interlayer::native_amx(amx) else {
                return 0;
            };

            let mut args = samp::args::Args::new(amx, args);
            #plugin_binding

            #args_parsing

            unsafe {
                #invocation
            }
        }
    };

    let reg_native = gen_reg_native(vis, &reg_name, &native_name, amx_name);
    // A `default(...)` or `sizeof(...)` naming an argument the function does not
    // have is a rename the attribute did not follow: the include would come out
    // wrong, so it is a compile error rather than a surprise at load.
    let unknown = native
        .shape
        .unknown_arguments(&argument_names(&origin_fn, skip_count));
    if let Some(arg) = unknown.first() {
        return Error::new(
            origin_fn.sig.ident.span(),
            format!("`{arg}` is not an argument of this native"),
        )
        .to_compile_error()
        .into();
    }

    let decl = native_decl(&origin_fn, amx_name, native.raw, skip_count, &native.shape);
    let inc_decl = quote! {
        #[doc(hidden)]
        #vis fn #inc_name() -> &'static str {
            #decl
        }
    };

    // The native's own doc comment, so the include can carry the documentation
    // the plugin author already wrote instead of a second copy in the template.
    let doc_text = doc_comment(&origin_fn);
    let doc_fn = quote! {
        #[doc(hidden)]
        #vis fn #doc_name() -> &'static str {
            #doc_text
        }
    };

    let generated = quote! {
        #origin_fn
        #reg_native
        #inc_decl
        #doc_fn
        #native_generated
    };

    generated.into()
}

/// For each "real" function arg (after `self`/`amx`), generates the token to use
/// in the call: `&ident` if the signature declares `&T`, `ident` if it declares an owned `T`.
fn gen_fn_input_idents(origin_fn: &ItemFn, skip_count: usize) -> Vec<proc_macro2::TokenStream> {
    origin_fn
        .sig
        .inputs
        .iter()
        .skip(skip_count)
        .filter_map(|arg| match arg {
            FnArg::Typed(pat_type) => {
                let Pat::Ident(pat_ident) = &*pat_type.pat else {
                    return None;
                };
                let ident = &pat_ident.ident;
                let by_ref = matches!(&*pat_type.ty, Type::Reference(_));
                Some(if by_ref {
                    quote_spanned!(pat_type.span() => &#ident)
                } else {
                    quote_spanned!(pat_type.span() => #ident)
                })
            }
            FnArg::Receiver(_) => None,
        })
        .collect()
}

/// Generates the `let Some(arg) = args.next_arg() else { log; return 0; };` for
/// each "real" arg. `raw` mode skips this (the native receives `Args` directly).
fn gen_args_parsing(
    origin_fn: &ItemFn,
    skip_count: usize,
    raw: bool,
    amx_name: &str,
) -> proc_macro2::TokenStream {
    if raw {
        return proc_macro2::TokenStream::new();
    }
    origin_fn
        .sig
        .inputs
        .iter()
        .skip(skip_count)
        .enumerate()
        .filter_map(|(idx, arg)| match arg {
            FnArg::Typed(pat_type) => {
                let Pat::Ident(pat_ident) = &*pat_type.pat else {
                    return None;
                };
                let ident = &pat_ident.ident;
                let ty = &pat_type.ty;
                Some(quote_spanned! {
                    pat_type.span() =>
                        let Some(#ident) = args.next_arg() else {
                            samp::log::error!(
                                "[{}] failed to parse argument #{} '{}' (expected type: {})",
                                #amx_name,
                                #idx,
                                stringify!(#ident),
                                stringify!(#ty),
                            );
                            return 0;
                        };
                })
            }
            FnArg::Receiver(_) => None,
        })
        .collect()
}

/// Only natives with `self` need to access the plugin via `samp::plugin::get`.
/// Associated functions call directly via `Self::name(...)`.
fn gen_plugin_binding(has_self: bool) -> proc_macro2::TokenStream {
    if has_self {
        quote! {
            // Marks the frame so the SDK can tell that `&mut self` is out: a
            // main-thread job asking for the plugin underneath this call is
            // refused instead of aliasing it.
            let _plugin_frame = samp::plugin::NativeFrame::enter();
            let Some(mut plugin) = samp::plugin::try_get::<Self>() else {
                return 0;
            };
        }
    } else {
        proc_macro2::TokenStream::new()
    }
}

/// Form of the call to the native: `plugin.as_mut().method(...)` for methods,
/// `Self::function(...)` for associated functions. `raw` mode passes `args`
/// directly; normal mode passes each parsed arg.
fn gen_call_origin(
    origin_name: &Ident,
    has_self: bool,
    raw: bool,
    fn_input_idents: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    if raw {
        if has_self {
            quote!(plugin.as_mut().#origin_name(amx, args))
        } else {
            quote!(Self::#origin_name(amx, args))
        }
    } else if has_self {
        quote!(plugin.as_mut().#origin_name(amx, #(#fn_input_idents),*))
    } else {
        quote!(Self::#origin_name(amx, #(#fn_input_idents),*))
    }
}

/// `catch_unwind` converts a panic from the native body into a log + return 0.
/// Without it, a panic crossing the `extern "C"` boundary aborts the entire
/// process (the whole server dies) — behavior guaranteed by Rust since 1.71+.
/// `AssertUnwindSafe` is required because `&mut Plugin` is not `UnwindSafe`
/// by default; it is safe here because we do not touch the plugin after the panic.
fn gen_invocation(
    origin_fn: &ItemFn,
    call_origin: &proc_macro2::TokenStream,
    amx_name: &str,
) -> proc_macro2::TokenStream {
    let handle_user_return = if returns_result(&origin_fn.sig.output) {
        quote! {
            match user_return {
                Ok(retval) => {
                    return samp::plugin::convert_return_value(retval);
                },

                Err(err) => {
                    samp::log::error!("[{}] {}", #amx_name, err);
                    return 0;
                }
            }
        }
    } else {
        quote! {
            return samp::plugin::convert_return_value(user_return);
        }
    };

    quote! {
        let user_return = match samp::panic_guard::catch(|| #call_origin) {
            Ok(v) => v,
            Err(msg) => {
                samp::log::error!("[{}] panic in native: {}", #amx_name, msg);
                return 0;
            }
        };
        #handle_user_return
    }
}

/// `__samp_reg_*` function that produces the `AMX_NATIVE_INFO` (name as a C-string
/// + wrapper pointer). Consumed by `initialize_plugin!`.
fn gen_reg_native(
    vis: &syn::Visibility,
    reg_name: &Ident,
    native_name: &Ident,
    amx_name: &str,
) -> proc_macro2::TokenStream {
    // `NativeName::parse` already rejected a name with a NUL byte, so this
    // conversion cannot fail; the fallback only keeps the macro panic-free.
    let c_name = std::ffi::CString::new(amx_name)
        .map(|name| proc_macro2::Literal::c_string(&name))
        .unwrap_or_else(|_| proc_macro2::Literal::c_string(c""));

    quote! {
        #vis fn #reg_name() -> samp::raw::types::AMX_NATIVE_INFO {
            samp::raw::types::AMX_NATIVE_INFO {
                // A C string literal: `'static`, in the binary, with nothing to
                // allocate, leak or unwrap. The server keeps the pointer for as
                // long as the plugin is loaded, which a literal outlives.
                name: #c_name.as_ptr() as *mut std::os::raw::c_char,
                func: Self::#native_name,
            }
        }
    }
}

fn prepend(ident: &Ident, prefix: &str) -> Ident {
    Ident::new(&format!("{prefix}{ident}"), ident.span())
}

/// Syntactic check: does the return type end in `Result` or `AmxResult`?
/// Used to decide whether the FFI wrapper should match `Ok`/`Err` or call
/// the native directly. Cannot resolve type aliases other than these two
/// conventional names — users always write one of them by convention.
fn returns_result(output: &ReturnType) -> bool {
    let ReturnType::Type(_, ty) = output else {
        return false;
    };
    let Type::Path(tp) = &**ty else {
        return false;
    };
    let Some(last) = tp.path.segments.last() else {
        return false;
    };
    last.ident == "Result" || last.ident == "AmxResult"
}
