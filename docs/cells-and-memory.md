# Cells and memory

The AMX VM operates on **32-bit cells**. Every value inside the VM —
integers, floats, booleans, addresses — is a single `i32`. rust-samp
wraps those cells with typed abstractions.

## `AmxCell`

`AmxCell` is the trait used by `#[native]` to parse arguments and to
encode return values.

```rust
pub trait AmxCell<'amx>: Sized {
    fn from_raw(amx: &'amx Amx, cell: i32) -> AmxResult<Self> {
        Err(AmxError::General)
    }
    fn as_cell(&self) -> i32;
}
```

`from_raw` has a default implementation that returns
`Err(AmxError::General)`; primitives and complex types provide their
own concrete impls.

### Primitive implementations

| Type                                                              | `as_cell()`                | `from_raw()`                       |
| ----------------------------------------------------------------- | -------------------------- | ---------------------------------- |
| `i32`                                                             | Identity                   | Identity                           |
| `i8` / `u8` / `i16` / `u16` / `u32` / `isize` / `usize`           | Cast to `i32`              | Cast from `i32`                    |
| `f32`                                                             | `f32::to_bits().cast_signed()` | `f32::from_bits(cell as u32)`  |
| `bool`                                                            | `true` → `1`, `false` → `0` | `cell != 0`                       |
| `&T` / `&mut T` (where `T: AmxCell`)                              | Forwarded to `T::as_cell()`| —                                  |

### Custom implementations

Any type can implement `AmxCell` to be returned from a native:

```rust
#[derive(Debug, Clone, Copy)]
enum Outcome {
    Success(i32),
    Error,
}

impl AmxCell<'_> for Outcome {
    fn as_cell(&self) -> i32 {
        match self {
            Outcome::Success(v) => *v,
            Outcome::Error => -1,
        }
    }
}
```

The `from_raw` default is fine for return-only types — they will never
be parsed from a cell.

## `AmxPrimitive`

`AmxPrimitive` is an `unsafe` marker trait for types that fit in a
single 32-bit cell. It is used as a bound on generics that manipulate
values inside the AMX stack/heap (e.g. `Ref<T>`, `Buffer::get_as::<T>`).

Implemented for: `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `isize`,
`usize`, `f32`, `bool`.

> The trait is `unsafe` because the SDK assumes a 32-bit footprint —
> implementing it for a larger type corrupts VM memory.

## `Ref<T>`

`Ref<T>` is a typed pointer to a live cell in the AMX heap/data
section. It implements `Deref` and `DerefMut`, so the value is read or
written through `*r`.

```rust
#[native(name = "GetStats")]
fn get_stats(
    &mut self,
    _amx: &Amx,
    mut health: Ref<f32>,
    mut armor: Ref<f32>,
) -> AmxResult<bool> {
    *health = 100.0;
    *armor = 50.0;
    Ok(true)
}
```

| Method            | Description                                                       |
| ----------------- | ----------------------------------------------------------------- |
| `address()`       | AMX-relative address of the cell (useful when re-passing to AMX). |
| `as_ptr()`        | Read-only physical pointer to the cell.                           |
| `as_mut_ptr()`    | Mutable physical pointer to the cell.                             |

## `AmxString`

`AmxString` represents a Pawn string and supports both packed (4 bytes
per cell) and unpacked (1 byte per cell) layouts.

```rust
#[native(name = "PrintString")]
fn print_string(&mut self, _amx: &Amx, text: AmxString) -> AmxResult<bool> {
    let owned = text.to_string();   // decoded String (honors the configured encoding)
    let raw   = text.to_bytes();    // raw bytes
    let len   = text.len();         // length in characters
    let empty = text.is_empty();    // emptiness check

    println!("{owned}");
    Ok(true)
}
```

`AmxString::Deref<Target = str>` decodes the underlying cells on first
access and caches the result in a `OnceCell<String>`. The encoding
respects the value set via [`samp::encoding::set_default_encoding`](encoding.md)
when the `encoding` feature is on, and falls back to UTF-8 lossy
otherwise.

## `CellConvert` and typed arrays

`CellConvert` is the per-cell conversion trait used by `Buffer`:

```rust
pub trait CellConvert: Sized {
    fn from_cell(raw: i32) -> Self;
    fn into_cell(self) -> i32;
}
```

It is implemented for the same primitive set as `AmxPrimitive`. Unlike
`AmxCell`, it does not need an `&Amx` because each cell stands on its
own.

| Trait         | Where it is used                                                | Needs `&Amx`?           |
| ------------- | --------------------------------------------------------------- | ----------------------- |
| `AmxCell`     | Argument and return types of `#[native]`                        | Yes (for complex types) |
| `CellConvert` | Elements of a `Buffer` (`get_as`, `set_as`, `iter_as`)          | No                      |

### `Buffer` accessors backed by `CellConvert`

```rust
// Read a float from a Pawn array
if let Some(value) = buffer.get_as::<f32>(0) {
    println!("{value}");
}

// Write a bool back
buffer.set_as::<bool>(1, true);

// Iterate every cell as f32
for value in buffer.iter_as::<f32>() {
    println!("{value}");
}
```

The default implementations cover the common Pawn array shapes
(`Float:arr[]`, `bool:arr[]`) without any manual bit manipulation.

## `Buffer` and `UnsizedBuffer`

### `UnsizedBuffer`

`UnsizedBuffer` is the type produced when a native argument is an
array with unknown length (e.g. `array[]`). Convert it into a `Buffer`
once the size argument is known:

```rust
#[native(name = "FillBuffer")]
fn fill_buffer(
    &mut self,
    _amx: &Amx,
    buffer: UnsizedBuffer,
    size: usize,
) -> AmxResult<bool> {
    let mut buffer = buffer.into_sized_buffer(size);
    // buffer is now a Buffer with the declared length
    Ok(true)
}
```

`into_sized_buffer` clamps `size` at 1 MiB cells as a defense against a
script passing a corrupted length. In debug builds an oversized value
triggers a `debug_assert!`.

### `Buffer`

`Buffer` is a sized cell vector. It implements `Deref<Target = [i32]>`
and `DerefMut`, so the entire `&[i32]` / `&mut [i32]` API is available
directly.

```rust
let slice = buffer.as_slice();       // &[i32]
let mut_slice = buffer.as_mut_slice(); // &mut [i32]
```

### Writing strings into buffers

`write_str` copies a Rust string into the buffer (one byte per cell)
and appends the terminator:

```rust
// From an UnsizedBuffer — sizes the buffer and writes in one step
buffer.write_str(size, "text for Pawn")?;

// From a Buffer that already has a known size
let mut buffer = unsized_buffer.into_sized_buffer(size);
buffer.write_str("text for Pawn")?;
```

A `?` propagates `AmxError::General` when the encoded string is too
long to fit alongside the terminator.

## `Allocator`

`Allocator` is the RAII heap allocator obtained via `amx.allocator()`.
It captures the heap pointer at construction time and restores it on
`Drop`, freeing every allocation in a single step.

```rust
let allocator = amx.allocator();

let cell    = allocator.allot(42_i32)?;                 // one cell
let array   = allocator.allot_array(&[1_i32, 2, 3])?;   // array
let string  = allocator.allot_string("hello")?;         // string + terminator
let buffer  = allocator.allot_buffer(256)?;             // empty buffer
```

Do not nest two allocators on the same `Amx` — each one snapshots the
heap pointer and would restore to its own value.
