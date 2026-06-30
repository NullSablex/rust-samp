# String encoding

SA-MP and its scripts use legacy 8-bit Windows code pages instead of
UTF-8. rust-samp provides transparent conversion between those
code pages and Rust strings through the optional `encoding` feature.

## Why it matters

A Pawn string is a sequence of bytes in a specific code page:

- **Windows-1252** — extended Latin (Western servers).
- **Windows-1251** — Cyrillic (Russian / Slavic servers).

Rust strings are always UTF-8. Without an explicit conversion, accented
or Cyrillic characters end up corrupted.

## Enabling the feature

From crates.io:

```toml
[dependencies]
samp = { package = "rust-samp", version = "3", features = ["encoding"] }
```

Or via git for earlier versions:

```toml
[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git", tag = "vX.Y.Z", features = ["encoding"] }
```

Without the feature, `AmxString` decodes through `String::from_utf8_lossy`
and `Allocator::allot_string` copies the raw bytes.

## Setting the active encoding

Pick the code page once inside the constructor block of
`initialize_plugin!`:

```rust
initialize_plugin!(
    natives: [],
    {
        // Western Latin (the default — explicit for clarity)
        samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1252);

        // Or, on a Russian server:
        // samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1251);

        return MyPlugin::default();
    }
);
```

The default before any call is `WINDOWS_1252`.

## How it propagates

The configured encoding is consulted by:

1. `AmxString::deref` (and therefore `to_string()`, `Display`,
   comparisons against `&str` / `String`) — when decoding the cells
   into a Rust string.
2. `Allocator::allot_string` — when encoding a Rust string for the AMX
   heap.

```rust
#[native(name = "ProcessText")]
fn process_text(&mut self, _amx: &Amx, text: AmxString) -> AmxResult<bool> {
    // Decoded once, cached in a OnceCell<String>
    println!("{}", &*text);
    Ok(true)
}
```

## Available encodings

| Constant       | Code page  | Typical use                |
| -------------- | ---------- | -------------------------- |
| `WINDOWS_1252` | CP-1252    | Extended Latin (default).  |
| `WINDOWS_1251` | CP-1251    | Cyrillic.                  |

The two constants are re-exports of `encoding_rs::WINDOWS_1252` /
`WINDOWS_1251`. Any `&'static Encoding` accepted by `encoding_rs` can
be passed to `set_default_encoding`.

## Storage

The active encoding is stored in an `AtomicPtr<Encoding>` with
`Ordering::Release` on writes and `Ordering::Acquire` on reads. The
setting is global to the plugin.

## When the feature is unnecessary

Pure-ASCII servers (letters A–Z, digits, basic punctuation) do not need
the feature — ASCII is identical between UTF-8 and the two Windows code
pages. Enable `encoding` only when one of the following is required:

- Latin accented characters (`á`, `é`, `ñ`, `ç`, …).
- Cyrillic characters (`а`, `б`, `в`, `г`, …).
- Any other byte outside the ASCII range.
