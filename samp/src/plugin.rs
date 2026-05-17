//! API the Rust plugin uses: trait [`SampPlugin`] (lifecycle) + global
//! functions to enable features (`enable_server_tick`, `logger`, `omp_query`).

use std::ptr::NonNull;

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

/// Enables the periodic [`SampPlugin::on_server_tick`] callback.
///
/// Call inside `initialize_plugin!`. Without this opt-in the tick stays inert
/// — useful for purely reactive plugins that do not need the cycle.
///
/// On SA-MP, exposes the `Supports::PROCESS_TICK` flag in the `Supports()`
/// export. On native Open Multiplayer, creates a timer in `ITimersComponent` in `on_ready`.
pub fn enable_server_tick() {
    let runtime = Runtime::get();
    runtime.enable_server_tick();
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

    /// Called periodically by the server (~5ms between calls).
    ///
    /// Identical behavior on SA-MP and Open Multiplayer:
    /// - **SA-MP**: the server invokes the plugin's `ProcessTick()` export.
    /// - **native Open Multiplayer**: the SDK creates a timer via `ITimersComponent` in `on_ready`
    ///   and fires this method on every timeout.
    ///
    /// Requires calling `samp::plugin::enable_server_tick()` in `initialize_plugin!`
    /// to enable the cycle. Without it, this method is never invoked (default).
    fn on_server_tick(&mut self) {}

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
