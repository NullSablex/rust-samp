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

use crate::NATIVE_PREFIX;
use crate::REG_PREFIX;

/// Args of `#[native(...)]`: `name = "..."` (Pawn name) and optional `raw`.
struct NativeName {
    pub name: String,
    pub raw: bool,
}

impl Parse for NativeName {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let mut name = String::new();
        let mut raw = false;

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
            } else {
                return Err(Error::new(
                    ident.span(),
                    "Unexpected argument name. Currently supports only \"name\" and \"raw\".",
                ));
            }

            let _: Option<Token![,]> = input.parse()?;
        }

        Ok(NativeName { name, raw })
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
            let amx_ident = samp::amx::AmxIdent::from(amx);

            let amx = match samp::amx::get(amx_ident) {
                Some(amx) => amx,
                None => {
                    samp::amx::add(amx);  // For GDK
                    samp::amx::get(amx_ident).expect("AMX not found after insertion")
                }
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

    let generated = quote! {
        #origin_fn
        #reg_native
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
        quote!(let mut plugin = samp::plugin::get::<Self>();)
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
        let user_return = match ::std::panic::catch_unwind(
            ::std::panic::AssertUnwindSafe(|| #call_origin)
        ) {
            Ok(v) => v,
            Err(panic) => {
                let msg = panic.downcast_ref::<&str>()
                    .copied()
                    .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                    .unwrap_or("(non-string payload)");
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
    quote! {
        #vis fn #reg_name() -> samp::raw::types::AMX_NATIVE_INFO {
            samp::raw::types::AMX_NATIVE_INFO {
                // Intentional leak: the native name must live forever
                // since the server holds a reference to the pointer.
                // `unwrap` here is unreachable — `#amx_name` was already validated
                // against null bytes at proc-macro time (see `NativeName::parse`).
                name: Box::leak(
                    std::ffi::CString::new(#amx_name)
                        .unwrap()
                        .into_boxed_c_str()
                ).as_ptr() as *mut std::os::raw::c_char,
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
