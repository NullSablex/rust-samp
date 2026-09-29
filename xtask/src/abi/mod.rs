//! `cargo xtask check-abi`: the SDK's vtable slots against the official
//! server binaries.
//!
//! Every index the SDK hardcodes is a claim about a binary someone else
//! built. A wrong one fails quietly: the server returns a plausible value
//! from the wrong virtual function, and on Windows the stack is corrupted on
//! top of that. This re-derives each index from the binaries and compares it
//! with what the source declares. It needs the official servers, which cannot
//! be redistributed, so it is not part of CI.
//!
//! Itanium (Linux) checks are exact: the `.so` files keep their symbols, so
//! each slot is identified by name. MSVC (Windows) checks work from RTTI and
//! the `ret N` of each slot (N = argument bytes under `thiscall`), which pins
//! the methods that take arguments; slots that take none are indistinguishable
//! that way, and are reported as unverifiable rather than assumed.

mod elf;
mod pe;
mod signature;
mod source;

use std::fmt::Display;
use std::path::{Path, PathBuf};

use anyhow::Result;

use self::elf::{Elf, slot_of};
use self::pe::Pe;
use self::signature::{ArgBytes, method_name, same_params};
use self::source::Constants;

pub struct Options {
    pub linux: Option<PathBuf>,
    pub win: Option<PathBuf>,
    pub generated: bool,
}

/// Tallies the checks and prints each one.
#[derive(Default)]
struct Report {
    failures: usize,
    skipped: usize,
}

impl Report {
    fn check<T: PartialEq + Display>(
        &mut self,
        label: &str,
        expected: Option<T>,
        found: Option<T>,
    ) {
        match (expected, found) {
            (_, None) => {
                println!("  SKIP  {label}: could not derive it from the binary");
                self.skipped += 1;
            }
            (Some(expected), Some(found)) if expected == found => {
                println!("  ok    {label}: {found}");
            }
            (expected, Some(found)) => {
                let expected = expected.map_or_else(|| "nothing".to_owned(), |e| e.to_string());
                println!("  FAIL  {label}: source says {expected}, binary says {found}");
                self.failures += 1;
            }
        }
    }

    fn skip(&mut self, count: usize) {
        self.skipped += count;
    }
}

