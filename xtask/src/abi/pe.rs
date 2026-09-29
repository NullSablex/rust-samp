//! The MSVC side: a Windows `.dll`/`.exe` keeps no symbols, so a vtable is
//! found through its RTTI, and a slot is identified by how many bytes of
//! arguments its function pops (`ret N` under `thiscall`).

use std::path::Path;

use anyhow::{Context, Result, bail};
use iced_x86::{Decoder, DecoderOptions, Mnemonic, OpKind};

struct Section {
    name: String,
    address: u32,
    size: u32,
    raw: u32,
    raw_size: u32,
}

pub struct Pe {
    data: Vec<u8>,
    base: u32,
    sections: Vec<Section>,
}

/// How far a function is disassembled looking for its `ret`.
const WINDOW: usize = 0x600;

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

impl Pe {
    pub fn open(path: &Path) -> Result<Self> {
        let data = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let malformed = || format!("{} is not a PE32 image", path.display());
        let pe = u32_at(&data, 0x3C).with_context(malformed)? as usize;
        let count = u16_at(&data, pe + 6).with_context(malformed)?;
        let optional = u16_at(&data, pe + 20).with_context(malformed)? as usize;
        let base = u32_at(&data, pe + 24 + 28).with_context(malformed)?;
        let mut sections = Vec::new();
        for i in 0..usize::from(count) {
            let at = pe + 24 + optional + 40 * i;
            let name = data.get(at..at + 8).with_context(malformed)?;
            let name = String::from_utf8_lossy(name)
                .trim_end_matches('\0')
                .to_owned();
            sections.push(Section {
                name,
                size: u32_at(&data, at + 8).with_context(malformed)?,
                address: u32_at(&data, at + 12).with_context(malformed)?,
                raw_size: u32_at(&data, at + 16).with_context(malformed)?,
                raw: u32_at(&data, at + 20).with_context(malformed)?,
            });
        }
        if sections.is_empty() {
            bail!(malformed());
        }
        Ok(Self {
            data,
            base,
            sections,
        })
    }

    fn file_to_virtual(&self, offset: usize) -> Option<u32> {
        let offset = u32::try_from(offset).ok()?;
        self.sections
            .iter()
            .find(|s| (s.raw..s.raw + s.raw_size).contains(&offset))
            .map(|s| self.base + s.address + (offset - s.raw))
    }

    fn section_of(&self, virtual_address: u32) -> Option<&str> {
        let rva = virtual_address.checked_sub(self.base)?;
        self.sections
            .iter()
            .find(|s| (s.address..s.address + s.size).contains(&rva))
            .map(|s| s.name.as_str())
    }

    fn virtual_to_file(&self, virtual_address: u32) -> Option<usize> {
        let rva = virtual_address.checked_sub(self.base)?;
        self.sections
            .iter()
            .find(|s| {
                (s.address..s.address + s.size).contains(&rva) && rva - s.address < s.raw_size
            })
            .map(|s| (s.raw + (rva - s.address)) as usize)
    }

    pub fn base(&self) -> u32 {
        self.base
    }

    /// The function pointers of the vtable at `address`: every entry up to
    /// the first that does not point into `.text`.
    pub fn vtable_at(&self, address: u32) -> Vec<u32> {
        let Some(start) = self.virtual_to_file(address) else {
            return Vec::new();
        };
        (start..)
            .step_by(4)
            .map_while(|at| u32_at(&self.data, at))
            .take_while(|&pointer| pointer != 0 && self.section_of(pointer) == Some(".text"))
            .take(512)
            .collect()
    }

    /// The address and function pointers of the primary vtable of `class`,
    /// via RTTI: the type descriptor names the class, a complete object
    /// locator points at the descriptor, and the vtable follows a pointer to
    /// the locator.
    pub fn vtable(&self, class: &str) -> Result<(u32, Vec<u32>), String> {
        let marker = format!(".?AV{class}@@");
        let index = memchr::memmem::find(&self.data, marker.as_bytes())
            .ok_or_else(|| format!("no RTTI for {class}"))?;
        let descriptor = index
            .checked_sub(8)
            .and_then(|at| self.file_to_virtual(at))
            .ok_or("RTTI outside every section")?;

        let mut best: (u32, Vec<u32>) = (0, Vec::new());
        for found in memchr::memmem::find_iter(&self.data, &descriptor.to_le_bytes()) {
            let Some(locator) = found.checked_sub(12) else {
                continue;
            };
            // Only the primary vtable: signature 0, subobject offset 0.
            if u32_at(&self.data, locator) != Some(0) || u32_at(&self.data, locator + 4) != Some(0)
            {
                continue;
            }
            let Some(locator_address) = self.file_to_virtual(locator) else {
                continue;
            };
            for reference in memchr::memmem::find_iter(&self.data, &locator_address.to_le_bytes()) {
                let Some(address) = self.file_to_virtual(reference + 4) else {
                    continue;
                };
                let slots = self.vtable_at(address);
                if slots.len() > best.1.len() {
                    best = (address, slots);
                }
            }
        }
        if best.1.is_empty() {
            return Err(format!("vtable for {class} not located"));
        }
        Ok(best)
    }

    /// The bytes of arguments `function` pops: N from its first `ret N`, 0 for
    /// a bare `ret`. `None` when it tail-calls another function (`jmp` through
    /// a pointer) or reaches the `int3` padding first — past either, the next
    /// `ret` is another function's.
    pub fn ret_bytes(&self, function: u32) -> Option<usize> {
        let start = self.virtual_to_file(function)?;
        let end = (start + WINDOW).min(self.data.len());
        let mut decoder = Decoder::with_ip(
            32,
            &self.data[start..end],
            u64::from(function),
            DecoderOptions::NONE,
        );
        for instruction in &mut decoder {
            match instruction.mnemonic() {
                Mnemonic::Ret => {
                    return Some(if instruction.op_count() == 0 {
                        0
                    } else {
                        usize::from(instruction.immediate16())
                    });
                }
                Mnemonic::Jmp
                    if !matches!(
                        instruction.op0_kind(),
                        OpKind::NearBranch16 | OpKind::NearBranch32
                    ) =>
                {
                    return None;
                }
                Mnemonic::Int3 => return None,
                _ => {}
            }
        }
        None
    }
}
