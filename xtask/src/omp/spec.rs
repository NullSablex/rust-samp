//! `xtask/omp-wrappers.toml`: the interfaces to wrap.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, bail};
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
    check(&file.interface).with_context(|| format!("checking {}", path.display()))?;
    Ok(file.interface)
}

/// What the parser cannot see: names that become Rust identifiers and file
/// names, and two entries that would write over each other.
fn check(interfaces: &[Interface]) -> Result<()> {
    let identifier = |s: &str| {
        s.chars()
            .next()
            .is_some_and(|c| c.is_ascii_lowercase() || c == '_')
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    };
    let mut classes = std::collections::HashSet::new();
    let mut modules = std::collections::HashSet::new();
    for entry in interfaces {
        let names = [&entry.prefix, &entry.module]
            .into_iter()
            .chain(&entry.this)
            .chain(entry.overloads.values());
        if let Some(bad) = names.into_iter().find(|name| !identifier(name)) {
            bail!("{}: `{bad}` is not a snake_case identifier", entry.class);
        }
        if !classes.insert(&entry.class) {
            bail!("{} is listed twice", entry.class);
        }
        if !modules.insert(&entry.module) {
            bail!(
                "{}: module `{}` is already taken",
                entry.class,
                entry.module
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<Vec<Interface>> {
        let file: File = toml::from_str(text)?;
        check(&file.interface)?;
        Ok(file.interface)
    }

    const ENTRY: &str = r#"
[[interface]]
class = "IA"
header = "a.hpp"
handle = "IA"
prefix = "a"
module = "a"
impl = "A"
library = "A"
"#;

    #[test]
    fn a_valid_entry_passes() {
        assert!(parse(ENTRY).is_ok());
    }

    #[test]
    fn duplicates_and_bad_names_are_refused() {
        let twice = format!("{ENTRY}{ENTRY}");
        assert!(parse(&twice).is_err(), "the same class twice");
        let same_module = format!("{ENTRY}{}", ENTRY.replace("\"IA\"", "\"IB\""));
        assert!(parse(&same_module).is_err(), "two classes, one module");
        let traversal = ENTRY.replace("module = \"a\"", "module = \"../../x\"");
        assert!(parse(&traversal).is_err(), "a module that is a path");
        let overload = ENTRY.replace(
            "library = \"A\"",
            "library = \"A\"\noverloads = { \"f(int)\" = \"f-int\" }",
        );
        assert!(
            parse(&overload).is_err(),
            "an overload name that is not an identifier"
        );
    }
}
