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

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::macros::sdk_warn;

/// Work queued from another thread, to run on the main thread.
type Job = Box<dyn FnOnce() + Send + 'static>;

/// Backlog at which the SDK warns once. A plugin bursting a few hundred jobs
/// between ticks is normal; five figures means nothing is draining them.
const BACKLOG_WARNING: usize = 10_000;

/// The jobs, and their count mirrored outside the lock: the main thread asks
/// on every tick, and almost always the answer is "none", which then costs a
/// load instead of locking and unlocking the mutex.
struct Queue {
    jobs: Mutex<Vec<Job>>,
    /// `jobs.len()`, written only while `jobs` is locked.
    len: AtomicUsize,
}

impl Queue {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Job>> {
        // A poisoned lock means some earlier holder panicked. The queue itself
        // is still consistent — a `Vec` of jobs — so recovering beats refusing
        // every later post for the lifetime of the process.
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Appends `job`; returns the new length.
    fn push(&self, job: Job) -> usize {
        let mut jobs = self.lock();
        jobs.push(job);
        self.len.store(jobs.len(), Ordering::Release);
        jobs.len()
    }

    /// Takes every queued job, without locking when there is none.
    fn take(&self) -> Vec<Job> {
        if self.len.load(Ordering::Acquire) == 0 {
            return Vec::new();
        }
        let mut jobs = self.lock();
        self.len.store(0, Ordering::Release);
        std::mem::take(&mut *jobs)
    }

    /// Puts `left` back ahead of whatever was posted since it was taken.
    fn put_back(&self, left: Vec<Job>) {
        let mut jobs = self.lock();
        let newer = std::mem::replace(&mut *jobs, left);
        jobs.extend(newer);
        self.len.store(jobs.len(), Ordering::Release);
    }
}

fn queue() -> &'static Queue {
    static QUEUE: Queue = Queue {
        jobs: Mutex::new(Vec::new()),
        len: AtomicUsize::new(0),
    };
    &QUEUE
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
    let backlog = queue().push(Box::new(job));

    // Outside the lock: the warning goes through the logger, which may post.
    if backlog >= BACKLOG_WARNING {
        static WARNED: AtomicBool = AtomicBool::new(false);
        if !WARNED.swap(true, Ordering::Relaxed) {
            sdk_warn!(
                "{} jobs queued for the main thread and nothing is draining them \
                 — is `samp::plugin::enable_tick()` missing from `initialize_plugin!`?",
                backlog
            );
        }
    }
}

