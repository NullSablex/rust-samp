//! `cargo xtask vtable <Class>`: the slots of one open.mp interface, as clang
//! lays it out for each ABI.
//!
//! For writing a slot by hand, or checking one: the answer is what the
//! compiler would emit, which is what the server was built with — unless the
//! server was built from a different revision of the headers. Treat it as the
//! map, and a server run as the proof.

use std::path::PathBuf;

use anyhow::{Result, bail};

use super::mirror::snake;
use super::sdk::{Abi, Sdk, tempdir};
use super::vtables::{self, Slot, key_of};

pub struct Options {
    pub class: String,
    pub header: String,
    /// Print `slots!` entries instead of the table.
    pub rust: bool,
    /// Only methods whose signature contains this text.
    pub filter: Option<String>,
    pub sdk: Option<PathBuf>,
    pub xwin: Option<PathBuf>,
}

/// A signature with every `Class::` qualifier removed: what matches the same
/// method across the two dumps, which may attribute it differently.
fn unqualified(signature: &str) -> String {
    let mut out = String::with_capacity(signature.len());
    let mut word = String::new();
    let mut chars = signature.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_alphanumeric() || c == '_' {
            word.push(c);
        } else if c == ':' && chars.peek() == Some(&':') && !word.is_empty() {
            chars.next();
            word.clear();
        } else {
            out.push_str(&word);
            word.clear();
            out.push(c);
        }
    }
    out.push_str(&word);
    out
}

pub fn run(options: &Options) -> Result<bool> {
    let sdk = Sdk::locate(options.sdk.as_deref(), options.xwin.as_deref())?;
    let work = tempdir::Dir::new("omp-vtable")?;
    let headers = [options.header.as_str()];
    let classes = [options.class.as_str()];
    let table = |abi: Abi| -> Result<Vec<Slot>> {
        let mut indices = vtables::indices(&sdk, abi, &headers, &classes, work.path())?;
        Ok(indices.remove(&options.class).unwrap_or_default())
    };
    let itanium = table(Abi::Itanium)?;
    let msvc = table(Abi::Msvc)?;
    if itanium.is_empty() {
        bail!(
            "clang laid out no vtable for {} — is it declared in {}?",
            options.class,
            options.header
        );
    }

    let msvc_slot = |signature: &str| {
        let wanted = unqualified(signature);
        msvc.iter()
            .find(|slot| unqualified(&slot.signature) == wanted)
    };
    let shown: Vec<&Slot> = itanium
        .iter()
        .filter(|slot| {
            options
                .filter
                .as_ref()
                .is_none_or(|f| slot.signature.to_lowercase().contains(&f.to_lowercase()))
        })
        .collect();

    if options.rust {
        println!(
            "// Slots of `{}`, from `cargo xtask vtable --rust`: Itanium first, MSVC second.",
            options.class
        );
        println!("// Verify against a running server.");
        println!("slots! {{");
        let methods: Vec<&Slot> = shown
            .iter()
            .copied()
            .filter(|slot| !slot.signature.contains('~'))
            .collect();
        let name_of = |slot: &Slot| {
            key_of(&slot.signature)
                .split('(')
                .next()
                .unwrap_or_default()
                .to_owned()
        };
        for slot in &methods {
            let signature = &slot.signature;
            match msvc_slot(signature) {
                Some(other) if other.vfptr == 0 => {
                    let name = name_of(slot);
                    if methods.iter().filter(|m| name_of(m) == name).count() > 1 {
                        println!("    // Overloaded: give each `{name}` a name of its own.");
                    }
                    println!("    /// `{signature}`.");
                    println!(
                        "    SLOT_{}: usize = {}, {};",
                        snake(&name).to_uppercase(),
                        slot.index,
                        other.index
                    );
                }
                Some(other) => println!(
                    "    // {signature}: MSVC keeps it in the vtable at offset {} (slot {})",
                    other.vfptr, other.index
                ),
                None => println!("    // {signature}: not in the MSVC dump"),
            }
        }
        println!("}}");
    } else {
        println!("{}  (Itanium / MSVC)", options.class);
        for slot in shown {
            let other = msvc_slot(&slot.signature);
            let column = other.map_or_else(|| "-".to_owned(), |o| o.index.to_string());
            let note = other
                .filter(|o| o.vfptr != 0)
                .map(|o| format!("   (MSVC: in the vtable at offset {})", o.vfptr))
                .unwrap_or_default();
            println!(
                "  {:3} / {column:>3}   {}{note}",
                slot.index, slot.signature
            );
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualifiers_are_dropped() {
        assert_eq!(
            unqualified("const FlatPtrHashSet<IPlayer> &IPlayerPool::entries()"),
            "const FlatPtrHashSet<IPlayer> &entries()"
        );
        assert_eq!(unqualified("void std::chrono::X::f(int)"), "void f(int)");
    }
}
