//! Implementation of the `initialize_plugin!` and `#[derive(SampPlugin)]` proc macros.
//!
//! `initialize_plugin!` generates quite a bit of code: the `Supports`/`Load`/
//! `Unload`/`AmxLoad`/`AmxUnload`/`ProcessTick` exports required by SA-MP, plus
//! the `ComponentEntryPoint` + Rust `IComponent` vtable for native Open Multiplayer.
//! The Open Multiplayer block is included only when the `samp-only` feature is disabled.
//!
//! Also includes automatic metadata resolution (UID via FNV-1a of
//! `name@version`, name/version read from `Cargo.toml` with fallback to
//! `CARGO_PKG_*`) — the macro tries `[package.metadata.samp]` first, and if
//! not found, writes the generated UID there to pin it across builds.

use proc_macro::TokenStream;
use quote::quote;

use proc_macro2::Literal;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Block, DeriveInput, Expr, Ident, LitInt, LitStr, Path, Result, Stmt, Token, bracketed,
    parse_macro_input,
};

use crate::REG_PREFIX;

// ---------------------------------------------------------------------------
// Helpers for automatic Open Multiplayer metadata resolution
// ---------------------------------------------------------------------------

/// Fields read from `[package.metadata.samp]` in `Cargo.toml`.
#[derive(Default)]
struct SampMetadata {
    uid: Option<u64>,
    name: Option<String>,
    version: Option<(u8, u8, u8)>,
}

/// 64-bit FNV-1a — deterministic UID generation from crate metadata.
fn fnv1a_64(data: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Converts a hexadecimal (`"0x..."`) or decimal string into a `u64`.
/// Accepts underscores as separators (Rust literal style: `0x12_3abc`).
fn parse_uid_str(s: &str) -> Option<u64> {
    let cleaned: String = s.chars().filter(|c| *c != '_').collect();
    if let Some(hex) = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else {
        cleaned.parse::<u64>().ok()
    }
}

/// Converts a "major.minor.patch" string into `(u8, u8, u8)`.
fn parse_version_str(s: &str) -> Option<(u8, u8, u8)> {
    let mut parts = s.splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts
        .next()
        .map(|s| s.trim_end_matches(|c: char| !c.is_ascii_digit()))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Some((major, minor, patch))
}

/// Reads `[package.metadata.samp]` from the contents of a `Cargo.toml`.
fn read_samp_metadata_from_content(content: &str) -> SampMetadata {
    let mut meta = SampMetadata::default();
    let mut in_section = false;

    for line in content.lines() {
        let line = line.trim();
        if line == "[package.metadata.samp]" {
            in_section = true;
            continue;
        }
        if in_section {
            if line.starts_with('[') {
                break;
            }
            if let Some((key, val)) = line.split_once('=') {
                let key = key.trim();
                let val = val.trim().trim_matches('"').trim_matches('\'');
                match key {
                    "uid" => meta.uid = parse_uid_str(val),
                    "name" => meta.name = Some(val.to_owned()),
                    "version" => meta.version = parse_version_str(val),
                    _ => {}
                }
            }
        }
    }
    meta
}

/// Reads `[package.metadata.samp]` from the `Cargo.toml` of the crate being compiled.
fn read_samp_metadata() -> SampMetadata {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return SampMetadata::default();
    };
    let toml_path = std::path::Path::new(&manifest_dir).join("Cargo.toml");
    match std::fs::read_to_string(toml_path) {
        Ok(content) => read_samp_metadata_from_content(&content),
        Err(_) => SampMetadata::default(),
    }
}

/// Resolves the UID following the priority chain:
/// 1. `[package.metadata.samp] uid` in `Cargo.toml`
/// 2. FNV-1a of `CARGO_PKG_NAME@CARGO_PKG_VERSION` — generated and written to `Cargo.toml`
///    whenever it is not defined; from then on the fixed value is reused.
///
/// Should only be called when `samp-only` is not active.
fn resolve_uid(meta: &SampMetadata) -> u64 {
    if let Some(uid) = meta.uid {
        return uid;
    }
    let name = std::env::var("CARGO_PKG_NAME").unwrap_or_default();
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let uid = fnv1a_64(format!("{name}@{version}").as_bytes());
    persist_uid_in_cargo_toml(uid);
    uid
}

