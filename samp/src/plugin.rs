//! API the Rust plugin uses: trait [`SampPlugin`] (lifecycle) + global
//! functions to enable features (`enable_tick`, `logger`, `omp_query`).

use std::ptr::NonNull;
use std::time::Duration;

use samp_sdk::amx::Amx;
use samp_sdk::cell::AmxCell;

use crate::runtime::Runtime;

#[doc(hidden)]
pub fn initialize<F, T>(constructor: F)
where
    F: FnOnce() -> T + 'static,
    T: SampPlugin + 'static,
{
    let rt = Runtime::initialize();
    let plugin = constructor();

    rt.set_plugin(plugin);
    rt.post_initialize();
}

/// Tells the SDK how often [`SampPlugin::on_tick`] should fire on each
/// server.
///
/// The two servers schedule periodic callbacks differently:
///
/// - **SA-MP** exports `ProcessTick`. The server's main loop invokes it on
///   every iteration — the cadence is whatever the server is configured for.
///   The SDK has no say over the interval; the [`sa_mp`] flag only decides
///   whether the export is advertised at all.
/// - **native Open Multiplayer** has no built-in `ProcessTick` equivalent.
///   The SDK installs a repeating timer on the server's `ITimersComponent`
///   in `on_ready` and dispatches the timeout into [`on_tick`]. The
///   interval is the [`omp_interval`] field.
///
/// [`sa_mp`]: TickConfig::sa_mp
/// [`omp_interval`]: TickConfig::omp_interval
/// [`on_tick`]: SampPlugin::on_tick
#[derive(Debug, Clone, Copy)]
pub struct TickConfig {
    /// Enable the tick on SA-MP. When `false`, the plugin does not advertise
    /// `Supports::PROCESS_TICK` and the export becomes inert.
    pub sa_mp: bool,
    /// Enable the tick on native Open Multiplayer. When `false`, the SDK
    /// does not create the `ITimersComponent` timer in `on_ready`.
    pub omp: bool,
    /// Interval the SDK uses when creating the Open Multiplayer timer.
    /// Ignored when [`omp`] is `false`. Ignored entirely on SA-MP (the
    /// server controls the cadence).
    ///
    /// [`omp`]: TickConfig::omp
    pub omp_interval: Duration,
}

impl Default for TickConfig {
    /// Default: enabled on both servers, 5 ms timer on Open Multiplayer.
    fn default() -> Self {
        Self {
            sa_mp: true,
            omp: true,
            omp_interval: Duration::from_millis(5),
        }
    }
}

impl TickConfig {
    /// Equivalent to `TickConfig::default()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builder: sets [`sa_mp`].
    ///
    /// [`sa_mp`]: TickConfig::sa_mp
    #[must_use]
    pub fn sa_mp(mut self, enabled: bool) -> Self {
        self.sa_mp = enabled;
        self
    }

    /// Builder: sets [`omp`].
    ///
    /// [`omp`]: TickConfig::omp
    #[must_use]
    pub fn omp(mut self, enabled: bool) -> Self {
        self.omp = enabled;
        self
    }

    /// Builder: sets [`omp_interval`].
    ///
    /// [`omp_interval`]: TickConfig::omp_interval
    #[must_use]
    pub fn omp_interval(mut self, interval: Duration) -> Self {
        self.omp_interval = interval;
        self
    }

    /// Shortcut: tick only on SA-MP. Equivalent to
    /// `TickConfig::new().omp(false)`.
    ///
    /// Use when the plugin has no meaningful work to do on the Open
    /// Multiplayer tick — for example, a pure SA-MP plugin running in
    /// legacy mode under Open Multiplayer.
    #[must_use]
    pub fn sa_mp_only() -> Self {
        Self::default().omp(false)
    }

    /// Shortcut: tick only on native Open Multiplayer, at the supplied
    /// interval. Equivalent to
    /// `TickConfig::new().sa_mp(false).omp_interval(interval)`.
    ///
    /// Use when the plugin needs a controlled cadence specifically on
    /// Open Multiplayer and should stay silent on SA-MP — for example,
    /// a component that drives a long-poll loop only meaningful when
    /// the component API is reachable.
    #[must_use]
    pub fn omp_only(interval: Duration) -> Self {
        Self::default().sa_mp(false).omp_interval(interval)
    }
}

