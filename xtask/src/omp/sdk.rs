//! Where the open.mp SDK and the Windows headers are, and how clang is told.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

/// The two C++ ABIs the server is built for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Abi {
    /// Linux, GCC: `i686-pc-linux-gnu`.
    Itanium,
    /// Windows, MSVC: `i686-pc-windows-msvc`.
    Msvc,
}

impl Abi {
    pub const BOTH: [Abi; 2] = [Abi::Itanium, Abi::Msvc];

    pub fn triple(self) -> &'static str {
        match self {
            Abi::Itanium => "i686-pc-linux-gnu",
            Abi::Msvc => "i686-pc-windows-msvc",
        }
    }
}

/// What the SDK's `CMakeLists.txt` defines for every consumer, the server
/// included: they decide the quaternion's field order and which `string_view`
/// and `span` the headers use.
const DEFINES: [&str; 5] = [
    "-DGLM_FORCE_QUAT_DATA_WXYZ",
    "-DGLM_FORCE_SSE2",
    "-DNOMINMAX",
    "-Dnssv_CONFIG_SELECT_STRING_VIEW=nssv_STRING_VIEW_NONSTD",
    "-Dspan_CONFIG_SELECT_SPAN=span_SPAN_NONSTD",
];

/// The headers to read and how to compile them for each ABI.
pub struct Sdk {
    root: PathBuf,
    xwin: PathBuf,
    /// Case-corrected links into the Windows headers: the SDK includes
    /// `Winsock2.h`, the kit ships `winsock2.h`, and the filesystem is
    /// case-sensitive. Removed with the `Sdk`.
    casefix: tempdir::Dir,
}

impl Sdk {
    pub fn locate(sdk: Option<&Path>, xwin: Option<&Path>) -> Result<Self> {
        let root = find_sdk(sdk)?;
        let xwin = find_xwin(xwin)?;
        let casefix = tempdir::Dir::new("omp-casefix")?;
        let target = xwin.join("sdk/include/um/winsock2.h");
        if target.exists() {
            std::os::unix::fs::symlink(&target, casefix.path().join("Winsock2.h"))
                .context("linking Winsock2.h")?;
        }
        Ok(Self {
            root,
            xwin,
            casefix,
        })
    }

    /// The directory holding the headers, as `#include <...>` resolves them.
    pub fn include_dir(&self) -> PathBuf {
        self.root.join("include")
    }

    /// Everything clang needs besides the target: defines, include paths and,
    /// for MSVC, the Windows headers.
    pub fn flags(&self, abi: Abi) -> Vec<String> {
        let lib = self.root.join("lib");
        let mut flags: Vec<String> = DEFINES.iter().map(|d| (*d).to_owned()).collect();
        for dir in [
            self.root.join("include"),
            lib.join("glm"),
            lib.join("robin-hood-hashing/src/include"),
            lib.join("span-lite/include"),
            lib.join("string-view-lite/include"),
        ] {
            flags.push(format!("-I{}", dir.display()));
        }
        if abi == Abi::Msvc {
            flags.push(format!("-I{}", self.casefix.path().display()));
            for dir in [
                "crt/include",
                "sdk/include/ucrt",
                "sdk/include/um",
                "sdk/include/shared",
            ] {
                flags.push(format!("-isystem{}", self.xwin.join(dir).display()));
            }
            flags.push("-fms-compatibility".to_owned());
            flags.push("-fms-extensions".to_owned());
        }
        flags
    }
}

/// A checkout with the headers and the vendored libraries they include. The
/// libraries are git submodules; without them clang finds no vtable, an error
/// far from its cause.
fn check_sdk(path: &Path) -> Result<()> {
    if !path.join("include/player.hpp").is_file() {
        bail!(
            "{} has no include/player.hpp — is it an open.mp SDK checkout?",
            path.display()
        );
    }
    if !path.join("lib/glm/glm/vec2.hpp").is_file() {
        bail!(
            "{0} is missing its vendored libraries (lib/glm and friends).\n  git -C {0} submodule update --init --recursive",
            path.display()
        );
    }
    Ok(())
}

fn find_sdk(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        check_sdk(path)?;
        return Ok(path.to_owned());
    }
    if let Some(env) = std::env::var_os("OPENMP_SDK") {
        let path = PathBuf::from(env);
        check_sdk(&path).context("$OPENMP_SDK")?;
        return Ok(path);
    }
    let home = home()?;
    let candidates = [
        PathBuf::from("open.mp-sdk"),
        PathBuf::from("../open.mp-sdk"),
        home.join("open.mp-sdk"),
        home.join("src/open.mp-sdk"),
    ];
    candidates.into_iter().find(|path| check_sdk(path).is_ok()).context(
        "no open.mp SDK found. Pass --sdk, set $OPENMP_SDK, or clone one:\n  git clone --recursive https://github.com/openmultiplayer/open.mp-sdk",
    )
}

fn find_xwin(explicit: Option<&Path>) -> Result<PathBuf> {
    let usable = |path: &Path| path.join("crt/include").is_dir();
    if let Some(path) = explicit {
        return usable(path)
            .then(|| path.to_owned())
            .context("--xwin holds no crt/include");
    }
    if let Some(env) = std::env::var_os("XWIN_CACHE") {
        let path = PathBuf::from(env);
        return usable(&path)
            .then_some(path)
            .context("$XWIN_CACHE holds no crt/include");
    }
    let cache = match std::env::var_os("XDG_CACHE_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => home()?.join(".cache"),
    };
    let path = cache.join("cargo-xwin/xwin");
    usable(&path)
        .then_some(path)
        .context("the MSVC column needs the Windows headers: run `cargo xwin build` once")
}

fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("$HOME is not set")
}

/// A temporary directory, removed on drop.
pub mod tempdir {
    use std::path::{Path, PathBuf};

    use anyhow::{Context, Result};

    pub struct Dir(PathBuf);

    impl Dir {
        pub fn new(name: &str) -> Result<Self> {
            let path = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
            if path.exists() {
                std::fs::remove_dir_all(&path)
                    .with_context(|| format!("clearing {}", path.display()))?;
            }
            std::fs::create_dir_all(&path)
                .with_context(|| format!("creating {}", path.display()))?;
            Ok(Self(path))
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Dir {
        fn drop(&mut self) {
            // Best effort: a leftover temporary directory is harmless.
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