/// Writes `[package.metadata.samp] uid` to `Cargo.toml` if the field does not yet exist.
/// I/O failures are ignored — the derived UID continues to be used normally.
fn persist_uid_in_cargo_toml(uid: u64) {
    let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let toml_path = std::path::Path::new(&manifest_dir).join("Cargo.toml");
    let Ok(content) = std::fs::read_to_string(&toml_path) else {
        return;
    };
    if read_samp_metadata_from_content(&content).uid.is_some() {
        return;
    }
    let new_content = format!(
        "{}\n\n[package.metadata.samp]\nuid = \"{uid:#018x}\"\n",
        content.trim_end()
    );
    let _ = std::fs::write(&toml_path, new_content);
}

// ---------------------------------------------------------------------------
// Constructor forms
// ---------------------------------------------------------------------------

enum Constructor {
    /// `{ ... block ... }` — explicit constructor defined by the user.
    Block(Vec<Stmt>),
    /// `type: TypePath` — uses `<T as Default>::default()`.
    Default(Path),
}

/// Optional fields declared in the macro to override the automatic values.
/// All Open Multiplayer fields are optional — native mode is generated by default
/// regardless of the presence or absence of these fields.
struct InitPlugin {
    natives_list: Option<Punctuated<Path, Token![,]>>,
    constructor: Constructor,
    /// Explicit UID in the macro (`uid: 0x...`). Overrides Cargo.toml and the automatic fallback.
    explicit_uid: Option<Expr>,
    /// Explicit name in the macro (`component_name: "..."`). Overrides `Cargo.toml` and `CARGO_PKG_NAME`.
    explicit_name: Option<LitStr>,
    /// Explicit version in the macro (`component_version: (1,0,0)`). Overrides `Cargo.toml` and `CARGO_PKG_VERSION`.
    explicit_version: Option<(u8, u8, u8)>,
}

impl Parse for InitPlugin {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut natives_list = None;
        let mut default_type: Option<Path> = None;
        let mut explicit_uid: Option<Expr> = None;
        let mut explicit_name: Option<LitStr> = None;
        let mut explicit_version: Option<(u8, u8, u8)> = None;

