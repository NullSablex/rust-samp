# Error handling

rust-samp follows Rust's `Result` convention for error reporting, with
a single error type for every AMX VM failure.

## `AmxResult<T>`

```rust
pub type AmxResult<T> = Result<T, AmxError>;
```

Most SDK methods return `AmxResult<T>`. A native can return either
`AmxResult<T>` (or any `Result<T, E: Display>`) — in which case the
generated wrapper logs the `Err` and returns `0` — or a bare `T` that
implements `AmxCell`.

```rust
#[native(name = "Divide")]
fn divide(&mut self, _amx: &Amx, a: i32, b: i32) -> AmxResult<i32> {
    if b == 0 {
        return Err(AmxError::Divide); // division by zero
    }
    Ok(a / b)
}
```

## `AmxError`

`AmxError` mirrors the codes defined by the AMX C header. Built via
`AmxError::from(code)` from any `i32`; unknown values become
`AmxError::Unknown` instead of panicking.

| Variant              | Code | Display                                                  |
| -------------------- | :--: | -------------------------------------------------------- |
| `Exit`               | 1    | `Forced exit`                                            |
| `Assert`             | 2    | `Assertion failed`                                       |
| `StackError`         | 3    | `Stack / heap collision`                                 |
| `Bounds`             | 4    | `Index out of bounds`                                    |
| `MemoryAccess`       | 5    | `Invalid memory access`                                  |
| `InvalidInstruction` | 6    | `Invalid instruction`                                    |
| `StackLow`           | 7    | `Stack underflow`                                        |
| `HeapLow`            | 8    | `Heap underflow`                                         |
| `Callback`           | 9    | `No callback or invalid callback`                        |
| `Native`             | 10   | `Native function failed`                                 |
| `Divide`             | 11   | `Divide by zero`                                         |
| `Sleep`              | 12   | `Go into sleepmode`                                      |
| `InvalidState`       | 13   | `No implementation for this state, no fall-back`         |
| `Memory`             | 16   | `Out of memory`                                          |
| `Format`             | 17   | `Invalid file format`                                    |
| `Version`            | 18   | `File is for a newer version of AMX`                     |
| `NotFound`           | 19   | `Function not found`                                     |
| `Index`              | 20   | `Invalid index parameter (bad entry point)`              |
| `Debug`              | 21   | `Debugger cannot run`                                    |
| `Init`               | 22   | `AMX not initialize`                                     |
| `UserData`           | 23   | `Unable to set user data field`                          |
| `InitJit`            | 24   | `Cannot initialize the JIT`                              |
| `Params`             | 25   | `Parameter error`                                        |
| `Domain`             | 26   | `Domain error, expression result does not fit in range`  |
| `General`            | 27   | `General error (unknown or unspecific error)`            |
| `Overlay`            | 28   | `Overlays are unsupported (JIT) or uninitialized`        |
| `Unknown`            | —    | `Unknown error`                                          |

`AmxError` implements `std::error::Error`, so it composes with `?`,
`anyhow`, `eyre`, and friends.

## Propagation with `?`

```rust
#[native(name = "CallCallback")]
fn call_callback(&mut self, amx: &Amx) -> AmxResult<bool> {
    let index  = amx.find_public("OnMyCallback")?;
    let result = amx.exec(index)?;
    Ok(result > 0)
}
```

## Converting from `i32`

```rust
let err  = AmxError::from(19);   // AmxError::NotFound
let code = AmxError::NotFound as i32; // 19
```

## Display

Every variant has a stable, English message that maps 1:1 to the
original C header strings:

```rust
let err = AmxError::NotFound;
println!("{err}"); // "Function not found"
```

## Common patterns

### Reporting status to Pawn through the return value

Sometimes it is more convenient to return a status code from the native
than to log an error:

```rust
#[native(name = "TryConnect")]
fn try_connect(&mut self, _amx: &Amx, address: &AmxString) -> i32 {
    match self.connect(&**address) {
        Ok(_)  => 1,
        Err(_) => -1,
    }
}
```

### Custom return type implementing `AmxCell`

When several outcomes share the same native, a small enum + `AmxCell`
keeps the call sites readable:

```rust
#[derive(Clone, Copy)]
enum Status {
    Ok,
    NotFound,
    Error,
}

impl AmxCell<'_> for Status {
    fn as_cell(&self) -> i32 {
        match self {
            Status::Ok       => 1,
            Status::NotFound => 0,
            Status::Error    => -1,
        }
    }
}

#[native(name = "DoSomething")]
fn do_something(&mut self, _amx: &Amx) -> Status {
    Status::Ok
}
```
