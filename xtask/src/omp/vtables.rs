//! Vtable slots and subobject offsets, as clang lays them out for each ABI.
//!
//! libclang exposes neither, so these come from `clang++` itself: one run per
//! ABI over a single file holding every interface, instead of one per
//! interface.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

use super::sdk::{Abi, Sdk};

/// `{class: {"name(types)": slot}}` for one ABI.
pub type SlotTable = HashMap<String, HashMap<String, usize>>;

/// One line of an "indices" block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub index: usize,
    /// Where the vtable pointer holding it sits in the object: 0 for the
    /// primary vtable. MSVC keeps an override of a secondary base's method
    /// only in that base's vtable (`getUID` at +56).
    pub vfptr: usize,
    pub signature: String,
}

/// Each class's "indices" block as clang prints it, in order, destructors
/// included.
pub type Indices = HashMap<String, Vec<Slot>>;

/// `setModel(int)` for `void IBaseObject::setModel(int)`: the method's name and
/// its parameters with the spacing removed — what matches a declaration to its
/// line in clang's dump.
pub fn key_of(signature: &str) -> String {
    let Some(open) = signature.find('(') else {
        return signature.to_owned();
    };
    let Some(close) = signature.rfind(')') else {
        return signature.to_owned();
    };
    let head = &signature[..open];
    let name_start = head
        .rfind(|c: char| !(c.is_alphanumeric() || c == '_'))
        .map_or(0, |i| i + 1);
    let params: String = signature[open + 1..close]
        .chars()
        .filter(|c| *c != ' ')
        .collect();
    format!("{}({params})", &head[name_start..])
}

/// The slot tables of every class in `classes`, for `abi`.
///
/// Each class gets a stub deriving from it with a constructor defined out of
/// line. Emitting that constructor makes clang lay out the stub's vtable — and
/// with it every base's — without the stub having to implement anything.
pub fn slot_tables(
    sdk: &Sdk,
    abi: Abi,
    headers: &[&str],
    classes: &[&str],
    work: &Path,
) -> Result<SlotTable> {
    let indices = indices(sdk, abi, headers, classes, work)?;
    // Destructors are left out: nothing calls them through a wrapper.
    Ok(indices
        .into_iter()
        .map(|(class, entries)| {
            let slots = entries
                .into_iter()
                .filter(|slot| slot.vfptr == 0 && !slot.signature.contains('~'))
                .map(|slot| (key_of(&slot.signature), slot.index))
                .collect();
            (class, slots)
        })
        .collect())
}

/// The indices blocks of every class in `classes` and its bases, for `abi`.
pub fn indices(
    sdk: &Sdk,
    abi: Abi,
    headers: &[&str],
    classes: &[&str],
    work: &Path,
) -> Result<Indices> {
    let mut source = includes(headers);
    for (i, class) in classes.iter().enumerate() {
        let _ = writeln!(
            source,
            "struct Stub{i} : {class} {{ Stub{i}(); }};\nStub{i}::Stub{i}() {{}}"
        );
    }
    let path = work.join(format!("stubs-{}.cpp", abi.triple()));
    std::fs::write(&path, source).with_context(|| format!("writing {}", path.display()))?;
    let dump = clang(
        sdk,
        abi,
        &path,
        &["-fdump-vtable-layouts", "-emit-llvm-only"],
    )?;
    Ok(parse_indices(&dump))
}

/// The "indices" blocks of a vtable dump: the slots as a caller indexes them.
/// The layout blocks above them also count RTTI and offset-to-top entries,
/// which no call goes through.
fn parse_indices(dump: &str) -> Indices {
    let mut tables = Indices::new();
    let mut lines = dump.lines();
    while let Some(line) = lines.next() {
        let Some(class) = line
            .strip_prefix("VTable indices for '")
            .or_else(|| line.strip_prefix("VFTable indices for '"))
            .and_then(|rest| rest.split_once('\'').map(|(class, _)| class))
        else {
            continue;
        };
        // A class printed twice (once per stub deriving from it) lists the
        // same slots; the first listing is kept.
        let mut vfptr = 0;
        let mut entries = Vec::new();
        for entry in lines.by_ref().take_while(|l| !l.is_empty()) {
            if let Some(offset) = entry
                .trim()
                .strip_prefix("-- accessible via vfptr at offset ")
            {
                vfptr = offset.trim_end_matches(" --").trim().parse().unwrap_or(0);
                continue;
            }
            let Some((index, method)) = entry.split_once(" | ") else {
                continue;
            };
            let Ok(index) = index.trim().parse() else {
                continue;
            };
            entries.push(Slot {
                index,
                vfptr,
                signature: method.trim().to_owned(),
            });
        }
        tables.entry(class.to_owned()).or_insert(entries);
    }
    tables
}

