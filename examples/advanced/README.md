# `advanced` — memcache client

Real-world plugin combining the `encoding` feature, a custom return
type, an external dependency, and a layered `fern` dispatch.

## What it demonstrates

- Custom return type implementing `AmxCell` (`MemcacheResult`).
- Persistent plugin state (`Vec<memcache::Client>`).
- Multiple native shapes — input strings (`&AmxString`), output
  buffers (`UnsizedBuffer`), output by reference (`Ref<i32>`).
- Working with `&AmxString` in generic contexts (`&**name` when the
  target trait bound is `T: SomeTrait`-style).
- `samp::encoding::set_default_encoding(WINDOWS_1251)` — switches
  the active code page (Cyrillic in this example).
- Layered `fern` dispatch: server log at `Info` + a separate
  `Trace`-level file dispatch (`myplugin.log`).

## Required dependencies

This example pulls `memcache = "0.19"` and uses
`samp = { … features = ["encoding"] }`. The `samp-only` feature is
not used, so the binary is also a native Open Multiplayer component.

A running memcached server is required to exercise the natives.

## Natives

```pawn
native Memcached_Connect(const address[]);                                          // returns connection id (>=0) or error (<0)
native Memcached_Get(con, const key[], &value);                                     // writes into &value
native Memcached_GetString(con, const key[], buffer[], size = sizeof(buffer));      // writes into buffer
native Memcached_Set(con, const key[], value, expire);                              // expire in seconds
native Memcached_SetString(con, const key[], const value[], expire);
native Memcached_Increment(con, const key[], value);
native Memcached_Delete(con, const key[]);
```

`MemcacheResult` encoding (`AmxCell::as_cell`):

| Variant            | Cell value | Meaning                                    |
| ------------------ | :--------: | ------------------------------------------ |
| `Success(n)`       | `n`        | Operation succeeded. For `Connect`, `n` is the slot in the internal client list. |
| `NoData`           | `-1`       | Key existed but no value was stored.       |
| `NoClient`         | `-2`       | `con` is out of range (no such connection).|
| `NoKey`            | `-3`       | Backend reported an error (write failed, missing key, etc.). |

## Pawn usage

```pawn
public OnGameModeInit()
{
    new id = Memcached_Connect("memcache://127.0.0.1:11211");
    if (id < 0) {
        printf("memcached: connect failed (%d)", id);
        return 1;
    }

    Memcached_Set(id, "hits", 0, 0);
    Memcached_Increment(id, "hits", 5);

    new value;
    Memcached_Get(id, "hits", value);
    printf("hits = %d", value);

    new buf[64];
    Memcached_SetString(id, "name", "rust-samp", 0);
    Memcached_GetString(id, "name", buf);
    printf("name = %s", buf);
    return 1;
}
```

## Build

```sh
cargo build --release --target i686-unknown-linux-gnu -p advanced
```

Output: `target/i686-unknown-linux-gnu/release/libadvanced.so`.

## Source

See [`src/lib.rs`](src/lib.rs).
