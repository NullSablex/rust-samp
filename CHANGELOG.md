# Changelog

Current release only. Previous releases are split per major line under
[`changelog/`](changelog/) — see [`changelog/index.md`](changelog/index.md)
for the full directory.

## [v3.0.0] — 2026/05/17

Compared to **v2.2.0** (2026/03/15).

Headline: native Open Multiplayer component ABI implemented in pure
Rust (Itanium **and** MSVC); a single binary now works as a SA-MP
plugin and as a first-class Open Multiplayer component, with no extra
configuration.

### Migrating from v2.x (or from the upstream `samp-rs` fork)

This release contains **breaking changes**. A plugin written against
v2.x will not compile against v3.0.0 without edits. Minimum diff:

| Before (v2.x / upstream)              | After (v3.0.0)                                                          |
| ------------------------------------- | ----------------------------------------------------------------------- |
| `fn process_tick(&mut self) { … }`    | `fn on_tick(&mut self, _ctx: TickContext) { … }`                        |
| `samp::plugin::enable_process_tick()` | `samp::plugin::enable_tick()` (or `enable_tick_with(TickConfig)`)       |
| `samp::cell::string::put_in_buffer(buf, s)?` | `buf.write_str(s)?` / `unsized.write_str(size, s)?` (`put_in_buffer` is `pub(crate)` now) |
| `samp::raw::functions::Logprintf` (variadic) | Same path, now `extern "C" fn(*const i8)` — the SDK formats in Rust and passes a single C string |
| `example-hello/` / `example-counter/` / `plugin-example/` | `examples/hello/` / `examples/counter/` / `examples/advanced/` |

Open Multiplayer support comes turned on by default — the build
produces both the SA-MP exports **and** the `ComponentEntryPoint`. To
keep the v2.x behavior unchanged, enable the new `samp-only` feature:

```toml
samp = { git = "...", tag = "v3.0.0", features = ["samp-only"] }
```

Two new requirements that affect builds:

- The workspace's `[profile.release]` adds `lto = "thin"`,
  `codegen-units = 1`, `strip = true`. Override in your own
  `Cargo.toml` if you need otherwise.
- The i686 target is now strictly required at compile time
  (`OmpComponent` has `const _` layout asserts). Set
  `target = "i686-unknown-linux-gnu"` in `.cargo/config.toml` if you
  were relying on the host target picking up automatically.

The full step-by-step walkthrough lives in
[`docs/migration.md`](docs/migration.md) — including the new
`TickConfig` knobs (`sa_mp_only()`, `omp_only(Duration)`, custom
`omp_interval`) and how to choose between them.

### Crate versions

- `samp`: 2.2.0 → 3.0.0
- `samp-sdk`: 2.2.0 → 3.0.0
- `samp-codegen`: 1.2.0 → 1.3.0

### Breaking changes

- **`SampPlugin::process_tick`** replaced by
  **`SampPlugin::on_tick(&mut self, ctx: TickContext)`** — unified
  callback that fires on both servers. Cadence is the server's main
  loop on SA-MP and the SDK-owned `ITimersComponent` timer on native
  Open Multiplayer (interval configurable). `TickContext::source`
  reports the origin (`TickSource::SaMp` /
  `TickSource::OmpTimer`); `TickContext::elapsed` is the
  wall-clock interval since the previous dispatch.
- **`samp::plugin::enable_process_tick`** replaced by
  **`samp::plugin::enable_tick()`** (default config) and
  **`samp::plugin::enable_tick_with(config: TickConfig)`** (explicit
  per-server toggle + Open Multiplayer interval).
- **`samp::cell::string::put_in_buffer`** is now `pub(crate)` (was
  `pub`). The public API for writing strings is `Buffer::write_str`
  and `UnsizedBuffer::write_str`.
- **`samp_sdk::raw::functions::Logprintf`** signature changed from
  variadic `extern "C" fn(*const i8, ...)` to fixed-arity
  `extern "C" fn(*const i8)` — the SDK formats the message in Rust
  and passes a single C string, matching what `logprintf("%s", msg)`
  does at the ABI level.
- Example crates renamed: `example-hello/` → `examples/hello/`,
  `example-counter/` → `examples/counter/`,
  `plugin-example/` → `examples/advanced/`. Workspace members
  updated accordingly.

### Added — native Open Multiplayer support

