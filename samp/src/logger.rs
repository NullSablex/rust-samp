//! Turnkey logging for plugins.
//!
//! Provides a high-level `install` entry point (wrapped by the
//! [`enable_logger!`] and [`enable_logger_with!`] macros at the crate root)
//! that wires up:
//!
//! - A per-plugin file under `logs/{crate}.log` with size-based rotation
//! - A dual sink to the server's own log (SA-MP `logprintf` /
//!   open.mp `ICore::logLn`)
//! - A prefix derived from the caller's `CARGO_PKG_NAME` (overridable)
//! - A runtime-adjustable log level so the plugin can expose its own
//!   Pawn-side knob (`SetLogLevel`, etc.)
//! - A startup banner that introspects `CARGO_PKG_*` at the caller's site
//!
//! Plain `log::info!` / `log::warn!` / `log::error!` calls inside the
//! plugin route through this implementation once installed; there is no
//! parallel set of helper macros to remember.
//!
//! [`enable_logger!`]: crate::enable_logger
//! [`enable_logger_with!`]: crate::enable_logger_with

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::ptr;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, Ordering};

use log::{LevelFilter, Log, Metadata, Record};
use time::OffsetDateTime;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;

use crate::runtime::Runtime;

/// File timestamp format — `YYYY-MM-DD HH:MM:SS`.
const TIMESTAMP_FORMAT: &[BorrowedFormatItem<'_>] =
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");

/// Default rotation threshold — 50 MB per archived file.
const DEFAULT_ROTATION_BYTES: u64 = 50 * 1024 * 1024;

/// Default layout for lines written to the plugin's dedicated log file.
/// Placeholders: `{timestamp}` (`YYYY-MM-DD HH:MM:SS`), `{level}` (`INFO`,
/// `WARN`, ...), `{message}` (formatted args).
const DEFAULT_FILE_FORMAT: &str = "[{timestamp}] [{level}] {message}";

/// Default layout for lines forwarded to the server console. The server
/// adds its own timestamp; the SDK only contributes prefix + level +
/// message. Placeholders: `{prefix}`, `{level}`, `{message}`.
const DEFAULT_SERVER_FORMAT: &str = "{prefix} {message}";

/// Configuration for [`install`]. Built via the fluent setters; defaults
/// are derived from `CARGO_PKG_NAME` (captured at the caller's compile time
/// by [`enable_logger!`]).
///
/// [`enable_logger!`]: crate::enable_logger
pub struct LoggerConfig {
    crate_name: String,
    directory: PathBuf,
    filename: Option<String>,
    prefix: Option<String>,
    level: LevelFilter,
    also_to_server: bool,
    banner: BannerMode,
    rotation: Option<Rotation>,
    file_format: String,
    server_format: String,
    /// When the `compression` Cargo feature is enabled, rotated archives
    /// are gzipped into `{filename}.{N}.gz` and the uncompressed file is
    /// removed. Opt-in via [`LoggerConfig::compress_archives`].
    #[cfg(feature = "compression")]
    compress_archives: bool,
    /// Additional log sinks supplied by the plugin author. The SDK never
    /// instantiates one of these on its own — see [`Sink`].
    sinks: Vec<Box<dyn Sink>>,
}

/// Receiver of formatted log records for forwarding to an **external
/// destination chosen by the plugin author** — Sentry, an OTLP
/// collector, an in-house HTTP endpoint, a database, anything.
///
/// # Privacy
///
/// The SDK never installs a `Sink` on its own. Records are only ever
/// forwarded when the plugin's own source code calls
/// [`LoggerConfig::add_sink`] with an instance. There is no default
/// destination, no telemetry built into `rust-samp`, no opt-out
/// switch hiding silent network traffic — if your plugin does not
/// construct a `Sink`, nothing leaves the host.
///
/// Server operators who want to audit this can grep the plugin's
/// source for `add_sink(`: zero hits means nothing is exported. The
/// SDK itself contains zero `add_sink` calls.
///
/// # Implementing
///
/// Implement [`Sink::emit`] to forward records however you like. The
/// method is called from inside the logger's lock and should not
/// block on slow I/O — use a background thread / channel if your
/// transport is HTTP-based.
pub trait Sink: Send + Sync {
    /// Called once per accepted log record.
    fn emit(&self, record: &SinkRecord<'_>);
}

/// Single log record handed to a [`Sink`]. Borrows from the active
/// `log::Record` and the SDK's per-record context — do not retain
/// references past the `emit` call.
#[derive(Debug)]
pub struct SinkRecord<'a> {
    /// Formatted timestamp (`YYYY-MM-DD HH:MM:SS`) of the record.
    pub timestamp: &'a str,
    /// Log level of the record (`ERROR`, `WARN`, `INFO`, `DEBUG`, `TRACE`).
    pub level: log::Level,
    /// Log target reported by the `log::Record` (the originating
    /// module path by default).
    pub target: &'a str,
    /// Formatted message body.
    pub message: &'a str,
    /// Plugin prefix (`[crate-name]` by default).
    pub prefix: &'a str,
}

/// Type alias for the custom banner builder — receives the metadata
/// captured by the macro and returns the lines to render.
pub type BannerBuilder = dyn Fn(&BannerMetadata) -> Vec<String> + Send + Sync;

/// What [`install`] does when the configuration's banner is reached.
pub enum BannerMode {
    /// No banner at all.
    Off,
    /// Built-in 5-line banner with `CARGO_PKG_NAME`, version, authors and
    /// repository. The standard choice.
    Default,
    /// Lines produced by the caller. Each line goes out at `Info` level
    /// through the same pipeline as the rest of the logger.
    Custom(Box<BannerBuilder>),
}

impl std::fmt::Debug for BannerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => f.write_str("Off"),
            Self::Default => f.write_str("Default"),
            Self::Custom(_) => f.write_str("Custom(<fn>)"),
        }
    }
}

impl std::fmt::Debug for LoggerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("LoggerConfig");
        s.field("crate_name", &self.crate_name)
            .field("directory", &self.directory)
            .field("filename", &self.filename)
            .field("prefix", &self.prefix)
            .field("level", &self.level)
            .field("also_to_server", &self.also_to_server)
            .field("banner", &self.banner)
            .field("rotation", &self.rotation)
            .field("file_format", &self.file_format)
            .field("server_format", &self.server_format)
            .field("sinks", &format_args!("[{} sink(s)]", self.sinks.len()));
        #[cfg(feature = "compression")]
        s.field("compress_archives", &self.compress_archives);
        s.finish()
    }
}