/// Origin of the current [`SampPlugin::on_tick`] invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickSource {
    /// Fired by SA-MP's `ProcessTick` export, on every iteration of the
    /// server's main loop.
    SaMp,
    /// Fired by the SDK-owned repeating timer on native Open Multiplayer
    /// (created via `ITimersComponent` in `on_ready`). Matches the
    /// `omp` / `Omp*` identifier convention used elsewhere in the SDK
    /// (`OmpComponent`, `OmpComponentHandle`, …).
    OmpTimer,
}

/// Per-call context delivered to [`SampPlugin::on_tick`].
#[derive(Debug, Clone, Copy)]
pub struct TickContext {
    /// Wall-clock time elapsed since the previous `on_tick` dispatch in
    /// this plugin instance. `Duration::ZERO` on the very first call.
    pub elapsed: Duration,
    /// Which server scheduled this dispatch.
    pub source: TickSource,
}

/// Enables [`SampPlugin::on_tick`] with default settings: tick on both
/// servers, 5 ms interval on Open Multiplayer.
///
/// Call inside `initialize_plugin!`. Without this opt-in the tick stays
/// inert — useful for purely reactive plugins that do not need the cycle.
pub fn enable_tick() {
    enable_tick_with(TickConfig::default());
}

/// Enables [`SampPlugin::on_tick`] with an explicit [`TickConfig`].
///
/// Use this form to disable the tick on one server, or to choose a
/// different Open Multiplayer timer interval.
///
/// # Example
/// ```rust,no_run
/// # use std::time::Duration;
/// # use samp::plugin::{enable_tick_with, TickConfig};
/// // Tick every 50 ms on Open Multiplayer; rely on SA-MP's default cadence.
/// enable_tick_with(TickConfig::new().omp_interval(Duration::from_millis(50)));
/// ```
pub fn enable_tick_with(config: TickConfig) {
    Runtime::get().set_tick_config(config);
}

/// Installs the SDK's debug hook on `amx`, routing every executed line into
/// [`SampPlugin::on_debug_break`]. Call from [`SampPlugin::on_amx_load`] for
/// each AMX you want to debug (typically the gamemode).
///
/// The `.amx` must have been compiled with `-d2`/`-d3` for the VM to invoke the
/// hook. To stop receiving callbacks, call [`disable_debug_hook`].
///
/// This is the turnkey alternative to [`Amx::install_debug_hook`]: instead of
/// managing a raw `extern "C"` callback and global state yourself, the SDK owns
/// a panic-guarded trampoline and dispatches into your plugin instance.
///
/// # Example
/// ```rust,ignore
/// impl SampPlugin for MyDebugger {
///     fn on_amx_load(&mut self, amx: &Amx) {
///         samp::plugin::enable_debug_hook(amx);
///     }
///     fn on_debug_break(&mut self, amx: &Amx) {
///         let line = amx.cip();
///         // inspect / pause / forward to a DAP client...
///     }
/// }
/// ```
pub fn enable_debug_hook(amx: &Amx) {
    amx.install_debug_hook(debug_hook_trampoline);
}

/// Removes the SDK debug hook previously installed by [`enable_debug_hook`] on
/// `amx`, so [`SampPlugin::on_debug_break`] stops firing for it.
pub fn disable_debug_hook(amx: &Amx) {
    amx.remove_debug_hook();
}

/// SDK-owned debug hook callback. The VM calls this on every source line of an
/// AMX that opted in via [`enable_debug_hook`]. It wraps the raw `*mut AMX` and
/// dispatches into the plugin's [`SampPlugin::on_debug_break`].
///
/// Crosses the FFI boundary, so it must never unwind: the dispatch is wrapped in
/// `catch_unwind` and always returns `AMX_ERR_NONE` (0).
extern "C" fn debug_hook_trampoline(amx: *mut samp_sdk::raw::types::AMX) -> i32 {
    let _ = std::panic::catch_unwind(|| {
        let Some(rt) = Runtime::try_get() else { return };
        let wrapped = Amx::new(amx, rt.amx_exports());
        Runtime::plugin().on_debug_break(&wrapped);
    });
    0 // AMX_ERR_NONE
}

