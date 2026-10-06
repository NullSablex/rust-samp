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
mod pdb;
mod pe;
mod signature;
mod source;

use std::collections::HashMap;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use anyhow::Result;

use self::elf::{Elf, slot_of};
use self::pdb::Symbols;
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
    let omp = repo.join("crates/samp-sdk/src/omp");
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

/// A Windows binary, with its `.pdb` when the server ships one.
struct Windows {
    pe: Pe,
    symbols: Option<Symbols>,
    /// The file name, for messages.
    name: String,
}

impl Windows {
    fn open(path: &Path) -> Result<Self> {
        let pe = Pe::open(path)?;
        let symbols = Symbols::beside(path, pe.base())?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(Self { pe, symbols, name })
    }

    /// The primary vtable of `class`. RTTI decides where there is RTTI: its
    /// locator records the subobject offset, so the vtable at offset 0 is
    /// found without assuming anything about the bases. Where the binary keeps
    /// none — the server executable's own classes — the PDB's vftable for
    /// `IExtensible` stands in, which holds for those classes: each one's
    /// first base chain starts there.
    fn vtable(&self, class: &str) -> Result<Vec<u32>, String> {
        match self.pe.vtable(class) {
            Ok((_, slots)) => Ok(slots),
            Err(why) => match self.symbols.as_ref().and_then(|s| s.primary_vftable(class)) {
                Some(address) => Ok(self.pe.vtable_at(address)),
                None => Err(format!("{why} in {}", self.name)),
            },
        }
    }

    /// Whether the function at `pointer` is `method` by name, by parameters
    /// too for an overload; `None` without a PDB.
    fn holds(&self, pointer: u32, method: &str, signature: &str, overloaded: bool) -> Option<bool> {
        let names = self.symbols.as_ref()?.names_at(pointer);
        let wanted = format!("::{method}(");
        Some(
            names.iter().any(|name| {
                name.contains(&wanted) && (!overloaded || same_params(signature, name))
            }),
        )
    }
}

/// One hand-written slot checked on Windows by the method it holds.
struct NameCheck {
    /// The binary, relative to the server directory.
    file: &'static str,
    class: &'static str,
    label: String,
    slot: Option<usize>,
    method: &'static str,
    /// Text the demangled name must end with, to pick one overload.
    ending: Option<&'static str>,
}

/// The hand-written slots under MSVC: by the bytes each method pops, and by
/// name from the PDBs.
fn check_msvc(server: &Path, omp: &Path, report: &mut Report) -> Result<()> {
    println!("\nMSVC ABI — {}", server.display());
    let timers_rs = Constants::read(&omp.join("timers.rs"))?;
    let component_rs = Constants::read(&omp.join("component_api.rs"))?;
    let server_rs = Constants::read(&omp.join("server.rs"))?;
    let players_rs = Constants::read(&omp.join("players.rs"))?;

    let timers_dll = Windows::open(&server.join("components/Timers.dll"))?;
    let timers = timers_dll
        .vtable("TimersComponent")
        .map_err(anyhow::Error::msg)?;
    // `create(handler, Milliseconds, bool)` pops 4 + 8 + 4 = 16 bytes; the
    // four-argument overload pops 24. MSVC emits an overload set in reverse,
    // so this is exactly the pair that was swapped in v3.5.0.
    let found = timers
        .iter()
        .enumerate()
        .position(|(i, f)| i >= 15 && timers_dll.pe.ret_bytes(*f) == Some(16));
    let interval = timers_rs.get("SLOT_CREATE_INTERVAL", true);
    report.check(
        "ITimersComponent::create(handler, interval, repeating)",
        interval,
        found,
    );
    // `removeExtension(UID)` pops 8, `removeExtension(IExtension*)` 4.
    let popped = |slot: usize| timers.get(slot).and_then(|f| timers_dll.pe.ret_bytes(*f));
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

    let checks = hand_name_checks(interval, &timers_rs, &component_rs, &server_rs, &players_rs);
    check_names(server, &checks, report)
}