/// Size-based rotation rules.
///
/// `keep`:
/// - `Some(N)` — shift-style rotation: the active file becomes
///   `{name}.log.1`, existing archives shift down (`.1` → `.2`, ...),
///   and `.log.N` is deleted to enforce the cap. The most recent `N`
///   archives stay on disk.
/// - `None` — append-style rotation: every rotated file is renamed to
///   the next free `{name}.log.{index}` slot and never deleted by the
///   SDK. The dev keeps full control over cleanup (manual, `logrotate`,
///   external script, etc.).
#[derive(Debug, Clone, Copy)]
struct Rotation {
    max_bytes: u64,
    keep: Option<u32>,
}

impl LoggerConfig {
    /// Builds a config seeded from the caller's `CARGO_PKG_NAME` — the
    /// `enable_logger!` macro is the intended entry point and forwards
    /// `env!("CARGO_PKG_NAME")` here automatically.
    #[must_use]
    pub fn new(crate_name: impl Into<String>) -> Self {
        Self {
            crate_name: crate_name.into(),
            directory: PathBuf::from("logs"),
            filename: None,
            prefix: None,
            level: LevelFilter::Info,
            also_to_server: true,
            banner: BannerMode::Default,
            rotation: Some(Rotation {
                max_bytes: DEFAULT_ROTATION_BYTES,
                // Never auto-delete by default. The dev opts in to
                // pruning via `.rotation_keep(N)`.
                keep: None,
            }),
            file_format: DEFAULT_FILE_FORMAT.to_owned(),
            server_format: DEFAULT_SERVER_FORMAT.to_owned(),
            #[cfg(feature = "compression")]
            compress_archives: false,
            sinks: Vec::new(),
        }
    }

    /// Registers an additional [`Sink`] that will receive every accepted
    /// log record alongside the file and server writes.
    ///
    /// **The SDK does not install any sink on its own.** This builder is
    /// the *only* way a `Sink` ends up active — calling it is an
    /// explicit choice by the plugin author. There is no hidden
    /// telemetry, no default destination, no environment variable that
    /// flips this on, and no automatic data collection. Server
    /// operators auditing what a `rust-samp` plugin can export only need
    /// to grep its source for `add_sink(` — zero hits means zero
    /// external traffic from the logger.
    ///
    /// Multiple sinks may be registered; each one receives every record.
    /// The order of calls is the order of dispatch.
    ///
    /// Sinks run inside the logger's lock — for HTTP-style backends
    /// (Sentry, OTLP, …) forward to a background thread / channel
    /// rather than calling the network from `emit`.
    #[must_use]
    pub fn add_sink(mut self, sink: Box<dyn Sink>) -> Self {
        self.sinks.push(sink);
        self
    }

    /// Gzip-compresses every rotated archive into `{filename}.{N}.gz` and
    /// removes the uncompressed file. Off by default.
    ///
    /// Requires the `compression` Cargo feature, which pulls in `flate2`
    /// with the pure-Rust backend. Compression runs synchronously inside
    /// the rotation step — for verbose plugins this is a worthwhile
    /// trade vs. unbounded disk usage; for low-volume plugins it is
    /// usually unnecessary.
    ///
    /// Compatible with both rotation strategies (append-style and the
    /// `rotation_keep(N)` shift-style cleanup).
    #[cfg(feature = "compression")]
    #[must_use]
    pub fn compress_archives(mut self, yes: bool) -> Self {
        self.compress_archives = yes;
        self
    }

    /// Applies overrides read from environment variables. Lets server
    /// operators retune the logger **without recompiling** the plugin —
    /// flip a level, redirect the directory, change the rotation
    /// threshold etc. by exporting an env var before starting the
    /// server.
    ///
    /// The prefix is derived from the crate name passed to
    /// [`LoggerConfig::new`] (typically `CARGO_PKG_NAME`) uppercased,
    /// with non-alphanumeric characters replaced by `_`. For a plugin
    /// named `streamer-rs` the prefix is `STREAMER_RS_LOG_`.
    ///
    /// | Env var | Equivalent builder |
    /// | --- | --- |
    /// | `<PREFIX>_LOG_LEVEL` (`off`/`error`/`warn`/`info`/`debug`/`trace`) | [`level`](Self::level) |
    /// | `<PREFIX>_LOG_DIR` (path) | [`directory`](Self::directory) |
    /// | `<PREFIX>_LOG_FILE` (filename) | [`filename`](Self::filename) |
    /// | `<PREFIX>_LOG_ROTATION_MB` (u64) | [`rotation_size_mb`](Self::rotation_size_mb) |
    /// | `<PREFIX>_LOG_ROTATION_KEEP` (u32) | [`rotation_keep`](Self::rotation_keep) |
    /// | `<PREFIX>_LOG_NO_ROTATION` (`1`/`true`) | [`no_rotation`](Self::no_rotation) |
    /// | `<PREFIX>_LOG_NO_BANNER` (`1`/`true`) | [`no_banner`](Self::no_banner) |
    /// | `<PREFIX>_LOG_SERVER` (`0`/`false`) | [`also_to_server(false)`](Self::also_to_server) |
    /// | `<PREFIX>_LOG_COMPRESS` (`1`/`true`, requires `compression` feature) | [`compress_archives(true)`](Self::compress_archives) |
    ///
    /// Missing env vars leave the existing value untouched, so calling
    /// `.from_env()` at the end of a builder chain treats the env vars
    /// as **overrides** of the code defaults — production wins. Invalid
    /// values (unparseable integers, unknown level names) are reported
    /// through the server console at startup and the previous value is
    /// kept.
    #[must_use]
    pub fn from_env(mut self) -> Self {
        let prefix = env_var_prefix(&self.crate_name);
        self.apply_env(&prefix);
        self
    }

    fn apply_env(&mut self, prefix: &str) {
        if let Some(raw) = read_env(prefix, "LEVEL") {
            match parse_level(&raw) {
                Some(l) => self.level = l,
                None => warn_invalid(prefix, "LEVEL", &raw),
            }
        }
        if let Some(raw) = read_env(prefix, "DIR") {
            self.directory = PathBuf::from(raw);
        }
        if let Some(raw) = read_env(prefix, "FILE") {
            self.filename = Some(raw);
        }
        if let Some(raw) = read_env(prefix, "ROTATION_MB") {
            match raw.parse::<u64>() {
                Ok(0) => self.rotation = None,
                Ok(mb) => {
                    let max_bytes = mb.saturating_mul(1024 * 1024);
                    let keep = self.rotation.and_then(|r| r.keep);
                    self.rotation = Some(Rotation { max_bytes, keep });
                }
                Err(_) => warn_invalid(prefix, "ROTATION_MB", &raw),
            }
        }
        if let Some(raw) = read_env(prefix, "ROTATION_KEEP") {
            match raw.parse::<u32>() {
                Ok(keep) => {
                    let max_bytes = self
                        .rotation
                        .map_or(DEFAULT_ROTATION_BYTES, |r| r.max_bytes);
                    self.rotation = Some(Rotation {
                        max_bytes,
                        keep: Some(keep),
                    });
                }
                Err(_) => warn_invalid(prefix, "ROTATION_KEEP", &raw),
            }
        }
        if let Some(raw) = read_env(prefix, "NO_ROTATION")
            && parse_bool(&raw)
        {
            self.rotation = None;
        }
        if let Some(raw) = read_env(prefix, "NO_BANNER")
            && parse_bool(&raw)
        {
            self.banner = BannerMode::Off;
        }
        if let Some(raw) = read_env(prefix, "SERVER") {
            self.also_to_server = parse_bool(&raw);
        }
        #[cfg(feature = "compression")]
        if let Some(raw) = read_env(prefix, "COMPRESS") {
            self.compress_archives = parse_bool(&raw);
        }
    }

