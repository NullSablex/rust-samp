//! Global runtime state of the Rust plugin — singleton accessible via
//! [`Runtime::get`].
//!
//! Encapsulates pointers received from the server (SA-MP `ppData` or Open Multiplayer `ICore`),
//! the list of active AMXs, the plugin instantiated by the dev and flags (logger,
//! tick enabled, etc).
//!
//! The `Runtime` lives in a global `AtomicPtr` and uses `UnsafeCell` for interior
//! mutation — safe because the server is single-threaded and all callbacks
//! run on the main thread.

use samp_sdk::consts::{ServerData, Supports};
#[cfg(not(feature = "samp-only"))]
use samp_sdk::omp::component::ICore;
#[cfg(not(feature = "samp-only"))]
use samp_sdk::omp::events::PawnEventHandler;
#[cfg(not(feature = "samp-only"))]
use samp_sdk::omp::server::{ServerComponent, ServerComponentList};
#[cfg(not(feature = "samp-only"))]
use samp_sdk::omp::timers::{ITimer, TimerTimeOutHandler};
#[cfg(not(feature = "samp-only"))]
use samp_sdk::raw::types::AMX_NATIVE_INFO;
use samp_sdk::raw::{functions::Logprintf, types::AMX};

use std::cell::UnsafeCell;
use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::time::{Duration, Instant};

use crate::amx::{Amx, AmxIdent};
use crate::plugin::{SampPlugin, TickConfig};

static RUNTIME: AtomicPtr<Runtime> = AtomicPtr::new(std::ptr::null_mut());

struct RuntimeInner {
    plugin: Option<NonNull<dyn SampPlugin + 'static>>,
    /// Set by `samp::plugin::enable_tick` / `enable_tick_with`. `None`
    /// means the tick is disabled on both servers (the default).
    tick_config: Option<TickConfig>,
    /// Wall-clock timestamp of the previous tick dispatch. Used to compute
    /// `TickContext::elapsed`. `None` until the first tick fires.
    last_tick_at: Option<Instant>,
    server_exports: *const usize,
    /// AMX function table obtained from `IPawnComponent` in native Open Multiplayer mode.
    #[cfg(not(feature = "samp-only"))]
    omp_amx_exports: Option<usize>,
    /// Pointer to the Open Multiplayer server's `ICore`, stored in `on_load`.
    #[cfg(not(feature = "samp-only"))]
    omp_core: Option<NonNull<ICore>>,
    /// Open Multiplayer server component list, stored in `on_init`.
    #[cfg(not(feature = "samp-only"))]
    omp_component_list: Option<NonNull<ServerComponentList>>,
    /// Pawn event handler registered in the `IEventDispatcher` of `IPawnComponent`.
    #[cfg(not(feature = "samp-only"))]
    pawn_event_handler: Option<NonNull<PawnEventHandler>>,
    /// Natives to register on the AMX in native Open Multiplayer mode (via `pawn_on_amx_load`).
    /// In SA-MP/legacy mode this Vec stays empty — natives are passed via `AmxLoad()`.
    #[cfg(not(feature = "samp-only"))]
    omp_natives: Vec<AMX_NATIVE_INFO>,
    /// AMXs that arrived via `on_amx_load` before `on_ready` (without `getAmxFunctions`
    /// available). Processed in `on_ready` when the `fn_table` is stored.
    #[cfg(not(feature = "samp-only"))]
    omp_pending_amx: Vec<*mut AMX>,
    /// Timer created via `ITimersComponent` to deliver `on_tick` in Open
    /// Multiplayer mode. Stored so `omp_cleanup` can kill it on shutdown.
    #[cfg(not(feature = "samp-only"))]
    omp_tick_timer: Option<NonNull<ITimer>>,
    /// Tick handler (heap allocated). Released in the timer's `free` callback.
    #[cfg(not(feature = "samp-only"))]
    omp_tick_handler: Option<NonNull<TimerTimeOutHandler>>,
    amx_list: Vec<(AmxIdent, Amx)>,
    logger_enabled: bool,
}

pub struct Runtime {
    inner: UnsafeCell<RuntimeInner>,
}

// SAFETY: SA-MP and Open Multiplayer servers are single-threaded — every access to the
// `Runtime` happens via callbacks on the main thread. The impls here are a
// formality to satisfy the `AtomicPtr<Runtime>`.
unsafe impl Sync for Runtime {}
unsafe impl Send for Runtime {}

