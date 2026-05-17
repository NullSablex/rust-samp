# Natives

Natives are Rust functions exposed to the Pawn script. The `#[native]`
attribute generates the `extern "C"` FFI wrapper, parses each argument
from the AMX cell array, catches panics, and converts the return value
back into an AMX cell.

## Basic shape

```rust
impl MyPlugin {
    #[native(name = "MyNative")]
    fn my_native(&mut self, amx: &Amx, /* arguments */) -> AmxResult</* type */> {
        Ok(value)
    }
}
```

### Signature rules

- The first parameter is `&mut self` for plugin methods. Associated
  functions (no `self`) are also accepted — useful for stateless natives.
- The next parameter is `&Amx`. Use `_amx: &Amx` when the AMX handle is
  not needed.
- Subsequent parameters are the native arguments, parsed via the
  `AmxCell` trait.
- Returns either `AmxResult<T>` / `Result<T, E: Display>` (the wrapper
  matches `Ok`/`Err`, logging the error and returning `0` on `Err`) or
  `T` directly (`T: AmxCell<'static>`) for infallible natives.

> The macro detects the return type syntactically: if the last path
> segment is `Result` or `AmxResult`, the wrapper handles the
> `Ok`/`Err` branches. Any other return type is used as the cell value
> directly.

## The `name` argument

`name` is the Pawn-visible identifier:

```rust
#[native(name = "GetPlayerScore")]
fn get_score(&mut self, _amx: &Amx, player_id: i32) -> AmxResult<i32> {
    Ok(player_id * 100)
}
```

```pawn
native GetPlayerScore(playerid);
```

`name` is validated at proc-macro time — interior NUL bytes fail
compilation rather than panic at server load.

## Argument types

Arguments are converted from AMX cells through `AmxCell::from_raw`:

| Rust type        | Pawn equivalent     | Notes                                                                    |
| ---------------- | ------------------- | ------------------------------------------------------------------------ |
| `i32` / `u32`    | `value`             | Pawn cells are 32-bit integers.                                          |
| `i8` / `u8` / `i16` / `u16` / `isize` / `usize` | `value` | Cast from `i32` via the Pawn ABI conventions.                            |
| `f32`            | `Float:value`       | Bit-reinterprets the cell (`f32::from_bits`).                            |
| `bool`           | `bool:value`        | `0` → false, any non-zero → true.                                        |
| `&AmxString`     | `const string[]`    | Recommended for input strings. Macro injects the borrow automatically.   |
| `AmxString`      | `const string[]`    | Same data, taken by value.                                               |
| `Ref<T>`         | `&value`            | Output by reference — write through `*r`.                                |
| `UnsizedBuffer`  | `array[]`           | Unknown-length array; pair with a size argument and convert via `into_sized_buffer`. |

### Strings — `AmxString` and `&AmxString`

`AmxString` implements `Deref<Target = str>`, `Display`, and
`PartialEq<{&str, str, String}>`. Every `&str` method is available
directly and the decoded string is computed once and cached in a
`OnceCell<String>`.

```rust
#[native(name = "ProcessName")]
fn process_name(&mut self, _amx: &Amx, name: &AmxString) -> AmxResult<bool> {
    // Direct comparison with &str — no extra allocation.
    if *name == "Admin" {
        println!("[ADMIN] welcome!");
    } else if name.starts_with("VIP_") {
        println!("[VIP] welcome, {}!", &**name);
    } else {
        println!("Hello, {}! ({} chars)", &**name, name.len());
    }
    Ok(true)
}
```

Two patterns to obtain a `&str` from a `&AmxString` parameter:

- **Concrete `&str` parameter** — let auto-deref do the work. Passing
  `name` works because `&AmxString` derefs to `&str`.
- **Generic parameter** (e.g. `T: AsRef<str>`) — Rust does not apply
  deref coercion on generic bounds. Force it with `&**name`.

> The decode is lazy: if the native never accesses the string content
> through `Deref`, no `String` is allocated. Use `.to_string()` only
> when a `String` with independent ownership is required.

### Output strings — `UnsizedBuffer::write_str`

```rust
#[native(name = "GetPlayerInfo")]
fn get_player_info(
    &mut self,
    _amx: &Amx,
    player_id: i32,
    buffer: UnsizedBuffer,
    size: usize,
) -> AmxResult<bool> {
    let info = format!("Player #{player_id}");
    buffer.write_str(size, &info)?;
    Ok(true)
}
```