    /// Directory under which the active log file lives. Default: `logs/`.
    /// The path is resolved relative to the server's working directory.
    /// Rotated archives are always placed under `{directory}/archive/` —
    /// the active log stays directly in `directory` so the folder root
    /// shows only current files.
    #[must_use]
    pub fn directory(mut self, path: impl Into<PathBuf>) -> Self {
        self.directory = path.into();
        self
    }

    /// Filename inside [`directory`]. Default: `{crate-name}.log`.
    ///
    /// [`directory`]: Self::directory
    #[must_use]
    pub fn filename(mut self, name: impl Into<String>) -> Self {
        self.filename = Some(name.into());
        self
    }

    /// Prefix prepended to every line written to the server's log.
    /// Default: `[{crate-name}]`. The plugin's dedicated file omits the
    /// prefix because every line in it already belongs to this plugin.
    #[must_use]
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Threshold below which `log::warn!`, `log::info!` and friends are
    /// silently dropped. Default: [`LevelFilter::Info`]. Can be adjusted
    /// at runtime via [`set_level`].
    #[must_use]
    pub fn level(mut self, level: LevelFilter) -> Self {
        self.level = level;
        self
    }

    /// Whether each log line is also forwarded to the server's own log
    /// (visible in the server console and the server's main log file).
    /// Default: `true`.
    #[must_use]
    pub fn also_to_server(mut self, enabled: bool) -> Self {
        self.also_to_server = enabled;
        self
    }

    /// Selects the banner strategy. Default: [`BannerMode::Default`] (the
    /// built-in 5-line banner). Pass [`BannerMode::Off`] to suppress
    /// every banner line, or [`BannerMode::Custom`] to render the lines
    /// yourself.
    #[must_use]
    pub fn banner(mut self, mode: BannerMode) -> Self {
        self.banner = mode;
        self
    }

    /// Shorthand for `banner(BannerMode::Off)`.
    #[must_use]
    pub fn no_banner(mut self) -> Self {
        self.banner = BannerMode::Off;
        self
    }

    /// Shorthand for `banner(BannerMode::Custom(Box::new(builder)))`.
    /// `builder` receives the manifest fields captured by the macro and
    /// returns the lines to render — each line goes out at `Info` level
    /// through the same pipeline as the rest of the logger.
    #[must_use]
    pub fn banner_with<F>(mut self, builder: F) -> Self
    where
        F: Fn(&BannerMetadata) -> Vec<String> + Send + Sync + 'static,
    {
        self.banner = BannerMode::Custom(Box::new(builder));
        self
    }

    /// Layout for lines written to the plugin's dedicated log file.
    /// Placeholders honoured: `{timestamp}`, `{level}`, `{message}`.
    /// Default: `"[{timestamp}] [{level}] {message}"`.
    #[must_use]
    pub fn file_format(mut self, format: impl Into<String>) -> Self {
        self.file_format = format.into();
        self
    }

    /// Layout for lines forwarded to the server console.
    /// Placeholders honoured: `{prefix}`, `{level}`, `{message}`.
    /// Default: `"{prefix} {message}"`.
    #[must_use]
    pub fn server_format(mut self, format: impl Into<String>) -> Self {
        self.server_format = format.into();
        self
    }

    /// Disable size-based rotation entirely. The active log file grows
    /// indefinitely — only set this if an external rotator (e.g.
    /// `logrotate`) takes over.
    #[must_use]
    pub fn no_rotation(mut self) -> Self {
        self.rotation = None;
        self
    }

    /// Threshold at which the active file is rotated, in megabytes.
    /// Default: 50 MB. Disables rotation if set to 0.
    ///
    /// Whether old archives are deleted is controlled separately by
    /// [`rotation_keep`] — by default the SDK never deletes; it only
    /// renames into the archive directory.
    ///
    /// [`rotation_keep`]: Self::rotation_keep
    #[must_use]
    pub fn rotation_size_mb(mut self, mb: u64) -> Self {
        if mb == 0 {
            self.rotation = None;
        } else {
            let max_bytes = mb.saturating_mul(1024 * 1024);
            let keep = self.rotation.and_then(|r| r.keep);
            self.rotation = Some(Rotation { max_bytes, keep });
        }
        self
    }

    /// Opts in to size-bounded cleanup: keep the latest `keep` archives
    /// (newest = `.log.1`, oldest = `.log.{keep}`) and delete anything
    /// older. Total disk footprint becomes `(keep + 1) * rotation_size_mb`.
    ///
    /// Off by default — the SDK never deletes log files unless the dev
    /// explicitly requests it.
    #[must_use]
    pub fn rotation_keep(mut self, keep: u32) -> Self {
        let max_bytes = self
            .rotation
            .map_or(DEFAULT_ROTATION_BYTES, |r| r.max_bytes);
        self.rotation = Some(Rotation {
            max_bytes,
            keep: Some(keep),
        });
        self
    }

    /// Reverts to append-style rotation — every rotated file gets a
    /// fresh, never-reused index and the SDK never deletes anything.
    /// This is the default; the method exists so a builder chain can
    /// undo a previous `.rotation_keep(N)`.
    #[must_use]
    pub fn rotation_no_cleanup(mut self) -> Self {
        let max_bytes = self
            .rotation
            .map_or(DEFAULT_ROTATION_BYTES, |r| r.max_bytes);
        self.rotation = Some(Rotation {
            max_bytes,
            keep: None,
        });
        self
    }

    fn resolved_filename(&self) -> String {
        self.filename
            .clone()
            .unwrap_or_else(|| format!("{}.log", self.crate_name))
    }

    fn resolved_prefix(&self) -> String {
        self.prefix
            .clone()
            .unwrap_or_else(|| format!("[{}]", self.crate_name))
    }

    fn log_path(&self) -> PathBuf {
        self.directory.join(self.resolved_filename())
    }