- **`samp_sdk::omp`** — new top-level module with eight submodules:
  - `component` — `OmpComponent`, `IComponentVTable`,
    `IUIDProviderVTable`, opaque types (`ICore`, `IComponentList`,
    `ILogger`, `IEarlyConfig`), default vtable implementations for
    both ABIs.
  - `component_api` — `OmpComponentHandle` trait, generic
    `component_name<T>()` / `component_version<T>()` helpers.
  - `core` — `LogLevel`, `core_print_ln`, `core_log_ln`,
    `core_print_ln_u8`, `core_log_ln_u8`.
  - `events` — `PawnEventHandler`, `PawnEventHandlerVTable`.
  - `server` — `PAWN_COMPONENT_UID`, `NUM_AMX_FUNCS`,
    `PawnComponent`, `ServerComponentList`, `ServerComponent`,
    `ServerPawnComponent`, `IEventDispatcherPawn`, `IPawnScript`,
    `AmxFunctionTable`, plus the free functions
    `query_component`, `add_pawn_event_handler`,
    `remove_pawn_event_handler`, `get_pawn_event_dispatcher`,
    `get_amx_from_script`, `get_amx_functions`.
  - `timers` — `TIMERS_COMPONENT_UID`, `TimersComponent`,
    `ITimersComponent`, `ITimer`, `TimerHandlerVTable`,
    `TimerTimeOutHandler`, `create_repeating_timer`, `kill_timer`,
    `query_timers_component`.
  - `types` — `UID`, `SemanticVersion` (with `new` and
    `with_prerel`), `StringView` (with `as_str`, `try_as_str`,
    `from_static`), `Colour` (with `rgb`, `rgba`, `from_rgba_u32`,
    `to_rgba_u32` and the `WHITE`, `BLACK`, `NONE` constants),
    `Vector2`, `Vector3`, `Vector4`, `ComponentType`.
  - `vtable` — `subobject_ptr`, `vtable_slot`,
    `secondary_call_target` helpers for safe access to secondary
    vtables.
- **`samp::omp`** re-exports the module above.
- **`samp::plugin`** new functions: `enable_tick`,
  `enable_tick_with`, `omp_core`, `omp_query_component`,
  `omp_query::<T>` (typed wrapper for `OmpComponentHandle`
  implementors).
- **`samp::plugin`** new types: `TickConfig`, `TickContext`,
  `TickSource`.
- **`SampPlugin`** new hooks: `on_tick(ctx)`,
  `on_omp_ready` (gated by `not(feature = "samp-only")`),
  `on_component_free` (same gating).
- **`samp::log`** re-export — `#[native]`-expanded code now uses
  `samp::log::error!`, so user crates no longer need to declare
  `log` as a direct dependency just to satisfy the macro.

### Added — `initialize_plugin!` extensions

- Optional metadata fields: `uid: <u64 expression>`,
  `component_name: "..."`, `component_version: (x, y, z)`.
- `samp-codegen` reads `[package.metadata.samp]` from the project's
  `Cargo.toml`. Resolution order per field:
  **macro argument > `[package.metadata.samp]` > derived value**
  (`CARGO_PKG_NAME`, parsed `CARGO_PKG_VERSION`, FNV-1a 64 of
  `CARGO_PKG_NAME@CARGO_PKG_VERSION` for the UID).
- When the UID is missing from both sources, the generated value is
  **persisted back** into `Cargo.toml` under `[package.metadata.samp]`
  so subsequent builds reuse the same identifier.
- Generates the SA-MP exports (`Load`, `Unload`, `Supports`,
  `AmxLoad`, `AmxUnload`, `ProcessTick`) **and** the Open Multiplayer
  `ComponentEntryPoint` by default. Opt out with the `samp-only`
  feature.

### Added — `#[native]` extensions

- Accepts **associated functions** (no `self`), in addition to
  methods.
- Return type detection: `Result` / `AmxResult` is matched against
  `Ok`/`Err`; any other type implementing `AmxCell` is used as the
  return cell directly (no spurious `Ok(...)` wrapping).
- Accepts `&AmxString` (and any other `&T`) parameters — the macro
  materializes the owned value from `args.next_arg()` and injects
  `&local` at the call site.
- Validates the `name = "..."` literal at proc-macro time:
  interior `\0` bytes now produce a compile error instead of
  panicking at `CString::new` during server load.
- Wraps every invocation in `std::panic::catch_unwind`. Panics that
  would otherwise cross the `extern "C"` boundary (process abort on
  Rust 1.71+) are caught, logged as
  `[<NativeName>] panic in native: <payload>`, and converted to a
  `0` return.
- Argument parsing failures now log
  `[<NativeName>] failed to parse argument #<i> '<name>' (expected type: <Type>)`
  — both the positional index and the expected type are included.

### Added — features and build infrastructure

- New `samp-only` feature on both `samp` and `samp-sdk`: removes
  every Open Multiplayer code path. The plugin still loads on Open
  Multiplayer, but in legacy mode (no component API).
