use samp::amx::Amx;
use samp::cell::{AmxCell, AmxString, Ref, UnsizedBuffer};
use samp::error::AmxResult;
use samp::plugin::SampPlugin;
use samp::{initialize_plugin, native};

use log::info;

use memcache::Client;

#[derive(Debug, Clone, Copy)]
enum MemcacheResult {
    Success(i32),
    NoData,
    NoClient,
    NoKey,
}

impl AmxCell<'_> for MemcacheResult {
    fn as_cell(&self) -> i32 {
        match self {
            MemcacheResult::Success(result) => *result,
            MemcacheResult::NoData => -1,
            MemcacheResult::NoClient => -2,
            MemcacheResult::NoKey => -3,
        }
    }
}

struct Memcached {
    clients: Vec<Client>,
}

impl Memcached {
    #[native(name = "Memcached_Connect")]
    pub fn connect(&mut self, _: &Amx, address: &AmxString) -> MemcacheResult {
        // Client::connect is generic over Connectable (impl'd for &str);
        // since Rust does not apply deref coerce on generic parameters, we force
        // &str explicitly with &**address.
        match Client::connect(&**address) {
            Ok(client) => {
                self.clients.push(client);
                let idx = i32::try_from(self.clients.len()).unwrap_or(i32::MAX);
                MemcacheResult::Success(idx - 1)
            }
            Err(_) => MemcacheResult::NoClient,
        }
    }

    #[native(name = "Memcached_Get")]
    pub fn get(
        &mut self,
        _: &Amx,
        con: usize,
        key: &AmxString,
        mut value: Ref<i32>,
    ) -> MemcacheResult {
        if con < self.clients.len() {
            match self.clients[con].get(key) {
                Ok(Some(data)) => {
                    *value = data;
                    MemcacheResult::Success(1)
                }
                Ok(None) => MemcacheResult::NoData,
                Err(_) => MemcacheResult::NoKey,
            }
        } else {
            MemcacheResult::NoClient
        }
    }

    #[native(name = "Memcached_GetString")]
    pub fn get_string(
        &mut self,
        _: &Amx,
        con: usize,
        key: &AmxString,
        buffer: UnsizedBuffer,
        size: usize,
    ) -> AmxResult<MemcacheResult> {
        if con < self.clients.len() {
            match self.clients[con].get::<String>(key) {
                Ok(Some(data)) => {
                    // write_str: combines into_sized_buffer + write in a single step
                    buffer.write_str(size, &data)?;
                    Ok(MemcacheResult::Success(1))
                }
                Ok(None) => Ok(MemcacheResult::NoData),
                Err(_) => Ok(MemcacheResult::NoKey),
            }
        } else {
            Ok(MemcacheResult::NoClient)
        }
    }

    #[native(name = "Memcached_Set")]
    pub fn set(
        &mut self,
        _: &Amx,
        con: usize,
        key: &AmxString,
        value: i32,
        expire: u32,
    ) -> MemcacheResult {
        if con < self.clients.len() {
            match self.clients[con].set(key, value, expire) {
                Ok(()) => MemcacheResult::Success(1),
                Err(_) => MemcacheResult::NoKey,
            }
        } else {
            MemcacheResult::NoClient
        }
    }

    #[native(name = "Memcached_SetString")]
    pub fn set_string(
        &mut self,
        _: &Amx,
        con: usize,
        key: &AmxString,
        value: &AmxString,
        expire: u32,
    ) -> MemcacheResult {
        if con < self.clients.len() {
            // `key` is &AmxString — deref coerce to &str (parameter `key` expects &str).
            // `value` needs ToMemcacheValue, impl'd for &str but not for
            // &AmxString; on generic parameters Rust does not apply deref coerce,
            // so we force &str via &**value.
            match self.clients[con].set(key, &**value, expire) {
                Ok(()) => MemcacheResult::Success(1),
                Err(_) => MemcacheResult::NoKey,
            }
        } else {
            MemcacheResult::NoClient
        }
    }

    #[native(name = "Memcached_Increment")]
    pub fn increment(
        &mut self,
        _: &Amx,
        con: usize,
        key: &AmxString,
        value: i32,
    ) -> MemcacheResult {
        if con < self.clients.len() {
            match self.clients[con].increment(key, u64::from(value.cast_unsigned())) {
                Ok(_) => MemcacheResult::Success(1),
                Err(_) => MemcacheResult::NoKey,
            }
        } else {
            MemcacheResult::NoClient
        }
    }

    #[native(name = "Memcached_Delete")]
    pub fn delete(&mut self, _: &Amx, con: usize, key: &AmxString) -> MemcacheResult {
        if con < self.clients.len() {
            match self.clients[con].delete(key) {
                Ok(true) => MemcacheResult::Success(1),
                Ok(false) => MemcacheResult::NoData,
                Err(_) => MemcacheResult::NoKey,
            }
        } else {
            MemcacheResult::NoClient
        }
    }
}

impl SampPlugin for Memcached {
    fn on_load(&mut self) {
        info!("Memcached plugin loaded");
    }
}

initialize_plugin!(
    natives: [
        Memcached::connect,
        Memcached::get,
        Memcached::set,
        Memcached::get_string,
        Memcached::set_string,
        Memcached::increment,
        Memcached::delete,
    ],
    {
        samp::plugin::enable_server_tick();
        samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1251);

        let samp_logger = samp::plugin::logger()
            .level(log::LevelFilter::Info);

        let log_file = fern::log_file("myplugin.log").expect("failed to open log file");

        let trace_level = fern::Dispatch::new()
            .level(log::LevelFilter::Trace)
            .chain(log_file);

        let _ = fern::Dispatch::new()
            .format(|callback, message, record| {
                callback.finish(format_args!(
                    "memcached {}: {}",
                    record.level().to_string().to_lowercase(),
                    message
                ));
            })
            .chain(samp_logger)
            .chain(trace_level)
            .apply();

        return Memcached {
            clients: Vec::new(),
        };
    }
);
