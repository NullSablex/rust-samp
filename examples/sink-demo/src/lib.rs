//! Worked example of `samp::logger::Sink` — a complete, working
//! **Sentry integration** for `rust-samp` plugins.
//!
//! The trait `samp::logger::Sink` is the SDK's extension point for
//! forwarding accepted log records to a destination chosen by the
//! plugin author. The SDK itself never constructs a `Sink`; every
//! active sink is the result of an explicit
//! `LoggerConfig::add_sink(...)` call inside the plugin — see the
//! `Privacy` section in the trait docs.
//!
//! # What this example ships
//!
//! - Real `sentry` crate wired up (`sentry::init` + `capture_event`).
//! - DSN read **from the env var `SINK_DEMO_SENTRY_DSN`** at plugin
//!   load. No DSN ever touches source code or VCS.
//! - Bounded `mpsc::sync_channel` between the logger lock and the
//!   Sentry client: a stuck consumer cannot grow memory and the
//!   logger never blocks on slow I/O.
//! - Background drainer thread that owns the `capture_event` call
//!   and the `sentry::ClientInitGuard` (flushes pending events on
//!   drop at plugin unload).
//! - Pawn natives that surface attempt / success / drop counts.
//!
//! # Default behavior is safe and self-contained
//!
//! When `SINK_DEMO_SENTRY_DSN` is not set, the example uses a fake
//! local DSN (`http://fake@127.0.0.1:9999/1`). The Sentry client
//! still initializes, the drainer still runs, `capture_event` still
//! returns an event id — but the underlying HTTP transport refuses
//! fast and the data never leaves the host. No real Sentry traffic,
//! no internet egress, no account required.
//!
//! To go to production, set one env var:
//!
//! ```sh
//! export SINK_DEMO_SENTRY_DSN=https://abc@o0.ingest.sentry.io/123
//! ./samp-server
//! ```
//!
//! No code change, no rebuild. Source stays clean; the secret stays
//! in the operator's environment (systemd `Environment=`, Docker
//! secret, vault sidecar, `.env` outside the repo, …).
//!
//! # Pawn side
//!
//! ```pawn
//! public OnGameModeInit()
//! {
//!     SinkDemo_EmitTest("hello from pawn");
//!     return 1;
//! }
//!
//! public OnPlayerCommandText(playerid, cmdtext[])
//! {
//!     if (strcmp(cmdtext, "/sinkstats", true) == 0)
//!     {
//!         new exported = SinkDemo_GetExportedCount();
//!         new dropped = SinkDemo_GetDroppedCount();
//!         printf("[sink-demo] exported=%d dropped=%d", exported, dropped);
//!         return 1;
//!     }
//!     return 0;
//! }
//! ```

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::thread;

use samp::prelude::*;
use samp::{initialize_plugin, native};

use samp::logger::{LoggerConfig, Sink, SinkRecord};

use sentry::protocol::{Event, Level as SentryLevel};

/// Env var holding the Sentry DSN. Source of truth at runtime; the
/// fake DSN below is only used when the env var is missing.
const SENTRY_DSN_ENV: &str = "SINK_DEMO_SENTRY_DSN";

/// Default DSN when `SINK_DEMO_SENTRY_DSN` is not set. Points at a
/// deliberately unreachable local host so the Sentry client still
/// initializes (the example runs end-to-end) but its HTTP transport
/// refuses fast — no real events leave the host until the operator
/// opts in by setting the env var.
const FAKE_SENTRY_DSN: &str = "http://fake@127.0.0.1:9999/1";

/// Owned snapshot of a [`SinkRecord`] suitable for crossing thread
/// boundaries. `SinkRecord` borrows from the active `log::Record`, so
/// it cannot itself be moved into a channel — copy the fields out.
#[derive(Debug)]
struct OwnedRecord {
    level: log::Level,
    target: String,
    message: String,
    prefix: String,
}

impl OwnedRecord {
    fn from_sink(record: &SinkRecord<'_>) -> Self {
        Self {
            level: record.level,
            target: record.target.to_owned(),
            message: record.message.to_owned(),
            prefix: record.prefix.to_owned(),
        }
    }

    /// Builds the Sentry event from the captured record. Maps the
    /// `log::Level` to `sentry::Level`, uses the `log` target as the
    /// Sentry `logger` field, and forwards the formatted message
    /// body. Real-world integrations would also attach tags
    /// (`server_id`, `gamemode`, …) and breadcrumbs here.
    fn into_sentry_event(self) -> Event<'static> {
        Event {
            level: match self.level {
                log::Level::Error => SentryLevel::Error,
                log::Level::Warn => SentryLevel::Warning,
                log::Level::Info => SentryLevel::Info,
                log::Level::Debug | log::Level::Trace => SentryLevel::Debug,
            },
            message: Some(format!("{} {}", self.prefix, self.message)),
            logger: Some(self.target),
            ..Default::default()
        }
    }
}

/// Custom `Sink` that forwards every record into a bounded channel.
///
/// The `emit` method runs **inside the logger's lock** (see the
/// `Sink::emit` doc), so it must not block on slow I/O. Push to a
/// channel and let a background thread own the Sentry call.
///
/// Bounded capacity means a stuck consumer cannot grow memory without
/// limit — when full, `try_send` drops the record (counted as
/// `dropped`) instead of stalling the plugin. Tune the capacity to
/// the expected burst size of the workload.
struct DemoSink {
    tx: SyncSender<OwnedRecord>,
    dropped: Arc<AtomicUsize>,
}

