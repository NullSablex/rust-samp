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

## Generating the Pawn include

`#[native]` derives each native's Pawn declaration from its Rust signature, so
the `.inc` the script side includes cannot drift from the code.

Start the server once with `SAMP_PAWN_INCLUDE` set to the path you want:

```sh
SAMP_PAWN_INCLUDE=counter.inc ./samp03svr
```

For the `counter` example that writes:

```pawn
// Generated by rust-samp from the #[native] signatures of counter.
// Edits are lost on the next build — change the Rust side instead.

#if defined _counter_included
    #endinput
#endif
#define _counter_included

native Counter_Increment();
native bool:Counter_Get(&out);
native bool:Counter_SetMax(max);
```

It works on either server and in either mode, since the declarations are stored
by both entry points. The same content is available programmatically through
`samp::plugin::pawn_include()` (a `String`) and
`samp::plugin::write_pawn_include(path)`.

### How types map

| Rust | Pawn |
| ---- | ---- |
| `i32`, `u32`, `usize`, … | `arg` |
| `f32` | `Float:arg` |
| `bool` | `bool:arg` |
| `&AmxString` / `AmxString` | `const arg[]` |
| `Ref<i32>` | `&arg` |
| `Ref<f32>` | `&Float:arg` |
| `Buffer` / `UnsizedBuffer` | `arg[]` |
| return `bool` / `f32` | `bool:` / `Float:` prefix on the native |

The mapping is syntactic: the macro reads the type as written, not as resolved.
A type it does not recognize becomes a plain cell argument — which is what the
AMX passes anyway, so only the Pawn tag is lost. A `raw` native comes back
commented out, because its arity is not in the signature:

```pawn
// native MyRawNative(...); // raw native — fill in the arguments
```

Argument names come from the Rust parameters, with a leading `_` stripped.

### Saying what the signature cannot

Pawn declarations carry things no Rust signature has a place for: a default
value, a length argument that measures an array, varargs. `#[native]` takes them,
so the declaration stays derived instead of hand-written:

```rust
#[native(name = "Email_Status", default(account = 0), sizeof(dest_len = dest))]
fn status(&mut self, _amx: &Amx, account: i32, dest: UnsizedBuffer, dest_len: usize)
    -> AmxResult<bool> { /* ... */ }

#[native(name = "Email_Send", raw, args = "account = 0, const to[], {Float,_}:...")]
fn send(&mut self, _amx: &Amx, args: Args) -> bool { /* ... */ }
```

```pawn
native bool:Email_Status(account = 0, dest[], dest_len = sizeof(dest));
native bool:Email_Send(account = 0, const to[], {Float,_}:...);
```

| In `#[native(...)]` | Effect |
| ------------------- | ------ |
| `default(arg = 0)` | `arg = 0`. A string literal is taken verbatim, so `default(name = "\"\"")` gives `name = ""` |
| `sizeof(len = dest)` | `len = sizeof(dest)`, Pawn's way of passing a buffer's size |
| `varargs` | closes the list with `{Float,_}:...` |
| `args = "..."` | the argument list, written out — the only way to declare a `raw` native |

`default` and `sizeof` naming an argument the function does not have is a compile
error: it means a rename on the Rust side that the attribute did not follow, and
the include would come out wrong.

## Generating from a template

A fully generated include has no room for documentation; one kept by hand drifts.
A template is both: the prose, the sections, the constants and the callback
documentation stay written by hand, and every `native` line is filled in from the
Rust signature.

```pawn
// counter v{{VERSION}} — generated from counter.inc.in

#if defined {{GUARD}}
    #endinput
#endif
#define {{GUARD}}

// Called once a Counter_WorkAsync job has finished.
forward OnCounterWorkDone(delay);

// Adds one. Returns the new value, or -1 when already at the maximum.
{{NATIVE:Counter_Increment}}

// The open.mp-styled spelling of the same native.
{{NATIVE:Counter_Get as Counter_Read}}

{{NATIVES}}
```

| Placeholder | Becomes |
| ----------- | ------- |
| `{{NATIVES}}` | every declaration not placed individually, in registration order |
| `{{NATIVE:Name}}` | that one declaration |
| `{{NATIVE:Name as Alias}}` | `native Alias(…) = Name;` — the same native under another name |
| `{{PLUGIN}}` | the plugin's crate name |
| `{{GUARD}}` | `_<plugin>_included` |
| `{{VERSION}}` | the plugin crate's version, unless you pass your own |
| anything else | what you supplied with `.var("NAME", value)` |

A registered native that no placeholder emits is an **error**, not a silent
omission — that is the drift this exists to prevent. So are an unknown
placeholder, a `{{NATIVE:...}}` naming a native that does not exist, and a `{{`
that never closes. All of them come back at once, so one run names everything.

### From a test

