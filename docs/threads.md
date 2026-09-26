# Background Work and the Main Thread

Both SA-MP and Open Multiplayer are single-threaded. Natives, callbacks and
the tick all run on one thread, and the AMX VM must only ever be touched from
it. So a plugin that talks to the network or to a disk has two rules:

1. **Never block that thread.** A native that waits two seconds for an HTTP
   response freezes the server for every player for two seconds.
2. **Never touch the VM from another thread.** No `Amx`, no `exec_public!`, no
   cell reads — those are not synchronized and the server is not expecting a
   second caller.

`samp::mainthread` is the return path that satisfies both: do the slow work on
your own thread, then post a closure back.

## The pattern

```rust
use samp::prelude::*;
use samp::{exec_public, native};

#[native(name = "MyPlugin_Fetch")]
fn fetch(&mut self, amx: &Amx, url: &AmxString) -> AmxResult<bool> {
    // An ident is just an address wrapper, so it crosses threads.
    // `&Amx` does not, which is why the job looks it up again on arrival.
    let ident = amx.ident();
    let url = url.to_string();

    std::thread::spawn(move || {
        let body = do_the_slow_request(&url);       // off the main thread

        samp::mainthread::post(move || {           // back on the main thread
            let Some(amx) = samp::amx::get(ident) else {
                return;                            // script unloaded meanwhile
            };
            let _ = exec_public!(amx, "OnFetchDone", &body => string);
        });
    });

    Ok(true)                                        // returns immediately
}
```

The native returns at once; the Pawn side hears back through a callback, which
is the same shape every threaded SA-MP plugin uses.

`samp::amx::get` returning `None` is not an edge case to ignore: a script can be
unloaded while the work is in flight. Handling it here is what keeps the job
from reaching a dead VM.

## Reaching the plugin's state

The result of background work usually has to land somewhere in the plugin, and a
worker thread cannot put it there: the plugin lives on the main thread and is not
`Sync`. `post_with` hands the job `&mut T` at the moment it runs:

```rust
samp::mainthread::post_with::<MyPlugin>(move |plugin| {
    plugin.delivered += 1;
});
```

`T` is the type `initialize_plugin!` creates. Naming another logs a warning and
skips the job, rather than reinterpreting the plugin's bytes.

The usual shape is both: record the result, then tell the script. Those are two
steps for a reason — while the closure holds `&mut T`, a `public` that re-entered
one of this plugin's natives would take a second `&mut` to the same plugin.
`post_with_amx` splits them, running what the closure returns after the borrow
has ended:

```rust
samp::mainthread::post_with_amx::<MyPlugin, _>(ident, move |plugin| {
    plugin.delivered += 1;
    let total = plugin.delivered;

    // Runs next, with no borrow of the plugin alive.
    move |amx: &Amx| {
        let _ = exec_public!(amx, "OnMailSent", total);
    }
});
```

The job is dropped, quietly, if the script was unloaded before it ran.

If a native does run while a job holds the plugin, the SDK says so once in the
log — it cannot refuse, because the native still has to answer the script, but it
will not let the aliasing pass unmentioned.

Outside the queue, the same access is `samp::plugin::with_instance`:

```rust
let total = samp::plugin::with_instance::<MyPlugin, _>(|plugin| plugin.delivered);
```

It returns `None`, with a warning naming the reason, when the plugin cannot be
lent out: the wrong `T`, no plugin yet, a borrow already alive, or a call from
inside a native — where `&mut self` is already in scope and is what to use.

## Draining

Queued jobs run from the same place as `SampPlugin::on_tick`, so the plugin
needs the tick enabled:

```rust
initialize_plugin!(
    natives: [ /* ... */ ],
    {
        samp::plugin::enable_tick();   // required for the queue to drain
        MyPlugin::default()
    }
);
```

Without it nothing drains the queue and jobs accumulate; the SDK logs a warning
once the backlog reaches 10,000, which is far past a normal burst. A plugin that
does not want a tick can drive the queue itself with
`samp::mainthread::run_pending()` — from inside a native, for instance.

`samp::mainthread::pending()` reports the backlog, which is useful in a
diagnostics native.

## Guarantees

- Jobs run **in the order they were posted**, one after another, on the main
  thread.
- A job posted while the queue is draining runs on the **next** tick. A job that
  re-posts itself therefore cannot spin inside a single tick.
- A **panic inside a job is caught and logged**, and the jobs after it still
  run. An unwind reaching the server's C++ frames would abort the process.
- The queue is process-wide and works in every mode: SA-MP `ProcessTick` and the
  Open Multiplayer timer both drain it.

## What this is not

It is not an async runtime. There is no executor, no `await`, no timer wheel —
just a queue that crosses the thread boundary. Pair it with whatever you already
use (`std::thread`, a thread pool, a Tokio runtime living inside the plugin) and
use `post` only for the final hop back.

It is also not a way to share the plugin. `with_instance` lends it for the length
of one closure, on one thread; there is no handle to keep, and nothing to lock.
