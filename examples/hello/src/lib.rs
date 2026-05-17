//! Minimal rust-samp plugin example.
//!
//! Demonstrates:
//! - `#[derive(SampPlugin, Default)]` — no lifecycle boilerplate
//! - `initialize_plugin!(type: T, ...)` — constructor via `Default::default()`
//! - `AmxString` with `Deref<Target=str>` — `&str` methods without allocation
//! - `UnsizedBuffer::write_str` — write output string in a single step
//!
//! Natives exposed to PAWN:
//! ```pawn
//! native Hello_Greet(const name[], greeting[] = "", size = sizeof(greeting));
//! ```

use samp::prelude::*;
use samp::{SampPlugin, initialize_plugin, native};

#[derive(SampPlugin, Default)]
struct Hello;

impl Hello {
    /// Greets a player, writing the message into `greeting`.
    ///
    /// ```pawn
    /// new msg[64];
    /// Hello_Greet("World", msg);
    /// // msg == "Hello, World! (5 letters)"
    /// ```
    #[native(name = "Hello_Greet")]
    fn greet(
        _amx: &Amx,
        name: &AmxString,
        greeting: UnsizedBuffer,
        size: usize,
    ) -> AmxResult<bool> {
        // AmxString implements Deref<Target=str> — &str methods available directly
        let msg = if name.is_empty() {
            "Hello, Anonymous!".to_string()
        } else if name.starts_with("Admin") {
            format!("[ADMIN] Welcome, {}!", &**name)
        } else {
            format!("Hello, {}! ({} letters)", &**name, name.len())
        };

        greeting.write_str(size, &msg)?;
        Ok(true)
    }
}

initialize_plugin!(
    type: Hello,
    natives: [Hello::greet],
);
