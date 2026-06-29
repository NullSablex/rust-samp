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
