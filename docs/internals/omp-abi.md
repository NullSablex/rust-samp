# Internals — Open Multiplayer ABI

This page documents the C++ ABI details rust-samp implements in pure
Rust to interoperate with the Open Multiplayer server. It is meant
for contributors to the SDK and for plugin authors who need to call
into other server components beyond the high-level wrappers shipped
with the SDK.

The information here was derived from the public specification of
the Open Multiplayer SDK
(<https://github.com/openmultiplayer/open.mp-sdk>) and validated
through runtime + disassembly of `omp-server.exe` / `Console.dll` /
`Console.so`. No SDK code was copied — only the layouts and slot
indices.

## Two ABIs

Open Multiplayer 32-bit binaries exist on two ABIs:

| Target                    | ABI       | Calling convention | Notes                                              |
| ------------------------- | --------- | ------------------ | -------------------------------------------------- |
| `i686-unknown-linux-gnu`  | Itanium   | `extern "C"`       | Default GCC behavior on i686 Linux.                |
| `i686-pc-windows-msvc`    | MSVC      | `extern "thiscall"` | `this` in `ECX`; cdecl for variadic virtuals.     |
| `i686-pc-windows-gnu`     | —         | unsupported        | The Windows server uses MSVC; do not target this. |

Every vtable struct in `samp_sdk::omp::*` is gated with
`#[cfg(target_env = "msvc")]` / `#[cfg(not(target_env = "msvc"))]` so
each build sees the layout that matches its ABI.

## `OmpComponent` memory layout

`OmpComponent` is the Rust type laid out to be cast directly into the
server's `IComponent*`. The header inheritance is
`IComponent : public IExtensible, public IUIDProvider`, which on a
typical Itanium/MSVC i686 layout means two vtable pointers and one
secondary subobject.

### Itanium ABI (Linux GCC, i686)

```text
offset  0   vtable*        (primary: IExtensible + IComponent)
offset  4   _misc_ext[36]  (robin_hood::unordered_flat_map, zero-init = empty)
offset 40   uid_vtable*    (secondary: IUIDProvider subobject)
offset 44   uid            (UID = u64; GCC i686 aligns uint64_t to 4 bytes)
offset 52   plugin_ptr     (*mut () — plugin's own field)
total       56 bytes
```

### MSVC ABI (Windows i686)

```text
offset  0   vtable*        (primary: IExtensible + IComponent)
offset  4   _misc_ext[52]  (padding + robin_hood + trailing pad, zero-init)
offset 56   uid_vtable*    (secondary: IUIDProvider subobject; offset hardcoded by the server)
offset 60   _uid_pad[4]    (alignment padding for uid)
offset 64   uid            (UID = u64; MSVC aligns uint64_t to 8 bytes)
offset 72   plugin_ptr
```

Both layouts are validated at compile time via `const _: () = { ... }`
asserting `std::mem::offset_of!(OmpComponent, uid_vtable)` and
`std::mem::size_of::<OmpComponent>()`.

## Primary vtable — `IExtensible` + `IComponent`

### Itanium ABI — 17 slots

| Slot | Method                                              | Notes                                                                                |
| :--: | --------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 0    | `getExtension(UID)`                                 |                                                                                      |
| 1    | `addExtension(IExtension*, bool)`                   |                                                                                      |
| 2    | `removeExtension(IExtension*)`                      |                                                                                      |
| 3    | `removeExtension(UID)`                              |                                                                                      |
| 4    | `~destructor` (D1 — complete object)                | Itanium emits two destructor slots per class.                                        |
| 5    | `~destructor` (D0 — deleting)                       |                                                                                      |
| 6    | `supportedVersion() -> i32`                         |                                                                                      |
| 7    | `componentName() -> StringView`                     | Returned by value (8 bytes on i686 Linux ABI).                                       |
| 8    | `componentType() -> ComponentType`                  |                                                                                      |
| 9    | `componentVersion() -> SemanticVersion`             | Returned by value (6 bytes).                                                         |
| 10   | `onLoad(ICore*)`                                    |                                                                                      |
| 11   | `onInit(IComponentList*)`                           |                                                                                      |
| 12   | `onReady()`                                         |                                                                                      |
| 13   | `onFree(IComponent*)`                               |                                                                                      |
| 14   | `provideConfiguration(ILogger*, IEarlyConfig*, bool)`|                                                                                     |
| 15   | `free()`                                            |                                                                                      |
| 16   | `reset()`                                           |                                                                                      |

### MSVC ABI — 16 slots

| Slot | Method                                              | Notes                                                                                |
| :--: | --------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 0    | `getExtension(UID)`                                 | `thiscall`                                                                           |
| 1    | `addExtension(IExtension*, bool)`                   | `thiscall`                                                                           |
| 2    | `removeExtension(IExtension*)`                      | `thiscall`                                                                           |
| 3    | `removeExtension(UID)`                              | `thiscall`                                                                           |
| 4    | `~destructor` (scalar deleting)                     | MSVC i686 with single inheritance emits a single destructor slot.                    |
| 5    | `supportedVersion() -> i32`                         | `thiscall`, no stack args (signature `fn()` so Rust emits `ret`, not `ret 4`).       |
| 6    | `componentName() -> StringView`                     | Returned **via hidden pointer** at `[ESP+4]` (naked asm; `ret 4`).                   |
| 7    | `componentType() -> ComponentType`                  | `thiscall`, no stack args.                                                            |
| 8    | `componentVersion() -> SemanticVersion`             | Returned **via hidden pointer** at `[ESP+4]` (naked asm; `ret 4`).                   |
| 9    | `onLoad(ICore*)`                                    |                                                                                      |
| 10   | `onInit(IComponentList*)`                           |                                                                                      |
| 11   | `onReady()`                                         | `thiscall`, no stack args.                                                            |
| 12   | `onFree(IComponent*)`                               |                                                                                      |
| 13   | `provideConfiguration(ILogger*, IEarlyConfig*, bool)`|                                                                                     |
| 14   | `free()`                                            | `thiscall`, no stack args.                                                            |
| 15   | `reset()`                                           | `thiscall`, no stack args.                                                            |

> **Why `fn()` instead of `fn(*mut Self)` for no-arg MSVC methods?** On
> `thiscall`, `this` arrives in `ECX`. Declaring an explicit `_this`
> argument would make Rust emit `ret 4` (cleaning up a stack slot
> that does not exist), corrupting the stack on return. Using `fn()`
> emits the correct `ret` and keeps `ECX` semantics intact.

## Secondary vtable — `IUIDProvider`

### Itanium ABI — 3 slots

| Slot | Method                          | Notes                                                              |
| :--: | ------------------------------- | ------------------------------------------------------------------ |
| 0    | `~destructor` D1 thunk          | Inherited destructor thunks. The SDK supplies no-op implementations.|
| 1    | `~destructor` D0 thunk          |                                                                    |
| 2    | `getUID() -> UID`               | `this` points to the `IUIDProvider` subobject at offset 40.        |

### MSVC ABI — 1 slot

| Slot | Method                          | Notes                                                              |
| :--: | ------------------------------- | ------------------------------------------------------------------ |
| 0    | `getUID() -> UID`               | `IUIDProvider` declares no virtual destructor under MSVC. `this` points to the subobject at offset 56. Disasm of `omp-server.exe` confirms `add ecx, 0x38; mov eax, [esi+0x38]; call [eax]`. |

`uid_get_uid` recovers the original `OmpComponent*` by subtracting
`offsetof(OmpComponent, uid_vtable)` from the received `this`.

## `IPawnComponent` vtable

`IPawnComponent : public IComponent` adds Pawn-specific virtuals
after the inherited slots. Runtime dumps on Open Multiplayer 1.5.8
confirmed the indices:

| ABI       | Slot for `getEventDispatcher` | Slot for `getAmxFunctions` |
| --------- | :----------------------------: | :------------------------: |
| Itanium   | 18                             | 19                         |
| MSVC      | 16                             | 17                         |

`AmxFunctionTable` is a `StaticArray<void*, 52>` (52 slots) — this is
the `NUM_AMX_FUNCS` constant exposed by the SDK as
`samp_sdk::omp::server::NUM_AMX_FUNCS`.

> `getAmxFunctions()` returns `0` during `on_init` on the current
> Open Multiplayer (1.5.x). The SDK calls it adaptively: it tries
> once in `on_init`, stores the pointer if non-zero, and otherwise
> retries in `on_ready`. This way future versions that populate the
> table earlier work without any change.

## `IPawnScript` vtable

No virtual destructor → identical slot layout on both ABIs. Only one
slot is used:

| Slot | Method                | Notes                                       |
| :--: | --------------------- | ------------------------------------------- |
| 57   | `GetAMX() -> *mut AMX`| Calling convention varies per ABI.          |

## `IEventDispatcher<PawnEventHandler>` vtable

No virtual destructor → identical slot layout on both ABIs.

| Slot | Method                                              | Notes                                       |
| :--: | --------------------------------------------------- | ------------------------------------------- |
| 0    | `addEventHandler(handler*, priority: i8) -> bool`   | Used by `add_pawn_event_handler`.           |
| 1    | `removeEventHandler(handler*) -> bool`              | Used by `remove_pawn_event_handler`.        |
| 2    | `hasEventHandler`                                   | Unused by the SDK.                          |
| 3    | `count`                                             | Unused by the SDK.                          |

The dispatcher fires our own `PawnEventHandler` (a Rust object we
own) whose vtable has 2 slots:

| Slot | Method                                  |
| :--: | --------------------------------------- |
| 0    | `onAmxLoad(IPawnScript*)`               |
| 1    | `onAmxUnload(IPawnScript*)`             |

`PawnEventHandler` has no virtual destructor in the Open Multiplayer
header — the only difference between Itanium and MSVC is the
calling convention.

## `ITimersComponent` and `ITimer`

`ITimersComponent` inherits from `IComponent`. The first new slot
after the 16 inherited ones (MSVC) or 17 (Itanium) is
`create(handler, ms, repeating)`.

The SDK uses slot **16** through the shared helper
`vtable::secondary_call_target(component_ptr, 0, 16)`, which works
on both ABIs because the helper reads from the primary vtable
pointer regardless of layout (only the *slot index* must match, and
in this case 16 happens to align on both ABIs).

`ITimer::kill()` lives at slot **10** and is called the same way.

## `ICore::ILogger` subobject

`ICore : public IExtensible, public ILogger`. The `ILogger` subobject
sits after the `IExtensible` block, at different offsets per ABI:

| ABI       | `ILogger` offset inside `ICore`                                |
| --------- | -------------------------------------------------------------- |
| MSVC      | 56 bytes (confirmed in `Console.dll`: `lea edx, [core+0x38]; mov ecx, [edx]; call [ecx+8]`). |
| Itanium   | 40 bytes (confirmed in `Console.so`: `add edi, 0x28; mov ebx, [edi]; call [ebx+8]`).         |

The `ILogger` vtable (8 slots, identical order on both ABIs):

| Slot | Method                                              |
| :--: | --------------------------------------------------- |
| 0    | `printLn(fmt, ...)`                                 |
| 1    | `vprintLn(fmt, va_list)`                            |
| 2    | `logLn(level, fmt, ...)`                            |
| 3    | `vlogLn(level, fmt, va_list)`                       |
| 4    | `printLnU8(fmt, ...)`                               |
| 5    | `vprintLnU8(fmt, va_list)`                          |
| 6    | `logLnU8(level, fmt, ...)`                          |
| 7    | `vlogLnU8(level, fmt, va_list)`                     |

Variadic virtual methods on x86 use **`__cdecl`** on both MSVC and
Itanium — `thiscall` does not support varargs. Stable Rust does not
expose `extern "C"` variadic (`c_variadic` is nightly), so the SDK
declares each entry point with fixed arity (`fn(this, fmt, arg)`)
and always passes `fmt = "%s"`. The caller formats the message in
Rust and passes the resulting `CString` as the single variadic
argument — ABI-equivalent to the variadic call.

## Helpers in `samp_sdk::omp::vtable`

Three small `unsafe fn`s centralize the repeated pattern of
"adjust pointer to a subobject, read its vtable, load slot N":

| Function                              | Returns                            | Used for                                           |
| ------------------------------------- | ---------------------------------- | -------------------------------------------------- |
| `subobject_ptr(obj, offset)`          | `Option<*mut u8>`                  | Pointer adjustment for secondary bases.            |
| `vtable_slot(subobject, slot)`        | `Option<usize>`                    | Read a slot pointer from a vtable.                 |
| `secondary_call_target(obj, off, n)`  | `Option<(*mut u8, usize)>`         | Combination — `(this_to_pass, fn_ptr)` in one go.  |

All three return `None` on null pointers, null vtables, or null
slot entries — they are the defensive layer that prevents an
incorrect ABI assumption from segfaulting.

## Adding a new component wrapper

To wrap another Open Multiplayer component:

1. Declare a `#[repr(C)] pub struct MyOpaque { _opaque: [u8; 0] }`
   for the server-owned object.
2. Find the component's `UID` and any slot offsets you need (read
   the open.mp SDK header, then verify with disasm).
