//! The MSVC side by name: the official Windows server ships a `.pdb` beside
//! every binary, whose public symbols name each vtable and each function.
//! With them a slot is checked by the method it holds, as on Linux, instead
//! of only by the bytes its function pops.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use pdb::FallibleIterator;

pub struct Symbols {
    /// Virtual address -> every function name at it. MSVC folds identical
    /// functions (`/OPT:ICF`) into one, which then carries several names.
    functions: HashMap<u32, Vec<String>>,
    /// Class -> its vftables: the base each one is for (`None` for a class
    /// with a single one) and its virtual address.
    vftables: HashMap<String, Vec<(Option<String>, u32)>>,
}

/// `??_7Class@@6B@` or `??_7Class@@6BBase@@@`: the class and the base, for a
/// class and base at global scope.
fn vftable_name(decorated: &str) -> Option<(String, Option<String>)> {
    let rest = decorated.strip_prefix("??_7")?;
    let (class, tail) = rest.split_once("@@6B")?;
    if class.contains('@') {
        return None;
    }
    let base = if tail == "@" {
        None
    } else {
        let base = tail.strip_suffix("@@@")?;
        if base.contains('@') {
            return None;
        }
        Some(base.to_owned())
    };
    Some((class.to_owned(), base))
}

impl Symbols {
    /// The `.pdb` beside `binary`, if there is one.
    pub fn beside(binary: &Path, image_base: u32) -> Result<Option<Self>> {
        let path = binary.with_extension("pdb");
        if !path.is_file() {
            return Ok(None);
        }
        let file =
            std::fs::File::open(&path).with_context(|| format!("opening {}", path.display()))?;
        let mut pdb =
            pdb::PDB::open(file).with_context(|| format!("reading {}", path.display()))?;
        let map = pdb.address_map()?;
        let globals = pdb.global_symbols()?;
        let mut symbols = Self {
            functions: HashMap::new(),
            vftables: HashMap::new(),
        };
        let mut iter = globals.iter();
        while let Some(symbol) = iter.next()? {
            let Ok(pdb::SymbolData::Public(public)) = symbol.parse() else {
                continue;
            };
            let Some(rva) = public.offset.to_rva(&map) else {
                continue;
            };
            let address = image_base + rva.0;
            let name = public.name.to_string();
            if public.function {
                let readable =
                    msvc_demangler::demangle(&name, msvc_demangler::DemangleFlags::llvm())
                        .unwrap_or_else(|_| name.to_string());
                symbols.functions.entry(address).or_default().push(readable);
            } else if let Some((class, base)) = vftable_name(&name) {
                symbols
                    .vftables
                    .entry(class)
                    .or_default()
                    .push((base, address));
            }
        }
        Ok(Some(symbols))
    }

    /// The address of `class`'s primary vftable: the one for `IExtensible`,
    /// the root of every open.mp interface's primary chain, or the class's
    /// only one. `None` when neither says which it is.
    pub fn primary_vftable(&self, class: &str) -> Option<u32> {
        let tables = self.vftables.get(class)?;
        if let Some((_, address)) = tables
            .iter()
            .find(|(base, _)| base.as_deref() == Some("IExtensible"))
        {
            return Some(*address);
        }
        match tables.as_slice() {
            [(None, address)] => Some(*address),
            _ => None,
        }
    }

    /// Every name the function at `address` goes by.
    pub fn names_at(&self, address: u32) -> &[String] {
        self.functions.get(&address).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vftable_names_give_class_and_base() {
        assert_eq!(
            vftable_name("??_7TimersComponent@@6B@"),
            Some(("TimersComponent".into(), None))
        );
        assert_eq!(
            vftable_name("??_7Player@@6BIExtensible@@@"),
            Some(("Player".into(), Some("IExtensible".into())))
        );
        assert_eq!(vftable_name("??_7exception@std@@6B@"), None);
    }
}
