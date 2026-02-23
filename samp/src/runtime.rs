use samp_sdk::consts::{ServerData, Supports};
use samp_sdk::raw::{functions::Logprintf, types::AMX};

use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::amx::{Amx, AmxIdent};
use crate::plugin::SampPlugin;

static RUNTIME: AtomicPtr<Runtime> = AtomicPtr::new(std::ptr::null_mut());

struct RuntimeInner {
    plugin: Option<NonNull<dyn SampPlugin + 'static>>,
    process_tick: bool,
    server_exports: *const usize,
    amx_list: HashMap<AmxIdent, Amx>,
    logger_enabled: bool,
}

pub struct Runtime {
    inner: UnsafeCell<RuntimeInner>,
}

// SAFETY: SA-MP server is single-threaded. All access to Runtime
// happens on the main server thread through the plugin callbacks.
unsafe impl Sync for Runtime {}
unsafe impl Send for Runtime {}

impl Runtime {
    /// Returns a mutable reference to the inner data.
    ///
    /// # Safety
    /// SA-MP server is single-threaded, so concurrent access cannot occur.
    /// `UnsafeCell` is the sanctioned way to achieve interior mutability in Rust.
    #[inline]
    #[allow(clippy::mut_from_ref)]
    fn inner(&self) -> &mut RuntimeInner {
        unsafe { &mut *self.inner.get() }
    }

    pub fn initialize() -> &'static Runtime {
        let inner = RuntimeInner {
            plugin: None,
            process_tick: false,
            server_exports: std::ptr::null(),
            amx_list: HashMap::default(),
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
        assert!(
            !inner.server_exports.is_null(),
            "server_exports não inicializado"
        );
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
            "server_exports não inicializado"
        );
        unsafe {
            (inner
                .server_exports
                .offset(ServerData::Logprintf.into()) as *const Logprintf)
                .read()
        }
    }

    pub fn disable_default_logger(&self) {
        self.inner().logger_enabled = false;
    }

    pub fn log<T: std::fmt::Display>(&self, message: T) {
        let log_fn = self.logger();
        let msg = format!("{}", message);

        if let Ok(cstr) = CString::new(msg) {
            log_fn(cstr.as_ptr());
        }
    }

    pub fn insert_amx(&self, amx: *mut AMX) -> Option<&Amx> {
        let inner = self.inner();
        let ident = AmxIdent::from(amx);
        let amx = Amx::new(amx, self.amx_exports());

        inner.amx_list.insert(ident, amx);
        inner.amx_list.get(&ident)
    }

    pub fn remove_amx(&self, amx: *mut AMX) -> Option<Amx> {
        let ident = AmxIdent::from(amx);
        self.inner().amx_list.remove(&ident)
    }

    pub fn supports(&self) -> Supports {
        let mut supports = Supports::VERSION | Supports::AMX_NATIVES;

        if self.inner().process_tick {
            supports.insert(Supports::PROCESS_TICK);
        }

        supports
    }

    #[inline]
    pub fn amx_list(&self) -> &HashMap<AmxIdent, Amx> {
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

    pub fn enable_process_tick(&self) {
        self.inner().process_tick = true;
    }

    #[inline]
    pub fn get() -> &'static Runtime {
        let ptr = RUNTIME.load(Ordering::Acquire);
        assert!(
            !ptr.is_null(),
            "Runtime::get() chamado antes de Runtime::initialize()"
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
                .expect("Runtime::plugin() chamado antes de set_plugin()")
                .as_mut()
        }
    }

    #[inline]
    pub fn plugin_cast<T: SampPlugin>() -> NonNull<T> {
        let rt = Runtime::get();
        rt.inner()
            .plugin
            .as_ref()
            .expect("Runtime::plugin_cast() chamado antes de set_plugin()")
            .cast()
    }
}
