# sink-demo

Complete, working **Sentry integration** for `samp::logger::Sink`.
Uses the real `sentry` crate — `sentry::init` + `sentry::capture_event`
— so the example shows the production pattern end-to-end, not a
hand-rolled HTTP stub.

## Configuration (env vars)

| Variable                | Purpose                | Default                                |
| ----------------------- | ---------------------- | -------------------------------------- |
| `SINK_DEMO_SENTRY_DSN`  | Sentry project DSN     | `http://fake@127.0.0.1:9999/1`         |

**Never put the DSN in `src/lib.rs` or `Cargo.toml`.** Source code
goes into git, env vars do not. Production DSNs belong in:

- systemd: `Environment=SINK_DEMO_SENTRY_DSN=...` in the unit file
- Docker: `--env-file` or Docker secrets
- Kubernetes: `Secret` mounted as env
- bare-metal: `.env` file outside the repo, sourced by the launch script

## What this example ships

- Real `sentry` crate (0.48), real `ClientInitGuard`, real
  `capture_event` — every `log::error!` / `warn!` / `info!` /
  `debug!` / `trace!` becomes a real Sentry event.
- DSN read from `SINK_DEMO_SENTRY_DSN` at plugin load, with a fake
  local fallback so the example runs out of the box.
- Bounded `mpsc::sync_channel` between the logger lock and Sentry
  — emit never blocks; a stuck consumer drops records instead of
  growing memory.
- Background drainer thread that owns the `ClientInitGuard` (its
  `Drop` flushes pending events at plugin unload).
- **Startup smoke test** — when a real DSN is configured, the plugin
  emits one `info` + one `warning` + one `error` on `on_load` so the
  operator sees the wiring working on the Sentry dashboard
  immediately.
- **Pawn-side emitters at every level** — `SinkDemo_EmitInfo`,
  `SinkDemo_EmitWarn`, `SinkDemo_EmitError` — so the gamemode can
  fire test events on demand.
- Pawn natives that surface `exported` / `dropped` counts for
  pipeline observability.

## Why it runs with zero configuration

When `SINK_DEMO_SENTRY_DSN` is not set, the example uses
`http://fake@127.0.0.1:9999/1`. The Sentry client still initializes,
the drainer still runs, `capture_event` still returns event ids — but
the underlying HTTP transport refuses fast and **no event ever
reaches a real Sentry server**. No internet egress, no account
required, no data leaving the host.

## Going to production

```sh
export SINK_DEMO_SENTRY_DSN=https://abc@o0.ingest.sentry.io/123
./samp-server
```

No code change, no rebuild — the plugin reads the DSN on plugin load
and the next emit shows up in the Sentry dashboard.

## Dependency footprint

`sentry = "0.43"` with `default-features = false` and
`features = ["reqwest", "rustls", "contexts"]` pulls in a non-trivial
tree (Tokio runtime + rustls + hyper + reqwest + sentry-core +
sentry-contexts). That cost is paid only by plugins that ship this
integration; the `rust-samp` SDK itself stays dependency-free on this
axis.

## Privacy note

`rust-samp` itself never registers a sink. This example is the
**plugin's** code, not the SDK's. Server operators auditing a plugin
can grep for `add_sink(` — zero hits means zero external traffic
from the logger. Even with this example loaded, the default fake DSN
keeps everything inert until the operator opts in.

## Pawn side

```pawn
public OnGameModeInit()
{
    SinkDemo_EmitInfo("hello from pawn");
    SinkDemo_EmitWarn("something looks off");
    SinkDemo_EmitError("something broke");
    return 1;
}

public OnPlayerCommandText(playerid, cmdtext[])
{
    if (strcmp(cmdtext, "/sinkstats", true) == 0)
    {
        new exported = SinkDemo_GetExportedCount();
        new dropped = SinkDemo_GetDroppedCount();
        printf("[sink-demo] exported=%d dropped=%d", exported, dropped);
        return 1;
    }
    return 0;
}
```

## Adapting to OpenTelemetry (or anything else)

The `Sink` trait is generic. To swap Sentry for an OTLP exporter,
replace `sentry = "0.43"` with the `opentelemetry` family in
`Cargo.toml`, replace `sentry::init(...)` + `capture_event(...)` in
the drainer thread with the equivalent OTLP `LogExporter::export`
call, and you are done. The channel + thread + bounded queue shape
stays the same.

## Build

```sh
cargo build --release --target i686-unknown-linux-gnu -p sink-demo
```

Drop the resulting `.so` / `.dll` into the server's `plugins/`.
