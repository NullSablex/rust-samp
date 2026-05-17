# Advanced examples

This chapter walks through the larger plugins shipped under
[`examples/`](../examples). They build on top of [Plugin
anatomy](plugin-anatomy.md) and [Natives](natives.md) and exercise the
more advanced features of the SDK.

## `examples/counter` — stateful plugin with the unified tick

[`examples/counter`](../examples/counter) demonstrates:

- A struct that holds plugin state (`count`, `max`, `ticks`).
- A handwritten `impl SampPlugin` overriding `on_load`, `on_unload`,
  and `on_server_tick`.
- The full `initialize_plugin!` form with a constructor block that
  enables the unified tick and a custom `fern` dispatch.
- Multiple natives, including one that writes through `Ref<i32>`.

```rust
use log::info;
use samp::prelude::*;
use samp::{initialize_plugin, native};

struct Counter {
    count: i32,
    max: i32,
    ticks: u32,
}

impl SampPlugin for Counter {
    fn on_load(&mut self) {
        info!("Counter plugin loaded. Max={}", self.max);
    }

    fn on_unload(&mut self) {
        info!("Counter plugin unloaded. Final value={}", self.count);
    }

    fn on_server_tick(&mut self) {
        self.ticks += 1;
        if self.ticks.is_multiple_of(1000) {
            info!("Counter tick={} count={}/{}", self.ticks, self.count, self.max);
        }
    }
}

impl Counter {
    #[native(name = "Counter_Get")]
    fn get(&mut self, _amx: &Amx, mut out: Ref<i32>) -> bool {
        *out = self.count;
        true
    }

    #[native(name = "Counter_Increment")]
    fn increment(&mut self, _amx: &Amx) -> i32 {
        if self.count >= self.max { return -1; }
        self.count += 1;
        self.count
    }
    // … decrement, reset, set_max, is_at_max
}

initialize_plugin!(
    natives: [
        Counter::increment, Counter::decrement, Counter::reset,
        Counter::get, Counter::set_max, Counter::is_at_max,
    ],
    {
        samp::plugin::enable_server_tick();

        let _ = fern::Dispatch::new()
            .level(log::LevelFilter::Info)
            .chain(samp::plugin::logger())
            .apply();

        return Counter { count: 0, max: 100, ticks: 0 };
    }
);
```

Pawn-side declarations:

```pawn
native Counter_Increment();
native Counter_Decrement();
native Counter_Reset();
native Counter_Get(&out);
native Counter_SetMax(max);
native bool:Counter_IsAtMax();
```

## `examples/advanced` — memcache client with custom types

[`examples/advanced`](../examples/advanced) demonstrates:

- A custom return type implementing `AmxCell`.
- Persistent plugin state (`Vec<memcache::Client>`).
- Multiple native shapes — strings, refs, output buffers.
- The `encoding` feature in use (Windows-1251 explicit).
- A layered `fern` dispatch: server log + custom file at `Trace`
  level.

### Custom return type

```rust
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
            MemcacheResult::Success(v) => *v,
            MemcacheResult::NoData     => -1,
            MemcacheResult::NoClient   => -2,
            MemcacheResult::NoKey      => -3,
        }
    }
}
```

From Pawn the result is read as a plain integer:

```pawn
new id = Memcached_Connect("memcache://127.0.0.1:11211");
if (id >= 0) {
    // id is the connection slot
} else if (id == -2) {
    // connection failed
}
```

### Working with `&AmxString` in generic contexts

`Client::connect` is generic over `Connectable`, which is implemented
for `&str` but not for `&AmxString`. Rust does not apply deref
coercion on generic bounds, so the explicit `&**` is required:

```rust
#[native(name = "Memcached_Connect")]
pub fn connect(&mut self, _: &Amx, address: &AmxString) -> MemcacheResult {
    match Client::connect(&**address) {
        Ok(client) => {
            self.clients.push(client);
            let idx = i32::try_from(self.clients.len()).unwrap_or(i32::MAX);
            MemcacheResult::Success(idx - 1)
        }
        Err(_) => MemcacheResult::NoClient,
    }
}
```

When the parameter has a concrete `&str` type (no generic bound), the
deref coercion happens automatically and `name` is enough.

### Output through `Ref<i32>`

```rust
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
            Ok(Some(data)) => { *value = data; MemcacheResult::Success(1) }
            Ok(None)       => MemcacheResult::NoData,
            Err(_)         => MemcacheResult::NoKey,
        }
    } else {
        MemcacheResult::NoClient
    }
}
```

### Writing a string back through `UnsizedBuffer`

```rust
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
                buffer.write_str(size, &data)?;
                Ok(MemcacheResult::Success(1))
            }
            Ok(None) => Ok(MemcacheResult::NoData),
            Err(_)   => Ok(MemcacheResult::NoKey),
        }
    } else {
        Ok(MemcacheResult::NoClient)
    }
}
```

### Layered `fern` dispatch

```rust
initialize_plugin!(
    natives: [
        Memcached::connect, Memcached::get, Memcached::set,
        Memcached::get_string, Memcached::set_string,
        Memcached::increment, Memcached::delete,
    ],
    {
        samp::plugin::enable_server_tick();
        samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1251);

        let samp_logger = samp::plugin::logger()
            .level(log::LevelFilter::Info);

        let log_file = fern::log_file("myplugin.log")
            .expect("failed to open log file");

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

        return Memcached { clients: Vec::new() };
    }
);
```

## Useful patterns

### Per-AMX state

When the server hosts both a gamemode and one or more filterscripts,
each gets its own `Amx`. Keep per-script state keyed by `AmxIdent`:

```rust
use std::collections::HashMap;
use samp::amx::AmxExt;

struct MyPlugin {
    by_amx: HashMap<samp::amx::AmxIdent, Vec<String>>,
}

impl SampPlugin for MyPlugin {
    fn on_amx_load(&mut self, amx: &Amx) {
        self.by_amx.insert(amx.ident(), Vec::new());
    }
    fn on_amx_unload(&mut self, amx: &Amx) {
        self.by_amx.remove(&amx.ident());
    }
}
```

### Throttling work inside the tick

```rust
struct MyPlugin { tick_count: u64 }

impl SampPlugin for MyPlugin {
    fn on_server_tick(&mut self) {
        self.tick_count += 1;
        if self.tick_count.is_multiple_of(1000) {
            self.periodic_work();
        }
    }
}
```

A 5 ms tick × 1000 iterations ≈ 5 seconds between runs.
