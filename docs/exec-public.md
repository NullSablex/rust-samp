# Calling Pawn from Rust — `exec_public!`

Pawn `public` functions can be called from the plugin. The
`exec_public!` macro (defined in `samp-sdk/src/macros.rs`, re-exported
as `samp::exec_public`) handles the boilerplate: it pushes arguments
in the correct order, allocates AMX
heap memory for owned Rust values, executes the function, and frees
everything when the call returns.

## No arguments

```rust
use samp::exec_public;

exec_public!(amx, "OnMyCallback");
```

The macro expands to the equivalent of
`amx.find_public("OnMyCallback").and_then(|idx| amx.exec(idx))` and
returns `AmxResult<i32>` — the Pawn return value (or the propagated
`AmxError`).

## With `AmxCell` arguments

Anything that implements `AmxCell` is pushed directly. The macro
pushes arguments in **reverse order** automatically — write them in the
same order as the Pawn signature:

```rust
exec_public!(amx, "OnPlayerScore", player_id, score);
```

Pawn side:

```pawn
forward OnPlayerScore(playerid, score);
public  OnPlayerScore(playerid, score) { /* ... */ }
```

The same form works for `Ref<T>`, `Buffer`, `UnsizedBuffer`, and any
custom type implementing `AmxCell`.

## With Rust strings — `expr => string`

A Rust `&str` (or anything that derefs to `&str`) is copied into the
AMX heap before the call. Use the `=> string` modifier:

```rust
let message = "Hello, Pawn!";
exec_public!(amx, "OnMessage", message => string);
```

The temporary heap allocation is tied to an `Allocator` created
internally and is reclaimed when the call returns.

## With Rust slices — `expr => array`

`&[T]` where `T: AmxCell + AmxPrimitive` is copied into a contiguous
AMX buffer:

```rust
let data = vec![1_i32, 2, 3, 4];
exec_public!(amx, "OnData", &data => array);
```

## Mixing argument forms

The three forms (`expr`, `expr => string`, `expr => array`) can appear
in any combination:

```rust
let public_name = pub_name.to_string();
let owned_msg   = String::from("another hello");
let table       = vec![10_i32, 20, 30];

exec_public!(
    amx,
    &public_name,
    string,                       // an existing AmxString argument
    "literal" => string,          // Rust &str → AMX string
    &owned_msg => string,         // Rust &String → AMX string
    &table     => array,          // Rust slice → AMX array
    reference,                    // Ref<T> argument
);
```

The order is the same Pawn sees on the stack — first positional
argument first.

## Manual equivalent

For full control, drop the macro:

```rust
let allocator = amx.allocator();
let idx       = amx.find_public("OnMessage")?;
let msg       = allocator.allot_string("Hello, Pawn!")?;
amx.push(msg)?;            // pushed first → last argument in Pawn
amx.push(123_i32)?;        // pushed second → first argument in Pawn
let result    = amx.exec(idx)?;
```

`allocator` releases every AMX heap allocation when it goes out of
scope — there is no need to free memory explicitly.

## Return value

`exec_public!` returns `AmxResult<i32>`:

- `Ok(value)` — the Pawn `return` value (cell-encoded; cast with
  `f32::from_bits` if the Pawn function returns `Float:`).
- `Err(AmxError::NotFound)` — `find_public` did not locate the
  function.
- Other `AmxError` variants are forwarded from `amx_Exec` directly
  (stack overflow, divide by zero, native failure, …).

See [Error handling](error-handling.md) for the full list.