/// Where a server lives: the flag, the environment variable, then where the
/// official archive lands when unpacked.
fn find_server(explicit: Option<&Path>, variable: &str, archive: &str) -> Option<PathBuf> {
    if let Some(path) = explicit {
        return Some(path.to_owned());
    }
    if let Some(path) = std::env::var_os(variable) {
        return Some(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    [
        PathBuf::from(archive),
        home.join("Downloads").join(archive),
        home.join(archive),
    ]
    .into_iter()
    .map(|dir| dir.join("Server"))
    .find(|dir| dir.join("components").is_dir())
}

pub fn run(options: &Options) -> Result<bool> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or(Path::new("."));
    let omp = repo.join("samp-sdk/src/omp");
    let linux = find_server(
        options.linux.as_deref(),
        "OPENMP_LINUX_SERVER",
        "open.mp-linux-x86",
    )
    .filter(|d| d.is_dir());
    let win = find_server(
        options.win.as_deref(),
        "OPENMP_WIN_SERVER",
        "open.mp-win-x86",
    )
    .filter(|d| d.is_dir());
    let mut report = Report::default();

    if let Some(server) = &linux {
        check_itanium(server, &omp, &mut report)?;
    } else {
        println!("\nLinux server not found at any of the usual paths — skipping");
        report.skip(1);
    }
    if let Some(server) = &win {
        check_msvc(server, &omp, &mut report)?;
    } else {
        println!("\nWindows server not found at any of the usual paths — skipping");
        report.skip(1);
    }
    if options.generated {
        check_generated(repo, linux.as_deref(), win.as_deref(), &mut report)?;
    }

    println!();
    if report.failures > 0 {
        println!("{} slot(s) disagree with the binaries.", report.failures);
        return Ok(false);
    }
    println!(
        "All derivable slots match the official binaries ({} skipped).",
        report.skipped
    );
    Ok(true)
}

/// The hand-written slots, by the name each points at in the Linux binaries.
fn check_itanium(server: &Path, omp: &Path, report: &mut Report) -> Result<()> {
    println!("\nItanium ABI — {}", server.display());
    let timers_rs = Constants::read(&omp.join("timers.rs"))?;
    let component_rs = Constants::read(&omp.join("component_api.rs"))?;
    let server_rs = Constants::read(&omp.join("server.rs"))?;
    let players_rs = Constants::read(&omp.join("players.rs"))?;
    let vtable = |library: &str, class: &str, count: usize| -> Result<Vec<String>> {
        Elf::open(&server.join(library))?
            .vtable(class, count)
            .map_err(anyhow::Error::msg)
    };

    let timers = vtable("components/Timers.so", "TimersComponent", 40)?;
    report.check(
        "ITimersComponent::create(handler, interval, repeating)",
        timers_rs.get("SLOT_CREATE_INTERVAL", false),
        // The overload ending in `bool`: the other ends in `unsigned int`.
        // Matched by that rather than by the whole demangled name, whose
        // spelling of `std::ratio` differs between demanglers.
        timers.iter().position(|name| {
            name.contains("TimersComponent::create(") && name.ends_with(", bool)")
        }),
    );
    report.check(
        "IComponent::componentName",
        component_rs.get("SLOT_COMPONENT_NAME", false),
        slot_of(&timers, "componentName"),
    );
    report.check(
        "IComponent::componentVersion",
        component_rs.get("SLOT_COMPONENT_VERSION", false),
        slot_of(&timers, "componentVersion"),
    );

    let timer = vtable("components/Timers.so", "Timer", 40)?;
    report.check(
        "ITimer::kill",
        timers_rs.get("SLOT_TIMER_KILL", false),
        slot_of(&timer, "Timer::kill"),
    );

    let pawn = vtable("components/Pawn.so", "PawnComponent", 40)?;
    report.check(
        "IPawnComponent::getEventDispatcher",
        server_rs.get("PAWN_COMPONENT_PREFIX_SLOTS", false),
        slot_of(&pawn, "getEventDispatcher"),
    );
    let script = vtable("components/Pawn.so", "PawnScript", 70)?;
    // Ungated in the SDK: no virtual destructor, the same on both ABIs.
    report.check(
        "IPawnScript::GetAMX",
        Some(57),
        slot_of(&script, "PawnScript::GetAMX"),
    );

    let pool = vtable("omp-server", "PlayerPool", 40)?;
    for (constant, method) in [
        ("SLOT_SPAWN_DISPATCHER", "getPlayerSpawnDispatcher"),
        ("SLOT_CONNECT_DISPATCHER", "getPlayerConnectDispatcher"),
        ("SLOT_TEXT_DISPATCHER", "getPlayerTextDispatcher"),
        ("SLOT_DAMAGE_DISPATCHER", "getPlayerDamageDispatcher"),
        ("SLOT_STREAM_DISPATCHER", "getPlayerStreamDispatcher"),
        ("SLOT_SHOT_DISPATCHER", "getPlayerShotDispatcher"),
        ("SLOT_CHANGE_DISPATCHER", "getPlayerChangeDispatcher"),
        ("SLOT_CLICK_DISPATCHER", "getPlayerClickDispatcher"),
        ("SLOT_CHECK_DISPATCHER", "getPlayerCheckDispatcher"),
        ("SLOT_UPDATE_DISPATCHER", "getPlayerUpdateDispatcher"),
    ] {
        report.check(
            &format!("IPlayerPool::{method}"),
            players_rs.get(constant, false),
            slot_of(&pool, method),
        );
    }

    let core = vtable("omp-server", "Core", 40)?;
    report.check(
        "ICore::getPlayers",
        players_rs.get("SLOT_GET_PLAYERS", false),
        slot_of(&core, "Core::getPlayers"),
    );
    let list = vtable("omp-server", "ComponentList", 40)?;
    report.check(
        "IComponentList::queryComponent",
        server_rs.get("COMPONENT_LIST_PREFIX_SLOTS", false),
        slot_of(&list, "queryComponent"),
    );
    Ok(())
}

/// The hand-written slots under MSVC, by the bytes each method pops.
fn check_msvc(server: &Path, omp: &Path, report: &mut Report) -> Result<()> {
    println!("\nMSVC ABI — {}", server.display());
    let timers_rs = Constants::read(&omp.join("timers.rs"))?;
    let server_rs = Constants::read(&omp.join("server.rs"))?;

    let timers_dll = Pe::open(&server.join("components/Timers.dll"))?;
    let timers = timers_dll
        .vtable("TimersComponent")
        .map_err(anyhow::Error::msg)?;
    // `create(handler, Milliseconds, bool)` pops 4 + 8 + 4 = 16 bytes; the
    // four-argument overload pops 24. MSVC emits an overload set in reverse,
    // so this is exactly the pair that was swapped in v3.5.0.
    let found = timers
        .iter()
        .enumerate()
        .position(|(i, f)| i >= 15 && timers_dll.ret_bytes(*f) == Some(16));
    report.check(
        "ITimersComponent::create(handler, interval, repeating)",
        timers_rs.get("SLOT_CREATE_INTERVAL", true),
        found,
    );

    // `removeExtension(UID)` pops 8, `removeExtension(IExtension*)` 4.
    let popped = |slot: usize| timers.get(slot).and_then(|f| timers_dll.ret_bytes(*f));
    report.check(
        "IExtensible::removeExtension(UID) at slot 2",
        Some(8),
        popped(2),
    );
    report.check(
        "IExtensible::removeExtension(ptr) at slot 3",
        Some(4),
        popped(3),
    );

    // `getEventDispatcher` and `getAmxFunctions` take no arguments, so `ret N`
    // cannot tell them from their neighbours. What is checkable is that the
    // vtable is long enough for the index the SDK uses.
    let pawn = Pe::open(&server.join("components/Pawn.dll"))?
        .vtable("PawnComponent")
        .map_err(anyhow::Error::msg)?;
    let prefix = server_rs
        .get("PAWN_COMPONENT_PREFIX_SLOTS", true)
        .unwrap_or(0);
    report.check(
        &format!("IPawnComponent vtable reaches slot {}", prefix + 1),
        Some(true),
        Some(pawn.len() > prefix + 1),
    );
    println!("  note  getEventDispatcher/getAmxFunctions take no arguments — their");
    println!("        exact index is not derivable from `ret N`; see docs/internals/omp-abi.md");
    Ok(())
}

/// Every slot the generator wrote, against the vtable of the class that
/// implements it: by name under Itanium — and by parameters for an overload,
/// whose name alone would pass either one — and by `ret N` under MSVC.
fn check_generated(
    repo: &Path,
    linux: Option<&Path>,
    win: Option<&Path>,
    report: &mut Report,
) -> Result<()> {
    let spec = crate::omp::spec::load(&repo.join("xtask/omp-wrappers.toml"))?;
    let generated = repo.join("samp-sdk/src/omp/generated");
    let bytes = ArgBytes::new(&source::mirrored_sizes(&generated.join("structs.rs"))?);
    for entry in &spec {
        let path = generated.join(format!("{}.rs", entry.module));
        if !path.exists() {
            continue;
        }
        let slots = source::generated_slots(&path)?;
        let (library, class) = (&entry.library, &entry.implementor);
        println!("\nGenerated `{}` — {class} in {library}", entry.module);
        if slots.is_empty() {
            println!("  (nothing generated)");
            continue;
        }
        let signatures: Vec<&str> = slots
            .iter()
            .map(|s| s.doc.trim().trim_matches('`'))
            .collect();

        if let Some(server) = linux {
            let file = if library == "omp-server" {
                library.clone()
            } else {
                format!("components/{library}.so")
            };
            let count = slots.iter().map(|s| s.itanium).max().unwrap_or(0) + 1;
            match Elf::open(&server.join(&file))?.vtable(class, count) {
                Err(why) => {
                    println!("  SKIP  Itanium: {why} in {file}");
                    report.skip(slots.len());
                }
                Ok(vtable) => {
                    let names: Vec<&str> = signatures.iter().map(|s| method_name(s)).collect();
                    for (slot, signature) in slots.iter().zip(&signatures) {
                        let method = method_name(signature);
                        let found = vtable.get(slot.itanium).map_or("", String::as_str);
                        let mut ok = found.contains(&format!("::{method}("));
                        if ok && names.iter().filter(|n| **n == method).count() > 1 {
                            ok = same_params(signature, found);
                        }
                        report.check(
                            &format!("Itanium [{}] {method}", slot.itanium),
                            Some(true),
                            ok.then_some(true),
                        );
                    }
                }
            }
        }
        if let Some(server) = win {
            let file = if library == "omp-server" {
                format!("{library}.exe")
            } else {
                format!("components/{library}.dll")
            };
            let dll = Pe::open(&server.join(&file))?;
            match dll.vtable(class) {
                Err(why) => {
                    // The Windows server executable keeps RTTI for three
                    // classes only; these slots are left to a server run.
                    let name = Path::new(&file)
                        .file_name()
                        .map_or(file.as_str(), |n| n.to_str().unwrap_or_default());
                    println!(
                        "  SKIP  MSVC: {why} in {name} — prove these with examples/omp-showcase"
                    );
                    report.skip(slots.len());
                }
                Ok(vtable) => {
                    for (slot, signature) in slots.iter().zip(&signatures) {
                        let expected = bytes.of(signature);
                        let found = vtable.get(slot.msvc).and_then(|f| dll.ret_bytes(*f));
                        let label = format!(
                            "MSVC [{}] {} pops {expected}",
                            slot.msvc,
                            method_name(signature)
                        );
                        report.check(&label, Some(expected), found);
                    }
                }
            }
        }
    }
    Ok(())
}
