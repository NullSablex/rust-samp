//! Handing work back to the server's main thread.
//!
//! Both servers are single-threaded: every native, callback and tick runs on
//! one thread, and the AMX VM must only ever be touched from it. A plugin that
//! does I/O — HTTP, a database, SMTP — has to do that work elsewhere and bring
//! the result back, because blocking the main thread freezes the server for
//! every player.
//!
//! This module is that return path. A worker thread calls [`post`] with a
//! closure; the closure runs on the main thread on the next tick.
//!
//! ```rust,no_run
//! use samp::exec_public;
//! use samp::prelude::*;
//! # fn example(amx_ident: samp::amx::AmxIdent) {
//! std::thread::spawn(move || {
//!     let answer = 42; // ... the slow work ...
//!
//!     samp::mainthread::post(move || {
//!         // Back on the main thread: safe to touch the VM.
//!         if let Some(amx) = samp::amx::get(amx_ident) {
//!             let _ = exec_public!(amx, "OnWorkDone", answer);
//!         }
//!     });
//! });
//! # }
//! ```
//!
//! An [`AmxIdent`] is a plain address wrapper, so it crosses thread boundaries;
//! the `&Amx` it resolves to does not, which is why the job looks it up again
//! after arriving. [`samp::amx::get`] returns `None` if the script was unloaded
//! meanwhile — the case this pattern makes easy to handle instead of dangling.
//!
//! ## Draining
//!
//! Jobs run from the same place as [`SampPlugin::on_tick`], so the plugin needs
//! `samp::plugin::enable_tick()` in `initialize_plugin!`. Without it nothing
//! drains the queue and jobs pile up; the SDK logs a warning once the backlog
//! is large enough to be a mistake rather than a burst. A plugin that does not
//! want the tick can call [`run_pending`] from wherever it prefers, such as
//! inside a native.
//!
//! A job posted while the queue is draining runs on the **next** tick, not the
//! current one. That keeps a job that re-posts itself from spinning forever
//! inside one tick.
//!
//! [`AmxIdent`]: crate::amx::AmxIdent
//! [`samp::amx::get`]: crate::amx::get
//! [`SampPlugin::on_tick`]: crate::plugin::SampPlugin::on_tick

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use crate::macros::sdk_warn;

/// Work queued from another thread, to run on the main thread.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// Backlog at which the SDK warns once. A plugin bursting a few hundred jobs
/// between ticks is normal; five figures means nothing is draining them.
const BACKLOG_WARNING: usize = 10_000;

fn queue() -> &'static Mutex<Vec<Job>> {
    static QUEUE: OnceLock<Mutex<Vec<Job>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

/// Queues `job` to run on the main thread at the next tick.
///
/// Callable from any thread. The job runs once, in the order it was posted.
///
/// A panic inside the job is caught and logged: it must not unwind into the
/// server's C++ frames, which would abort the process.
pub fn post<F>(job: F)
where
    F: FnOnce() + Send + 'static,
{
    // A poisoned lock means some earlier holder panicked. The queue itself is
    // still consistent — a `Vec` of jobs — so recovering beats refusing every
    // later post for the lifetime of the process.
    let mut guard = queue().lock().unwrap_or_else(PoisonError::into_inner);
    guard.push(Box::new(job));

    if guard.len() >= BACKLOG_WARNING {
        static WARNED: AtomicBool = AtomicBool::new(false);
        if !WARNED.swap(true, Ordering::Relaxed) {
            sdk_warn!(
                "{} jobs queued for the main thread and nothing is draining them \
                 — is `samp::plugin::enable_tick()` missing from `initialize_plugin!`?",
                guard.len()
            );
        }
    }
}

/// Number of jobs waiting to run.
#[must_use]
pub fn pending() -> usize {
    queue().lock().unwrap_or_else(PoisonError::into_inner).len()
}

/// Runs every queued job and returns how many ran.
///
/// Called by the SDK on each tick. A plugin only needs it when it drives the
/// queue itself — with the tick disabled, for instance.
///
/// Must be called from the main thread: the jobs assume they are on it.
pub fn run_pending() -> usize {
    // Take the jobs out under the lock and run them with it released: a job is
    // allowed to post more work (which lands on the next drain), and running
    // while holding the lock would deadlock on that.
    let jobs: Vec<Job> = {
        let mut guard = queue().lock().unwrap_or_else(PoisonError::into_inner);
        std::mem::take(&mut *guard)
    };

    let count = jobs.len();
    for job in jobs {
        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)).is_err() {
            sdk_warn!("a job posted to the main thread panicked; it was dropped");
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex as StdMutex};

    /// The queue is process-wide, so the tests take turns on it.
    static TEST_LOCK: StdMutex<()> = StdMutex::new(());

    fn drain_quietly() {
        run_pending();
    }

    #[test]
    fn jobs_run_in_the_order_they_were_posted() {
        let _g = TEST_LOCK.lock().unwrap();
        drain_quietly();

        let seen = Arc::new(StdMutex::new(Vec::new()));
        for i in 0..3 {
            let seen = Arc::clone(&seen);
            post(move || seen.lock().unwrap().push(i));
        }

        assert_eq!(run_pending(), 3);
        assert_eq!(*seen.lock().unwrap(), vec![0, 1, 2]);
    }

    #[test]
    fn pending_counts_and_draining_clears() {
        let _g = TEST_LOCK.lock().unwrap();
        drain_quietly();

        post(|| {});
        post(|| {});
        assert_eq!(pending(), 2);

        assert_eq!(run_pending(), 2);
        assert_eq!(pending(), 0);
        assert_eq!(run_pending(), 0);
    }

    #[test]
    fn a_job_posted_while_draining_waits_for_the_next_drain() {
        let _g = TEST_LOCK.lock().unwrap();
        drain_quietly();

        let runs = Arc::new(AtomicUsize::new(0));
        let inner = Arc::clone(&runs);
        post(move || {
            let deeper = Arc::clone(&inner);
            post(move || {
                deeper.fetch_add(1, Ordering::Relaxed);
            });
        });

        assert_eq!(run_pending(), 1, "only the outer job runs in this drain");
        assert_eq!(runs.load(Ordering::Relaxed), 0);
        assert_eq!(run_pending(), 1, "the re-posted job runs in the next one");
        assert_eq!(runs.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn a_panicking_job_does_not_stop_the_ones_after_it() {
        let _g = TEST_LOCK.lock().unwrap();
        drain_quietly();

        let ran = Arc::new(AtomicUsize::new(0));
        let after = Arc::clone(&ran);
        post(|| panic!("job blew up"));
        post(move || {
            after.fetch_add(1, Ordering::Relaxed);
        });

        assert_eq!(run_pending(), 2);
        assert_eq!(ran.load(Ordering::Relaxed), 1);

        // And the queue still works afterwards.
        let later = Arc::clone(&ran);
        post(move || {
            later.fetch_add(1, Ordering::Relaxed);
        });
        assert_eq!(run_pending(), 1);
        assert_eq!(ran.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn a_worker_thread_can_post() {
        let _g = TEST_LOCK.lock().unwrap();
        drain_quietly();

        let ran = Arc::new(AtomicUsize::new(0));
        let from_worker = Arc::clone(&ran);
        std::thread::spawn(move || {
            post(move || {
                from_worker.fetch_add(1, Ordering::Relaxed);
            });
        })
        .join()
        .unwrap();

        assert_eq!(run_pending(), 1);
        assert_eq!(ran.load(Ordering::Relaxed), 1);
    }
}
