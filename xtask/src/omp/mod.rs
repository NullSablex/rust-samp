//! `cargo xtask gen-omp`: typed Rust wrappers for the open.mp interfaces,
//! generated from the SDK headers.
//!
//! Hand-writing a wrapper means copying two slot indices, a C++ signature and
//! a Rust one, and every copy is a chance to be wrong in a way that fails
//! silently. The headers already say everything: clang lays out the vtables
//! for both ABIs, and libclang gives every parameter and return type.
//!
//! What goes in is listed in `xtask/omp-wrappers.toml`. What comes out is
//! only what can be stated with certainty; everything else is written into
//! the file as a `// skipped:` line with the reason, so a gap is visible
//! rather than guessed at. The output is a map, not the proof:
//! `cargo xtask check-abi --generated` compares every slot with the official
//! binaries, and a server run is the rest.
//!
//! The headers are read once per ABI — one translation unit holding all of
//! them — and clang is run once per ABI for the vtables and once for the
//! subobject offsets.

mod ast;
mod generator;
mod hand;
mod handlers;
pub mod inspect;
mod mirror;
pub mod output;
mod render;
mod sdk;
pub mod spec;
mod types;
mod vtables;
mod wrappers;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, anyhow};

use self::ast::{Records, UidKind};
use self::generator::Generator;
use self::hand::Hand;
use self::output::Output;
use self::sdk::{Abi, Sdk};
use self::wrappers::Layouts;

pub struct Options {
    pub check: bool,
    pub sdk: Option<PathBuf>,
    pub xwin: Option<PathBuf>,
}

/// Generates, or with `check` compares; `Ok(false)` when a file is stale.
pub fn run(options: &Options) -> Result<bool> {
    let started = Instant::now();
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("the xtask crate has no parent")?;
    let spec = spec::load(&repo.join("xtask/omp-wrappers.toml"))?;
    let sdk = Sdk::locate(options.sdk.as_deref(), options.xwin.as_deref())?;
    let work = sdk::tempdir::Dir::new("omp-gen")?;

    let headers: Vec<&str> = spec
        .iter()
        .map(|i| i.header.as_str())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let source = work.path().join("headers.cpp");
    std::fs::write(&source, vtables::includes(&headers))
        .with_context(|| format!("writing {}", source.display()))?;
    let classes: Vec<&str> = spec.iter().map(|i| i.class.as_str()).collect();

    let clang = clang::Clang::new().map_err(|e| anyhow!("loading libclang: {e}"))?;
    let index = clang::Index::new(&clang, false, false);

    // The vtable and record dumps are separate `clang++` processes: they run
    // while libclang parses, each ABI on its own thread.
    std::thread::scope(|scope| {
        let slots = Abi::BOTH.map(|abi| {
            let (sdk, headers, classes, work) = (&sdk, &headers, &classes, work.path());
            scope.spawn(move || vtables::slot_tables(sdk, abi, headers, classes, work))
        });
        let itanium_unit = ast::parse(&index, &sdk, Abi::Itanium, &source)?;
        let msvc_unit = ast::parse(&index, &sdk, Abi::Msvc, &source)?;
        let records = Records::index(&itanium_unit);
        let parsed = started.elapsed();

        let components = component_classes(&spec, &records);
        let offsets = Abi::BOTH.map(|abi| {
            // Each thread owns its list: the scope outlives this block.
            let (sdk, headers, components, work) =
                (&sdk, &headers, components.clone(), work.path());
            scope.spawn(move || vtables::component_offsets(sdk, abi, headers, &components, work))
        });
        let [slots_itanium, slots_msvc] = slots;
        let [offsets_itanium, offsets_msvc] = offsets;
        let layouts = Layouts {
            itanium: joined(slots_itanium)?,
            msvc: joined(slots_msvc)?,
            component_itanium: joined(offsets_itanium)?,
            component_msvc: joined(offsets_msvc)?,
        };

        let hand = Hand::scan(&repo.join("samp-sdk/src/omp"))?;
        let mut generator = Generator::new(
            records,
            Records::index(&msvc_unit),
            sdk.include_dir(),
            hand,
            &spec,
        );
        let mut output = Output::new(&repo.join("samp-sdk/src/omp/generated"), options.check);
        let modules = write_modules(&mut generator, &spec, &layouts, &mut output)?;

        eprintln!(
            "{modules} modules from {} headers: parsed in {parsed:.1?}, done in {:.1?}",
            headers.len(),
            started.elapsed()
        );
        Ok(output.finish())
    })
}

/// The interfaces that are components (`PROVIDE_UID`): the ones whose
/// `IComponent` subobject offset a `ComponentInterface` impl needs.
fn component_classes(spec: &[spec::Interface], records: &Records<'_>) -> Vec<String> {
    spec.iter()
        .filter(|i| {
            records
                .get(&i.class)
                .and_then(ast::uid_of)
                .is_some_and(|u| u.kind == UidKind::Component)
        })
        .map(|i| i.class.clone())
        .collect()
}

/// Every module, then the handlers, the structs and the index. Returns how
/// many modules there are.
fn write_modules(
    generator: &mut Generator<'_>,
    spec: &[spec::Interface],
    layouts: &Layouts,
    output: &mut Output,
) -> Result<usize> {
    let mut modules = Vec::new();
    for entry in spec {
        let text = generator
            .module(entry, layouts)
            .with_context(|| format!("generating {}", entry.module))?;
        let written = output.file(&format!("{}.rs", entry.module), &text)?;
        modules.push((entry.module.clone(), render::defines_anything(&written)));
    }
    for (name, text) in [
        ("handlers", generator.handlers_file()),
        ("structs", generator.structs()),
    ] {
        let written = output.file(&format!("{name}.rs"), &text)?;
        modules.push((name.to_owned(), render::defines_anything(&written)));
    }
    output.file("mod.rs", &render::index(&modules))?;
    Ok(modules.len())
}

/// A `clang++` thread's result, its panic turned into an error.
fn joined<T>(handle: std::thread::ScopedJoinHandle<'_, Result<T>>) -> Result<T> {
    handle
        .join()
        .map_err(|_| anyhow!("a clang++ thread panicked"))?
}
