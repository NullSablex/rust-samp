//! `xtask/omp-wrappers.toml`: the interfaces to wrap.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Deserialize)]
struct File {
    interface: Vec<Interface>,
}

/// One interface to wrap, one generated module.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Interface {
    /// The interface whose methods are wrapped, with its primary bases.
    pub class: String,
    /// The SDK header declaring it.
    pub header: String,
    /// The Rust handle the wrappers take.
    pub handle: String,
    /// The functions' prefix: `object` gives `object_set_model`.
    pub prefix: String,
    /// The generated file's name.
    pub module: String,
    /// The handle argument's name, when not the prefix.
    this: Option<String>,
    /// The class implementing it in the official binaries.
    #[serde(rename = "impl")]
    pub implementor: String,
    /// The binary holding that class.
    pub library: String,
    /// Methods a hand-written wrapper covers under another name.
    #[serde(default)]
    pub skip: Vec<String>,
    /// `method(ParamTypes)` -> the name each overload is wrapped under.
    #[serde(default)]
    pub overloads: BTreeMap<String, String>,
}

impl Interface {
    /// The handle argument's name.
    pub fn this(&self) -> &str {
        self.this.as_deref().unwrap_or(&self.prefix)
    }
}

pub fn load(path: &Path) -> Result<Vec<Interface>> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let file: File =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    Ok(file.interface)
}
