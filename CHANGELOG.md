# Changelog

Current release only. Previous releases are split per major line under
[`changelog/`](changelog/) — see [`changelog/index.md`](changelog/index.md)
for the full directory.

## [v3.2.0] — 2026/06/30

### New features

- **VM debugging primitives on `Amx`** — safe accessors that previously had to
  be hand-written by tooling poking the `#[repr(C, packed)]` `AMX` struct:
  register reads (`cip`, `frame`, `stack`, `heap`, `stp`), bounds-checked
  data-segment cell access (`read_cell`/`write_cell`, mirroring `amx_GetAddr`
  and usable inside a debug hook where no native context exists), and debug
  hook management (`install_debug_hook`/`remove_debug_hook`, the equivalent of
  `amx_SetDebugHook`). Always available, no feature gate.
- **`samp::debug` — AMX_DBG debug-info parser (feature `debug`)** — pure-logic
  decoder for the debug block `pawncc -d2`/`-d3` appends to the `.amx`. Maps a
  code address ↔ source line ↔ symbol ↔ function (`AmxDbg::from_amx`/`parse`,
  `lookup_line`, `lookup_file`, `lookup_function`, `line_to_address`,
  `symbols_in_scope`, `tag_name`), handling the 16-bit line-count overflow of
  large gamemodes and corrupted-count sanity ceilings. No extra dependencies;
  opt-in via the `debug` feature.

- **External sinks (`samp::logger::Sink` trait + `LoggerConfig::add_sink`)** —
  extension point for forwarding accepted log records to a destination
  chosen by the plugin author (Sentry, an OTLP collector, an in-house
  HTTP endpoint, anything). **No telemetry is built into the SDK.** No
  dependency on `sentry` / `opentelemetry` is added; `rust-samp`
  ships exactly the same dependency graph as before. The trait is an
  opt-in surface only — implementing it is the plugin author's call,
  and instances become active only through an explicit
  `LoggerConfig::add_sink(Box::new(...))` in the plugin's own source.
  The SDK contains zero `add_sink` invocations of its own; server
  operators auditing what a `rust-samp` plugin can export only need
  to grep its source for `add_sink(`. Zero hits means zero external
  traffic from the logger. There is no hidden flag, no environment
  override, and no default destination — this is not Microsoft-style
  always-on telemetry, it is a hook for plugin authors who already
  run their own observability stack to integrate with it on their own
  terms.
- **`samp::version()`** — free function returning the `CARGO_PKG_VERSION`
  of the `rust-samp` (`samp`) crate. Pair it with a Pawn-side native
  (e.g. `MyPlugin_GetSdkVersion()`) to surface the active SDK build in
  bug reports and diagnostic dashboards.
- **`samp::logger::flush()`** — public free function that flushes the
  active log file directly through the live `LoggerImpl`. Going through
  `log::logger().flush()` did not guarantee a sync of the SDK's own
  file handle; calling `samp::logger::flush()` does. Safe no-op when
  the logger has not been installed — meant for panic hooks and
  custom shutdown paths.
- **`LoggerConfig::from_env()`** — applies runtime overrides from
  environment variables, so server operators can flip the log level,
  redirect the directory, change the rotation threshold etc. **without
  recompiling the plugin**. The prefix is derived from the plugin's
  crate name uppercased with non-alphanumeric characters replaced by
  `_` (`streamer-rs` → `STREAMER_RS_LOG_*`). Recognised keys:
  `LEVEL`, `DIR`, `FILE`, `ROTATION_MB`, `ROTATION_KEEP`,
  `NO_ROTATION`, `NO_BANNER`, `SERVER`, and `COMPRESS` (the last only
  effective when the `compression` feature is enabled). Missing vars
  leave the existing value untouched; invalid values are reported to
  the server console and the previous value is kept. Pairs with
  `Runtime::try_get()` (also new) so the parser can warn gracefully
  even when called before the runtime is initialised (e.g. from a
  unit test).
