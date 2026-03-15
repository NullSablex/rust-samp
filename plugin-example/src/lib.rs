use samp::amx::Amx;
use samp::cell::{AmxCell, AmxString, Ref, UnsizedBuffer};
use samp::error::AmxResult;
use samp::plugin::SampPlugin;
use samp::{initialize_plugin, native};

use log::{debug, error, info};

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
    pub fn connect(&mut self, _: &Amx, address: AmxString) -> AmxResult<MemcacheResult> {
        // Deref<Target=str>: address em vez de address.to_string()
        match Client::connect(&*address) {
            Ok(client) => {
                self.clients.push(client);
                Ok(MemcacheResult::Success(self.clients.len() as i32 - 1))
            }
            Err(_) => Ok(MemcacheResult::NoClient),
        }
    }

    #[native(name = "Memcached_Get")]
    pub fn get(
        &mut self,
        _: &Amx,
        con: usize,
        key: AmxString,
        mut value: Ref<i32>,
    ) -> AmxResult<MemcacheResult> {
        if con < self.clients.len() {
            match self.clients[con].get(&key) {
                Ok(Some(data)) => {
                    *value = data;
                    Ok(MemcacheResult::Success(1))
                }
                Ok(None) => Ok(MemcacheResult::NoData),
                Err(_) => Ok(MemcacheResult::NoKey),
            }
        } else {
            Ok(MemcacheResult::NoClient)
        }
    }

    #[native(name = "Memcached_GetString")]
    pub fn get_string(
        &mut self,
        _: &Amx,
        con: usize,
        key: AmxString,
        buffer: UnsizedBuffer,
        size: usize,
    ) -> AmxResult<MemcacheResult> {
        if con < self.clients.len() {
            match self.clients[con].get::<String>(&key) {
                Ok(Some(data)) => {
                    // write_str: combina into_sized_buffer + put_in_buffer em um passo
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
        key: AmxString,
        value: i32,
        expire: u32,
    ) -> AmxResult<MemcacheResult> {
        if con < self.clients.len() {
            match self.clients[con].set(&key, value, expire) {
                Ok(_) => Ok(MemcacheResult::Success(1)),
                Err(_) => Ok(MemcacheResult::NoKey),
            }
        } else {
            Ok(MemcacheResult::NoClient)
        }
    }

    #[native(name = "Memcached_SetString")]
    pub fn set_string(
        &mut self,
        _: &Amx,
        con: usize,
        key: AmxString,
        value: AmxString,
        expire: u32,
    ) -> AmxResult<MemcacheResult> {
        if con < self.clients.len() {
            // &key: auto-deref &AmxString → &str (parâmetro key espera &str)
            // &*value: explícito — o valor precisa de ToMemcacheValue, implementado para
            //          &str mas não para &AmxString, então &*value força o tipo correto
            match self.clients[con].set(&key, &*value, expire) {
                Ok(_) => Ok(MemcacheResult::Success(1)),
                Err(_) => Ok(MemcacheResult::NoKey),
            }
        } else {
            Ok(MemcacheResult::NoClient)
        }
    }

    #[native(name = "Memcached_Increment")]
    pub fn increment(
        &mut self,
        _: &Amx,
        con: usize,
        key: AmxString,
        value: i32,
    ) -> AmxResult<MemcacheResult> {
        if con < self.clients.len() {
            match self.clients[con].increment(&key, value as u64) {
                Ok(_) => Ok(MemcacheResult::Success(1)),
                Err(_) => Ok(MemcacheResult::NoKey),
            }
        } else {
            Ok(MemcacheResult::NoClient)
        }
    }

    #[native(name = "Memcached_Delete")]
    pub fn delete(&mut self, _: &Amx, con: usize, key: AmxString) -> AmxResult<MemcacheResult> {
        if con < self.clients.len() {
            match self.clients[con].delete(&key) {
                Ok(true) => Ok(MemcacheResult::Success(1)),
                Ok(false) => Ok(MemcacheResult::NoData),
                Err(_) => Ok(MemcacheResult::NoKey),
            }
        } else {
            Ok(MemcacheResult::NoClient)
        }
    }
}

// impl SampPlugin manual porque há lógica em on_load
impl SampPlugin for Memcached {
    fn on_load(&mut self) {
        info!("that's a info msg");
        error!("that's an error msg");
        debug!("testique");
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
        samp::plugin::enable_process_tick();
        samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1251);

        let samp_logger = samp::plugin::logger()
            .level(log::LevelFilter::Info);

        let log_file = fern::log_file("myplugin.log").expect("Something wrong!");

        let trace_level = fern::Dispatch::new()
            .level(log::LevelFilter::Trace)
            .chain(log_file);

        let _ = fern::Dispatch::new()
            .format(|callback, message, record| {
                callback.finish(format_args!(
                    "memcached {}: {}",
                    record.level().to_string().to_lowercase(),
                    message
                ))
            })
            .chain(samp_logger)
            .chain(trace_level)
            .apply();

        return Memcached {
            clients: Vec::new(),
        };
    }
);
