# Contributing to rust-samp

Thanks for your interest in contributing! Please read this guide before
opening an issue or a pull request.

## Before you start

- Check whether an [issue](https://github.com/NullSablex/rust-samp/issues)
  already exists for the problem or feature.
- For significant changes, open an issue first to discuss the approach
  before implementing.
- By contributing, you agree that your code will be licensed under the
  same terms as the [project license](LICENSE) (MIT).

## Setting up the environment

**Prerequisites:**
- Rust stable (`rustup install stable`), MSRV **1.88**.
- The **i686** targets — SA-MP and open.mp are 32-bit:

```bash
rustup target add i686-unknown-linux-gnu
# Windows cross-compile from Linux (optional):
cargo install cargo-xwin
rustup target add i686-pc-windows-msvc
```

```bash
git clone https://github.com/NullSablex/rust-samp
cd rust-samp
cargo build --target i686-unknown-linux-gnu
```

**Run the tests:**
```bash
cargo test --target i686-unknown-linux-gnu
```

**Lint (required before a PR):**
```bash
cargo clippy --all-targets --target i686-unknown-linux-gnu -- -D warnings
cargo fmt --check
```

**Windows (MSVC ABI, full open.mp support) — cross-compiled from Linux:**
```bash
cargo xwin build --xwin-arch x86 --target i686-pc-windows-msvc
```

## Project structure

A Cargo workspace of crates:

```
samp/          ← main crate, re-exports sdk + codegen (lib name: samp)
samp-sdk/      ← FFI bindings to the AMX VM and the open.mp component ABI
samp-codegen/  ← proc macros (#[native], initialize_plugin!, derive SampPlugin)
examples/      ← hello / counter / advanced / sink-demo plugins
docs/          ← MkDocs (Material) sources, published to GitHub Pages
```

See [CLAUDE.md](CLAUDE.md) for the architecture notes (open.mp ABI,
feature flags, conventions).

## Code rules

- Edition 2024, default `rustfmt` (no custom config).
- No `static mut` — use `AtomicPtr` with `Ordering::Acquire`/`Release`.
- Use `#[unsafe(no_mangle)]` (required by edition 2024).
- Comments and all project content (including docs) in **English**; use
  `//` / `///`, never `/* */`.
- `cargo clippy -- -D warnings` must pass — required.
- FFI casts annotated with `#[allow(...)]` must carry a comment explaining
  the intent.
- For non-trivial refactors, plan first: list the cases
  (refactor / keep / unsure) with rationale before writing code.

## Mapping a new open.mp interface

Before writing a wrapper for an interface, ask the compiler where its methods
land. `scripts/omp-vtable.py` generates a stub deriving from the interface,
compiles it for both ABIs with `-fdump-vtable-layouts`, and prints the slot each
method occupies:

```sh
scripts/omp-vtable.py IPlayerPool
scripts/omp-vtable.py IPlayer --header player.hpp
```

```text
IPlayerPool  (Itanium / MSVC)
    6 /   5   const FlatPtrHashSet<IPlayer> &IPlayerPool::entries()
   10 /   9   IEventDispatcher<PlayerConnectEventHandler> &IPlayerPool::getPlayerConnectDispatcher()
```

This is what made the MSVC side tractable: the Windows server carries RTTI for
three classes and nothing else, so those indices used to be derived by hand from
the ABI rules. Now the compiler answers both.

It needs clang, the open.mp SDK sources (path in the script) and, for the MSVC
column, the Windows headers `cargo xwin` downloads on its first build.

**`--rust` emits the constants themselves, so adding an interface is mostly
mechanical:

```sh
scripts/omp-vtable.py IPickupsComponent \
    --header Server/Components/Pickups/pickups.hpp --rust --filter "create(int"
```

**It is the map, not the proof.**** The server may have been built from a
different revision of the headers, and a layout that compiles is not a layout
the running server agrees with. Still validate against a server — see the
testing notes in `CLAUDE.md`.

## Checking the ABI constants

The SDK hardcodes vtable slot indices — each one a claim about a binary someone
else built. A wrong index fails quietly: the server returns a plausible value
from the wrong virtual function, and on Windows the stack is corrupted on top of
that. Four such defects shipped in v3.5.0.

```sh
scripts/check-abi-slots.py                        # default install paths
scripts/check-abi-slots.py --linux DIR --win DIR  # servers elsewhere
```

It re-derives every index from the official `Timers.so`, `Pawn.so`, `Timers.dll`
and `omp-server`, and fails when the source disagrees. Run it after touching
anything under `samp-sdk/src/omp/`, and before a release. Not part of CI: it
needs the official servers, which cannot be redistributed.

## Fuzzing

`samp_sdk::debug::AmxDbg::parse` reads bytes the plugin did not produce — the
debug block of a `.amx` file on the server's disk. Its contract is that no input
panics or hangs it: a malformed block must come back as `DbgError`.

```sh
cargo install cargo-fuzz                                    # once
mkdir -p fuzz/corpus/parse_debug
cp fuzz/seeds/parse_debug/* fuzz/corpus/parse_debug/        # once
cd fuzz && cargo +nightly fuzz run parse_debug -- -max_total_time=120
```

`fuzz/seeds/` holds real debug blocks taken from compiled `.amx` files and is
versioned; the corpus a run grows from them is not, since it is machine-local
and large. `cargo fuzz cmin parse_debug` shrinks it after a long session. A
crash lands in `fuzz/artifacts/` — attach that file to the issue.

Touching a parser that reads external bytes? Run this before opening the pull
request.

## Opening a pull request

1. Branch off `master`: `git checkout -b feat/my-feature`
2. Make your changes following the rules above.
3. Make sure `cargo clippy --target i686-unknown-linux-gnu -- -D warnings`
   and `cargo test --target i686-unknown-linux-gnu` pass.
4. Open the PR with a clear description of what changed and why.
5. Commit messages are in **English**; do not add `Co-Authored-By` or any
   AI-attribution trailers.

## Reporting bugs

Include in the issue:
- SDK version (release tag or commit hash) and the target you built for.
- Operating system and server (SA-MP / open.mp).
- A minimal example (Rust native + Pawn snippet) that reproduces it.
- The observed behavior versus what you expected.

## Feature suggestions

Open an issue with the `enhancement` label describing:
- The problem the feature would solve.
- How you imagine it working in the SDK.
- Alternatives you considered.
