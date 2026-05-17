# Historical changelog — `samp-rs` (pre-fork)

This page collects the history of the upstream
[`samp-rs`](https://github.com/Pycckue-Bnepeg/samp-rs) project, by
[ZOTTCE](https://github.com/ZOTTCE) and contributors, before
rust-samp was created as a fork. Versions 1.x and later (after the
fork) live in [`v1.x.md`](v1.x.md), [`v2.x.md`](v2.x.md), and in the
root [`../CHANGELOG.md`](../CHANGELOG.md).

## 0.9.x (2019)

- New SDK API with `AmxString`, `AmxCell`, `Buffer`.
- Procedural macros `#[native]` and `initialize_plugin!` replaced
  the legacy `define_native!` and `new_plugin!`.
- Packed-string support.
- Raw native arguments via `#[native(raw)]`.
- `encoding` feature with Windows-1251 / Windows-1252 support.
- Integrated logger through `fern`.
- `process_tick` support.
- `exec_public!` macro for calling Pawn callbacks.
- Migration to Rust edition 2018.

## 0.1.x – 0.8.x (2018)

- Initial bindings for the SA-MP SDK (AMX).
- Macros `new_plugin!`, `define_native!`, `natives!`.
- AMX functions: `exec`, `find_native`, `find_public`, `push_string`,
  `push_array`, `allot`, `release`.
- Utility macros: `get_string!`, `set_string!`, `get_array!`, `exec_native!`.
- `ProcessTick` support.
- Documentation and examples.

## External contributors

- **Kaperstone** — improved example code.
- **povargek** — `Logprintf_t` signature fix.
- **xakdog** — CI (Travis / AppVeyor), Windows native-call fixes, doctests.
- **Southclaws** — removed the `detour` dependency.
- **Sreyas-Sreelal** — fixes to `push_string`, packed strings, `amxStrLen`, `amxGetAddr`.
- **Cheaterman** — GDK compatibility.
