//! Repository tooling, run as `cargo xtask <command>`.
//!
//! A host tool with its own workspace: the root workspace builds for i686,
//! and nothing here belongs in a plugin.

mod abi;
mod omp;
mod roundtrip;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "rust-samp repository tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate `samp-sdk/src/omp/generated/` from the open.mp SDK headers.
    GenOmp {
        /// Fail if the committed files differ from what would be generated.
        #[arg(long)]
        check: bool,
        /// The open.mp SDK checkout (default: `$OPENMP_SDK`, then the usual spots).
        #[arg(long)]
        sdk: Option<PathBuf>,
        /// The Windows headers cargo-xwin downloads (default: `$XWIN_CACHE`).
        #[arg(long)]
        xwin: Option<PathBuf>,
    },
    /// Check the SDK's vtable slots against the official server binaries.
    CheckAbi {
        /// The unpacked Linux server (default: `$OPENMP_LINUX_SERVER`, then the usual spots).
        #[arg(long)]
        linux: Option<PathBuf>,
        /// The unpacked Windows server (default: `$OPENMP_WIN_SERVER`, then the usual spots).
        #[arg(long)]
        win: Option<PathBuf>,
        /// Also check every slot in `samp-sdk/src/omp/generated/`.
        #[arg(long)]
        generated: bool,
    },
    /// Print the vtable slots of one open.mp interface, for both ABIs.
    Vtable {
        /// The interface, e.g. `IPlayerPool`.
        class: String,
        /// The SDK header declaring it.
        #[arg(long, default_value = "player.hpp")]
        header: String,
        /// Print `slots!` entries instead of the table.
        #[arg(long)]
        rust: bool,
        /// Only methods whose signature contains this text.
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        sdk: Option<PathBuf>,
        #[arg(long)]
        xwin: Option<PathBuf>,
    },
    /// Generate the showcase's set/get round trips from the generated wrappers.
    Roundtrip {
        /// Fail if the committed file differs from what would be generated.
        #[arg(long)]
        check: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::GenOmp { check, sdk, xwin } => omp::run(&omp::Options { check, sdk, xwin }),
        Command::CheckAbi {
            linux,
            win,
            generated,
        } => abi::run(&abi::Options {
            linux,
            win,
            generated,
        }),
        Command::Roundtrip { check } => roundtrip::run(check),
        Command::Vtable {
            class,
            header,
            rust,
            filter,
            sdk,
            xwin,
        } => omp::inspect::run(&omp::inspect::Options {
            class,
            header,
            rust,
            filter,
            sdk,
            xwin,
        }),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