    fn resolved_archive_directory(&self) -> PathBuf {
        self.directory.join("archive")
    }
}

/// Errors returned by [`install`].
#[derive(Debug)]
pub enum InstallError {
    /// The logger was already installed in this process. `log::set_logger`
    /// rejects a second installation, so the SDK enforces the same.
    AlreadyInstalled,
    /// Creating the directory or opening the log file failed.
    Io(std::io::Error),
}

impl std::fmt::Display for InstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyInstalled => f.write_str("logger already installed"),
            Self::Io(e) => write!(f, "i/o error: {e}"),
        }
    }
}

impl std::error::Error for InstallError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::AlreadyInstalled => None,
            Self::Io(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for InstallError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

/// Sentinel preventing two `install` calls from racing.
static INSTALLED: AtomicBool = AtomicBool::new(false);

/// Raw pointer to the live `LoggerImpl` after a successful `install`.
/// Set before `log::set_boxed_logger` takes ownership of the box, so it
/// always points to the same allocation the `log` crate dispatches to.
/// Used by [`flush`] to reach the file handle directly without going
/// through `log::logger()` (whose `flush` is a no-op for any logger that
/// returns from `Log::flush` without forcing the OS-level sync).
static INSTANCE: AtomicPtr<LoggerImpl> = AtomicPtr::new(ptr::null_mut());

/// Runtime-adjustable level filter. Mirrors `log::set_max_level` but lets
/// the SDK route through `LevelFilter` without re-importing the crate.
static LEVEL: AtomicU8 = AtomicU8::new(level_to_u8(LevelFilter::Info));

const fn level_to_u8(l: LevelFilter) -> u8 {
    match l {
        LevelFilter::Off => 0,
        LevelFilter::Error => 1,
        LevelFilter::Warn => 2,
        LevelFilter::Info => 3,
        LevelFilter::Debug => 4,
        LevelFilter::Trace => 5,
    }
}

const fn u8_to_level(v: u8) -> LevelFilter {
    match v {
        0 => LevelFilter::Off,
        1 => LevelFilter::Error,
        2 => LevelFilter::Warn,
        3 => LevelFilter::Info,
        4 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    }
}

/// Adjusts the global threshold without reinstalling. Intended for plugin
/// natives that expose a runtime knob — bind it to a Pawn-callable
/// helper such as `MyPlugin_SetLogLevel(level)`.
pub fn set_level(level: LevelFilter) {
    LEVEL.store(level_to_u8(level), Ordering::Relaxed);
    log::set_max_level(level);
}

/// Current threshold — useful for diagnostic natives that want to report
/// the active level back to scripts.
#[must_use]
pub fn level() -> LevelFilter {
    u8_to_level(LEVEL.load(Ordering::Relaxed))
}

/// Forces the active log file to flush to disk.
///
/// `log::logger().flush()` only triggers the `flush` method of the
/// currently registered logger, which is not guaranteed to drain the
/// SDK's file handle when called via the trait object. This helper
/// reaches the live `LoggerImpl` directly and calls `File::flush`
/// under the same `Mutex` the writer uses, so panic hooks and
/// shutdown paths can persist pending lines deterministically.
///
/// No-op when the logger has not been installed.
pub fn flush() {
    let ptr = INSTANCE.load(Ordering::Acquire);
    if ptr.is_null() {
        return;
    }
    // Safety: `INSTANCE` is set only inside `install` from the same
    // allocation handed to `log::set_boxed_logger`, which leaks it for
    // 'static. It is never written again, so the pointee outlives any
    // call to `flush`.
    let logger = unsafe { &*ptr };
    log::Log::flush(logger);
}

// ---------------------------------------------------------------------------
// Installer
// ---------------------------------------------------------------------------

/// Installs the SDK logger as the global `log` implementation.
///
/// Called by the [`enable_logger!`] and [`enable_logger_with!`] macros;
/// rarely invoked directly. Prefer the macros — they capture the caller's
/// `CARGO_PKG_NAME` for the default prefix and filename.
///
/// # Errors
/// - [`InstallError::AlreadyInstalled`] if a logger (this one or any
///   other) was already registered with the `log` crate in this process.
/// - [`InstallError::Io`] if the log directory or file could not be
///   opened.
///
/// [`enable_logger!`]: crate::enable_logger
/// [`enable_logger_with!`]: crate::enable_logger_with
pub fn install(config: LoggerConfig) -> Result<(), InstallError> {
    if INSTALLED.swap(true, Ordering::AcqRel) {
        return Err(InstallError::AlreadyInstalled);
    }

    fs::create_dir_all(&config.directory)?;
    let path = config.log_path();
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    let initial_size = file.metadata().map(|m| m.len()).unwrap_or(0);

    let prefix = config.resolved_prefix();
    let level = config.level;
    let filename = config.resolved_filename();
    let archive_directory = config.resolved_archive_directory();
    let next_archive_index = find_next_archive_index(&archive_directory, &filename);
    #[cfg(feature = "compression")]
    let compress_archives = config.compress_archives;
    let LoggerConfig {
        also_to_server,
        banner,
        rotation,
        file_format,
        server_format,
        sinks,
        ..
    } = config;

    let logger = Box::new(LoggerImpl {
        prefix,
        also_to_server,
        rotation,
        path,
        filename,
        archive_directory,
        file_format,
        server_format,
        #[cfg(feature = "compression")]
        compress_archives,
        sinks,
        state: Mutex::new(LoggerState {
            file: Some(file),
            current_size: initial_size,
            file_write_reported: false,
            next_archive_index,
        }),
    });

    set_level(level);

    let logger_ptr: *const LoggerImpl = &raw const *logger;
    INSTANCE.store(logger_ptr.cast_mut(), Ordering::Release);
    log::set_boxed_logger(logger).map_err(|_| {
        INSTANCE.store(ptr::null_mut(), Ordering::Release);
        INSTALLED.store(false, Ordering::Release);
        InstallError::AlreadyInstalled
    })?;

    print_banner_inner(&banner);

    Ok(())
}

// ---------------------------------------------------------------------------
// log::Log implementation
// ---------------------------------------------------------------------------

struct LoggerImpl {
    prefix: String,
    also_to_server: bool,
    rotation: Option<Rotation>,
    path: PathBuf,
    filename: String,
    archive_directory: PathBuf,
    file_format: String,
    server_format: String,
    #[cfg(feature = "compression")]
    compress_archives: bool,
    sinks: Vec<Box<dyn Sink>>,
    state: Mutex<LoggerState>,
}

struct LoggerState {
    file: Option<File>,
    current_size: u64,
    /// `true` once a file-write failure has been surfaced to the server
    /// console. Prevents one transient I/O glitch from spamming the log
    /// loop with the same error on every line.
    file_write_reported: bool,
    /// Next archive index to use under append-style rotation
    /// (`rotation.keep == None`). Seeded by [`find_next_archive_index`]
    /// at install time; bumped on every rotate.
    next_archive_index: u32,
}

impl Log for LoggerImpl {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= u8_to_level(LEVEL.load(Ordering::Relaxed))
    }

    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let message = format!("{}", record.args());
        let level = record.level().as_str();

        let timestamp = OffsetDateTime::now_local()
            .unwrap_or_else(|_| OffsetDateTime::now_utc())
            .format(TIMESTAMP_FORMAT)
            .unwrap_or_else(|_| String::from("0000-00-00 00:00:00"));

        // Forward to the server's own log honouring `server_format`.
        if self.also_to_server {
            let server_line = apply_format(
                &self.server_format,
                Some(&self.prefix),
                &timestamp,
                level,
                &message,
            );
            Runtime::get().log(server_line);
        }

        // Write to the plugin's dedicated file honouring `file_format`.
        let mut line = apply_format(&self.file_format, None, &timestamp, level, &message);
        line.push('\n');

        let mut state = match self.state.lock() {
            Ok(s) => s,
            Err(p) => p.into_inner(),
        };

        if let Some(rotation) = self.rotation
            && state.current_size + line.len() as u64 > rotation.max_bytes
        {
            self.rotate(&mut state, rotation);
        }

        if let Some(file) = state.file.as_mut() {
            match file.write_all(line.as_bytes()) {
                Ok(()) => state.current_size += line.len() as u64,
                Err(e) => {
                    if !state.file_write_reported {
                        state.file_write_reported = true;
                        Runtime::get().log(format!(
                            "{} failed to write {}: {}. Further file-write errors will be suppressed.",
                            self.prefix,
                            self.path.display(),
                            e,
                        ));
                    }
                }
            }
        }

        // Forward to plugin-supplied external sinks (Sentry, OTLP, custom
        // backends). Empty by default — the SDK never registers a sink.
        if !self.sinks.is_empty() {
            let sink_record = SinkRecord {
                timestamp: &timestamp,
                level: record.level(),
                target: record.target(),
                message: &message,
                prefix: &self.prefix,
            };
            for sink in &self.sinks {
                sink.emit(&sink_record);
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut state) = self.state.lock()
            && let Some(file) = state.file.as_mut()
        {
            let _ = file.flush();
        }
    }
}