```pawn
native GetPlayerInfo(playerid, buffer[], size = sizeof(buffer));
```

`UnsizedBuffer::write_str(size, s)` combines `into_sized_buffer(size)`
and the actual write in one step, propagating `Err(AmxError::General)`
when the encoded string is too long (no room for the terminator).

### Typed arrays — `get_as` / `set_as` / `iter_as`

For `Float:arr[]` and `bool:arr[]` parameters, convert through
`UnsizedBuffer::into_sized_buffer` and use the typed accessors:

```rust
use samp::prelude::*;

#[native(name = "SumFloats")]
fn sum_floats(
    &mut self,
    _amx: &Amx,
    array: UnsizedBuffer,
    len: usize,
) -> AmxResult<f32> {
    let buf = array.into_sized_buffer(len);
    Ok(buf.iter_as::<f32>().sum())
}

#[native(name = "ScaleArray")]
fn scale_array(
    &mut self,
    _amx: &Amx,
    array: UnsizedBuffer,
    len: usize,
    factor: f32,
) -> AmxResult<bool> {
    let mut buf = array.into_sized_buffer(len);
    for i in 0..buf.len() {
        if let Some(v) = buf.get_as::<f32>(i) {
            buf.set_as(i, v * factor);
        }
    }
    Ok(true)
}
```

> `get_as` / `set_as` / `iter_as` operate through the `CellConvert`
> trait. Importing it explicitly is unnecessary — `use samp::prelude::*;`
> already brings it in.

| Trait         | Use it when…                                                                |
| ------------- | --------------------------------------------------------------------------- |
| `AmxCell`     | Declaring a native argument (`AmxString`, `Ref<T>`, primitive types).       |
| `CellConvert` | Implementing typed-array support for a custom value type.                   |

## Output by reference — `Ref<T>`

```rust
#[native(name = "GetHealth")]
fn get_health(
    &mut self,
    _amx: &Amx,
    player_id: i32,
    mut health: Ref<f32>,
) -> AmxResult<bool> {
    *health = 100.0; // writes the Pawn variable directly
    Ok(true)
}
```

```pawn
native GetHealth(playerid, &Float:health);

new Float:hp;
GetHealth(0, hp);
// hp == 100.0
```

`Ref<T>` requires `T: AmxPrimitive` and implements `Deref` and
`DerefMut`. Use `address()` to obtain the AMX-relative address when
re-passing the value to other AMX calls.

## Raw mode

For full control over the argument array, opt into raw mode:

```rust
use samp::args::Args;

#[native(name = "RawNative", raw)]
fn raw_native(&mut self, amx: &Amx, args: Args) -> AmxResult<bool> {
    let count = args.count();
    let first: Option<i32> = args.get(0);
    Ok(first.is_some() && count > 0)
}
```

Raw mode is useful when:

- The argument count is variable.
- Positional access is needed.
- Automatic conversion does not fit a custom protocol.

## Return values

The return value is converted to `i32` via `AmxCell::as_cell`:

| Rust return    | Pawn observed value                            |
| -------------- | ---------------------------------------------- |
| `bool`         | `true` → `1`, `false` → `0`                    |
| `i32`          | Identity                                       |
| `f32`          | Bit-reinterpretation (`f32::to_bits`)          |
| Custom         | `<T as AmxCell>::as_cell(&value)` of choice    |

For `Result<T, E>` returns, the wrapper logs the `Err` via
`samp::log::error!` (formatted with `Display`) and returns `0`.

## Registering the native

Every native must appear in `initialize_plugin!`:

```rust
initialize_plugin!(
    type: MyPlugin,
    natives: [
        MyPlugin::function_a,
        MyPlugin::function_b,
        MyPlugin::function_c,
    ],
);
```

Order is irrelevant — the Pawn-visible name is whatever was passed to
`#[native(name = "...")]`, not the Rust method name.

## Panic safety

The generated wrapper invokes the native body inside
`std::panic::catch_unwind`. A panic that would otherwise cross the
`extern "C"` boundary (which aborts the process on Rust 1.71+) is
captured, logged with the native name plus payload, and converted to a
`0` return.
