//! Formatting the generated files and writing them — or comparing them.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

/// The text as `cargo fmt` would leave it. The generated files live inside a
/// crate `cargo fmt` formats; written unformatted, the next `cargo fmt` would
/// rewrite them and `--check` report them stale forever after.
pub fn rustfmt(text: &str) -> Result<String> {
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("running rustfmt")?;
    child
        .stdin
        .take()
        .context("rustfmt's stdin")?
        .write_all(text.as_bytes())?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        bail!(
            "rustfmt rejected the generated code:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    String::from_utf8(output.stdout).context("rustfmt printed invalid UTF-8")
}

/// Writes the files, or with `check` compares them, and reports which differ.
pub struct Output {
    dir: PathBuf,
    check: bool,
    stale: Vec<String>,
}

impl Output {
    pub fn new(dir: &Path, check: bool) -> Self {
        Self {
            dir: dir.to_owned(),
            check,
            stale: Vec::new(),
        }
    }

    /// Formats `text` and writes it as `name`, returning what was written.
    pub fn file(&mut self, name: &str, text: &str) -> Result<String> {
        let text = rustfmt(text)?;
        let path = self.dir.join(name);
        if self.check {
            if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
                self.stale.push(name.to_owned());
            }
        } else {
            std::fs::create_dir_all(&self.dir)
                .with_context(|| format!("creating {}", self.dir.display()))?;
            std::fs::write(&path, &text).with_context(|| format!("writing {}", path.display()))?;
        }
        Ok(text)
    }

    /// Whether everything was current (always, when writing).
    pub fn finish(self) -> bool {
        if !self.check {
            return true;
        }
        if self.stale.is_empty() {
            println!("generated wrappers are current");
            true
        } else {
            println!("stale: {}", self.stale.join(", "));
            false
        }
    }
}
