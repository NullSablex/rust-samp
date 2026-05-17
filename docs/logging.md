# Logging

rust-samp integrates with the `log` and `fern` crates. By default the
SDK installs a routing `Dispatch` that delivers every record to the
active server log:

- **SA-MP** — via the server's `logprintf` (console + log file
  configured by the server).
- **Native Open Multiplayer** — via `ICore::logLnU8`, the server's
  UTF-8 log pipeline, mapping `log::Level` to `LogLevel` automatically
  (`Error` → `Error`, `Warn` → `Warning`, `Info` → `Message`, `Debug`
  / `Trace` → `Debug`).

## Out-of-the-box

The default routing is installed during plugin initialization, so no
configuration is required to start emitting structured logs:

```rust
use log::{info, warn, error};

impl SampPlugin for MyPlugin {
    fn on_load(&mut self) {
        info!("Plugin loaded");
        warn!("This is a warning");
        error!("This is an error");
    }
}
```

## Customizing the dispatch

`samp::plugin::logger()` returns a `fern::Dispatch` already chained to
the server's log sink, and disables the SDK's default routing so the
returned dispatch becomes authoritative.

```rust
initialize_plugin!(
    natives: [],
    {
        // Server dispatch (SA-MP logprintf / ICore::logLnU8)
        let samp_logger = samp::plugin::logger()
            .level(log::LevelFilter::Info);

        // File dispatch
        let log_file = fern::log_file("my_plugin.log")
            .expect("failed to open log file");

        let file_logger = fern::Dispatch::new()
            .level(log::LevelFilter::Trace)
            .chain(log_file);

        // Combine both with a custom format
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

In the server console:

```
[MyPlugin][INFO]: Plugin loaded
[MyPlugin][ERROR]: Something failed
```

In `my_plugin.log`:

```
[MyPlugin][TRACE]: granular internals
[MyPlugin][DEBUG]: debug information
[MyPlugin][INFO]: Plugin loaded
```

## Log levels

| Level    | Suggested use                                                |
| -------- | ------------------------------------------------------------ |
| `error!` | Errors that affect plugin behavior.                          |
| `warn!`  | Unexpected but non-critical situations.                      |
| `info!`  | Important lifecycle events (load, connections, …).           |
| `debug!` | Information useful while developing.                         |
| `trace!` | Detailed internal traces.                                    |

## Filtering by level

Different dispatches can apply independent filters:

```rust
// Console: warn and above only
let samp_logger = samp::plugin::logger()
    .level(log::LevelFilter::Warn);

// File: everything
let file_logger = fern::Dispatch::new()
    .level(log::LevelFilter::Trace)
    .chain(log_file);
```

## Limitation: logs before `on_load`

The SDK log routing depends on the server's export table. On SA-MP this
table is populated only when `Load()` runs; on native Open Multiplayer
the `ICore*` pointer is delivered in `onLoad(ICore*)`. Any `log::*`
call made earlier — for example inside the constructor block — has no
visible destination and falls back to `eprintln!`.

> This is a server-imposed constraint, not an SDK limitation. SA-MP and
> Open Multiplayer behave the same way.

**Rule of thumb:** put initialization logs inside `on_load`, never in
the constructor block.

## Dependencies

`log` provides the macros (`info!`, `warn!`, …). The `samp` crate
re-exports `log` internally so `#[native]` does not leak a `log`
dependency into the plugin's `Cargo.toml`.

`fern` is optional — only needed to customize destinations or
formatting:

```toml
[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "v3.0.0" }
log  = "0.4"      # for the log::{info, warn, error, debug, trace} macros
fern = "0.7"      # only when customizing the dispatch
```