impl Runtime {
    /// Mutable access to the inner state via [`UnsafeCell`].
    ///
    /// # Safety
    /// Single-threaded server — concurrency does not happen. `UnsafeCell` is
    /// the canonical Rust path for shared mutation in this regime.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    fn inner(&self) -> &mut RuntimeInner {
        unsafe { &mut *self.inner.get() }
    }

    pub fn initialize() -> &'static Runtime {
        let inner = RuntimeInner {
            plugin: None,
            tick_config: None,
            last_tick_at: None,
            server_exports: std::ptr::null(),
            #[cfg(not(feature = "samp-only"))]
            omp_amx_exports: None,
            #[cfg(not(feature = "samp-only"))]
            omp_core: None,
            #[cfg(not(feature = "samp-only"))]
            omp_component_list: None,
            #[cfg(not(feature = "samp-only"))]
            pawn_event_handler: None,
            #[cfg(not(feature = "samp-only"))]
            omp_natives: Vec::new(),
            #[cfg(not(feature = "samp-only"))]
            omp_pending_amx: Vec::new(),
            #[cfg(not(feature = "samp-only"))]
            omp_tick_timer: None,
            #[cfg(not(feature = "samp-only"))]
            omp_tick_handler: None,
            amx_list: Vec::new(),
            logger_enabled: true,
        };

        let rt = Runtime {
            inner: UnsafeCell::new(inner),
        };

        let boxed = Box::new(rt);

        RUNTIME.store(Box::into_raw(boxed), Ordering::Release);

        Runtime::get()
    }

    pub fn post_initialize(&self) {
        if !self.inner().logger_enabled {
            return;
        }

        let logger = crate::plugin::logger();
        let _ = logger.apply();
    }

    #[inline]
    pub fn amx_exports(&self) -> usize {
        let inner = self.inner();

        // Native Open Multiplayer mode: exports obtained via IPawnComponent::getAmxFunctions()
        #[cfg(not(feature = "samp-only"))]
        if let Some(exports) = inner.omp_amx_exports {
            return exports;
        }

        // SA-MP mode: exports obtained via server_data passed in Load()
        if inner.server_exports.is_null() {
            // Native Open Multiplayer without an AMX table available (on_init failed or has not been called yet).
            return 0;
        }
        unsafe {
            inner
                .server_exports
                .offset(ServerData::AmxExports.into())
                .read()
        }
    }

    #[inline]
    pub fn logger(&self) -> Logprintf {
        let inner = self.inner();
        assert!(
            !inner.server_exports.is_null(),
            "server_exports not initialized"
        );
        unsafe {
            inner
                .server_exports
                .offset(ServerData::Logprintf.into())
                .cast::<Logprintf>()
                .read()
        }
    }

    pub fn disable_default_logger(&self) {
        self.inner().logger_enabled = false;
    }

    /// Default SDK log routing:
    ///
    /// - **SA-MP**: uses the server's `logprintf` (console + log file configured
    ///   by SA-MP).
    /// - **native Open Multiplayer**: uses `ICore::logLn(Message, ...)` — writes to the console and
    ///   to the log file Open Multiplayer has configured, with timestamp + `[Info]` level.
    ///   Equivalent to SA-MP's `logprintf` in terms of destination. If `ICore` is not
    ///   yet available (before `on_load`), falls back to `eprintln!`.
    ///
    /// The file destination is decided by the server — nothing hardcoded here.
    ///
    /// No SDK prefix. To customize (different level, format, separate file,
    /// etc), use `samp::plugin::logger()` in `on_load` and configure a `fern::Dispatch` —
    /// the `log` crate level is mapped to Open Multiplayer's `LogLevel` automatically.
    pub fn log<T: std::fmt::Display>(&self, message: T) {
        // SA-MP mode: routes via the server's logprintf.
        if !self.inner().server_exports.is_null() {
            let log_fn = self.logger();
            let msg = format!("{message}");
            if let Ok(cstr) = CString::new(msg) {
                log_fn(cstr.as_ptr());
            }
            return;
        }

        // Native Open Multiplayer mode: routes via ICore::logLnU8 (the server's UTF-8 pipeline).
        // Rust strings are already UTF-8 — we always use the U8 variant, ensuring that accents
        // and non-ASCII characters pass through correctly regardless of the console locale.
        #[cfg(not(feature = "samp-only"))]
        if let Some(core) = self.omp_core() {
            let msg = format!("{message}");
            if unsafe {
                samp_sdk::omp::core_log_ln_u8(core, samp_sdk::omp::LogLevel::Message, &msg)
            } {
                return;
            }
        }

        // Fallback: stderr (in case the plugin logs before on_load or ICore is unavailable).
        eprintln!("{message}");
    }

    /// Logs with a specific level on the Open Multiplayer server via `ICore::logLnU8`.
    /// In SA-MP mode the level is ignored and the message goes through `logprintf`.
    ///
    /// Available only when the `samp-only` feature is disabled — without Open Multiplayer,
    /// there is no channel that uses `LogLevel`. In pure `samp-only` use `log()`.
    #[cfg(not(feature = "samp-only"))]
    pub fn log_level<T: std::fmt::Display>(&self, level: samp_sdk::omp::LogLevel, message: T) {
        if self.inner().server_exports.is_null() {
            if let Some(core) = self.omp_core() {
                let msg = format!("{message}");
                if unsafe { samp_sdk::omp::core_log_ln_u8(core, level, &msg) } {
                    return;
                }
            }
            eprintln!("{message}");
            return;
        }
        // SA-MP mode: falls back to the standard log (no level).
        self.log(message);
    }

    /// Inserts an AMX in the runtime and returns a reference to the freshly inserted one.
    ///
    /// Push on `Vec` always succeeds (until OOM); the freshly pushed slot always
    /// exists — hence the return by value without `Option`.
    pub fn insert_amx(&self, amx: *mut AMX) -> &Amx {
        let inner = self.inner();
        let ident = AmxIdent::from(amx);
        let amx = Amx::new(amx, self.amx_exports());

        inner.amx_list.push((ident, amx));
        &inner
            .amx_list
            .last()
            .expect("Vec::last() after push() always returns Some")
            .1
    }

    pub fn remove_amx(&self, amx: *mut AMX) -> Option<Amx> {
        let list = &mut self.inner().amx_list;
        let ident = AmxIdent::from(amx);
        list.iter()
            .position(|(k, _)| *k == ident)
            .map(|pos| list.swap_remove(pos).1)
    }

    pub fn supports(&self) -> Supports {
        let mut supports = Supports::VERSION | Supports::AMX_NATIVES;

        if self.tick_enabled_for_sa_mp() {
            supports.insert(Supports::PROCESS_TICK);
        }

        supports
    }

    #[inline]
    pub fn amx_list(&self) -> &[(AmxIdent, Amx)] {
        &self.inner().amx_list
    }

    pub fn set_plugin<T>(&self, plugin: T)
    where
        T: SampPlugin + 'static,
    {
        let boxed = Box::new(plugin);
        self.inner().plugin = NonNull::new(Box::into_raw(boxed));
    }

    pub fn set_server_exports(&self, exports: *const usize) {
        self.inner().server_exports = exports;
    }

    /// Stores the [`TickConfig`] requested by the plugin. Called by
    /// `samp::plugin::enable_tick` / `enable_tick_with` in the constructor.
    pub fn set_tick_config(&self, config: TickConfig) {
        self.inner().tick_config = Some(config);
    }

    /// `Some` only after the plugin opted in via `enable_tick*`.
    #[inline]
    pub fn tick_config(&self) -> Option<TickConfig> {
        self.inner().tick_config
    }

    /// True iff the plugin opted in to the tick **and** allowed SA-MP
    /// delivery. Used by `Supports()` to decide whether to advertise
    /// `Supports::PROCESS_TICK`.
    #[inline]
    pub fn tick_enabled_for_sa_mp(&self) -> bool {
        self.tick_config().is_some_and(|c| c.sa_mp)
    }

    /// Open Multiplayer timer interval, or `None` if the tick is disabled
    /// on that server. Used by `interlayer::omp_on_ready` to decide whether
    /// to create the `ITimersComponent` timer and at which interval.
    #[cfg(not(feature = "samp-only"))]
    #[inline]
    pub fn omp_tick_interval(&self) -> Option<Duration> {
        self.tick_config()
            .filter(|c| c.omp)
            .map(|c| c.omp_interval)
    }

    /// Records the current instant as the latest tick dispatch and returns
    /// the elapsed time since the previous one (zero on the first call).
    pub fn record_tick(&self) -> Duration {
        let now = Instant::now();
        let prev = self.inner().last_tick_at.replace(now);
        prev.map(|t| now.duration_since(t)).unwrap_or(Duration::ZERO)
    }

    #[inline]
    pub fn get() -> &'static Runtime {
        let ptr = RUNTIME.load(Ordering::Acquire);
        assert!(
            !ptr.is_null(),
            "Runtime::get() called before Runtime::initialize()"
        );
        unsafe { &*ptr }
    }

    #[inline]
    pub fn plugin() -> &'static mut dyn SampPlugin {
        let rt = Runtime::get();
        let inner = rt.inner();
        unsafe {
            inner
                .plugin
                .as_mut()
                .expect("Runtime::plugin() called before set_plugin()")
                .as_mut()
        }
    }

    #[inline]
    pub fn plugin_cast<T: SampPlugin>() -> NonNull<T> {
        let rt = Runtime::get();
        rt.inner()
            .plugin
            .as_ref()
            .expect("Runtime::plugin_cast() called before set_plugin()")
            .cast()
    }
}