        loop {
            if input.peek(Token![type]) {
                let _: Token![type] = input.parse()?;
                let _: Token![:] = input.parse()?;
                default_type = Some(input.parse()?);
                let _: Option<Token![,]> = input.parse()?;
            } else if input.peek(Ident) {
                let fork = input.fork();
                let ident: Ident = fork.parse()?;

                match ident.to_string().as_str() {
                    "natives" => {
                        let _: Ident = input.parse()?;
                        let _: Token![:] = input.parse()?;
                        let content;
                        let _ = bracketed!(content in input);
                        natives_list = Some(Punctuated::parse_terminated(&content)?);
                        let _: Option<Token![,]> = input.parse()?;
                    }
                    "uid" => {
                        let _: Ident = input.parse()?;
                        let _: Token![:] = input.parse()?;
                        explicit_uid = Some(input.parse()?);
                        let _: Option<Token![,]> = input.parse()?;
                    }
                    "component_name" => {
                        let _: Ident = input.parse()?;
                        let _: Token![:] = input.parse()?;
                        explicit_name = Some(input.parse()?);
                        let _: Option<Token![,]> = input.parse()?;
                    }
                    "component_version" => {
                        let _: Ident = input.parse()?;
                        let _: Token![:] = input.parse()?;
                        let content;
                        syn::parenthesized!(content in input);
                        let major: LitInt = content.parse()?;
                        let _: Token![,] = content.parse()?;
                        let minor: LitInt = content.parse()?;
                        let _: Token![,] = content.parse()?;
                        let patch: LitInt = content.parse()?;
                        explicit_version = Some((
                            major.base10_parse()?,
                            minor.base10_parse()?,
                            patch.base10_parse()?,
                        ));
                        let _: Option<Token![,]> = input.parse()?;
                    }
                    _ => break,
                }
            } else {
                break;
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
            explicit_uid,
            explicit_name,
            explicit_version,
        })
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

pub fn create_plugin(input: TokenStream) -> TokenStream {
    let plugin = parse_macro_input!(input as InitPlugin);

    let natives = gen_natives_list(&plugin);
    let supports_body = gen_samp_constructor(&plugin.constructor);
    let samp_entry_points = gen_samp_entry_points(&natives, &supports_body);

    // Native Open Multiplayer entry point.
    //
    // Generated by default — the absence of the `samp-only` feature is enough.
    // `uid:`, `component_name:` and `component_version:` in the macro and in
    // `[package.metadata.samp]` are optional; when absent the SDK generates
    // the UID automatically via FNV-1a and uses CARGO_PKG_NAME/VERSION.
    //
    // The check uses an env var (not an emitted `#[cfg]`) to avoid
    // `unexpected_cfg` in crates that do not declare the `samp-only` feature.
    let samp_only = std::env::var("CARGO_FEATURE_SAMP_ONLY").is_ok();
    let cargo_meta = read_samp_metadata();

    let omp_entry_point = if samp_only {
        quote! {}
    } else {
        gen_omp_entry_point(&plugin, &cargo_meta, &natives)
    };

    let generated = quote! {
        #samp_entry_points
        #omp_entry_point
    };

    generated.into()
}

// ---------------------------------------------------------------------------
// Generation helpers — extracted from `create_plugin` to keep each piece
// small and named. All return `proc_macro2::TokenStream` and do not touch
// state outside of their arguments.
// ---------------------------------------------------------------------------

/// Converts the paths in the `natives: [...]` list into `__samp_reg_*()` calls,
/// emitting the sequence `path1(), path2(), ...` separated by commas.
fn gen_natives_list(plugin: &InitPlugin) -> proc_macro2::TokenStream {
    plugin
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
        .collect()
}

/// Block that initializes `samp::plugin` in the `Supports` entry point (SA-MP).
fn gen_samp_constructor(constructor: &Constructor) -> proc_macro2::TokenStream {
    match constructor {
        Constructor::Block(stmts) => quote! {
            let constructor = || { #(#stmts)* };
            samp::plugin::initialize(constructor);
        },
        Constructor::Default(ty) => quote! {
            samp::plugin::initialize(<#ty as Default>::default);
        },
    }
}

/// SA-MP entry points — always generated. `Load`/`Unload`/`AmxLoad`/`AmxUnload`/
/// `Supports`/`ProcessTick` are looked up by the server by fixed name.
fn gen_samp_entry_points(
    natives: &proc_macro2::TokenStream,
    supports_body: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
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

        // Export `ProcessTick` is the fixed name the SA-MP server looks up via GetProcAddress.
        // It cannot be renamed. Internally we call `server_tick`, which dispatches
        // `SampPlugin::on_server_tick` — unified method for SA-MP and Open Multiplayer.
        #[unsafe(no_mangle)]
        pub extern "system" fn ProcessTick() {
            samp::interlayer::server_tick();
        }
    }
}

/// UID priority chain: `uid:` in the macro > `[package.metadata.samp].uid`
/// > deterministic FNV-1a of `CARGO_PKG_NAME@CARGO_PKG_VERSION` (persisted).
fn resolve_uid_expr(plugin: &InitPlugin, cargo_meta: &SampMetadata) -> proc_macro2::TokenStream {
    if let Some(ref expr) = plugin.explicit_uid {
        quote! { #expr }
    } else {
        let lit = Literal::u64_suffixed(resolve_uid(cargo_meta));
        quote! { #lit }
    }
}

/// Component name priority chain: `component_name:` in the macro >
/// `[package.metadata.samp].name` > `CARGO_PKG_NAME`.
fn resolve_component_name(plugin: &InitPlugin, cargo_meta: &SampMetadata) -> String {
    if let Some(ref name) = plugin.explicit_name {
        name.value()
    } else {
        cargo_meta
            .name
            .clone()
            .or_else(|| std::env::var("CARGO_PKG_NAME").ok())
            .unwrap_or_default()
    }
}

/// Version priority chain: `component_version:` in the macro >
/// `[package.metadata.samp].version` > parsed `CARGO_PKG_VERSION`.
fn resolve_component_version(plugin: &InitPlugin, cargo_meta: &SampMetadata) -> (u8, u8, u8) {
    plugin
        .explicit_version
        .or(cargo_meta.version)
        .or_else(|| {
            std::env::var("CARGO_PKG_VERSION")
                .ok()
                .and_then(|v| parse_version_str(&v))
        })
        .unwrap_or((1, 0, 0))
}

/// Block that initializes the plugin in the `ComponentEntryPoint` entry point (Open Multiplayer).
fn gen_omp_constructor(constructor: &Constructor) -> proc_macro2::TokenStream {
    match constructor {
        Constructor::Block(stmts) => quote! {
            let constructor = || { #(#stmts)* };
            samp::interlayer::omp_initialize(constructor);
        },
        Constructor::Default(ty) => quote! {
            samp::interlayer::omp_initialize(<#ty as Default>::default);
        },
    }
}

/// Generates `mod __omp_component { ... }` with Itanium/MSVC vtables, FFI
/// handlers for `IComponent`, the secondary `IUIDProvider` vtable and the
/// `ComponentEntryPoint` entry point.
///
/// The body is mostly a single `quote!{}` literal — naked asm, internal
/// macros and static definitions. Splitting into sub-helpers would break
/// token continuity with no readability gain.
#[allow(clippy::too_many_lines)]
fn gen_omp_entry_point(
    plugin: &InitPlugin,
    cargo_meta: &SampMetadata,
    natives: &proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    let uid_expr = resolve_uid_expr(plugin, cargo_meta);
    let name_str = resolve_component_name(plugin, cargo_meta);
    let name_len: usize = name_str.len();
    let name_bytes: Vec<u8> = name_str.bytes().collect();
    let name_sv = {
        let name_lit = name_str.clone();
        quote! {{
            const __S: &str = #name_lit;
            samp::omp::types::StringView { data: __S.as_ptr(), len: #name_len }
        }}
    };
    let (major, minor, patch) = resolve_component_version(plugin, cargo_meta);
    let omp_initialize = gen_omp_constructor(&plugin.constructor);

    quote! {
        mod __omp_component {
            use super::*;
            use samp::omp::component::*;
            use samp::omp::types::*;

            // Internal macro: generates the vtable handlers for both ABIs
            // without duplicating the logic — only the calling convention changes.
            macro_rules! __def_comp_fns {
                ($abi:literal) => {
                    // Itanium ABI: normal by-value return.
                    #[cfg(not(target_env = "msvc"))]
                    pub unsafe extern "C" fn comp_name(_this: *const OmpComponent) -> StringView {
                        #name_sv
                    }

                    // MSVC: StringView (8 bytes) is returned via hidden pointer at [ESP+4].
                    // Naked asm implementation identical to the official Open Multiplayer components:
                    //   mov eax, [esp+4]; mov [eax], data_ptr; mov [eax+4], len; ret 4
                    // Name bytes in a static array so they can be referenced via `sym`.
                    #[cfg(target_env = "msvc")]
                    #[unsafe(no_mangle)]
                    static COMP_NAME_BYTES: [u8; #name_len] = [#(#name_bytes),*];

                    #[cfg(target_env = "msvc")]
                    #[unsafe(naked)]
                    pub unsafe extern "thiscall" fn comp_name() {
                        core::arch::naked_asm!(
                            "mov eax, [esp+4]",
                            "mov ecx, offset {data_ptr}",
                            "mov dword ptr [eax], ecx",
                            "mov dword ptr [eax+4], {data_len}",
                            "ret 4",
                            data_ptr = sym COMP_NAME_BYTES,
                            data_len = const #name_len,
                        );
                    }

                    // Itanium ABI: normal by-value return.
                    #[cfg(not(target_env = "msvc"))]
                    pub unsafe extern "C" fn comp_version(_this: *const OmpComponent) -> SemanticVersion {
                        SemanticVersion::new(#major, #minor, #patch)
                    }

                    // MSVC: SemanticVersion (6 bytes) is returned via hidden pointer at [ESP+4].
                    // major in byte 0, minor in byte 1, patch in byte 2, byte 3 padding,
                    // prerel in bytes 4-5 (uint16_t).
                    #[cfg(target_env = "msvc")]
                    #[unsafe(naked)]
                    pub unsafe extern "thiscall" fn comp_version() {
                        core::arch::naked_asm!(
                            "mov eax, [esp+4]",
                            "mov dword ptr [eax], {sver_lo}",
                            "mov word ptr [eax+4], 0",
                            "ret 4",
                            sver_lo = const ((#major as u32) | ((#minor as u32) << 8) | ((#patch as u32) << 16)),
                        );
                    }

                    pub unsafe extern $abi fn comp_on_load(_this: *mut OmpComponent, core: *mut ICore) {
                        let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                            samp::interlayer::omp_load(core);
                        }));
                    }

                    pub unsafe extern $abi fn comp_on_init(
                        _this: *mut OmpComponent,
                        components: *mut IComponentList,
                    ) {
                        let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                            let component_list_ptr = components as *mut samp::omp::server::ServerComponentList;
                            unsafe { samp::interlayer::omp_on_init(component_list_ptr) };
                        }));
                    }

                    // Itanium ABI: comp_on_ready, comp_free, comp_reset with explicit _this.
                    #[cfg(not(target_env = "msvc"))]
                    pub unsafe extern "C" fn comp_on_ready(_this: *mut OmpComponent) {
                        let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                            samp::interlayer::omp_on_ready();
                        }));
                    }

                    pub unsafe extern $abi fn comp_on_free(
                        _this: *mut OmpComponent,
                        _component: *mut OmpComponent,
                    ) {
                        let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                            samp::interlayer::omp_on_free();
                        }));
                    }

                    // Itanium ABI: comp_free and comp_reset with explicit _this.
                    #[cfg(not(target_env = "msvc"))]
                    pub unsafe extern "C" fn comp_free(_this: *mut OmpComponent) {
                        let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                            samp::interlayer::omp_cleanup();
                            samp::interlayer::unload();
                            let _ = unsafe { Box::from_raw(_this) };
                        }));
                    }