/// Queues `job` to run on the main thread with the plugin instance.
///
/// The plugin's own state is where the result of background work usually has to
/// land, and a worker thread cannot reach it: the plugin is not `Sync`, and only
/// the main thread may touch it. This posts the job and hands it `&mut T` at the
/// moment it runs.
///
/// ```rust,no_run
/// # use samp::prelude::*;
/// # struct Mailer { sent: u32 }
/// # impl SampPlugin for Mailer {}
/// # fn example() {
/// std::thread::spawn(move || {
///     let delivered = true; // ... the slow work ...
///
///     samp::mainthread::post_with::<Mailer>(move |plugin| {
///         if delivered {
///             plugin.sent += 1;
///         }
///     });
/// });
/// # }
/// ```
///
/// `T` must be the type `initialize_plugin!` creates; naming another logs a
/// warning and skips the job, rather than reinterpreting the plugin's bytes.
///
/// **Do not call into Pawn from inside the closure.** A `public` that re-enters
/// one of this plugin's natives would take a second `&mut` to the same plugin.
/// Collect what to send, let the closure end, and send it after — or use
/// [`post_with_amx`], which is that shape already.
pub fn post_with<T>(job: impl FnOnce(&mut T) + Send + 'static)
where
    T: crate::plugin::SampPlugin + 'static,
{
    post(move || {
        crate::plugin::with_instance::<T, _>(job);
    });
}

/// Queues `job` to run on the main thread with the plugin and a resolved
/// [`Amx`], in that order.
///
/// The common shape of background work coming back: record the result in the
/// plugin, then tell the script. The closure returns what to do with the script,
/// and the SDK runs it after the plugin borrow has ended, so calling a `public`
/// from there is safe.
///
/// ```rust,no_run
/// # use samp::prelude::*;
/// # use samp::exec_public;
/// # struct Mailer { sent: u32 }
/// # impl SampPlugin for Mailer {}
/// # fn example(script: samp::amx::AmxIdent) {
/// samp::mainthread::post_with_amx::<Mailer, _>(script, |plugin| {
///     plugin.sent += 1;
///     let total = plugin.sent;
///
///     // Runs next, with no borrow of the plugin alive.
///     move |amx: &Amx| {
///         let _ = exec_public!(amx, "OnMailSent", total);
///     }
/// });
/// # }
/// ```
///
/// The job is skipped, quietly, if the script was unloaded before it ran: that
/// is the normal end of a gamemode restart, not a fault.
pub fn post_with_amx<T, R>(
    script: crate::amx::AmxIdent,
    job: impl FnOnce(&mut T) -> R + Send + 'static,
) where
    T: crate::plugin::SampPlugin + 'static,
    R: FnOnce(&samp_sdk::amx::Amx),
{
    post(move || {
        // Resolved first: with the script gone there is nothing to report, so
        // the plugin is not disturbed either.
        if crate::amx::get(script).is_none() {
            return;
        }
        let Some(reply) = crate::plugin::with_instance::<T, _>(job) else {
            return;
        };
        if let Some(amx) = crate::amx::get(script) {
            reply(amx);
        }
    });
}

/// Number of jobs waiting to run.
#[must_use]
pub fn pending() -> usize {
    queue().len.load(Ordering::Acquire)
}

/// Time one drain may spend running jobs, in nanoseconds; `0` for no limit.
static BUDGET_NANOS: AtomicU64 = AtomicU64::new(0);

/// Limits how long one drain spends running jobs; `None` (the default) runs
/// every queued job.
///
/// The server is frozen while jobs run, so a burst of ten thousand replies
/// arriving at once becomes one long stall. With a budget, a drain stops at the
/// first job that ends past it and leaves the rest, in order, for the next
/// tick — the stall is spread over several ticks instead. A job is never cut
/// short, and at least one runs per drain, so the queue always advances.
///
/// ```rust,no_run
/// // At most ~2 ms of a 5 ms SA-MP tick goes to background replies.
/// samp::mainthread::set_budget(Some(std::time::Duration::from_millis(2)));
/// ```
pub fn set_budget(budget: Option<Duration>) {
    let nanos = budget.map_or(0, |budget| {
        u64::try_from(budget.as_nanos()).unwrap_or(u64::MAX).max(1)
    });
    BUDGET_NANOS.store(nanos, Ordering::Release);
}

/// The limit [`set_budget`] set, if any.
#[must_use]
pub fn budget() -> Option<Duration> {
    match BUDGET_NANOS.load(Ordering::Acquire) {
        0 => None,
        nanos => Some(Duration::from_nanos(nanos)),
    }
}

/// Runs the queued jobs and returns how many ran: all of them, or as many as
/// fit in the [`budget`] when one is set.
///
/// Called by the SDK on each tick. A plugin only needs it when it drives the
/// queue itself — with the tick disabled, for instance.
///
/// Must be called from the main thread: the jobs assume they are on it.
pub fn run_pending() -> usize {
    // Take the jobs out under the lock and run them with it released: a job is
    // allowed to post more work (which lands on the next drain), and running
    // while holding the lock would deadlock on that.
    let jobs = queue().take();
    if jobs.is_empty() {
        return 0;
    }

    let deadline = budget().map(|budget| Instant::now() + budget);
    let mut jobs = jobs.into_iter();
    let mut ran = 0;
    for job in jobs.by_ref() {
        if crate::panic_guard::catch(job).is_err() {
            sdk_warn!("a job posted to the main thread panicked; it was dropped");
        }
        ran += 1;
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            break;
        }
    }

    // Over budget: what is left goes back ahead of anything posted meanwhile,
    // keeping the order jobs were posted in.
    let left: Vec<Job> = jobs.collect();
    if !left.is_empty() {
        queue().put_back(left);
    }
    ran
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex as StdMutex};

    use crate::test_support::{TestPlugin, exclusive, value};

    /// A second plugin type, to check that naming it reaches nothing.
    struct OtherPlugin;
    impl crate::plugin::SampPlugin for OtherPlugin {}

    /// Leaves the queue empty, so a test starts from a known state.
    fn drain_quietly() {
        run_pending();
    }

    #[test]
    fn jobs_run_in_the_order_they_were_posted() {
        let _g = exclusive();
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
        let _g = exclusive();
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
        let _g = exclusive();
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
        let _g = exclusive();
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
    fn a_job_reaches_the_plugin_from_a_worker_thread() {
        let _g = exclusive();
        drain_quietly();

        let before = value();

        std::thread::spawn(|| {
            post_with::<TestPlugin>(|plugin| plugin.value += 5);
        })
        .join()
        .unwrap();

        assert_eq!(run_pending(), 1);
        assert_eq!(value(), before + 5);
    }

    #[test]
    fn a_job_naming_the_wrong_plugin_type_is_skipped() {
        let _g = exclusive();
        drain_quietly();

        let before = value();
        post_with::<OtherPlugin>(|_| unreachable!("must not run"));

        assert_eq!(run_pending(), 1, "the job ran and refused itself");
        assert_eq!(value(), before, "the plugin was left alone");
    }

    #[test]
    fn a_reply_to_a_script_that_went_away_is_dropped() {
        let _g = exclusive();
        drain_quietly();

        let before = value();
        // An ident no AMX was ever registered under: the script is gone.
        let gone =
            crate::amx::AmxIdent::from(std::ptr::without_provenance_mut(0xDEAD_BEEF_u32 as _));

        post_with_amx::<TestPlugin, _>(gone, |plugin| {
            plugin.value += 1;
            |_amx: &samp_sdk::amx::Amx| unreachable!("there is no script to answer")
        });

        assert_eq!(run_pending(), 1);
        assert_eq!(
            value(),
            before,
            "the plugin is not disturbed when there is nothing to report to"
        );
    }

    #[test]
    fn posting_from_many_threads_while_draining_loses_nothing() {
        let _g = exclusive();
        drain_quietly();

        const THREADS: usize = 8;
        const JOBS: usize = 5_000;
        let ran = Arc::new(AtomicUsize::new(0));
        let workers: Vec<_> = (0..THREADS)
            .map(|t| {
                let ran = Arc::clone(&ran);
                std::thread::spawn(move || {
                    for i in 0..JOBS {
                        let ran = Arc::clone(&ran);
                        if i % 997 == t {
                            post(|| panic!("a job blew up"));
                        }
                        post(move || {
                            ran.fetch_add(1, Ordering::Relaxed);
                        });
                    }
                })
            })
            .collect();

        // The main thread drains while the workers are still posting.
        while workers.iter().any(|w| !w.is_finished()) {
            run_pending();
        }
        for worker in workers {
            worker.join().unwrap();
        }
        run_pending();
        assert_eq!(ran.load(Ordering::Relaxed), THREADS * JOBS);
        assert_eq!(pending(), 0);
    }

    #[test]
    fn a_budget_leaves_the_rest_for_the_next_drain_in_order() {
        let _g = exclusive();
        drain_quietly();

        let seen = Arc::new(StdMutex::new(Vec::new()));
        for i in 0..4 {
            let seen = Arc::clone(&seen);
            post(move || {
                std::thread::sleep(Duration::from_millis(2));
                seen.lock().unwrap().push(i);
            });
        }
        set_budget(Some(Duration::from_millis(1)));
        // One job runs past the budget: the drain stops after it.
        assert_eq!(run_pending(), 1);
        let late = Arc::clone(&seen);
        post(move || late.lock().unwrap().push(99));
        set_budget(None);
        assert_eq!(run_pending(), 4);
        assert_eq!(*seen.lock().unwrap(), vec![0, 1, 2, 3, 99]);
        assert_eq!(budget(), None);
    }

    #[test]
    fn a_worker_thread_can_post() {
        let _g = exclusive();
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
