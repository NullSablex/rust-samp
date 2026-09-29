//! The Itanium side: a Linux `.so` keeps its symbols, so every vtable slot
//! can be read back as the name of the function it points at.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use object::{Object, ObjectSegment, ObjectSymbol, SymbolKind};

/// A shared object's symbols and loadable segments.
pub struct Elf {
    data: Vec<u8>,
    /// Address -> mangled name. Where several symbols share an address, the
    /// one that sorts first by name, as `nm -n` lists it.
    names: HashMap<u64, String>,
    /// Mangled name -> address, for finding a vtable by its symbol.
    addresses: HashMap<String, u64>,
    /// `(address, size in memory, file offset)` of each loadable segment.
    segments: Vec<(u64, u64, u64)>,
}

impl Elf {
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let file =
            object::File::parse(&*data).with_context(|| format!("parsing {}", path.display()))?;
        let mut names: HashMap<u64, String> = HashMap::new();
        let mut addresses = HashMap::new();
        for symbol in file.symbols() {
            if !symbol.is_definition()
                || matches!(symbol.kind(), SymbolKind::Section | SymbolKind::File)
            {
                continue;
            }
            let Ok(name) = symbol.name() else { continue };
            if name.is_empty() {
                continue;
            }
            let address = symbol.address();
            let entry = names.entry(address).or_insert_with(|| name.to_owned());
            if name < entry.as_str() {
                name.clone_into(entry);
            }
            if name.starts_with("_ZTV") {
                addresses.insert(name.to_owned(), address);
            }
        }
        let segments = file
            .segments()
            .map(|segment| (segment.address(), segment.size(), segment.file_range().0))
            .collect();
        Ok(Self {
            data,
            names,
            addresses,
            segments,
        })
    }

    /// Demangled names of the first `count` slots of `vtable for <class>`.
    pub fn vtable(&self, class: &str, count: usize) -> Result<Vec<String>, String> {
        // `vtable for X`, for a class at global scope, is `_ZTV<length>X`.
        let symbol = format!("_ZTV{}{class}", class.len());
        let address = *self
            .addresses
            .get(&symbol)
            .ok_or_else(|| format!("no vtable for {class}"))?;
        let (segment, _, offset) = self
            .segments
            .iter()
            .find(|(start, size, _)| (*start..start + size).contains(&address))
            .ok_or_else(|| format!("vtable for {class} is outside every segment"))?;
        // `vtable for X` points at the start of the structure, whose first two
        // entries are offset-to-top and the typeinfo pointer. What an object's
        // vptr holds — slot 0 — starts 8 bytes later.
        let start = usize::try_from(offset + (address - segment) + 8).map_err(|e| e.to_string())?;
        Ok((0..count)
            .map(|i| {
                let at = start + 4 * i;
                let pointer = self
                    .data
                    .get(at..at + 4)
                    .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]));
                match self.names.get(&u64::from(pointer)) {
                    Some(name) => demangle(name),
                    None => format!("{pointer:#x}"),
                }
            })
            .collect())
    }
}

/// A symbol's C++ name, or the name as it is when it is not a mangled one.
fn demangle(name: &str) -> String {
    cpp_demangle::Symbol::new(name.as_bytes())
        .ok()
        .and_then(|symbol| symbol.demangle().ok())
        .unwrap_or_else(|| name.to_owned())
}

/// Index of the first slot whose name contains `needle`.
pub fn slot_of(vtable: &[String], needle: &str) -> Option<usize> {
    vtable.iter().position(|name| name.contains(needle))
}