impl Sink for DemoSink {
    fn emit(&self, record: &SinkRecord<'_>) {
        if self.tx.try_send(OwnedRecord::from_sink(record)).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[derive(Default)]
struct SinkDemo {
    exported: Arc<AtomicUsize>,
    dropped: Arc<AtomicUsize>,
}

impl SinkDemo {
    /// Returns how many log records the background drainer has
    /// successfully handed off to the Sentry client since plugin
    /// load. Reset to 0 every server boot.
    ///
    /// "Handed off" here means `capture_event` returned a non-nil
    /// event id — the client accepted the event into its outbound
    /// queue. Whether Sentry's backend actually accepts the event is
    /// reported on the Sentry dashboard, not here.
    #[native(name = "SinkDemo_GetExportedCount")]
    pub fn get_exported_count(&mut self, _amx: &Amx) -> i32 {
        let v = self.exported.load(Ordering::Relaxed);
        i32::try_from(v).unwrap_or(i32::MAX)
    }

    /// Records lost — backpressure (channel full when `emit` ran) or
    /// rejected by the Sentry client before the network step. With
    /// the default fake DSN every record counts here as transport
    /// failure on flush.
    #[native(name = "SinkDemo_GetDroppedCount")]
    pub fn get_dropped_count(&mut self, _amx: &Amx) -> i32 {
        let v = self.dropped.load(Ordering::Relaxed);
        i32::try_from(v).unwrap_or(i32::MAX)
    }

    /// Emits a synthetic `info!` line through the logger pipeline. With
    /// a real DSN this becomes a `level: "info"` event in Sentry.
    #[native(name = "SinkDemo_EmitInfo")]
    #[allow(clippy::unused_self)]
    pub fn emit_info(&mut self, _amx: &Amx, message: &AmxString) -> bool {
        log::info!("[sink-demo] info from pawn: {}", &**message);
        true
    }

    /// Emits a synthetic `warn!` line. With a real DSN this becomes a
    /// `level: "warning"` event in Sentry. Useful for testing alerts
    /// that fire on `warning`.
    #[native(name = "SinkDemo_EmitWarn")]
    #[allow(clippy::unused_self)]
    pub fn emit_warn(&mut self, _amx: &Amx, message: &AmxString) -> bool {
        log::warn!("[sink-demo] warning from pawn: {}", &**message);
        true
    }

    /// Emits a synthetic `error!` line. With a real DSN this becomes a
    /// `level: "error"` event in Sentry — the same severity as a panic
    /// hook capture or a hand-built `sentry::capture_message`.
    #[native(name = "SinkDemo_EmitError")]
    #[allow(clippy::unused_self)]
    pub fn emit_error(&mut self, _amx: &Amx, message: &AmxString) -> bool {
        log::error!("[sink-demo] error from pawn: {}", &**message);
        true
    }
}

impl SampPlugin for SinkDemo {}

/// Resolves the Sentry DSN from the environment, falling back to the
/// unreachable fake when the operator has not opted in.
fn resolve_dsn() -> String {
    std::env::var(SENTRY_DSN_ENV)
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| FAKE_SENTRY_DSN.to_owned())
}

initialize_plugin!(
    natives: [
        SinkDemo::get_exported_count,
        SinkDemo::get_dropped_count,
        SinkDemo::emit_info,
        SinkDemo::emit_warn,
        SinkDemo::emit_error,
    ],
    {
        let exported = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicUsize::new(0));

        let dsn = resolve_dsn();
        let using_fake_dsn = dsn == FAKE_SENTRY_DSN;

        // Bounded channel: enough headroom for normal bursts, small
        // enough that a deadlocked consumer cannot eat memory.
        let (tx, rx) = mpsc::sync_channel::<OwnedRecord>(1024);

        // Background drainer: owns the Sentry client guard. The
        // guard's `Drop` flushes pending events with a default
        // timeout — keeping it alive inside the thread means the
        // flush runs at plugin unload, not at the end of `on_load`.
        let exported_for_thread = Arc::clone(&exported);
        let dropped_for_thread = Arc::clone(&dropped);
        thread::Builder::new()
            .name("sink-demo-drainer".into())
            .spawn(move || {
                // `ClientInitGuard` keeps the Sentry transport
                // running for the lifetime of this thread.
                let _guard = sentry::init((
                    dsn,
                    sentry::ClientOptions {
                        release: sentry::release_name!(),
                        ..Default::default()
                    },
                ));

                while let Ok(record) = rx.recv() {
                    let event = record.into_sentry_event();
                    let event_id = sentry::capture_event(event);
                    if event_id == sentry::types::Uuid::nil() {
                        dropped_for_thread.fetch_add(1, Ordering::Relaxed);
                    } else {
                        exported_for_thread.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
            .expect("failed to spawn sink-demo drainer thread");

        let cfg = LoggerConfig::new(env!("CARGO_PKG_NAME")).add_sink(Box::new(DemoSink {
            tx,
            dropped: Arc::clone(&dropped),
        }));
        let _ = samp::enable_logger_with!(cfg);

        // Logged AFTER install so the line goes through the regular
        // pipeline (file + server + sink). Logging before install
        // would be silently dropped.
        if using_fake_dsn {
            log::info!(
                "[sink-demo] Sentry initialised with the fake local DSN — set {SENTRY_DSN_ENV} to point at a real Sentry project",
            );
        } else {
            log::info!(
                "[sink-demo] Sentry initialised from {SENTRY_DSN_ENV}",
            );
            // Smoke test: emits one event at each level on plugin
            // load so the operator can immediately see the wiring is
            // working on the Sentry dashboard. Three rows will appear
            // tagged `logger: sink-demo`, levels info/warning/error.
            // Remove these in production if the noise is unwanted.
            log::info!("[sink-demo] startup smoke test — info event");
            log::warn!("[sink-demo] startup smoke test — warning event");
            log::error!("[sink-demo] startup smoke test — error event");
        }

        return SinkDemo { exported, dropped };
    }
);