- **`LoggerConfig::compress_archives(bool)`** — opt-in gzip of rotated
  archives. When enabled, every rotation produces
  `{filename}.{N}.gz` instead of `{filename}.{N}` and removes the
  uncompressed file. Works with both rotation strategies (append-style
  and `rotation_keep(N)` shift-style). Gated by the new `compression`
  Cargo feature, which pulls in `flate2` with the pure-Rust backend —
  not enabled by default, so plugins that do not need it pay no extra
  dependency cost. The next-archive scan also recognizes `.gz`
  variants so an index is never reused across restarts.
- **`Amx::call_native()`** — invoke a native registered by **another
  plugin** in the same AMX, straight from Rust. Resolves the host
  function pointer through `amx_FindNative` + the natives table in the
  `AMX_HEADER`, builds the `params` block in the AMX convention
  (`[argc * sizeof(cell), arg0, ...]`) and surfaces VM-side errors back
  via `amx.error`. Unblocks integration with the entire existing C++
  plugin ecosystem (Streamer, MySQL, sscanf, …) without dropping down
  to `samp_sdk::raw`. Originally surfaced by
  [@Day-OS](https://github.com/Day-OS) (Discord `@daytheipc`), who
  found `rust-samp` on crates.io while trying to drive the Streamer
  plugin from Rust for an in-game PNG / video / YouTube-live 3D panel
  and hit the gap that this API closes. May or may not have been
  exactly what she needed — but it should help.

### Examples

- **New `examples/sink-demo/`** — complete, working **Sentry
  integration** for the new `Sink` trait. Uses the real `sentry`
  crate (`sentry = "0.43"` with `reqwest` + `rustls` + `contexts`,
  `default-features = false`), with `sentry::init` and
  `sentry::capture_event` wired up end-to-end — every `log!` call
  becomes a real Sentry event. **DSN is read from the env var
  `SINK_DEMO_SENTRY_DSN` at plugin load — never hardcoded.** Source
  code stays clean, the DSN stays in the operator's environment
  (systemd `Environment=`, Docker secret, vault sidecar, `.env`
  outside the repo, …). Implements the full backpressure pattern
  (`mpsc::sync_channel` between the logger lock and Sentry +
  dedicated background drainer thread that owns the
  `ClientInitGuard`, so its `Drop` flushes pending events at plugin
  unload). When the env var is missing the example falls back to a
  fake local DSN (`http://fake@127.0.0.1:9999/1`) — the Sentry
  client still initializes but its HTTP transport refuses fast, so
  no event ever reaches a real Sentry server. Going to production
  is one `export` statement. When a real DSN is configured, the
  plugin also emits a startup smoke test (one `info` + one
  `warning` + one `error`) on `on_load` so the operator immediately
  sees the wiring working on the Sentry dashboard. The heavy
  `sentry` dep (pinned to 0.48.3) is paid by this example crate,
  not by the SDK. Pawn natives: `SinkDemo_GetExportedCount`,
  `SinkDemo_GetDroppedCount` for pipeline observability;
  `SinkDemo_EmitInfo`, `SinkDemo_EmitWarn`, `SinkDemo_EmitError`
  for firing test events at each severity from the gamemode.

### Build

- **New Cargo feature `compression`** on the `rust-samp` crate. Opt-in;
  pulls in `flate2 = "1"` with the pure-Rust backend
  (`default-features = false`, `features = ["rust_backend"]`) so plugins
  that do not enable it remain dependency-free on this axis.
- **`time` bumped to `>= 0.3.47`** (also pulls in `time-core 0.1.8`
  and `time-macros 0.2.27`).
- **MSRV bumped to Rust 1.88** (was 1.87) to satisfy those versions.
  Declared via `[workspace.package].rust-version = "1.88"`.

### Security & governance

- **OpenSSF Scorecard** — new `.github/workflows/scorecard.yml` that runs
  the OpenSSF Scorecard analysis, uploads the SARIF to code-scanning and
  publishes the result. Scorecard badge added to the README.
- **All GitHub Actions pinned by commit SHA** — every `uses:` across the
  six workflows is now pinned to a full commit SHA (with a `# vX` comment),
  satisfying the Scorecard *Pinned-Dependencies* check.
- **`docs/requirements.txt` pinned by hash** — the MkDocs Material build
  dependencies are now a fully hashed lockfile (`pip-compile
  --generate-hashes` from the new `docs/requirements.in`), installed with
  `pip install --require-hashes` in the docs workflow.
- **`.github/dependabot.yml`** — weekly version updates for the
  `github-actions` and `cargo` ecosystems, keeping the pinned SHAs and
  crate dependencies fresh (Scorecard *Dependency-Update-Tool*).
- **`SECURITY.md`** — security policy and private vulnerability reporting
  via GitHub Security Advisory.
- **`CODE_OF_CONDUCT.md`** — Contributor Covenant 2.1.
- **`CONTRIBUTING.md`** — build/test/lint workflow for the i686 targets,
  project structure and code rules.

### Crate versions

- `rust-samp` (lib `samp`): 3.1.0 → 3.2.0
- `rust-samp-sdk` (lib `samp_sdk`): 3.0.0 → 3.1.0 (new VM debugging
  primitives on `Amx` and the `samp::debug` parser are additive public API)
- `rust-samp-codegen` (lib `samp_codegen`): 1.3.0 — unchanged

### CHANGELOG correction (v3.1.0)

The v3.1.0 entry below described the `CNAME` removal as a switch to
the default GitHub Pages URL. That was wrong: the file was simply
unnecessary and the documentation URL is unchanged. Recorded here;
the v3.1.0 section below is left as-published.

## [v3.1.0] — 2026/06/09

Headline: turnkey logger — `samp::enable_logger!()` installs a complete
per-plugin logging pipeline (file under `logs/`, size-based rotation
into `logs/archive/`, prefix derived from `CARGO_PKG_NAME`, startup
banner, runtime-adjustable level) in a single call. The previous
`samp::plugin::logger()` DIY path stays unchanged for advanced cases.

v3.1.0 is also the **first version available on crates.io** —
[`rust-samp`](https://crates.io/crates/rust-samp),
[`rust-samp-sdk`](https://crates.io/crates/rust-samp-sdk) and
[`rust-samp-codegen`](https://crates.io/crates/rust-samp-codegen).
Earlier releases (v3.0.0 and the entire v2.x line) are not published to
the registry; plugins targeting those versions must keep using a git
dependency. The **library** names (`samp`, `samp_sdk`, `samp_codegen`)
are unchanged; only the **package** names differ on the registry to
avoid colliding with the upstream `samp-rs` fork.

### Crate versions

- `rust-samp` (lib `samp`): 3.0.0 → 3.1.0
- `rust-samp-sdk` (lib `samp_sdk`): 3.0.0 — unchanged (metadata only)
- `rust-samp-codegen` (lib `samp_codegen`): 1.3.0 — unchanged (metadata only)

### Installation

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
samp = { package = "rust-samp", version = "3" }
log  = "0.4"
```

The package is published as `rust-samp`; the alias keeps the
source-level `use samp::prelude::*;` imports unchanged. Git consumers
do not need to update anything — both names continue to resolve to the
same library.

### New features

- **Turnkey logger** — new module `samp::logger` plus the
  `samp::enable_logger!()` and `samp::enable_logger_with!(cfg)` macros.
  The macros capture the caller's `CARGO_PKG_*` at compile time and
  install a `log::Log` implementation that routes through both the
  server's log sink and a per-plugin file.
- **`LoggerConfig` builder** — every aspect of the pipeline is
  configurable through fluent setters: `directory`, `filename`,
  `prefix`, `level`, `also_to_server`, `banner`, `file_format`,
  `server_format`, `rotation_size_mb`, `rotation_keep`,
  `rotation_no_cleanup`, `no_rotation`, `no_banner`, `banner_with`.
- **Format templates** — `file_format` and `server_format` accept
  `{timestamp}`, `{level}`, `{message}` and (server-only) `{prefix}`
  placeholders with optional alignment specifiers (`{level:>5}`,
  `{level:<5}`, `{level:^5}`). Unknown placeholders pass through
  verbatim so typos are visible.
- **Banner modes** — `BannerMode::Default` (5-line banner from
  `CARGO_PKG_*`), `BannerMode::Off`, and `BannerMode::Custom` (closure
  receiving `BannerMetadata` and returning the lines to render).
- **Size-based rotation** — when the active file passes
  `rotation_size_mb` (default 50 MB), it is renamed into
  `{directory}/archive/{filename}.{N}` and a fresh active file is
  opened. Two strategies are available:
  - **Append-style** (default): every rotation uses the next free
    index, archives are **never deleted** by the SDK, the dev keeps
    full control over cleanup. The archive folder is created lazily on
    the first rotation; the next index survives restarts (rescanned at
    install time).
  - **Shift-style** — opt-in via `rotation_keep(N)`: `.log.N` is
    deleted, every other archive shifts down, active becomes `.log.1`.
    Disk footprint becomes `(keep + 1) * rotation_size_mb`.
- **Runtime level adjustment** — `samp::logger::set_level(...)` and
  `samp::logger::level()` let plugins expose a Pawn-side knob for log
  verbosity (e.g. a `MyPlugin_SetLogLevel(level)` native).
- **`InstallError`** — `Display` + `Error` with `source()` exposing
  the inner `std::io::Error` for the `Io` variant.

### Migrating to the turnkey logger

The previous handcrafted `fern::Dispatch` pattern keeps working —
adoption is optional.

| Before (v3.0.0)                                                                                                       | After (v3.1.0)                                |
| --------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `fern::Dispatch::new().level(...).chain(samp::plugin::logger()).chain(fern::log_file(...)?).format(...).apply()?;`    | `samp::enable_logger!()` inside `on_load`     |
| Hand-built prefix string `format_args!("[my-plugin][{}]: {}", record.level(), message)`                               | Automatic `[CARGO_PKG_NAME]` prefix           |
| Manually managing the log filename                                                                                    | Default `logs/{CARGO_PKG_NAME}.log`           |
| External `logrotate` setup for file growth                                                                            | Built-in 50 MB rotation into `logs/archive/`  |
| Pawn-side `SetLogLevel(level)` native backed by a `static AtomicI32`                                                  | `samp::logger::set_level(LevelFilter)` direct |

See [`docs/logging.md`](docs/logging.md) for the full reference,
including the three layers (turnkey / `LoggerConfig` / DIY with fern),
format placeholders, rotation modes, and runtime tuning.

### Packaging (crates.io)

The three workspace crates now have crates.io-ready metadata
(`description`, `keywords`, `categories`, `rust-version`, per-crate
`README.md`) and centralized shared fields in `[workspace.package]`.

- **Package names**: `rust-samp`, `rust-samp-sdk`, `rust-samp-codegen`.
  The upstream `samp` / `samp-sdk` / `samp-codegen` names on crates.io
  belong to the original `samp-rs` author and are not the publication
  target of this fork.
- **Library names**: unchanged — `samp`, `samp_sdk`, `samp_codegen`.
  Existing `use samp::prelude::*;` keeps compiling.
- **Crates.io consumers** add the package alias to their
  `Cargo.toml`:

  ```toml
  [dependencies]
  samp = { package = "rust-samp", version = "3" }
  ```

- **Git consumers** (`samp = { git = "..." }`) need no change — the
  workspace exposes both names.

### Build

- **MSRV bumped to Rust 1.88** (was 1.85). Required by stable
  `i32::cast_unsigned` / `u32::cast_signed` (used internally for AMX
  cell bit conversions) and by the patched `time 0.3.47` / `time-core
  0.1.8` / `time-macros 0.2.27`. Declared via
  `[workspace.package].rust-version = "1.88"`.
- **New transitive dependency** — `time = "0.3.47"` (features
  `local-offset`, `formatting`, `macros`) is pulled in by the turnkey
  logger for timestamp formatting. The `chrono` crate is **not** added.
  The minimum is pinned to `0.3.47` to pick up the fix for
  [RUSTSEC-2026-0009](https://rustsec.org/advisories/RUSTSEC-2026-0009)
  (DoS via stack exhaustion, medium severity).

### CI / release infrastructure

- **Release notes are now auto-assembled** — workflow `.github/workflows/release.yml`
  combines the curated CHANGELOG section with the GitHub-native
  `releases/generate-notes` API output, keeping the "New Contributors"
  block and the "Full Changelog" comparison link while dropping the
  redundant `## What's Changed` header.
- **Crates.io publication is wired into the release workflow** — on
  `v*` tag push (and manual `workflow_dispatch` with a `dry_run`
  input), the workflow validates the workspace, then publishes
  `rust-samp-sdk` → `rust-samp-codegen` → `rust-samp` in dependency
  order with a 30 s sleep between steps. Each `cargo publish` step
  gracefully skips when the version is already on crates.io, so a
  patch release that bumps only one crate goes through unattended.
- **Bench jobs were updated** to reference `rust-samp-sdk` instead of
  the pre-rename `samp-sdk` package id.
- **Release-drafter template** no longer emits the duplicated
  `## What's Changed` heading at the top of release notes.

### Documentation

- **`docs/logging.md`** — rewritten end-to-end (139 → 413 lines).
  Covers the three layers, format placeholders and alignment specs,
  banner modes, rotation strategies, runtime level adjustment, and a
  pitfalls section.
- **`docs/api-reference.md`** — new `samp::logger` section with the
  full builder signature, error type, and placeholder reference.
- **`docs/migration.md`** — new v3.0.0 → v3.1.0 section with the
  before/after migration table and crates.io adoption notes.
- **`docs/first-plugin.md`** — new "Enabling logging" section showing
  the one-liner inside `on_load`.
- **`docs/plugin-anatomy.md`** — explicit note that `enable_logger!`
  belongs in `on_load`, not the constructor block (server's log sink
  is not connected yet during construction).
- **`docs/index.md`** — the integrated-logging bullet now describes
  the turnkey path; workspace table reflects the new `samp` version.
- **`docs/advanced-examples.md`** — the `examples/counter` snippet
  matches the source change (uses `samp::enable_logger!()` instead of
  the handcrafted `fern::Dispatch`).
- **Dual-availability sweep** — `README.md`, `migration.md`,
  `docs/setup.md`, `docs/encoding.md` and `docs/migration.md` now show
  both installation paths side-by-side (crates.io for v3.1.0+, git for
  any version including v3.0.0 and earlier) with a consistent
  `package = "rust-samp"` snippet. The workspace version table in
  `README.md` was bumped to reflect the new `samp` 3.1.0.

### Examples

- **`examples/counter` (1.0.0 → 1.1.0)** — `on_load` now calls
  `samp::enable_logger!()`; the `fern` dependency was dropped.
- **`examples/hello` (1.0.0 → 1.0.1)**, **`examples/advanced` (1.1.0 →
  1.1.1)** — switched to the `package = "rust-samp"` alias so the path
  dependency matches the published name. No source changes.

### Repository housekeeping

- `ROADMAP.md` moved out of version control (now under `.gitignore`)
  — it stays as a local working document for the maintainer and is no
  longer shipped to crates.io tarballs or GitHub.
- `.github/CODEOWNERS` added.
- `CNAME` removed (project pages are served from the default
  `nullsablex.github.io/rust-samp/` URL).

### Code quality

- Clippy `-D warnings` and `-W clippy::pedantic` both report **zero**
  warnings.
- 232 tests pass (was 219 in v3.0.0; +13 covering the new logger
  config, format substitution, width specifiers, rotation modes, error
  source, and macro-driven metadata capture).
- `cargo fmt --check` green; `cargo machete` reports no unused
  dependencies.

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