impl LoggerImpl {
    /// Closes the active file, moves it into the archive directory and
    /// reopens a fresh one. Two strategies depending on `rotation.keep`:
    ///
    /// - `Some(N > 0)`: shift-style. Deletes `.log.N`, shifts every
    ///   existing archive down by one (`.{i}` → `.{i+1}`), active becomes
    ///   `.log.1`. The most recent `N` archives are retained.
    /// - `None` (or `Some(0)`): append-style. Active is renamed to the
    ///   next free `.log.{next_archive_index}` slot, with no cleanup.
    fn rotate(&self, state: &mut LoggerState, rotation: Rotation) {
        // Drop the file handle before renaming to avoid Windows file locks.
        state.file = None;

        // Lazy: only create when the first rotation actually happens.
        if let Err(e) = fs::create_dir_all(&self.archive_directory) {
            self.report_file_error(state, "create archive directory", &e);
            // We still try to open a fresh active file below so logging
            // does not die outright.
            self.reopen_active(state);
            return;
        }

        match rotation.keep {
            Some(keep) if keep > 0 => self.rotate_shift(keep),
            // Append-style: `None` or `Some(0)`. Active → next free slot.
            _ => {
                let index = state.next_archive_index;
                state.next_archive_index = state.next_archive_index.saturating_add(1);
                let archived = self.archive_path(index);
                if fs::rename(&self.path, &archived).is_ok() {
                    self.compress_archive(&archived);
                }
            }
        }

        self.reopen_active(state);
    }

    /// Gzips `archived` into `archived.gz` and removes the original when
    /// the `compression` feature is enabled **and** the dev opted in via
    /// [`LoggerConfig::compress_archives`]. Silent no-op otherwise.
    ///
    /// Errors are intentionally swallowed: rotation must not block log
    /// emission. A failure leaves the uncompressed `.log.N` in place,
    /// which is still strictly better than data loss.
    #[cfg(feature = "compression")]
    fn compress_archive(&self, archived: &std::path::Path) {
        if !self.compress_archives {
            return;
        }
        let gz_path = {
            let mut p = archived.as_os_str().to_owned();
            p.push(".gz");
            PathBuf::from(p)
        };
        let Ok(input) = fs::File::open(archived) else {
            return;
        };
        let Ok(output) = fs::File::create(&gz_path) else {
            return;
        };
        let mut encoder = flate2::write::GzEncoder::new(output, flate2::Compression::default());
        let mut reader = std::io::BufReader::new(input);
        if std::io::copy(&mut reader, &mut encoder).is_err() {
            let _ = fs::remove_file(&gz_path);
            return;
        }
        if encoder.finish().is_err() {
            let _ = fs::remove_file(&gz_path);
            return;
        }
        let _ = fs::remove_file(archived);
    }

    #[cfg(not(feature = "compression"))]
    #[allow(clippy::unused_self)]
    fn compress_archive(&self, _archived: &std::path::Path) {}

    /// Shift-style rotation: `.log.{keep}` is dropped, `.{i}` shifts to
    /// `.{i+1}`, active becomes `.log.1`. When the `compression` feature
    /// is enabled, `.gz` variants are handled in lockstep so a mixed
    /// archive directory (compressed + uncompressed) stays consistent.
    fn rotate_shift(&self, keep: u32) {
        let _ = fs::remove_file(self.archive_path(keep));
        #[cfg(feature = "compression")]
        let _ = fs::remove_file(append_gz(&self.archive_path(keep)));
        for index in (1..keep).rev() {
            let src = self.archive_path(index);
            let dst = self.archive_path(index + 1);
            if src.exists() {
                let _ = fs::rename(&src, &dst);
            }
            #[cfg(feature = "compression")]
            {
                let src_gz = append_gz(&src);
                let dst_gz = append_gz(&dst);
                if src_gz.exists() {
                    let _ = fs::rename(&src_gz, &dst_gz);
                }
            }
        }
        let archived = self.archive_path(1);
        if fs::rename(&self.path, &archived).is_ok() {
            self.compress_archive(&archived);
        }
    }