/// Where the `IComponent` subobject sits inside each class in `classes`.
pub fn component_offsets(
    sdk: &Sdk,
    abi: Abi,
    headers: &[&str],
    classes: &[String],
    work: &Path,
) -> Result<HashMap<String, usize>> {
    let mut source = includes(headers);
    for class in classes {
        let _ = writeln!(source, "static_assert(sizeof({class}) > 0, \"\");");
    }
    let path = work.join(format!("offsets-{}.cpp", abi.triple()));
    std::fs::write(&path, source).with_context(|| format!("writing {}", path.display()))?;
    let dump = clang(sdk, abi, &path, &["-fdump-record-layouts", "-fsyntax-only"])?;

    let mut offsets = HashMap::new();
    for class in classes {
        let header = format!("| struct {class}");
        let block = dump
            .split("*** Dumping AST Record Layout")
            .find(|block| {
                block
                    .lines()
                    .nth(1)
                    .is_some_and(|l| l.trim_end().ends_with(&header))
            })
            .with_context(|| format!("no record layout for {class} ({})", abi.triple()))?;
        let offset = block
            .lines()
            .find_map(|line| {
                let (offset, rest) = line.split_once(" | ")?;
                rest.trim_start()
                    .starts_with("struct IComponent (")
                    .then(|| offset.trim().parse().ok())?
            })
            .with_context(|| format!("no IComponent subobject in {class} ({})", abi.triple()))?;
        offsets.insert(class.clone(), offset);
    }
    Ok(offsets)
}

/// `#include <...>` for each header, one per line.
pub fn includes(headers: &[&str]) -> String {
    headers.iter().fold(String::new(), |mut out, header| {
        let _ = writeln!(out, "#include <{header}>");
        out
    })
}

/// Runs `clang++` over `source` for `abi` and returns what it printed.
fn clang(sdk: &Sdk, abi: Abi, source: &Path, dump: &[&str]) -> Result<String> {
    let mut command = Command::new("clang++");
    command
        .arg("-std=c++17")
        .arg(format!("--target={}", abi.triple()))
        .args(sdk.flags(abi))
        .arg("-w");
    for flag in dump {
        command.arg("-Xclang").arg(flag);
    }
    let output = command
        .arg("-c")
        .arg(source)
        .output()
        .context("running clang++")?;
    if !output.status.success() {
        bail!(
            "clang++ failed for {}:\n{}",
            abi.triple(),
            String::from_utf8_lossy(&output.stderr)
                .chars()
                .take(4000)
                .collect::<String>()
        );
    }
    String::from_utf8(output.stdout).context("clang++ printed invalid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_drop_the_class_and_the_spacing() {
        assert_eq!(key_of("void IBaseObject::setModel(int)"), "setModel(int)");
        assert_eq!(
            key_of("const FlatPtrHashSet<IPlayer> &IPlayerPool::bots()"),
            "bots()"
        );
        assert_eq!(
            key_of("IVehicle * IVehiclesComponent::create(const VehicleSpawnData &)"),
            "create(constVehicleSpawnData&)"
        );
        assert_eq!(key_of("StringView IPlayer::getName() const"), "getName()");
    }

    #[test]
    fn indices_are_read_block_by_block() {
        let dump = "\
VTable indices for 'IPlayerPool' (3 entries).
   4 | IPlayerPool::~IPlayerPool() [complete]
   6 | const FlatPtrHashSet<IPlayer> &IPlayerPool::entries()
   7 | const FlatPtrHashSet<IPlayer> &IPlayerPool::players()

VFTable indices for 'IActor' (1 entry).
   5 | void IActor::setSkin(int)

VFTable indices for 'ITimersComponent' (2 entries).
 -- accessible via vfptr at offset 0 --
  18 | const size_t ITimersComponent::count() const
 -- accessible via vfptr at offset 56 --
   0 | UID ITimersComponent::getUID()
";
        let tables = parse_indices(dump);
        assert_eq!(tables["IPlayerPool"][1].index, 6);
        assert_eq!(
            tables["IPlayerPool"][1].signature,
            "const FlatPtrHashSet<IPlayer> &IPlayerPool::entries()"
        );
        assert_eq!(tables["IPlayerPool"].len(), 3);
        assert_eq!(tables["IActor"][0].index, 5);
        assert_eq!(
            tables["ITimersComponent"][1],
            Slot {
                index: 0,
                vfptr: 56,
                signature: "UID ITimersComponent::getUID()".into()
            }
        );
    }
}