                    #[cfg(not(target_env = "msvc"))]
                    pub unsafe extern "C" fn comp_reset(_this: *mut OmpComponent) {}
                };
            }

            #[cfg(not(target_env = "msvc"))]
            __def_comp_fns!("C");

            #[cfg(target_env = "msvc")]
            __def_comp_fns!("thiscall");

            // MSVC: functions with only `this` (no stack args) need a signature
            // without explicit parameters — Rust emits `ret` (no N), correct for this ABI.
            #[cfg(target_env = "msvc")]
            pub unsafe extern "thiscall" fn comp_on_ready() {
                let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                    samp::interlayer::omp_on_ready();
                }));
            }

            #[cfg(target_env = "msvc")]
            pub unsafe extern "thiscall" fn comp_free() {
                let _ = ::std::panic::catch_unwind(::std::panic::AssertUnwindSafe(|| {
                    samp::interlayer::omp_cleanup();
                    samp::interlayer::unload();
                }));
            }

            #[cfg(target_env = "msvc")]
            pub unsafe extern "thiscall" fn comp_reset() {}

            // VTABLE — Itanium ABI (Linux): destructor at slot [4], D0 at [5]
            #[cfg(not(target_env = "msvc"))]
            static VTABLE: IComponentVTable = IComponentVTable {
                get_extension:         samp::omp::component::ext_get_extension,
                add_extension:         samp::omp::component::ext_add_extension,
                remove_extension_ptr:  samp::omp::component::ext_remove_extension_ptr,
                remove_extension_uid:  samp::omp::component::ext_remove_extension_uid,
                destructor:            samp::omp::component::ext_destructor,
                destructor_deleting:   samp::omp::component::ext_destructor_deleting,
                supported_version:     samp::omp::component::comp_supported_version,
                component_name:        comp_name,
                component_type:        samp::omp::component::comp_component_type,
                component_version:     comp_version,
                on_load:               comp_on_load,
                on_init:               comp_on_init,
                on_ready:              comp_on_ready,
                on_free:               comp_on_free,
                provide_configuration: samp::omp::component::comp_provide_configuration,
                free:                  comp_free,
                reset:                 comp_reset,
            };

            // VTABLE — MSVC ABI (Windows): 16 slots.
            // IExtensible: getExtension[0], addExtension[1], removeExtension(ptr)[2],
            //              removeExtension(uid)[3], ~dtor[4] (single scalar deleting).
            // IComponent:  supportedVersion[5], componentName[6], componentType[7],
            //              componentVersion[8], onLoad[9], onInit[10], onReady[11],
            //              onFree[12], provideConfiguration[13], free[14], reset[15].
            #[cfg(target_env = "msvc")]
            static VTABLE: IComponentVTable = IComponentVTable {
                get_extension:         samp::omp::component::ext_get_extension,
                add_extension:         samp::omp::component::ext_add_extension,
                remove_extension_ptr:  samp::omp::component::ext_remove_extension_ptr,
                remove_extension_uid:  samp::omp::component::ext_remove_extension_uid,
                destructor:            samp::omp::component::ext_destructor,
                supported_version:     samp::omp::component::comp_supported_version,
                component_name:        comp_name,
                component_type:        samp::omp::component::comp_component_type,
                component_version:     comp_version,
                on_load:               comp_on_load,
                on_init:               comp_on_init,
                on_ready:              comp_on_ready,
                on_free:               comp_on_free,
                provide_configuration: samp::omp::component::comp_provide_configuration,
                free:                  comp_free,
                reset:                 comp_reset,
            };

            // UID_VTABLE — Itanium ABI: two destructor thunk slots
            #[cfg(not(target_env = "msvc"))]
            static UID_VTABLE: IUIDProviderVTable = IUIDProviderVTable {
                destructor_complete: samp::omp::component::uid_destructor_noop,
                destructor_deleting: samp::omp::component::uid_destructor_noop,
                get_uid:             samp::omp::component::uid_get_uid,
            };

            // UID_VTABLE — MSVC ABI: ONLY getUID (IUIDProvider has no virtual destructor).
            #[cfg(target_env = "msvc")]
            static UID_VTABLE: IUIDProviderVTable = IUIDProviderVTable {
                get_uid: samp::omp::component::uid_get_uid,
            };

            #[unsafe(no_mangle)]
            pub extern "C" fn ComponentEntryPoint() -> *mut OmpComponent {
                #omp_initialize
                samp::interlayer::omp_store_natives(vec![#natives]);
                let component = Box::new(OmpComponent::new(&VTABLE, &UID_VTABLE, #uid_expr));
                Box::into_raw(component)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Derive macro: #[derive(SampPlugin)]
// ---------------------------------------------------------------------------

/// Generates `impl samp::prelude::SampPlugin for T {}` with all default methods.
pub fn derive_samp_plugin(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let generated = quote! {
        impl #impl_generics samp::prelude::SampPlugin for #name #ty_generics #where_clause {}
    };

    generated.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- fnv1a_64 ---

    #[test]
    fn fnv1a_64_empty_returns_offset_basis() {
        assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
    }

    #[test]
    fn fnv1a_64_known_vector_a() {
        // FNV-1a 64-bit reference vector: "a" → 0xaf63dc4c8601ec8c
        assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn fnv1a_64_is_deterministic() {
        assert_eq!(fnv1a_64(b"plugin@1.0.0"), fnv1a_64(b"plugin@1.0.0"));
    }

    #[test]
    fn fnv1a_64_different_inputs_differ() {
        assert_ne!(fnv1a_64(b"plugin@1.0.0"), fnv1a_64(b"plugin@1.0.1"));
    }

    // --- parse_uid_str ---

    #[test]
    fn parse_uid_hex_lowercase() {
        assert_eq!(parse_uid_str("0x12_3abc"), Some(0x12_3abc));
    }

    #[test]
    fn parse_uid_hex_uppercase_prefix() {
        assert_eq!(parse_uid_str("0X123ABC"), Some(0x12_3abc));
    }

    #[test]
    fn parse_uid_decimal() {
        assert_eq!(parse_uid_str("12345"), Some(12345));
    }

    #[test]
    fn parse_uid_max_u64() {
        assert_eq!(parse_uid_str("0xFFFFFFFFFFFFFFFF"), Some(u64::MAX));
    }

    #[test]
    fn parse_uid_invalid_returns_none() {
        assert_eq!(parse_uid_str("invalid"), None);
    }

    #[test]
    fn parse_uid_empty_returns_none() {
        assert_eq!(parse_uid_str(""), None);
    }

    // --- parse_version_str ---

    #[test]
    fn parse_version_full() {
        assert_eq!(parse_version_str("1.2.3"), Some((1, 2, 3)));
    }

    #[test]
    fn parse_version_zeros() {
        assert_eq!(parse_version_str("0.0.0"), Some((0, 0, 0)));
    }

    #[test]
    fn parse_version_max_values() {
        assert_eq!(parse_version_str("255.255.255"), Some((255, 255, 255)));
    }

    #[test]
    fn parse_version_with_prerelease_suffix() {
        assert_eq!(parse_version_str("1.2.3-beta"), Some((1, 2, 3)));
    }

    #[test]
    fn parse_version_missing_patch_defaults_zero() {
        assert_eq!(parse_version_str("1.2"), Some((1, 2, 0)));
    }

    #[test]
    fn parse_version_missing_minor_returns_none() {
        assert_eq!(parse_version_str("1"), None);
    }

    #[test]
    fn parse_version_invalid_returns_none() {
        assert_eq!(parse_version_str("invalid"), None);
    }

    #[test]
    fn parse_version_invalid_component_returns_none() {
        assert_eq!(parse_version_str("1.invalid.3"), None);
    }

    // --- read_samp_metadata_from_content ---

    #[test]
    fn metadata_empty_content_returns_default() {
        let meta = read_samp_metadata_from_content("");
        assert!(meta.uid.is_none());
        assert!(meta.name.is_none());
        assert!(meta.version.is_none());
    }

    #[test]
    fn metadata_reads_uid() {
        let content = "[package.metadata.samp]\nuid = \"0x4D455550CAFEBABE\"\n";
        let meta = read_samp_metadata_from_content(content);
        assert_eq!(meta.uid, Some(0x4D45_5550_CAFE_BABE));
    }

    #[test]
    fn metadata_reads_name() {
        let content = "[package.metadata.samp]\nname = \"MeuPlugin\"\n";
        let meta = read_samp_metadata_from_content(content);
        assert_eq!(meta.name.as_deref(), Some("MeuPlugin"));
    }

    #[test]
    fn metadata_reads_version() {
        let content = "[package.metadata.samp]\nversion = \"1.2.3\"\n";
        let meta = read_samp_metadata_from_content(content);
        assert_eq!(meta.version, Some((1, 2, 3)));
    }

    #[test]
    fn metadata_reads_all_fields() {
        let content = "[package.metadata.samp]\nuid = \"0xDEADBEEF\"\nname = \"Plugin\"\nversion = \"2.0.1\"\n";
        let meta = read_samp_metadata_from_content(content);
        assert_eq!(meta.uid, Some(0xDEAD_BEEF));
        assert_eq!(meta.name.as_deref(), Some("Plugin"));
        assert_eq!(meta.version, Some((2, 0, 1)));
    }

    #[test]
    fn metadata_no_section_returns_default() {
        let content = "[package]\nname = \"meu-crate\"\nversion = \"1.0.0\"\n";
        let meta = read_samp_metadata_from_content(content);
        assert!(meta.uid.is_none());
    }

    #[test]
    fn metadata_stops_at_next_section() {
        let content =
            "[package.metadata.samp]\nuid = \"0x1234\"\n[other.section]\nuid = \"0x5678\"\n";
        let meta = read_samp_metadata_from_content(content);
        assert_eq!(meta.uid, Some(0x1234));
    }

    #[test]
    fn metadata_accepts_single_quoted_values() {
        let content = "[package.metadata.samp]\nname = 'MinhaPlugin'\n";
        let meta = read_samp_metadata_from_content(content);
        assert_eq!(meta.name.as_deref(), Some("MinhaPlugin"));
    }
}
