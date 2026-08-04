//! Implementation of the `#[event]` proc macro.
//!
//! An *event* is a Pawn callback (`OnPlayerConnect`, `OnPlayerSpawn`, …) the
//! plugin wants to observe. Unlike a native (which the gamemode calls into),
//! a callback runs in the gamemode's own AMX — the SDK receives it by
//! intercepting `amx_Exec` and dispatching into the registered handlers.
//!
//! For each marked method this macro generates:
//! - a **handler wrapper** `__samp_event_<fn>(amx, args)` that parses the
//!   callback arguments (via [`Args`], exactly like `#[native]`) and invokes
//!   the original method, returning its value as an AMX cell;
//! - a **registration function** `__samp_event_reg_<fn>()` that produces a
//!   [`samp::events::EventInfo`] (Pawn callback name + wrapper pointer)
//!   consumed by `initialize_plugin!(events: [...])`.
//!
//! The argument-marshalling half mirrors `#[native]` one-to-one; the difference
//! is purely the call source: a native receives a raw `*mut i32` param table,
//! whereas an event receives an [`Args`] the dispatcher already built from the
//! VM stack.
//!
//! [`Args`]: samp::args::Args

use proc_macro::TokenStream;
use quote::{quote, quote_spanned};

use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{
    Error, FnArg, Ident, ItemFn, LitStr, Pat, Result as SynResult, ReturnType, Token, Type,
    parse_macro_input,
};

use crate::{EVENT_PREFIX, EVENT_REG_PREFIX};

/// Args of `#[event(...)]`: `name = "..."` (the Pawn callback name) and an
/// optional `raw` flag (hand the handler the `Args` cursor unparsed).
struct EventName {
    name: String,
    raw: bool,
}

impl Parse for EventName {
    fn parse(input: ParseStream) -> SynResult<Self> {
        let mut name = String::new();
        let mut raw = false;

        while !input.is_empty() {
            let ident: Ident = input.parse()?;

            if ident == "name" {
                let _: Token![=] = input.parse()?;
                let callback_name: LitStr = input.parse()?;
                let value = callback_name.value();
                // The callback name becomes a `CString` at runtime for the
                // `amx_FindPublic` lookup — an interior NUL would panic there.
                // A compile error at the call site is far more useful.
                if value.contains('\0') {
                    return Err(Error::new(
                        callback_name.span(),
                        "event name cannot contain null bytes ('\\0')",
                    ));
                }
                name = value;
            } else if ident == "raw" {
                raw = true;
            } else {
                return Err(Error::new(
                    ident.span(),
                    "Unexpected argument name. `#[event]` supports only \"name\" and \"raw\".",
                ));
            }

            let _: Option<Token![,]> = input.parse()?;
        }

        if name.is_empty() {
            return Err(input.error("`#[event]` requires `name = \"OnSomething\"`"));
        }

        Ok(EventName { name, raw })
    }
}

/// Entry point of `#[event]`. Like `#[native]`, requires the function to be a
/// method (or associated function) of a struct that implements `SampPlugin`.
pub fn create_event(args: TokenStream, input: TokenStream) -> TokenStream {
    let event = parse_macro_input!(args as EventName);
    let origin_fn = parse_macro_input!(input as ItemFn);

    let vis = &origin_fn.vis;
    let origin_name = &origin_fn.sig.ident;
    let wrapper_name = prepend(origin_name, EVENT_PREFIX);
    let reg_name = prepend(origin_name, EVENT_REG_PREFIX);
    let callback_name = &event.name;

    // Accept both `fn(&mut self, amx: &Amx, ...)` and `fn(amx: &Amx, ...)`.
    let has_self = matches!(origin_fn.sig.inputs.first(), Some(FnArg::Receiver(_)));
    let skip_count = if has_self { 2 } else { 1 };

    let fn_input_idents = gen_fn_input_idents(&origin_fn, skip_count);
    let args_parsing = gen_args_parsing(&origin_fn, skip_count, event.raw, callback_name);
    let plugin_binding = gen_plugin_binding(has_self);
    let call_origin = gen_call_origin(origin_name, has_self, event.raw, &fn_input_idents);
    let invocation = gen_invocation(&origin_fn, &call_origin, callback_name);

    // The wrapper is a plain Rust fn (not `extern "C"`): the dispatcher calls it
    // directly with an `&Amx` and the `Args` it built from the VM stack.
    let wrapper = quote! {
        #vis fn #wrapper_name(
            amx: &samp::amx::Amx,
            args: &mut samp::args::Args,
        ) -> samp::events::EventReturn {
            #plugin_binding
            #args_parsing
            unsafe {
                #invocation
            }
        }
    };

    let reg = gen_reg_event(vis, &reg_name, &wrapper_name, callback_name);

    let generated = quote! {
        #origin_fn
        #reg
        #wrapper
    };

    generated.into()
}

