//! Stateful plugin example with full lifecycle.
//!
//! Demonstrates:
//! - State on the plugin struct (`count`, `max`, `ticks`)
//! - Manual `impl SampPlugin` with `on_load` and `process_tick`
//! - `initialize_plugin!` with constructor block
//! - `Ref<i32>` for output by reference (`&value` in PAWN)
//! - Multiple natives with real logic
//!
//! Natives exposed to PAWN:
//! ```pawn
//! native Counter_Increment();
//! native Counter_Decrement();
//! native Counter_Reset();
//! native Counter_Get(&out);
//! native Counter_SetMax(max);
//! native bool:Counter_IsAtMax();
//! ```

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
        // Logs the state every ~5 seconds (1000 ticks x ~5ms)
        if self.ticks.is_multiple_of(1000) {
            info!(
                "Counter tick={} count={}/{}",
                self.ticks, self.count, self.max
            );
        }
    }
}

impl Counter {
    /// Increments the counter. Returns the new value, or -1 if already at the maximum.
    #[native(name = "Counter_Increment")]
    fn increment(&mut self, _amx: &Amx) -> i32 {
        if self.count >= self.max {
            return -1;
        }
        self.count += 1;
        self.count
    }

    /// Decrements the counter. Returns the new value, or -1 if already zero.
    #[native(name = "Counter_Decrement")]
    fn decrement(&mut self, _amx: &Amx) -> i32 {
        if self.count <= 0 {
            return -1;
        }
        self.count -= 1;
        self.count
    }

    /// Resets the counter. Returns the value that was discarded.
    #[native(name = "Counter_Reset")]
    fn reset(&mut self, _amx: &Amx) -> i32 {
        let old = self.count;
        self.count = 0;
        old
    }

    /// Writes the current value into `out` (output by reference).
    ///
    /// ```pawn
    /// new val;
    /// Counter_Get(val);
    /// printf("Value: %d", val);
    /// ```
    #[native(name = "Counter_Get")]
    fn get(&mut self, _amx: &Amx, mut out: Ref<i32>) -> bool {
        *out = self.count;
        true
    }

    /// Sets the maximum value of the counter.
    #[native(name = "Counter_SetMax")]
    fn set_max(&mut self, _amx: &Amx, max: i32) -> bool {
        if max <= 0 {
            return false;
        }
        self.max = max;
        if self.count > self.max {
            self.count = self.max;
        }
        true
    }

    /// Returns true if the counter is at the maximum value.
    #[native(name = "Counter_IsAtMax")]
    fn is_at_max(&mut self, _amx: &Amx) -> bool {
        self.count >= self.max
    }
}

initialize_plugin!(
    natives: [
        Counter::increment,
        Counter::decrement,
        Counter::reset,
        Counter::get,
        Counter::set_max,
        Counter::is_at_max,
    ],
    {
        samp::plugin::enable_server_tick();

        let _ = fern::Dispatch::new()
            .level(log::LevelFilter::Info)
            .chain(samp::plugin::logger())
            .apply();

        return Counter {
            count: 0,
            max: 100,
            ticks: 0,
        };
    }
);