/// The hand-written slots checked by name: the ones `check_itanium` checks on
/// Linux, with their MSVC values.
// A table: its length is its rows, one per slot, not logic to break up.
#[allow(clippy::too_many_lines)]
fn hand_name_checks(
    interval: Option<usize>,
    timers_rs: &Constants,
    component_rs: &Constants,
    server_rs: &Constants,
    players_rs: &Constants,
) -> Vec<NameCheck> {
    let check = |file, class, label: &str, slot, method, ending| NameCheck {
        file,
        class,
        label: label.to_owned(),
        slot,
        method,
        ending,
    };
    let mut checks = vec![
        check(
            "components/Timers.dll",
            "TimersComponent",
            "ITimersComponent::create(handler, interval, repeating)",
            interval,
            "create",
            Some(", bool)"),
        ),
        check(
            "components/Timers.dll",
            "TimersComponent",
            "IComponent::componentName",
            component_rs.get("SLOT_COMPONENT_NAME", true),
            "componentName",
            None,
        ),
        check(
            "components/Timers.dll",
            "TimersComponent",
            "IComponent::componentVersion",
            component_rs.get("SLOT_COMPONENT_VERSION", true),
            "componentVersion",
            None,
        ),
        check(
            "components/Timers.dll",
            "Timer",
            "ITimer::kill",
            timers_rs.get("SLOT_TIMER_KILL", true),
            "kill",
            None,
        ),
        // These two take no arguments, so `ret N` cannot tell them from their
        // neighbours; their names can.
        check(
            "components/Pawn.dll",
            "PawnComponent",
            "IPawnComponent::getEventDispatcher",
            server_rs.get("PAWN_COMPONENT_PREFIX_SLOTS", true),
            "getEventDispatcher",
            None,
        ),
        check(
            "components/Pawn.dll",
            "PawnComponent",
            "IPawnComponent::getAmxFunctions",
            server_rs
                .get("PAWN_COMPONENT_PREFIX_SLOTS", true)
                .map(|p| p + 1),
            "getAmxFunctions",
            None,
        ),
        check(
            "components/Pawn.dll",
            "PawnScript",
            "IPawnScript::GetAMX",
            Some(57),
            "GetAMX",
            None,
        ),
        check(
            "omp-server.exe",
            "Core",
            "ICore::getPlayers",
            players_rs.get("SLOT_GET_PLAYERS", true),
            "getPlayers",
            None,
        ),
        check(
            "omp-server.exe",
            "ComponentList",
            "IComponentList::queryComponent",
            server_rs.get("COMPONENT_LIST_PREFIX_SLOTS", true),
            "queryComponent",
            None,
        ),
    ];
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
        checks.push(check(
            "omp-server.exe",
            "PlayerPool",
            &format!("IPlayerPool::{method}"),
            players_rs.get(constant, true),
            method,
            None,
        ));
    }
    checks
}

/// Runs `checks`, opening each binary and each vtable once. A binary without
/// a `.pdb` leaves its checks to `ret N` and to a server run.
fn check_names(server: &Path, checks: &[NameCheck], report: &mut Report) -> Result<()> {
    let mut binaries: HashMap<&str, Windows> = HashMap::new();
    let mut vtables: HashMap<(&str, &str), Vec<u32>> = HashMap::new();
    for check in checks {
        if !binaries.contains_key(check.file) {
            binaries.insert(check.file, Windows::open(&server.join(check.file))?);
        }
        let binary = &binaries[check.file];
        let Some(symbols) = binary.symbols.as_ref() else {
            println!(
                "  note  no .pdb beside {}: {} is left to `ret N`",
                binary.name, check.label
            );
            continue;
        };
        let key = (check.file, check.class);
        if let std::collections::hash_map::Entry::Vacant(slot) = vtables.entry(key) {
            slot.insert(binary.vtable(check.class).map_err(anyhow::Error::msg)?);
        }
        let wanted = format!("::{}(", check.method);
        let holds = check
            .slot
            .and_then(|slot| vtables[&key].get(slot))
            .map(|pointer| {
                symbols.names_at(*pointer).iter().any(|name| {
                    name.contains(&wanted) && check.ending.is_none_or(|end| name.ends_with(end))
                })
            });
        report.check(&format!("{} (PDB)", check.label), Some(true), holds);
    }
    Ok(())
}

/// Every slot the generator wrote, against the vtable of the class that
/// implements it: by name — and by parameters for an overload, whose name
/// alone would pass either one — on Linux, and on Windows through the PDBs;
/// by `ret N` on Windows as well.
fn check_generated(
    repo: &Path,
    linux: Option<&Path>,
    win: Option<&Path>,
    report: &mut Report,
) -> Result<()> {
    let spec = crate::omp::spec::load(&repo.join("xtask/omp-wrappers.toml"))?;
    let generated = repo.join("crates/samp-sdk/src/omp/generated");
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
        let methods: Vec<&str> = signatures.iter().map(|s| method_name(s)).collect();
        let overloaded = |method: &str| methods.iter().filter(|m| **m == method).count() > 1;

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
                    for (slot, signature) in slots.iter().zip(&signatures) {
                        let method = method_name(signature);
                        let found = vtable.get(slot.itanium).map_or("", String::as_str);
                        let mut ok = found.contains(&format!("::{method}("));
                        if ok && overloaded(method) {
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
            let windows = Windows::open(&server.join(&file))?;
            match windows.vtable(class) {
                Err(why) => {
                    println!("  SKIP  MSVC: {why} — prove these with examples/omp-showcase");
                    report.skip(slots.len());
                }
                Ok(vtable) => {
                    for (slot, signature) in slots.iter().zip(&signatures) {
                        let method = method_name(signature);
                        let pointer = vtable.get(slot.msvc).copied();
                        let holds = pointer
                            .and_then(|p| windows.holds(p, method, signature, overloaded(method)));
                        if windows.symbols.is_some() {
                            report.check(
                                &format!("MSVC [{}] {method} (PDB)", slot.msvc),
                                Some(true),
                                holds,
                            );
                        }
                        let expected = bytes.of(signature);
                        let found = pointer.and_then(|p| windows.pe.ret_bytes(p));
                        report.check(
                            &format!("MSVC [{}] {method} pops {expected}", slot.msvc),
                            Some(expected),
                            found,
                        );
                    }
                }
            }
        }
    }
    Ok(())
}