    fn reopen_active(&self, state: &mut LoggerState) {
        match OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            Ok(file) => {
                state.file = Some(file);
                state.current_size = 0;
            }
            Err(e) => self.report_file_error(state, "reopen", &e),
        }
    }

    fn report_file_error(&self, state: &mut LoggerState, action: &str, e: &std::io::Error) {
        if !state.file_write_reported {
            state.file_write_reported = true;
            Runtime::get().log(format!(
                "{} failed to {} {}: {}. Further file-write errors will be suppressed.",
                self.prefix,
                action,
                self.path.display(),
                e,
            ));
        }
    }

    fn archive_path(&self, index: u32) -> PathBuf {
        self.archive_directory
            .join(format!("{}.{}", self.filename, index))
    }
}

/// Uppercased prefix derived from the crate name for env var lookup.
/// Non-alphanumeric characters become `_`. `streamer-rs` → `STREAMER_RS`.
fn env_var_prefix(crate_name: &str) -> String {
    crate_name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn read_env(prefix: &str, key: &str) -> Option<String> {
    let name = format!("{prefix}_LOG_{key}");
    std::env::var(&name).ok().filter(|s| !s.is_empty())
}

fn parse_level(raw: &str) -> Option<LevelFilter> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "off" => Some(LevelFilter::Off),
        "error" => Some(LevelFilter::Error),
        "warn" | "warning" => Some(LevelFilter::Warn),
        "info" => Some(LevelFilter::Info),
        "debug" => Some(LevelFilter::Debug),
        "trace" => Some(LevelFilter::Trace),
        _ => None,
    }
}

fn parse_bool(raw: &str) -> bool {
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Surfaces an invalid env var value to the server console at install
/// time. The logger is not registered yet, so this routes through the
/// raw [`Runtime`] log sink directly. Falls back to `eprintln!` when
/// the runtime is not initialised (e.g. unit tests that exercise
/// [`LoggerConfig::from_env`] in isolation).
fn warn_invalid(prefix: &str, key: &str, raw: &str) {
    let msg = format!(
        "[rust-samp] ignoring invalid env var {prefix}_LOG_{key}={raw:?} — keeping previous value",
    );
    if let Some(rt) = Runtime::try_get() {
        rt.log(msg);
    } else {
        eprintln!("{msg}");
    }
}

#[cfg(feature = "compression")]
fn append_gz(path: &std::path::Path) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(".gz");
    PathBuf::from(s)
}

/// Scans the archive directory for existing `{filename}.{N}` siblings of
/// the active log and returns the next free `N`. Used to seed
/// [`LoggerState::next_archive_index`] so append-style rotation never
/// reuses an index across restarts.
fn find_next_archive_index(archive_dir: &std::path::Path, filename: &str) -> u32 {
    let prefix = format!("{filename}.");
    let mut max = 0u32;
    if let Ok(entries) = fs::read_dir(archive_dir) {
        for entry in entries.flatten() {
            if let Some(name) = entry.file_name().to_str()
                && let Some(rest) = name.strip_prefix(&prefix)
            {
                // Accept both `.{N}` and `.{N}.gz` so the next index
                // skips slots reused by an earlier compressed rotation.
                let idx_str = rest.strip_suffix(".gz").unwrap_or(rest);
                if let Ok(index) = idx_str.parse::<u32>() {
                    max = max.max(index);
                }
            }
        }
    }
    max.saturating_add(1)
}

// ---------------------------------------------------------------------------
// Banner
// ---------------------------------------------------------------------------

thread_local! {
    /// Captured by [`crate::enable_logger`] before [`install`] is called so
    /// the banner can introspect the caller's manifest. Each macro
    /// invocation overwrites it, which is fine because installation is a
    /// one-shot event per process.
    static BANNER_METADATA: std::cell::RefCell<Option<BannerMetadata>> =
        const { std::cell::RefCell::new(None) };
}

/// Macro plumbing — captures the caller's `CARGO_PKG_*` values so
/// [`print_banner`] can render them. Not part of the public API surface;
/// the macros call this on the user's behalf.
#[doc(hidden)]
pub fn __set_banner_metadata(metadata: BannerMetadata) {
    BANNER_METADATA.with(|cell| {
        *cell.borrow_mut() = Some(metadata);
    });
}

/// Manifest fields fed by the macro from the caller's `env!` values.
#[derive(Debug, Clone)]
pub struct BannerMetadata {
    pub name: &'static str,
    pub version: &'static str,
    pub authors: &'static str,
    pub repository: &'static str,
}

impl BannerMetadata {
    /// Constructor used by [`crate::enable_logger`] — there is no reason
    /// to call this directly; the macro is the API.
    #[must_use]
    pub fn new(
        name: &'static str,
        version: &'static str,
        authors: &'static str,
        repository: &'static str,
    ) -> Self {
        Self {
            name,
            version,
            authors,
            repository,
        }
    }
}