/// Returns a [`fern::Dispatch`] already chained into the server's log system,
/// disabling the SDK's default routing.
///
/// Lets the plugin customize format, level, sink (file, console) without
/// giving up delivery to the server (SA-MP `logprintf` or
/// `ICore::logLnU8`). The `log` crate level is mapped automatically to
/// [`samp_sdk::omp::LogLevel`] in Open Multiplayer mode.
///
/// # Example
/// ```rust,ignore
/// initialize_plugin!({
///     let _ = fern::Dispatch::new()
///         .format(|cb, msg, rec| cb.finish(format_args!("[MyPlugin][{}]: {}", rec.level(), msg)))
///         .level(log::LevelFilter::Info)
///         .chain(samp::plugin::logger())
///         .apply();
///     MyPlugin
/// });
/// ```
pub fn logger() -> fern::Dispatch {
    let rt = Runtime::get();
    rt.disable_default_logger();

    fern::Dispatch::new().chain(fern::Output::call(|record| {
        let rt = Runtime::get();
        // In Open Multiplayer mode, maps log::Level → LogLevel and routes via ICore::logLn.
        // In SA-MP mode, log_level falls back to the standard log() (logprintf has no level).
        #[cfg(not(feature = "samp-only"))]
        {
            let level = match record.level() {
                log::Level::Error => samp_sdk::omp::LogLevel::Error,
                log::Level::Warn => samp_sdk::omp::LogLevel::Warning,
                log::Level::Info => samp_sdk::omp::LogLevel::Message,
                log::Level::Debug | log::Level::Trace => samp_sdk::omp::LogLevel::Debug,
            };
            rt.log_level(level, record.args());
        }
        #[cfg(feature = "samp-only")]
        rt.log(record.args());
    }))
}

#[doc(hidden)]
#[must_use]
pub fn get<T: SampPlugin + 'static>() -> NonNull<T> {
    Runtime::plugin_cast()
}

/// Returns the Open Multiplayer server's `ICore*` pointer received in `on_load`.
///
/// Available only in native Open Multiplayer mode (without the `samp-only` feature).
/// Returns `None` if the plugin was loaded via SA-MP or if `on_load` has not
/// been called yet.
#[cfg(not(feature = "samp-only"))]
#[must_use]
pub fn omp_core() -> Option<*mut samp_sdk::omp::component::ICore> {
    crate::runtime::Runtime::get().omp_core()
}

/// Looks up an Open Multiplayer component by UID in the list received in `on_init`.
///
/// Returns `None` if the server has not yet called `on_init` or if the component
/// is not registered.
///
/// # Example
/// ```rust,no_run
/// use samp::plugin::omp_query_component;
/// use samp_sdk::omp::server::PAWN_COMPONENT_UID;
///
/// if let Some(_pawn) = omp_query_component(PAWN_COMPONENT_UID) {
///     // IPawnComponent available
/// }
/// ```
#[cfg(not(feature = "samp-only"))]
#[must_use]
pub fn omp_query_component(
    uid: samp_sdk::omp::types::UID,
) -> Option<*mut samp_sdk::omp::server::ServerComponent> {
    crate::runtime::Runtime::get().omp_query_component(uid)
}

