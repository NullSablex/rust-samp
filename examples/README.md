# Examples

Three sample plugins, each one progressively richer. They share the
same workspace, so a single `cargo build --target i686-unknown-linux-gnu`
at the repository root produces every artefact.

| Plugin                  | Focus                                                                                | Path                          |
| ----------------------- | ------------------------------------------------------------------------------------ | ----------------------------- |
| [`hello`](hello/)       | Minimal stateless plugin. `#[derive(SampPlugin)]`, `&AmxString`, `write_str`.        | [`examples/hello/`](hello/)   |
| [`counter`](counter/)   | Stateful plugin with the unified tick. `Ref<i32>`, full constructor block, `fern`.   | [`examples/counter/`](counter/)|
| [`advanced`](advanced/) | Memcache client. Custom `AmxCell`, `encoding` feature, layered `fern` dispatch.      | [`examples/advanced/`](advanced/)|

Each example folder ships its own `README.md` covering the natives
exposed to Pawn, the Rust patterns it demonstrates, and how to call
it from a script.

## Building one example

```sh
cargo build --release --target i686-unknown-linux-gnu -p hello
cargo build --release --target i686-unknown-linux-gnu -p counter
cargo build --release --target i686-unknown-linux-gnu -p advanced
```

The compiled `.so` lands in
`target/i686-unknown-linux-gnu/release/lib<name>.so`. Drop it into a
server's `plugins/` directory to load it.

## Suggested reading order

1. [`hello`](hello/) — read this first to see the smallest viable
   plugin (one native, no state, no lifecycle overrides).
2. [`counter`](counter/) — adds custom state, a hand-written
   `impl SampPlugin`, the unified `on_tick`, and `Ref<T>`
   output by reference.
3. [`advanced`](advanced/) — combines everything plus the `encoding`
   feature, a custom return type implementing `AmxCell`, and a
   non-trivial `fern` dispatch.

For the underlying concepts referenced by each example, see
[`docs/`](../docs/) — in particular
[`docs/first-plugin.md`](../docs/first-plugin.md) and
[`docs/plugin-anatomy.md`](../docs/plugin-anatomy.md).