/// Replaces `{timestamp}`, `{level}`, `{message}` and (when provided)
/// `{prefix}` placeholders in the layout templates. Supports optional
/// alignment+width specifiers borrowed from Rust's format syntax:
///
/// - `{level:<5}` — left-aligned, padded to width 5
/// - `{level:>5}` — right-aligned, padded to width 5
/// - `{level:^5}` — centred, padded to width 5
///
/// Unknown placeholders pass through untouched so the dev can spot typos
/// in their format string.
fn apply_format(
    template: &str,
    prefix: Option<&str>,
    timestamp: &str,
    level: &str,
    message: &str,
) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'{'
            && let Some(close) = template[i + 1..].find('}')
        {
            let end = i + 1 + close;
            let spec = &template[i + 1..end];
            if let Some(rendered) = render_placeholder(spec, prefix, timestamp, level, message) {
                out.push_str(&rendered);
            } else {
                // Unknown placeholder — emit verbatim so devs see typos.
                out.push_str(&template[i..=end]);
            }
            i = end + 1;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

/// Resolves a single `{...}` group. Returns `None` for unknown names so
/// the caller can pass the raw `{spec}` through.
fn render_placeholder(
    spec: &str,
    prefix: Option<&str>,
    timestamp: &str,
    level: &str,
    message: &str,
) -> Option<String> {
    let (name, format_spec) = spec.split_once(':').unwrap_or((spec, ""));
    let value: &str = match name {
        "timestamp" => timestamp,
        "level" => level,
        "message" => message,
        "prefix" => prefix.unwrap_or(""),
        _ => return None,
    };

    if format_spec.is_empty() {
        return Some(value.to_owned());
    }

    let (alignment, width_str) = match format_spec.chars().next() {
        Some('<') => (Alignment::Left, &format_spec[1..]),
        Some('>') => (Alignment::Right, &format_spec[1..]),
        Some('^') => (Alignment::Center, &format_spec[1..]),
        _ => return Some(value.to_owned()),
    };

    let Ok(width) = width_str.parse::<usize>() else {
        return Some(value.to_owned());
    };

    Some(match alignment {
        Alignment::Left => format!("{value:<width$}"),
        Alignment::Right => format!("{value:>width$}"),
        Alignment::Center => format!("{value:^width$}"),
    })
}

enum Alignment {
    Left,
    Right,
    Center,
}

fn print_banner_inner(mode: &BannerMode) {
    let metadata = BANNER_METADATA.with(|cell| cell.borrow().clone());
    let Some(meta) = metadata else {
        // The free-function `install` was called without the macro. Skip
        // the banner instead of emitting half-empty defaults.
        return;
    };

    let lines = match mode {
        BannerMode::Off => return,
        BannerMode::Default => default_banner_lines(&meta),
        BannerMode::Custom(builder) => builder(&meta),
    };

    for line in lines {
        log::info!("{line}");
    }
}

fn default_banner_lines(meta: &BannerMetadata) -> Vec<String> {
    let authors = if meta.authors.trim().is_empty() {
        "Unknown"
    } else {
        meta.authors
    };
    let repository = if meta.repository.trim().is_empty() {
        "N/A"
    } else {
        meta.repository
    };

    vec![
        String::new(),
        format!("  | {} {}", meta.name, meta.version),
        String::from("  |-------------------------------"),
        format!("  | Author: {}", authors),
        format!("  | Repository: {}", repository),
        String::new(),
    ]
}

/// Re-emits the default banner after installation. Rarely useful at
/// runtime; kept public so plugins can re-print on demand (for
/// instance, after a Pawn-driven reload). Honours [`BannerMode::Default`]
/// regardless of what was supplied to `install` — custom banners are
/// not memoised, so plugins that need their own format should call
/// `log::info!` themselves.
pub fn print_banner() {
    // The runtime mode is not stored after install (the logger has no
    // banner field); we always re-emit the default style. Plugins with a
    // custom banner can call `log::info!` themselves to repeat it.
    print_banner_inner(&BannerMode::Default);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn config_resolves_defaults() {
        let cfg = LoggerConfig::new("my-plugin");
        assert_eq!(cfg.resolved_filename(), "my-plugin.log");
        assert_eq!(cfg.resolved_prefix(), "[my-plugin]");
        assert_eq!(cfg.log_path(), Path::new("logs/my-plugin.log"));
        assert_eq!(cfg.resolved_archive_directory(), Path::new("logs/archive"));
        assert_eq!(cfg.level, LevelFilter::Info);
        assert!(cfg.also_to_server);
        assert!(matches!(cfg.banner, BannerMode::Default));
        assert_eq!(cfg.file_format, DEFAULT_FILE_FORMAT);
        assert_eq!(cfg.server_format, DEFAULT_SERVER_FORMAT);
        let rotation = cfg.rotation.expect("default rotation enabled");
        assert_eq!(rotation.max_bytes, 50 * 1024 * 1024);
        // Default is append-style: never auto-delete archives.
        assert_eq!(rotation.keep, None);
    }

    #[test]
    fn config_overrides_apply() {
        let cfg = LoggerConfig::new("foo")
            .directory("custom")
            .filename("custom.log")
            .prefix("[Custom]")
            .level(LevelFilter::Warn)
            .also_to_server(false)
            .no_banner()
            .rotation_size_mb(10)
            .rotation_keep(3)
            .file_format("{level}: {message}")
            .server_format("<{prefix}> {message}");
        assert_eq!(cfg.directory, Path::new("custom"));
        // Archive folder is always `{directory}/archive` — overriding
        // `directory` automatically retargets the archive directory too.
        assert_eq!(
            cfg.resolved_archive_directory(),
            Path::new("custom/archive")
        );
        assert_eq!(cfg.resolved_filename(), "custom.log");
        assert_eq!(cfg.resolved_prefix(), "[Custom]");
        assert_eq!(cfg.level, LevelFilter::Warn);
        assert!(!cfg.also_to_server);
        assert!(matches!(cfg.banner, BannerMode::Off));
        assert_eq!(cfg.file_format, "{level}: {message}");
        assert_eq!(cfg.server_format, "<{prefix}> {message}");
        let rotation = cfg.rotation.expect("explicit rotation kept");
        assert_eq!(rotation.max_bytes, 10 * 1024 * 1024);
        assert_eq!(rotation.keep, Some(3));
    }

    #[test]
    fn rotation_no_cleanup_resets_keep_to_none() {
        let cfg = LoggerConfig::new("foo")
            .rotation_keep(5)
            .rotation_no_cleanup();
        let rotation = cfg.rotation.expect("rotation still active");
        assert_eq!(rotation.keep, None);
    }

    #[test]
    fn apply_format_substitutes_placeholders() {
        let line = apply_format(
            "[{timestamp}] [{level}] {message}",
            None,
            "2026-06-08 12:30:45",
            "INFO",
            "ready",
        );
        assert_eq!(line, "[2026-06-08 12:30:45] [INFO] ready");

        let server = apply_format(
            "{prefix} {message}",
            Some("[my-plugin]"),
            "2026-06-08 12:30:45",
            "WARN",
            "stalled",
        );
        assert_eq!(server, "[my-plugin] stalled");
    }

    #[test]
    fn apply_format_supports_width_specifiers() {
        let right = apply_format("[{level:>5}] {message}", None, "ts", "INFO", "msg");
        assert_eq!(right, "[ INFO] msg");

        let left = apply_format("[{level:<5}] {message}", None, "ts", "INFO", "msg");
        assert_eq!(left, "[INFO ] msg");

        let center = apply_format("[{level:^6}] {message}", None, "ts", "INFO", "msg");
        assert_eq!(center, "[ INFO ] msg");
    }

    #[test]
    fn apply_format_width_smaller_than_value_does_not_truncate() {
        let line = apply_format("[{level:>2}] {message}", None, "ts", "INFO", "msg");
        // Rust's `{:>w$}` only pads — never truncates — so INFO survives.
        assert_eq!(line, "[INFO] msg");
    }

    #[test]
    fn apply_format_leaves_unknown_placeholders_untouched() {
        let line = apply_format(
            "{foo} {message}",
            None,
            "2026-06-08 12:30:45",
            "INFO",
            "ready",
        );
        assert_eq!(line, "{foo} ready");
    }

    #[test]
    fn custom_banner_lines_emit_in_order() {
        let cfg = LoggerConfig::new("foo").banner_with(|meta| {
            vec![
                String::from("=== plugin start ==="),
                format!("hello {}!", meta.name),
            ]
        });
        let lines = match &cfg.banner {
            BannerMode::Custom(builder) => builder(&BannerMetadata::new(
                "foo",
                "1.0",
                "ZOTTCE",
                "https://example.com",
            )),
            _ => unreachable!(),
        };
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0], "=== plugin start ===");
        assert_eq!(lines[1], "hello foo!");
    }

    #[test]
    fn no_rotation_disables_archives() {
        let cfg = LoggerConfig::new("foo").rotation_size_mb(20).no_rotation();
        assert!(cfg.rotation.is_none());
    }

    #[test]
    fn rotation_size_mb_zero_disables() {
        let cfg = LoggerConfig::new("foo").rotation_size_mb(0);
        assert!(cfg.rotation.is_none());
    }

    #[test]
    fn level_round_trip() {
        for level in [
            LevelFilter::Off,
            LevelFilter::Error,
            LevelFilter::Warn,
            LevelFilter::Info,
            LevelFilter::Debug,
            LevelFilter::Trace,
        ] {
            assert_eq!(u8_to_level(level_to_u8(level)), level);
        }
    }

    #[test]
    fn set_and_read_level() {
        set_level(LevelFilter::Warn);
        assert_eq!(level(), LevelFilter::Warn);
        set_level(LevelFilter::Trace);
        assert_eq!(level(), LevelFilter::Trace);
    }

    #[cfg(feature = "compression")]
    #[test]
    fn compress_archives_builder_sets_flag() {
        let cfg = LoggerConfig::new("foo").compress_archives(true);
        assert!(cfg.compress_archives);
        let cfg = LoggerConfig::new("foo").compress_archives(false);
        assert!(!cfg.compress_archives);
        let cfg = LoggerConfig::new("foo");
        assert!(
            !cfg.compress_archives,
            "compression must stay opt-in when the builder is not called"
        );
    }

    #[cfg(feature = "compression")]
    #[test]
    fn find_next_archive_index_counts_gz_variants() {
        let tmp = std::env::temp_dir().join(format!(
            "rust-samp-test-archive-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&tmp).unwrap();
        // mixed archive directory: one .gz and one plain
        std::fs::write(tmp.join("foo.log.2.gz"), b"compressed").unwrap();
        std::fs::write(tmp.join("foo.log.5"), b"plain").unwrap();

        // Next free index must skip past the highest seen — regardless
        // of whether the highest is `.N` or `.N.gz`.
        let next = super::find_next_archive_index(&tmp, "foo.log");
        assert_eq!(next, 6);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn env_var_prefix_uppercases_and_sanitises() {
        assert_eq!(super::env_var_prefix("memcached"), "MEMCACHED");
        assert_eq!(super::env_var_prefix("streamer-rs"), "STREAMER_RS");
        assert_eq!(super::env_var_prefix("my.plugin"), "MY_PLUGIN");
        assert_eq!(super::env_var_prefix("plugin_v2"), "PLUGIN_V2");
    }

    #[test]
    fn parse_level_accepts_known_names() {
        assert_eq!(super::parse_level("off"), Some(LevelFilter::Off));
        assert_eq!(super::parse_level("ERROR"), Some(LevelFilter::Error));
        assert_eq!(super::parse_level("warn"), Some(LevelFilter::Warn));
        assert_eq!(super::parse_level("warning"), Some(LevelFilter::Warn));
        assert_eq!(super::parse_level("  info "), Some(LevelFilter::Info));
        assert_eq!(super::parse_level("debug"), Some(LevelFilter::Debug));
        assert_eq!(super::parse_level("trace"), Some(LevelFilter::Trace));
        assert_eq!(super::parse_level("nope"), None);
        assert_eq!(super::parse_level(""), None);
    }

    #[test]
    fn parse_bool_accepts_common_truthy() {
        for s in ["1", "true", "TRUE", " yes ", "on"] {
            assert!(super::parse_bool(s), "{s:?} should parse as true");
        }
        for s in ["0", "false", "no", "off", "", "anything"] {
            assert!(!super::parse_bool(s), "{s:?} should parse as false");
        }
    }

    #[test]
    fn from_env_applies_overrides_and_ignores_garbage() {
        // Use a unique prefix per test process to avoid collision with
        // any real env the test runner already has set.
        let crate_name = format!("rust_samp_test_{}", std::process::id());
        let prefix = super::env_var_prefix(&crate_name);

        // Set: valid level + dir + invalid rotation_mb (should be
        // ignored, not panic) + bool toggles.
        // SAFETY: tests run single-threaded enough for env mutation; the
        // env is otherwise unused by the rest of the suite.
        unsafe {
            std::env::set_var(format!("{prefix}_LOG_LEVEL"), "debug");
            std::env::set_var(format!("{prefix}_LOG_DIR"), "/tmp/rust-samp-from-env");
            std::env::set_var(format!("{prefix}_LOG_ROTATION_MB"), "not-a-number");
            std::env::set_var(format!("{prefix}_LOG_NO_BANNER"), "1");
            std::env::set_var(format!("{prefix}_LOG_SERVER"), "false");
        }

        let cfg = LoggerConfig::new(crate_name).from_env();

        assert_eq!(cfg.level, LevelFilter::Debug);
        assert_eq!(cfg.directory, PathBuf::from("/tmp/rust-samp-from-env"));
        assert!(matches!(cfg.banner, BannerMode::Off));
        assert!(!cfg.also_to_server);
        // ROTATION_MB was garbage — must have stayed on the default.
        assert!(matches!(
            cfg.rotation,
            Some(Rotation {
                max_bytes: DEFAULT_ROTATION_BYTES,
                ..
            })
        ));

        // SAFETY: same as above — cleaning up after ourselves.
        unsafe {
            std::env::remove_var(format!("{prefix}_LOG_LEVEL"));
            std::env::remove_var(format!("{prefix}_LOG_DIR"));
            std::env::remove_var(format!("{prefix}_LOG_ROTATION_MB"));
            std::env::remove_var(format!("{prefix}_LOG_NO_BANNER"));
            std::env::remove_var(format!("{prefix}_LOG_SERVER"));
        }
    }

    #[test]
    fn add_sink_appends_in_call_order() {
        struct Counter(std::sync::atomic::AtomicUsize);
        impl super::Sink for Counter {
            fn emit(&self, _record: &super::SinkRecord<'_>) {
                self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        let cfg = LoggerConfig::new("foo");
        assert_eq!(cfg.sinks.len(), 0, "no sinks by default");

        let cfg = cfg
            .add_sink(Box::new(Counter(0.into())))
            .add_sink(Box::new(Counter(0.into())));
        assert_eq!(cfg.sinks.len(), 2);
    }

    #[test]
    fn flush_without_install_is_noop() {
        // `install` is not callable in unit tests (it tries to register a
        // global `log` logger and create the `logs/` directory), but the
        // public `flush` helper must remain safe when no instance is
        // registered. Should not panic, deadlock, or touch any file.
        super::flush();
        super::flush();
    }
}
