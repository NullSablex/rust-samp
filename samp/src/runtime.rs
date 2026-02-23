use samp_sdk::consts::{ServerData, Supports};
use samp_sdk::raw::{functions::Logprintf, types::AMX};

use std::collections::HashMap;
use std::ffi::CString;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::amx::{Amx, AmxIdent};
use crate::plugin::SampPlugin;

static RUNTIME: AtomicPtr<Runtime> = AtomicPtr::new(std::ptr::null_mut());

pub struct Runtime {
    plugin: Option<NonNull<dyn SampPlugin + 'static>>,
    process_tick: bool,
    server_exports: *const usize,
    amx_list: HashMap<AmxIdent, Amx>,
    logger_enabled: bool,
}

impl Runtime {
    pub fn initialize() -> &'static mut Runtime {
        let rt = Runtime {
            plugin: None,
            process_tick: false,
            server_exports: std::ptr::null(),
            amx_list: HashMap::default(),
            logger_enabled: true,
        };

        let boxed = Box::new(rt);

        RUNTIME.store(Box::into_raw(boxed), Ordering::Release);

        Runtime::get()
    }

    pub fn post_initialize(&self) {
        if !self.logger_enabled {
            return;
        }

        let logger = crate::plugin::logger();
        let _ = logger.apply();
    }

    #[inline]
    pub fn amx_exports(&self) -> usize {
        assert!(
            !self.server_exports.is_null(),
            "server_exports não inicializado"
        );
        unsafe {
            self.server_exports
                .offset(ServerData::AmxExports.into())
                .read()
        }
    }

    #[inline]
    pub fn logger(&self) -> Logprintf {
        assert!(
            !self.server_exports.is_null(),
            "server_exports não inicializado"
        );
        unsafe {
            (self.server_exports.offset(ServerData::Logprintf.into()) as *const Logprintf).read()
        }
    }

    pub fn disable_default_logger(&mut self) {
        self.logger_enabled = false;
    }

    pub fn log<T: std::fmt::Display>(&self, message: T) {
        let log_fn = self.logger();
        let msg = format!("{}", message);

        if let Ok(cstr) = CString::new(msg) {
            log_fn(cstr.as_ptr());
        }
    }

    pub fn insert_amx(&mut self, amx: *mut AMX) -> Option<&Amx> {
        let ident = AmxIdent::from(amx);
        let amx = Amx::new(amx, self.amx_exports());

        self.amx_list.insert(ident, amx);
        self.amx_list.get(&ident)
    }

    pub fn remove_amx(&mut self, amx: *mut AMX) -> Option<Amx> {
        let ident = AmxIdent::from(amx);
        self.amx_list.remove(&ident)
    }

    pub fn supports(&self) -> Supports {
        let mut supports = Supports::VERSION | Supports::AMX_NATIVES;

        if self.process_tick {
            supports.insert(Supports::PROCESS_TICK);
        }

        supports
    }

    #[inline]
    pub fn amx_list(&self) -> &HashMap<AmxIdent, Amx> {
        &self.amx_list
    }

    pub fn set_plugin<T>(&mut self, plugin: T)
    where
        T: SampPlugin + 'static,
    {
        let boxed = Box::new(plugin);
        self.plugin = NonNull::new(Box::into_raw(boxed));
    }

    pub fn set_server_exports(&mut self, exports: *const usize) {
        self.server_exports = exports;
    }

    pub fn enable_process_tick(&mut self) {
        self.process_tick = true;
    }

    #[inline]
    pub fn get() -> &'static mut Runtime {
        let ptr = RUNTIME.load(Ordering::Acquire);
        assert!(
            !ptr.is_null(),
            "Runtime::get() chamado antes de Runtime::initialize()"
        );
        unsafe { &mut *ptr }
    }

    #[inline]
    pub fn plugin() -> &'static mut dyn SampPlugin {
        let rt = Runtime::get();
        unsafe {
            rt.plugin
                .as_mut()
                .expect("Runtime::plugin() chamado antes de set_plugin()")
                .as_mut()
        }
    }

    #[inline]
    pub fn plugin_cast<T: SampPlugin>() -> NonNull<T> {
        let rt = Runtime::get();
        rt.plugin
            .as_ref()
            .expect("Runtime::plugin_cast() chamado antes de set_plugin()")
            .cast()
    }
}