```rust
#[test]
fn the_shipped_include_is_the_rendered_template() {
    let rendered = samp::pawn_include::Template::new(
        include_str!("../counter.inc.in"),
        &pawn_native_decls(),
    )
    .render()
    .unwrap();

    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/counter.inc");
    if std::env::var_os("UPDATE_INCLUDE").is_some() {
        std::fs::write(path, &rendered).unwrap();
        return;
    }
    assert_eq!(std::fs::read_to_string(path).unwrap(), rendered);
}
```

The include is committed, CI checks it is current, and `UPDATE_INCLUDE=1 cargo
test` regenerates it. The `counter` example works exactly this way.

### Without writing any code

Start the server once with the template and the output named:

```sh
SAMP_PAWN_INCLUDE_TEMPLATE=include/my_plugin.inc.in \
SAMP_PAWN_INCLUDE=include/my_plugin.inc \
SAMP_PAWN_VAR_RELEASED=2026-09-26 \
./omp-server
```

`SAMP_PAWN_VAR_<NAME>` supplies `{{<NAME>}}`. Because the environment belongs to
the whole process, and a server may have several Rust plugins loaded, a value may
name the plugin it is for — otherwise they overwrite each other's file:

```sh
SAMP_PAWN_INCLUDE="counter=counter.inc,email_samp=email.inc" ./omp-server
```

A bare path applies to whichever plugin reads it, which is what a single-plugin
setup wants. This selector works for `SAMP_PAWN_INCLUDE`,
`SAMP_PAWN_INCLUDE_TEMPLATE` and `SAMP_PAWN_INCLUDE_CHECK`.

## Checking a hand-written include

A generated include cannot drift. One kept by hand can, and plugins keep theirs
by hand for reasons the Rust signature cannot express: default values,
`dest_len = sizeof(dest)`, varargs, documentation the script author reads. The
cost is that a native renamed in Rust and forgotten in the `.inc` fails only
when a script calls it.

Point `SAMP_PAWN_INCLUDE_CHECK` at the include and the SDK compares it with the
natives actually registered, at load:

```sh
SAMP_PAWN_INCLUDE_CHECK=include/my_plugin.inc ./omp-server
```

```
[my_plugin] include/my_plugin.inc disagrees with the registered natives:
[my_plugin]   MyPlugin_Reset is registered but missing from the include; a script calling it will not compile
[my_plugin]   MyPlugin_Legacy is declared in the include but not registered; a script calling it fails at runtime
[my_plugin]   MyPlugin_Status takes 3 argument(s), the include declares 2
[my_plugin]   MyPlugin_Get returns bool:, the include declares untagged
[my_plugin]   MyPlugin_Read argument 2 is &Float:arg, the include declares arg
```

Compared: the name, the return tag, the argument count, and each argument's
tag, `&` and `[]`. Not compared, because only the include can state them:
default values, size expressions, documentation, and varargs — a declaration
ending in `...` is open-ended on purpose, so it may name fewer arguments than
the native takes.

Three things a real include does are understood rather than flagged:

- **Aliases.** `native Email_Close(account = 0) = email_close;` presents a name
  the script calls, implemented by the registered native after the `=`. The
  comparison follows the `=`, so an include that renames the whole surface —
  an open.mp-styled variant next to the original, for instance — is not drift.
  Divergences are reported under the name the include declares.
- **`raw` natives.** They parse their own arguments, so `#[native]` cannot
  derive a shape and the include is the only place it is written down. Presence
  is compared, shape is not.
- **Templates.** A template is checked as what it renders to, so pointing the
  check at the `.inc.in` works as well as pointing it at the output — and is
  usually what you want, the template being the file edited by hand. A template
  that will not render is reported here too.

### In `cargo test`, without a server

`initialize_plugin!` also emits `pawn_native_decls()`, which needs no server, so
the whole check runs as an ordinary test — which is where a CI job wants it:

```rust
#[test]
fn the_include_matches_the_natives() {
    let findings = samp::pawn_include::compare_file_with(
        concat!(env!("CARGO_MANIFEST_DIR"), "/include/my_plugin.inc"),
        &pawn_native_decls(),
    )
    .unwrap();

    assert!(findings.is_empty(), "{findings:#?}");
}
```

The `counter` example ships one, next to the `counter.inc` it guards.

A plugin whose include is generated can write it from the same place, instead of
starting a server with `SAMP_PAWN_INCLUDE` set:

```rust
std::fs::write("include/my_plugin.inc",
    samp::pawn_include::render("my_plugin", &pawn_native_decls()))?;
```

Inside a running plugin the same comparison is one call:

```rust
for finding in samp::pawn_include::compare_file("include/my_plugin.inc")? {
    log::warn!("{finding}");
}
```

`samp::pawn_include::parse` reads declarations out of any Pawn source — it skips
commented-out ones, and handles declarations spread over several lines — and
`compare_declarations` compares two parsed lists without a server behind either.

## Panic safety

The generated wrapper invokes the native body inside
`std::panic::catch_unwind`. A panic that would otherwise cross the
`extern "C"` boundary (which aborts the process on Rust 1.71+) is
captured, logged with the native name plus payload, and converted to a
`0` return.