3. Use `samp::omp::vtable::secondary_call_target` to call methods —
   the SDK does this for `componentName`, `componentVersion`,
   `create_repeating_timer`, `kill_timer`, and the `ILogger` calls.
4. Implement `OmpComponentHandle` on a `#[derive(Debug, Clone, Copy)]`
   `MyComponent { ptr: NonNull<ServerComponent> }`. The trait gives
   the SDK's typed `omp_query::<MyComponent>()` helper a way to
   instantiate the wrapper.

`PawnComponent` and `TimersComponent` are the in-tree references.

## Verifying offsets against a binary

The defensive rule used throughout the SDK is: when the C++ ABI is
unclear, **disassemble an official binary** instead of inferring
from headers.

```sh
# Linux side
i686-w64-mingw32-objdump -dC Console.dll | less
objdump -dC omp-server.exe                 | less

# Search for "add ecx, 0x38" or "call dword ptr [eax+N]"
grep -nE '(add[[:space:]]+(ecx|edi),[[:space:]]+0x[0-9a-f]+|call[[:space:]]+\[(eax|ecx|edx)\+[0-9]+\])' dump.asm
```

Two regressions were caught this way:

- Initial guesses placed `uid_vtable` at offset 40, then 48, on MSVC.
  Disasm confirmed the server emits `add ecx, 0x38` → offset 56.
- The `IUIDProvider` MSVC vtable was initially modeled with 3 slots
  (matching Itanium). Disasm showed `call [eax]` immediately after
  the offset adjustment → slot 0, single-slot vtable.

Both fixes are now compile-time-asserted; do not regress them.