// ---------------------------------------------------------------------------
// Native Open Multiplayer state — available only when `samp-only` is disabled.
// Grouped in a separate `impl` to avoid repeating `#[cfg(...)]` per method.
// ---------------------------------------------------------------------------

#[cfg(not(feature = "samp-only"))]
impl Runtime {
    /// Sets the AMX function table obtained from `IPawnComponent` (native Open Multiplayer mode).
    pub fn set_omp_amx_exports(&self, exports: usize) {
        self.inner().omp_amx_exports = Some(exports);
    }

    /// Indicates whether we already have `getAmxFunctions()` stored — that is, whether `on_ready`
    /// has already been called and native registration can proceed immediately.
    pub fn omp_has_amx_exports(&self) -> bool {
        self.inner().omp_amx_exports.is_some()
    }

    /// Stores the `ICore*` pointer received in `onLoad` (native Open Multiplayer mode).
    pub fn set_omp_core(&self, core: *mut ICore) {
        self.inner().omp_core = NonNull::new(core);
    }

    /// Returns the Open Multiplayer server's `ICore*` pointer, if available.
    pub fn omp_core(&self) -> Option<*mut ICore> {
        self.inner().omp_core.map(std::ptr::NonNull::as_ptr)
    }

    /// Stores the component list received in `onInit` (native Open Multiplayer mode).
    pub fn set_omp_component_list(&self, list: *mut ServerComponentList) {
        self.inner().omp_component_list = NonNull::new(list);
    }

