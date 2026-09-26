//! Shared fixture for the tests that need a plugin installed.
//!
//! The runtime, the plugin and the main-thread queue are process-wide, so tests
//! that touch them cannot run side by side, and a second `set_plugin` would pull
//! the instance out from under whatever else is running. One plugin is installed
//! once, and [`exclusive`] serializes everyone who reaches for it.

use std::sync::{Mutex, MutexGuard, OnceLock};

/// The plugin every test shares. Its `value` is the observable state.
pub struct TestPlugin {
    pub value: u32,
}

impl crate::plugin::SampPlugin for TestPlugin {}

/// Takes the process-wide turn and installs the plugin on first use.
///
/// Holding the returned guard means no other test is touching the runtime, the
/// plugin or the queue.
pub fn exclusive() -> MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    static INSTALLED: OnceLock<()> = OnceLock::new();

    // A poisoned lock means a test panicked while holding it; the fixture is
    // still usable, and recovering keeps one failure from cascading.
    let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    INSTALLED.get_or_init(|| {
        let rt = crate::runtime::Runtime::initialize();
        rt.set_plugin(TestPlugin { value: 0 });
    });
    guard
}

/// The plugin's current `value`, for a test that only wants to observe it.
pub fn value() -> u32 {
    crate::plugin::with_instance::<TestPlugin, _>(|p| p.value)
        .expect("the fixture installs the plugin before any test body runs")
}