/// Looks up an Open Multiplayer component via its typed wrapper.
///
/// Typed version of `omp_query_component`: uses the `UID` declared in the type's
/// `OmpComponentHandle` trait, returns a wrapper that exposes specific methods.
///
/// # Example
/// ```rust,no_run
/// use samp_sdk::omp::PawnComponent;
///
/// if let Some(pawn) = samp::plugin::omp_query::<PawnComponent>() {
///     if let Some(version) = pawn.version() {
///         println!("Pawn component: {}.{}.{}", version.major, version.minor, version.patch);
///     }
/// }
/// ```
#[cfg(not(feature = "samp-only"))]
#[must_use]
pub fn omp_query<T>() -> Option<T>
where
    T: samp_sdk::omp::OmpComponentHandle,
{
    let raw = omp_query_component(T::UID)?;
    let nonnull_ptr = std::ptr::NonNull::new(raw)?;
    Some(unsafe { T::from_raw(nonnull_ptr) })
}

/// Plugin lifecycle. All methods are optional — the trait provides empty
/// implementations so the plugin only overrides the relevant ones.
///
/// Instead of implementing manually, use `#[derive(SampPlugin)]` if no
/// method needs custom logic.
pub trait SampPlugin {
    /// Server has finished loading the plugin (`Load()` on SA-MP /
    /// `onLoad(ICore*)` on Open Multiplayer). Good moment to initialize state.
    fn on_load(&mut self) {}

    /// Server is unloading the plugin. Release external resources here.
    fn on_unload(&mut self) {}

    /// A Pawn script (`.amx`) was loaded. On SA-MP it is called by the
    /// `AmxLoad` export; on Open Multiplayer by `IEventDispatcher<PawnEventHandler>`.
    fn on_amx_load(&mut self, amx: &Amx) {
        let _ = amx;
    }

    /// A Pawn script is being unloaded. Clean per-AMX state here.
    fn on_amx_unload(&mut self, amx: &Amx) {
        let _ = amx;
    }

    /// The VM's debug hook fired on a source line. Only called for AMXs the
    /// plugin opted in via [`enable_debug_hook`], and only when the `.amx` was
    /// compiled with `-d2`/`-d3`.
    ///
    /// This runs on the VM thread, synchronously, on every executed line — keep
    /// it cheap, and block here (e.g. waiting for a debugger client) only if you
    /// intend to freeze the server. Use the VM accessors on [`Amx`]
    /// (`cip`, `frame`, `read_cell`/`write_cell`) to read the paused state, and
    /// pair them with `samp::debug` (feature `debug`) to map addresses to source
    /// lines and symbols.
    fn on_debug_break(&mut self, amx: &Amx) {
        let _ = amx;
    }

    /// Periodic callback. Fires only when the plugin opted in via
    /// [`enable_tick`] (or [`enable_tick_with`]).
    ///
    /// The two servers schedule this differently:
    /// - **SA-MP**: the server invokes the `ProcessTick` export on every
    ///   iteration of its main loop. The cadence is whatever the server is
    ///   configured for — the SDK has no control over it.
    /// - **native Open Multiplayer**: there is no native equivalent of
    ///   `ProcessTick` for components. The SDK installs a repeating timer
    ///   on the server's `ITimersComponent` in `on_ready` and dispatches
    ///   its timeout here. The interval is whatever [`TickConfig::omp_interval`]
    ///   was set to (default: 5 ms).
    ///
    /// `ctx.source` tells which server scheduled the call; `ctx.elapsed`
    /// is the wall-clock time since the previous dispatch (zero on the
    /// first call).
    fn on_tick(&mut self, ctx: TickContext) {
        let _ = ctx;
    }

    /// Called when all Open Multiplayer components have finished initializing.
    ///
    /// This is the safe moment to interact with other server components,
    /// since all of them have already gone through their `on_init`.
    ///
    /// Available only in native Open Multiplayer mode (without the `samp-only` feature).
    #[cfg(not(feature = "samp-only"))]
    fn on_omp_ready(&mut self) {}

    /// Called when any Open Multiplayer component is being unloaded.
    ///
    /// Use together with `samp::plugin::omp_query_component()` to check
    /// which components are still available after the notification.
    ///
    /// Available only in native Open Multiplayer mode (without the `samp-only` feature).
    #[cfg(not(feature = "samp-only"))]
    fn on_component_free(&mut self) {}
}

#[doc(hidden)]
pub fn convert_return_value<T: AmxCell<'static>>(value: T) -> i32 {
    value.as_cell()
}
