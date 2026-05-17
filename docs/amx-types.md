# The `Amx` handle

`Amx` is the safe wrapper around the AMX VM. Each loaded Pawn script
(`.amx`) corresponds to its own `Amx` instance.

## Where it comes from

A `&Amx` reference is delivered in two places:

1. **Inside natives** — as the parameter right after `self`:
   ```rust
   #[native(name = "MyNative")]
   fn my_native(&mut self, amx: &Amx, /* ... */) -> AmxResult<bool> {
       // amx is the script that called this native
       Ok(true)
   }
   ```

2. **Inside `SampPlugin` hooks** — `on_amx_load` / `on_amx_unload`:
   ```rust
   fn on_amx_load(&mut self, amx: &Amx) {
       // amx is the script just loaded
   }
   ```

## `AmxIdent`

Each `Amx` has a stable identifier, `AmxIdent`, that is safe to keep
across callbacks (it stores the pointer address but never dereferences
it). Use it as a key in maps:

```rust
use samp::amx::AmxExt; // brings the .ident() method into scope

struct MyPlugin {
    scripts: Vec<samp::amx::AmxIdent>,
}

impl SampPlugin for MyPlugin {
    fn on_amx_load(&mut self, amx: &Amx) {
        self.scripts.push(amx.ident());
    }

    fn on_amx_unload(&mut self, amx: &Amx) {
        self.scripts.retain(|id| *id != amx.ident());
    }
}
```

To resolve an `AmxIdent` back into a live `&Amx`:

```rust
if let Some(amx) = samp::amx::get(ident) {
    // use amx
}
```

`samp::amx::get` returns `None` when the AMX has already been unloaded
by the server.

## Calling Pawn `public` functions

```rust
let result = amx.find_public("OnMyCallback")
    .and_then(|idx| amx.exec(idx));
```

### With arguments — `exec_public!`

The `exec_public!` macro handles argument pushing (in the correct
reverse order) and heap allocation for owned Rust values:

```rust
use samp::exec_public;

// No arguments
exec_public!(amx, "OnMyCallback");

// AmxCell-compatible primitives
exec_public!(amx, "OnPlayerScore", player_id, score);

// Rust strings (the macro allocates on the AMX heap automatically)
let msg = "Hello!";
exec_public!(amx, "OnMessage", msg => string);

// Rust arrays
let data = vec![1, 2, 3];
exec_public!(amx, "OnData", &data => array);
```

The heap allocations performed by the macro are tied to an
`Allocator` instance created internally and freed automatically when
the call returns.

## Registering natives manually

`initialize_plugin!` handles this for you. The low-level method is
still available for raw integrations:

```rust
use samp::raw::types::AMX_NATIVE_INFO;

amx.register(&natives)?;
```

## Method summary

| Method                                       | Purpose                                                         |
| -------------------------------------------- | --------------------------------------------------------------- |
| `find_public(name) -> AmxResult<AmxExecIdx>` | Resolve a `public` by name.                                     |
| `find_native(name) -> AmxResult<i32>`        | Resolve a native by name.                                       |
| `find_pubvar::<T>(name) -> AmxResult<Ref<T>>`| Resolve a `pubvar` and return a `Ref<T>` to its cell.           |
| `exec(idx) -> AmxResult<i32>`                | Execute a function previously pushed.                           |
| `push(value) -> AmxResult<()>`               | Push a value onto the VM stack (reverse argument order).        |
| `get_ref::<T>(address) -> AmxResult<Ref<T>>` | Build a `Ref<T>` from a raw AMX cell address.                   |
| `register(natives) -> AmxResult<()>`         | Register a native table via `amx_Register`.                     |
| `allocator() -> Allocator<'_>`               | RAII heap allocator (`allot`, `allot_buffer`, `allot_array`, `allot_string`). |
| `strlen(ptr) -> AmxResult<usize>`            | Length of an AMX string at the given physical pointer.          |
| `flags() -> AmxResult<AmxFlags>`             | Flags of the loaded `.amx` (debug, JIT, etc.).                  |
| `amx() -> Option<NonNull<AMX>>`              | Raw `*mut AMX` (non-null).                                      |
| `header() -> Option<NonNull<AMX_HEADER>>`    | Pointer to the `AMX_HEADER` of the loaded `.amx`.               |

## The allocator

`Allocator` captures the AMX heap pointer at construction time and
restores it on `Drop`, freeing every allocation in one shot. Nested
allocators are not safe — each one restores to its own snapshot.

```rust
let allocator = amx.allocator();

let cell    = allocator.allot(42_i32)?;                  // single cell
let buffer  = allocator.allot_buffer(256)?;              // empty buffer
let array   = allocator.allot_array(&[1_i32, 2, 3])?;    // initialized array
let string  = allocator.allot_string("Hello, AMX")?;     // string + terminator
```

Memory is reclaimed automatically when `allocator` goes out of scope.