- New workspace `[profile.release]`: `lto = "thin"`,
  `codegen-units = 1`, `strip = true`.
- `Cargo.lock` is now committed (removed from `.gitignore`).
- `Cargo.toml` per crate exposes
  `package.metadata.docs.rs.default-target = "i686-pc-windows-msvc"`
  + `features = ["encoding"]` so docs.rs builds with the right
  target.
- New build scripts:
  - `scripts/build-linux.sh` — produces `.so`
    (`i686-unknown-linux-gnu`) and `.dll`
    (`i686-pc-windows-msvc` via `cargo-xwin --xwin-arch x86`, or
    `i686-pc-windows-gnu` with `--samp-only`).
  - `scripts/build-windows.sh` — produces `.dll` natively and `.so`
    through WSL or Docker/cross (autodetected; forceable with
    `--wsl` / `--docker`).
- New helper scripts used by the benchmark workflow:
  `scripts/append-bench-history.py`, `scripts/extract-bench.py`,
  `scripts/render-bench-entry.py`.
- New GitHub workflows: `docs.yml` (publishes the MkDocs site),
  `release.yml` (creates releases on `v*` tags, attaches a source
  tarball with only the essential crates), `release-drafter.yml`,
  `labels.yml`, `bench-release.yml` (per-release benchmark history
  on the `bench-data` branch).
- `.github/labels.yml` and `.github/release-drafter.yml` for the
  workflows above.
- `rust.yml` workflow: action versions bumped to
  `actions/checkout@v6`, `actions/upload-artifact@v7`,
  `actions/cache/restore@v5`, `actions/cache/save@v5` (Node 24
  baseline); benchmark job restricted to `-p samp-sdk` to avoid
  `Unrecognized option: 'save-baseline'`; artefact retention now
  capped at 14 days.

### Added — tests and benchmarks

- Unit tests grew from 80 (v2.2.0) to 207 (this release) — **+127
  tests**.
- New per-module test files:
  - `samp-sdk/src/tests/amx_cell.rs` (10 tests).
  - `samp-sdk/src/tests/amx_string.rs` (8 tests).
  - `samp-sdk/src/tests/buffer.rs` (12 tests).
  - `samp-sdk/src/tests/omp_lifecycle.rs` (4 tests).
- Inline coverage added to every new `omp` submodule
  (`component`, `component_api`, `core`, `events`, `server`,
  `timers`, `types`, `vtable`) — 62 tests across them.
- `samp-codegen` gained 26 unit tests in `plugin.rs` covering
  `fnv1a_64`, `parse_uid_str`, `parse_version_str`, and
  `read_samp_metadata_from_content`.
- New `samp-sdk/benches/buffer_bench.rs` (Criterion): `get_as::<f32>`,
  `set_as::<bool>`, `iter_as::<i32>`, `iter_as::<f32>` at sizes
  8 / 64 / 256 / 1024.
- `samp-sdk/benches/string_bench.rs` reworked: uses
  `std::hint::black_box` (prevents LLVM DCE) and exercises
  `Buffer::write_str` / `UnsizedBuffer::write_str` alongside the
  existing baselines.

### Added — examples

- `examples/hello/src/lib.rs` and `examples/counter/src/lib.rs`
  ship with full source (previously only had `Cargo.toml`
  placeholders).
- `examples/README.md` plus one `README.md` per example
  (`hello`, `counter`, `advanced`) documenting the natives, the
  patterns demonstrated, and how to build each one in isolation.

### Changed

- All SDK diagnostic warnings are routed through the standard `log`
  facade with the `[rust-samp]` prefix. New warnings cover the Open
  Multiplayer lifecycle (null `ICore*` in `on_load`, missing
  `IPawnComponent` in `on_init`, null `IEventDispatcher`,
  `getAmxFunctions()` returning 0 in `on_ready`, missing
  `ITimersComponent` when the tick is enabled).
- Default log routing now writes via `ICore::logLnU8` when the
  plugin runs on native Open Multiplayer, mapping `log::Level` to
  `samp_sdk::omp::LogLevel` automatically; SA-MP behavior is
  unchanged (`logprintf`).
- `f32::as_cell` uses `f32::to_bits(*self).cast_signed()` (Rust
  1.87+ helper) instead of an `as i32` round trip.
- `Allocator::string_bytes` lifetime simplified to `&str → Cow<'_, [u8]>`.
- `Args::new` parameter renamed `args` → `params` (positional API
  unchanged).