/// For each "real" arg (after `self`/`amx`), the token used in the call:
/// `&ident` when the signature declares `&T`, `ident` for an owned `T`.
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
/// each callback argument. A parse failure means the callback signature does not
/// match what the gamemode actually pushed — logged and skipped.
fn gen_args_parsing(
    origin_fn: &ItemFn,
    skip_count: usize,
    raw: bool,
    callback_name: &str,
) -> proc_macro2::TokenStream {
    // Raw handlers receive the `Args` cursor directly and parse it themselves.
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
                                "[{}] failed to parse event argument #{} '{}' (expected type: {})",
                                #callback_name,
                                #idx,
                                stringify!(#ident),
                                stringify!(#ty),
                            );
                            return samp::events::EventReturn::Continue;
                        };
                })
            }
            FnArg::Receiver(_) => None,
        })
        .collect()
}

/// Only handlers with `self` need to reach the plugin via `samp::plugin::get`.
/// Associated functions call directly via `Self::name(...)`.
fn gen_plugin_binding(has_self: bool) -> proc_macro2::TokenStream {
    if has_self {
        quote!(let mut plugin = samp::plugin::get::<Self>();)
    } else {
        proc_macro2::TokenStream::new()
    }
}

/// Form of the call: `plugin.as_mut().method(amx, ...)` for methods,
/// `Self::function(amx, ...)` for associated functions. In `raw` mode the
/// handler receives the `Args` cursor directly instead of parsed arguments.
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

/// Converts the handler's return value into an [`EventReturn`] for the
/// dispatcher, in one of three modes:
/// - **suppression** — the handler returns `EventReturn` directly; forwarded
///   as-is so it can cancel the callback.
/// - **result observer** — the handler returns `AmxResult<T>` / `Result`; the
///   value is dropped (`Err` is logged) and the public runs (`Continue`).
/// - **value observer** — any other return type; dropped, public runs.
///
/// `catch_unwind` converts a panic in the handler body into a log + `Continue`.
/// The dispatcher is reached from the `amx_Exec` detour (an `extern "C"`
/// boundary), so a panic must never escape here. Mirrors `#[native]`.
fn gen_invocation(
    origin_fn: &ItemFn,
    call_origin: &proc_macro2::TokenStream,
    callback_name: &str,
) -> proc_macro2::TokenStream {
    let handle_user_return = if returns_event_return(&origin_fn.sig.output) {
        quote! {
            return user_return;
        }
    } else if returns_result(&origin_fn.sig.output) {
        quote! {
            if let Err(err) = user_return {
                samp::log::error!("[{}] {}", #callback_name, err);
            }
            return samp::events::EventReturn::Continue;
        }
    } else {
        quote! {
            let _ = user_return;
            return samp::events::EventReturn::Continue;
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
                samp::log::error!("[{}] panic in event handler: {}", #callback_name, msg);
                return samp::events::EventReturn::Continue;
            }
        };
        #handle_user_return
    }
}

/// `__samp_event_reg_*` function producing the [`EventInfo`] (callback name +
/// wrapper pointer). Consumed by `initialize_plugin!(events: [...])`.
///
/// [`EventInfo`]: samp::events::EventInfo
fn gen_reg_event(
    vis: &syn::Visibility,
    reg_name: &Ident,
    wrapper_name: &Ident,
    callback_name: &str,
) -> proc_macro2::TokenStream {
    quote! {
        #vis fn #reg_name() -> samp::events::EventInfo {
            samp::events::EventInfo {
                name: #callback_name,
                handler: Self::#wrapper_name,
            }
        }
    }
}

fn prepend(ident: &Ident, prefix: &str) -> Ident {
    Ident::new(&format!("{prefix}{ident}"), ident.span())
}

/// Syntactic check: does the return type end in `Result` or `AmxResult`?
/// Same convention as `#[native]` — decides whether to match `Ok`/`Err`.
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

/// Syntactic check: does the return type end in `EventReturn`? When it does the
/// handler opts into callback suppression and its value is forwarded verbatim to
/// the dispatcher instead of being treated as an observer result.
fn returns_event_return(output: &ReturnType) -> bool {
    let ReturnType::Type(_, ty) = output else {
        return false;
    };
    let Type::Path(tp) = &**ty else {
        return false;
    };
    tp.path
        .segments
        .last()
        .is_some_and(|last| last.ident == "EventReturn")
}
