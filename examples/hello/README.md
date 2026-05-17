# `hello` — minimal plugin

Smallest viable rust-samp plugin. One native, no state, no lifecycle
overrides.

## What it demonstrates

- `#[derive(SampPlugin, Default)]` — empty trait impl, no manual
  `impl SampPlugin`.
- `initialize_plugin!(type: Hello, natives: [...])` — short form
  that uses `Default::default()` as the constructor.
- `&AmxString` argument — the macro injects the borrow automatically.
- `AmxString` through `Deref<Target = str>` — `&**name` reads the
  decoded string without an extra allocation.
- `UnsizedBuffer::write_str` — output string written in one call.

## Native

```pawn
native Hello_Greet(const name[], greeting[] = "", size = sizeof(greeting));
```

Behavior:

- Empty `name` → writes `Hello, Anonymous!`.
- `name` starting with `Admin` → writes `[ADMIN] Welcome, <name>!`.
- Otherwise → writes `Hello, <name>! (<len> letters)`.

## Pawn usage

```pawn
public OnGameModeInit()
{
    new buf[64];
    Hello_Greet("World", buf);
    print(buf); // "Hello, World! (5 letters)"
    return 1;
}
```

## Build

```sh
cargo build --release --target i686-unknown-linux-gnu -p hello
```

Output: `target/i686-unknown-linux-gnu/release/libhello.so`.

## Source

See [`src/lib.rs`](src/lib.rs).
