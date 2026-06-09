# Logging

rust-samp provides three layers of logging, each one usable on its own:

1. **Turnkey** — a single call (`samp::enable_logger!()`) installs a
   complete pipeline: per-plugin file under `logs/`, size-based rotation,
   automatic prefix derived from the plugin's name, a banner at startup
   and a runtime-adjustable level. **Use this unless you have a specific
   reason not to.**
2. **Customizable** — `samp::enable_logger_with!(LoggerConfig::new(...))`
   keeps the same pipeline but lets you tweak every knob: directory,
   filename, prefix, level, banner, format templates, rotation policy.
3. **DIY** — `samp::plugin::logger()` returns a bare `fern::Dispatch`
   already chained to the server's log sink. Build whatever you want on
   top.

The first two layers route through the `log` crate's facade. Once
installed, plain `log::info!` / `log::warn!` / `log::error!` calls in
your plugin go through the configured pipeline.

> **Heads-up.** Only one global logger can be installed per process.
> Combining layers 1/2 with `samp::plugin::logger()` is a no-op for the
> second installation. Choose one path or hand-build it.

## Quick start

```rust
use samp::prelude::*;
use samp::{SampPlugin, initialize_plugin};

#[derive(SampPlugin, Default)]
struct MyPlugin;

impl SampPlugin for MyPlugin {
    fn on_load(&mut self) {
        let _ = samp::enable_logger!();
        log::info!("plugin loaded");
    }
}

initialize_plugin!(type: MyPlugin, natives: []);
```

That single `enable_logger!()` call:

- Creates `logs/` next to the server executable.
- Opens `logs/my-plugin.log` (filename derived from `CARGO_PKG_NAME`).
- Prepares `logs/archive/` for rotated files (created lazily on the
  first rotation).
- Prints a 5-line banner with the plugin name, version, authors and
  repository — read from `CARGO_PKG_*` at compile time.
- Routes every subsequent `log::info!` / `log::warn!` / `log::error!`
  to both the server console (with `[my-plugin]` prefix) and the file
  (with timestamp + level).
- Drops anything below `Info` by default — adjust via
  `samp::logger::set_level(LevelFilter::Debug)` at runtime.

## What gets written where

For a plugin whose `CARGO_PKG_NAME = "my-plugin"`:

### Server console / `server_log.txt`

```
[20:14:32] Loading plugin: my-plugin.so
[20:14:32]
[20:14:32]   | my-plugin 1.0.0
[20:14:32]   |-------------------------------
[20:14:32]   | Author: Some Author
[20:14:32]   | Repository: https://github.com/some-author/my-plugin
[20:14:32]
[20:14:32]   Loaded.
[20:14:35] [my-plugin] player 1 connected
[20:14:40] [my-plugin] vehicle 42 spawned
```

The `[20:14:32]` timestamp comes from the server itself; the SDK only
contributes the `[my-plugin]` prefix + message.

### `logs/my-plugin.log`

```
[2026-06-08 20:14:35] [INFO] player 1 connected
[2026-06-08 20:14:40] [INFO] vehicle 42 spawned
[2026-06-08 20:14:42] [WARN] connection retry on slot 3
```

The file has its own timestamp and level — it does **not** include the
plugin prefix because every line in it already belongs to this plugin.

## Customizing the pipeline