- Compile-time layout assertions for `OmpComponent`:
  - Linux i686 (gated by `target_os = "linux"`):
    `offset_of!(uid_vtable) == 40`, `size_of == 56`.
  - Windows MSVC i686 (gated by `target_env = "msvc"`):
    `offset_of!(uid_vtable) == 56`.
  - Both with explanatory error messages on mismatch.

### Fixed

- Open Multiplayer adaptive bootstrap: `getAmxFunctions()` is tried
  in `on_init` and the pointer is stored if non-zero; otherwise the
  SDK retries in `on_ready`. This works for the current Open
  Multiplayer release (1.5.x — returns 0 in `on_init`) **and** any
  future release that populates the table earlier, without code
  changes.
- AMX scripts that arrive via `on_amx_load` **before** the AMX
  function table is available are now queued and processed in
  `on_ready` instead of being dropped.
- `omp_cleanup` correctly kills the tick timer (if any) and
  removes the `PawnEventHandler` from the dispatcher before the
  component is unloaded, preventing use-after-free if the server
  fires Pawn events during shutdown.
- The native-name `CString` allocation is leaked through
  `Box::leak(CString::into_boxed_c_str())` and the leak is now
  documented at the leak site (was previously implicit via
  `CString::into_raw()`).
- All compiler-emitted error messages from `samp-codegen` are now
  in English (were a mix of English and Portuguese).

### Documentation

- README, `samp-sdk/readme.md`, `samp-codegen/readme.md`, and root
  `migration.md` rewritten in English.
- mdBook removed (`docs/book.toml`, every `docs/src/*` page) and
  replaced by a MkDocs Material site under `docs/`. New pages:
  `introduction`, `setup`, `first-plugin`, `plugin-anatomy`,
  `natives`, `amx-types`, `cells-and-memory`, `encoding`,
  `error-handling`, `logging`, `advanced-examples`,
  `api-reference`, `omp-native`, `migration`, plus the new
  `exec-public`, `build-scripts`, `diagnostics`, and
  `internals/omp-abi`.
- All source-code docstrings translated from Portuguese to English
  (in-tree only — user-facing release notes and prose stay in
  English as well).
- This `CHANGELOG.md` now only carries the current release; older
  releases moved to `changelog/v1.x.md`, `changelog/v2.x.md`, and
  `changelog/historical.md` (pre-fork `samp-rs`).

### Dependencies

- Dev dependency `criterion` bumped 0.5 → 0.8.
- No other runtime dependency changes.

### Platform support

| Target                    | SA-MP | Native Open Multiplayer | Notes                                  |
| ------------------------- | :---: | :---------------------: | -------------------------------------- |
| `i686-unknown-linux-gnu`  |  ✅   |   ✅ (Itanium ABI)       | Default on Linux.                      |
| `i686-pc-windows-msvc`    |  ✅   |   ✅ (MSVC ABI)          | **New in 3.0.0.** Cross-compile from Linux via `cargo xwin build --xwin-arch x86`. |
| `i686-pc-windows-gnu`     |  ✅   |   ❌                    | Use with `--features samp-only`.       |

### Repository hygiene

- `.gitignore`: `Cargo.lock` removed (now committed); added
  `dist/` (build-script artefacts), `site/` (MkDocs build),
  `bench-entry.json`, `bench-history.json`, `bench_report.md`,
  `bench_results.txt`, `bench_comparison.txt`, `__pycache__/`,
  `*.pyc`, `release_body.md` (release workflow tempfile),
  `rust-samp-*-src.tar.gz` (local release tarball name), and
  common editor / OS junk (`*.swp`, `*.swo`, `.DS_Store`,
  `Thumbs.db`); consolidated `target` ignore.
- New `.gitattributes` with `export-ignore` rules so docs,
  examples, scripts, `.github/`, `changelog/`, `notes/`,
  `mkdocs.yml`, `ROADMAP.md`, `CHANGELOG.md`, and `migration.md`
  are excluded from the auto-generated "Source code (zip/tar.gz)"
  archives GitHub attaches to every release/tag. Also normalizes
  line endings (`text=auto eol=lf`) so shell scripts survive
  Windows checkouts and tags common binary extensions explicitly.
- `release.yml` hardened: the `tar` step adds defensive
  `--exclude` flags (`target`, `site`, `dist`, `__pycache__`,
  `*.pyc`, `*.rs.bk`, `.DS_Store`, `Thumbs.db`, `.git*`, `*.swp`,
  `*.swo`) and a new verification step fails the release if any
  forbidden path slips into the SDK source tarball.
- Workspace members updated to the renamed example crates.
- Per-crate `authors` field normalized to
  `"ZOTTCE <zottce@gmail.com>", "NullSablex <https://github.com/NullSablex>"`.