    /// Returns the server's component list, if already received in `onInit`.
    pub fn omp_component_list(&self) -> Option<*mut ServerComponentList> {
        self.inner()
            .omp_component_list
            .map(std::ptr::NonNull::as_ptr)
    }

    /// Looks up a component in the Open Multiplayer server list by UID.
    ///
    /// Returns `None` if the list has not been received yet or the component does not exist.
    pub fn omp_query_component(
        &self,
        uid: samp_sdk::omp::types::UID,
    ) -> Option<*mut ServerComponent> {
        let list = self.inner().omp_component_list?.as_ptr();
        let ptr = unsafe { samp_sdk::omp::server::query_component(list, uid) };
        if ptr.is_null() { None } else { Some(ptr) }
    }

    /// Stores the Pawn event handler (keeps it alive for as long as the plugin exists).
    pub fn set_pawn_event_handler(&self, handler: *mut PawnEventHandler) {
        self.inner().pawn_event_handler = NonNull::new(handler);
    }

    /// Removes and returns the stored Pawn event handler, or `None` if there was none.
    ///
    /// Used in `omp_cleanup` to unregister the handler from the dispatcher before shutdown.
    pub fn take_pawn_event_handler(&self) -> Option<*mut PawnEventHandler> {
        self.inner()
            .pawn_event_handler
            .take()
            .map(std::ptr::NonNull::as_ptr)
    }

    /// Stores the list of natives to register on the AMX in native Open Multiplayer mode.
    ///
    /// Called by the generated `ComponentEntryPoint` before registering the `PawnEventHandler`,
    /// ensuring natives are available when `pawn_on_amx_load` fires.
    pub fn set_omp_natives(&self, natives: Vec<AMX_NATIVE_INFO>) {
        self.inner().omp_natives = natives;
    }

    /// Returns the list of natives to register on the AMX in native Open Multiplayer mode.
    pub fn omp_natives(&self) -> &[AMX_NATIVE_INFO] {
        &self.inner().omp_natives
    }

    /// Enqueues an AMX that arrived via `on_amx_load` before `on_ready`.
    /// It will be processed when `on_ready` stores the `fn_table`.
    pub fn enqueue_pending_amx(&self, amx: *mut AMX) {
        self.inner().omp_pending_amx.push(amx);
    }

    /// Drains the pending AMX queue. Called in `on_ready` after `set_omp_amx_exports`.
    pub fn take_pending_amx(&self) -> Vec<*mut AMX> {
        std::mem::take(&mut self.inner().omp_pending_amx)
    }

    /// Stores references to the timer/handler created in `on_ready` for the tick abstraction.
    pub fn set_omp_tick(&self, timer: *mut ITimer, handler: *mut TimerTimeOutHandler) {
        self.inner().omp_tick_timer = NonNull::new(timer);
        self.inner().omp_tick_handler = NonNull::new(handler);
    }

    /// Returns and clears the timer pointer (for `kill` on shutdown).
    pub fn take_omp_tick_timer(&self) -> Option<*mut ITimer> {
        self.inner()
            .omp_tick_timer
            .take()
            .map(std::ptr::NonNull::as_ptr)
    }

    /// Returns and clears the timer handler (in case the `free` callback does not fire and
    /// we need to release manually).
    pub fn take_omp_tick_handler(&self) -> Option<*mut TimerTimeOutHandler> {
        self.inner()
            .omp_tick_handler
            .take()
            .map(std::ptr::NonNull::as_ptr)
    }
}
