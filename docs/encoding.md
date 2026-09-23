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

## Which encodings are available

`set_default_encoding` takes any `&'static Encoding`, so **every encoding
`encoding_rs` implements** — the whole WHATWG set — can be used. These are
re-exported from `samp::encoding`, so a plugin does not have to depend on
`encoding_rs` to name one:

| Encoding | Where it is used |
| -------- | ---------------- |
| `WINDOWS_1250` | Polish, Czech, Slovak, Hungarian, Romanian, Croatian |
| `WINDOWS_1251` | Russian and other Cyrillic scripts |
| `WINDOWS_1252` | Western Europe, Latin America (the default) |
| `WINDOWS_1253` | Greek |
| `WINDOWS_1254` | Turkish |
| `WINDOWS_1256` | Arabic |
| `WINDOWS_1257` | Baltic — Lithuanian, Latvian, Estonian |
| `ISO_8859_2` | Central Europe, for scripts predating CP1250 |
| `UTF_8` | Open Multiplayer scripts written in UTF-8 |

Anything else still works through `encoding_rs` directly, or by label below.

## Choosing the encoding at runtime

A plugin whose users span several regions should not compile the encoding in.
`set_default_encoding_by_label` resolves the name a configuration file would
carry:

```rust
match samp::encoding::set_default_encoding_by_label(&configured) {
    Some(enc) => info!("encoding: {}", enc.name()),
    None => warn!("unknown encoding {configured:?}; keeping the current one"),
}
```

Label matching follows the [WHATWG Encoding Standard][labels]: case and
surrounding whitespace are ignored, and the documented aliases work, so
`windows-1251`, `WINDOWS-1251` and `x-cp1251` all name the same encoding. An
unknown label returns `None` and leaves the current encoding untouched — worth
reporting, since the alternative is running in the wrong encoding silently.

Aliases do not always say what they look like: `cyrillic` is ISO-8859-5, **not**
Windows-1251.

[labels]: https://encoding.spec.whatwg.org/#names-and-labels

## Multi-byte encodings

Pawn assumes one cell holds one character. That is true for every 8-bit encoding
in the table, and false for UTF-8, GBK, Big5, Shift_JIS and EUC-KR, where a
character may take several bytes.

Converting in and out of Rust stays correct. What changes is the Pawn side:
`strlen` counts bytes rather than characters, and `text[3]` is the fourth byte,
not the fourth character. Pick a multi-byte encoding only when the script was
written for it.

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
