use proc_macro::TokenStream;
use quote::quote;

use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Block, DeriveInput, Ident, Path, Result, Stmt, Token, bracketed, parse_macro_input};

use crate::REG_PREFIX;

// ---------------------------------------------------------------------------
// Formas do construtor
// ---------------------------------------------------------------------------

enum Constructor {
    /// `{ ... block ... }` — construtor explícito definido pelo usuário.
    Block(Vec<Stmt>),
    /// `type: TypePath` — usa `<T as Default>::default()`.
    Default(Path),
}

struct InitPlugin {
    natives_list: Option<Punctuated<Path, Token![,]>>,
    constructor: Constructor,
}

impl Parse for InitPlugin {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut natives_list = None;
        let mut default_type: Option<Path> = None;

        // Aceita `type: ...` e `natives: [...]` em qualquer ordem,
        // antes do bloco construtor opcional.
        loop {
            if input.peek(Token![type]) {
                let _: Token![type] = input.parse()?;
                let _: Token![:] = input.parse()?;
                default_type = Some(input.parse()?);
                let _: Option<Token![,]> = input.parse()?;
            } else if input.peek(Ident) {
                // Peek sem consumir para verificar se é "natives"
                let fork = input.fork();
                let ident: Ident = fork.parse()?;
                if ident == "natives" {
                    let _: Ident = input.parse()?; // consome do input real
                    let _: Token![:] = input.parse()?;
                    let content;
                    let _ = bracketed!(content in input);
                    natives_list = Some(Punctuated::parse_terminated(&content)?);
                    let _: Option<Token![,]> = input.parse()?;
                } else {
                    break; // ident desconhecido → início do bloco construtor
                }
            } else {
                break; // `{` ou fim do input
            }
        }

        let constructor = if let Some(ty) = default_type {
            Constructor::Default(ty)
        } else {
            Constructor::Block(input.call(Block::parse_within)?)
        };

        Ok(InitPlugin {
            natives_list,
            constructor,
        })
    }
}

// ---------------------------------------------------------------------------
// Geração de código
// ---------------------------------------------------------------------------

pub fn create_plugin(input: TokenStream) -> TokenStream {
    let plugin = parse_macro_input!(input as InitPlugin);

    let natives: proc_macro2::TokenStream = plugin
        .natives_list
        .iter()
        .flatten()
        .map(|path| {
            let mut path = path.clone();
            if let Some(last_part) = path.segments.last_mut() {
                let span = last_part.ident.span();
                last_part.ident = Ident::new(&format!("{}{}", REG_PREFIX, last_part.ident), span);
            }
            quote!(#path(),)
        })
        .collect();

    let supports_body = match &plugin.constructor {
        Constructor::Block(stmts) => quote! {
            let constructor = || { #(#stmts)* };
            samp::plugin::initialize(constructor);
        },
        Constructor::Default(ty) => quote! {
            samp::plugin::initialize(<#ty as Default>::default);
        },
    };

    let generated = quote! {
        #[unsafe(no_mangle)]
        pub extern "system" fn Load(server_data: *const usize) -> i32 {
            samp::interlayer::load(server_data);
            return 1;
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Unload() {
            samp::interlayer::unload();
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn AmxLoad(amx: *mut samp::raw::types::AMX) {
            let natives = vec![#natives];
            samp::interlayer::amx_load(amx, &natives);
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn AmxUnload(amx: *mut samp::raw::types::AMX) {
            samp::interlayer::amx_unload(amx);
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn Supports() -> u32 {
            #supports_body
            samp::interlayer::supports()
        }

        #[unsafe(no_mangle)]
        pub extern "system" fn ProcessTick() {
            samp::interlayer::process_tick();
        }
    };

    generated.into()
}

// ---------------------------------------------------------------------------
// Derive macro: #[derive(SampPlugin)]
// ---------------------------------------------------------------------------

/// Gera `impl samp::prelude::SampPlugin for T {}` com todos os métodos padrão.
pub fn derive_samp_plugin(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let generated = quote! {
        impl #impl_generics samp::prelude::SampPlugin for #name #ty_generics #where_clause {}
    };

    generated.into()
}
