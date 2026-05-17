# `counter` — stateful plugin

Plugin with persistent state, a hand-written `impl SampPlugin`, and
the unified server tick. Larger surface than `hello` without the
extra dependencies of `advanced`.

## What it demonstrates

- Plugin state held in the struct (`count`, `max`, `ticks`).
- Manual `impl SampPlugin` overriding `on_load`, `on_unload`, and
  `on_server_tick`.
- The full `initialize_plugin!` form with a constructor block.
- `samp::plugin::enable_server_tick()` — opts in to the unified
  tick (fires on SA-MP via `ProcessTick`, on native Open Multiplayer
  via a 5 ms `ITimersComponent` timer).
- `samp::plugin::logger()` — chains the default server log sink into
  a custom `fern::Dispatch`.
- `Ref<i32>` — output by reference (`Counter_Get(&out)`).
- Multiple natives sharing the same plugin state.

## Natives

```pawn
native Counter_Increment();                  // returns new value, or -1 if already at max
native Counter_Decrement();                  // returns new value, or -1 if already 0
native Counter_Reset();                      // returns the value that was discarded
native Counter_Get(&out);                    // writes the current value into `out`
native Counter_SetMax(max);                  // sets the cap; clamps current value if needed
native bool:Counter_IsAtMax();               // true when count >= max
```

Initial state: `count = 0`, `max = 100`, `ticks = 0`.

Every ~5 seconds (1000 ticks × ~5 ms) the plugin logs the current
tick count and counter value at `info` level.

## Pawn usage

```pawn
public OnGameModeInit()
{
    Counter_SetMax(10);

    for (new i = 0; i < 5; i++) {
        new value = Counter_Increment();
        printf("count = %d", value);
    }

    new current;
    Counter_Get(current);
    printf("current = %d, at max = %d", current, Counter_IsAtMax());

    Counter_Reset();
    return 1;
}
```

## Build

```sh
cargo build --release --target i686-unknown-linux-gnu -p counter
```

Output: `target/i686-unknown-linux-gnu/release/libcounter.so`.

## Source

See [`src/lib.rs`](src/lib.rs).