`enable_logger_with!` accepts a [`LoggerConfig`](#loggerconfig) and
returns the same `Result`. Every method on `LoggerConfig` is a fluent
builder:

```rust
use std::time::Duration;
use samp::logger::{LoggerConfig, BannerMode};

let _ = samp::enable_logger_with!(
    LoggerConfig::new(env!("CARGO_PKG_NAME"))
        .directory("logs/my-plugin")          // → logs/my-plugin/my-plugin.log
        .filename("audit.log")                 // → logs/my-plugin/audit.log
        .prefix("[Audit]")                     // override console prefix
        .level(log::LevelFilter::Debug)        // verbose by default
        .also_to_server(false)                 // file only, silent in console
        .no_banner()                           // suppress startup banner
        .rotation_size_mb(100)                 // rotate at 100 MB
        .rotation_keep(10)                     // keep last 10 archives
        .file_format("{timestamp} | {level:>5} | {message}")
        .server_format("({prefix}) [{level}] {message}")
);
```

The macro still captures the caller's `CARGO_PKG_*` for the banner,
which is why it can be a macro and not just a function call.

## Format templates

`file_format` and `server_format` accept template strings with the
following placeholders:

| Placeholder   | Value                                                   |
| ------------- | ------------------------------------------------------- |
| `{timestamp}` | `YYYY-MM-DD HH:MM:SS` (local time, UTC fallback)        |
| `{level}`     | `ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`               |
| `{message}`   | The formatted args from `log::info!(...)`               |
| `{prefix}`    | Only available in `server_format` (the per-plugin tag)  |

Defaults:

- `file_format`: `"[{timestamp}] [{level}] {message}"`
- `server_format`: `"{prefix} {message}"`

Unknown placeholders pass through verbatim so typos are visible.

### Alignment and padding

Each placeholder accepts an optional alignment + width spec, mirroring
Rust's own format syntax:

| Spec       | Effect                                  |
| ---------- | --------------------------------------- |
| `:<5`      | Left-aligned, padded to width 5         |
| `:>5`      | Right-aligned, padded to width 5        |
| `:^6`      | Centred, padded to width 6              |

Example template:

```rust
.file_format("{timestamp} | {level:>5} | {message}")
```

Produces:

```
2026-06-08 12:30:45 |  INFO | player connected
2026-06-08 12:30:46 |  WARN | retry scheduled
2026-06-08 12:30:47 | ERROR | timeout
```

Padding never truncates — a value longer than the requested width is
emitted in full.

## Banner

By default, install prints a 5-line banner at `Info` level introspecting
the caller's manifest. Three modes are available:

```rust
use samp::logger::{LoggerConfig, BannerMode};

// 1. Default (the standard banner)
LoggerConfig::new(env!("CARGO_PKG_NAME"))
    .banner(BannerMode::Default)            // implicit default

// 2. Off
LoggerConfig::new(env!("CARGO_PKG_NAME"))
    .no_banner()                            // shortcut for BannerMode::Off

// 3. Custom — closure receives the manifest metadata
LoggerConfig::new(env!("CARGO_PKG_NAME"))
    .banner_with(|meta| vec![
        String::new(),
        format!("=== {} v{} ===", meta.name, meta.version),
        format!("Author: {}", meta.authors),
        format!("Repository: {}", meta.repository),
        format!("Compiled: {}", env!("CARGO_PKG_VERSION")),
        String::new(),
    ])
```

Custom banner lines are emitted at `Info` level through the same
pipeline as everything else — they appear in both the server console
(with prefix) and the file (with timestamp).

## Rotation

Rotation kicks in when the active file exceeds `rotation_size_mb`
(default 50 MB). The active file is renamed into
`{directory}/archive/{filename}.{N}` and a fresh active file is
opened.

The SDK ships **two rotation strategies**, controlled by `rotation_keep`:

### Append-style (default)

```rust
// equivalent to: LoggerConfig::new(...).rotation_no_cleanup()
LoggerConfig::new(env!("CARGO_PKG_NAME"))
```

Each rotation picks the next free index. Existing archives are **never
deleted** by the SDK; the dev decides what to do with old files (rotate
externally via `logrotate`, manual cleanup, archive uploader, etc).

Initial scan at install time finds the highest existing
`{filename}.{N}` and starts from `N + 1`, so indices keep growing across
restarts without ever being reused.

```
logs/
├── my-plugin.log          ← active
└── archive/
    ├── my-plugin.log.1    ← oldest
    ├── my-plugin.log.2
    ├── my-plugin.log.3
    └── my-plugin.log.4    ← most recent archive
```

### Shift-style (opt-in)

```rust
LoggerConfig::new(env!("CARGO_PKG_NAME"))
    .rotation_keep(5)       // keep last 5 archives, delete the rest
```

`.log.5` is deleted, every other archive shifts down (`.4 → .5`, `.3 →
.4`, …), and the active file becomes `.log.1`. Disk footprint is bounded
to `(keep + 1) * rotation_size_mb`.

```
logs/
├── my-plugin.log          ← active (just rotated, empty)
└── archive/
    ├── my-plugin.log.1    ← most recent archive (was active)
    ├── my-plugin.log.2
    ├── my-plugin.log.3
    ├── my-plugin.log.4
    └── my-plugin.log.5    ← oldest retained (next rotation deletes it)
```

### Disabling rotation entirely

```rust
LoggerConfig::new(env!("CARGO_PKG_NAME"))
    .no_rotation()
```

The active file grows indefinitely. Useful when an external rotator
takes over (e.g. `logrotate` on a Linux server).

## Adjusting the level at runtime

```rust
use log::LevelFilter;

samp::logger::set_level(LevelFilter::Debug);
let current: LevelFilter = samp::logger::level();
```

A common pattern is to expose a Pawn native that maps an integer
coming from the gamemode into a `LevelFilter`:

```rust
#[native(name = "MyPlugin_SetLogLevel")]
fn set_log_level(_amx: &Amx, level: i32) -> bool {
    let target = match level {
        0 => log::LevelFilter::Off,
        1 => log::LevelFilter::Error,
        2 => log::LevelFilter::Warn,
        3 => log::LevelFilter::Info,
        4 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };
    samp::logger::set_level(target);
    true
}
```

## Layer 3: DIY with `fern`

When the turnkey pipeline does not fit (e.g. you need JSON output,
structured fields, an external log aggregator, or a non-standard
destination), fall back to the existing helper:

```rust
initialize_plugin!(
    natives: [],
    {
        // Server dispatch (SA-MP logprintf / ICore::logLnU8).
        let samp_logger = samp::plugin::logger()
            .level(log::LevelFilter::Info);

        // File dispatch with a fully custom format.
        let log_file = fern::log_file("custom.log")
            .expect("failed to open log file");

        let file_logger = fern::Dispatch::new()
            .level(log::LevelFilter::Trace)
            .chain(log_file);

        let _ = fern::Dispatch::new()
            .format(|callback, message, record| {
                callback.finish(format_args!(
                    "[MyPlugin][{}]: {}",
                    record.level(),
                    message
                ));
            })
            .chain(samp_logger)
            .chain(file_logger)
            .apply();

        return MyPlugin::default();
    }
);
```

This is the same API documented in earlier releases — it has not been
removed. It coexists with the turnkey logger but the two are mutually
exclusive (only one global logger lives in the process).

## Log levels

| Level    | Suggested use                                                |
| -------- | ------------------------------------------------------------ |
| `error!` | Errors that affect plugin behavior.                          |
| `warn!`  | Unexpected but non-critical situations.                      |
| `info!`  | Important lifecycle events (load, connections, …).           |
| `debug!` | Information useful while developing.                         |
| `trace!` | Detailed internal traces.                                    |

## Where things happen

```
log::info!("hello")
        │
        ▼
┌────────────────────┐
│ samp::logger       │  (only when enable_logger! ran)
│   - format         │
│   - level filter   │
└──────┬──────┬──────┘
       │      │
       │      └────────────► logs/{plugin}.log
       │                     [{timestamp}] [{level}] {message}
       ▼
Runtime::log(...)
       │
       ├── SA-MP                   → logprintf("[plugin] message")
       └── native Open Multiplayer → ICore::logLnU8("[plugin] message")
```

## Common pitfalls

### Logs before `on_load` are dropped

The server's log sink only becomes available when `Load()` runs on
SA-MP, or when `ICore*` is delivered in `onLoad(ICore*)` on native Open
Multiplayer. Any `log::*` call made earlier — for example inside the
constructor block — falls back to `eprintln!`.

Put initialization logs inside `on_load`, never in the constructor
block.

### `also_to_server(false)` does not drop the prefix from the file

The prefix is only used for the **server console** sink. The file uses
`file_format`, which has no prefix placeholder by default. Setting
`also_to_server(false)` removes the console destination entirely; the
file format keeps producing `[timestamp] [LEVEL] message` lines.

### Banner appears in the file too

The banner is emitted at `Info` level through the configured pipeline —
which means it goes to both the server console and the file. If a clean
file is important (e.g. for log shipping or analysis), use `.no_banner()`
or move the banner to a separate `println!` outside the logger.

### `enable_logger!` returns a `Result`

```rust
let _ = samp::enable_logger!();
```

The most common error is `InstallError::AlreadyInstalled` if a previous
`enable_logger!` (or `samp::plugin::logger().apply()`) already ran in
this process. The other variant, `InstallError::Io`, wraps a
`std::io::Error` from creating the directory or opening the file. Both
implement `Display` and `std::error::Error`, so they fit any standard
error-handling pipeline.

## Dependencies

The macros are part of the `samp` crate; no extra dependency is needed:

```toml
[dependencies]
samp = { package = "rust-samp", version = "3" }
log  = "0.4"      # for the log::{info, warn, error, debug, trace} macros
```

`fern` is only needed when using the Layer 3 DIY approach. The `log`
crate is re-exported through `samp::log` to spare `#[native]` from
leaking it into your `Cargo.toml`; depending on `log` explicitly is
still recommended for clarity at the call sites.
