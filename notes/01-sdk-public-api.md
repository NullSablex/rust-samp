# samp-sdk public API (verified from `.rs`)

## `samp-sdk/src/lib.rs`

Crate-level doc says: low-level layer; two binding sets — AMX (Pawn VM, SA-MP) and
Open Multiplayer (vtables, `IComponent` layout, typed wrappers of native server
interfaces). Raw-pointer, `unsafe`-friendly. End users should use `samp` crate.

Public modules:
- `amx`
- `args`
- `cell`
- `consts`
- `encoding` — gated `#[cfg(feature = "encoding")]`
- `error`
- `exports`
- `macros` — `#[doc(hidden)]`
- `omp` — gated `#[cfg(not(feature = "samp-only"))]`
- `raw`

`tests` is private (`#[cfg(test)]`).

## `samp-sdk/src/amx.rs`

### `pub struct Amx { ptr: *mut AMX, fn_table: usize }`
- `Debug`. Wraps `*mut AMX` + `amx_Exports` table address.

### `impl Amx`
- `pub fn new(ptr: *mut AMX, fn_table: usize) -> Amx`
- `pub fn register(&self, natives: &[AMX_NATIVE_INFO]) -> AmxResult<()>` — calls `amx_Register`.
- `pub(crate) fn allot<T: AmxPrimitive>(&self, cells: usize) -> AmxResult<Ref<'_, T>>` — validates `cells <= i32::MAX`, errors `AmxError::Memory` otherwise.
- `pub fn exec(&self, index: AmxExecIdx) -> AmxResult<i32>`
- `pub fn find_native(&self, name: &str) -> AmxResult<i32>`
- `pub fn find_public(&self, name: &str) -> AmxResult<AmxExecIdx>`
- `pub fn find_pubvar<T: AmxPrimitive>(&self, name: &str) -> AmxResult<Ref<'_, T>>`
- `pub fn flags(&self) -> AmxResult<AmxFlags>`
- `pub fn get_ref<T: AmxPrimitive>(&self, address: i32) -> AmxResult<Ref<'_, T>>`
- `pub(crate) fn release(&self, address: i32)` — clamps `amx.hea` down to `address`.
- `pub fn push<'a, T: AmxCell<'a>>(&'a self, value: T) -> AmxResult<()>`
- `pub fn strlen(&self, value: *const i32) -> AmxResult<usize>`
- `pub fn allocator(&self) -> Allocator<'_>` (`#[must_use]`)
- `pub fn amx(&self) -> Option<NonNull<AMX>>` (`#[must_use]`)
- `pub fn header(&self) -> Option<NonNull<AMX_HEADER>>` (`#[must_use]`)

### `pub struct Allocator<'amx> { amx, release_addr }`
- RAII over `amx.hea`. Captures heap pointer at construction, restores in `Drop`.
- Doc warns against nested allocators (each restores to its own snapshot).

### `impl<'amx> Allocator<'amx>`
- `pub(crate) fn new(amx: &'amx Amx) -> Self`
- `pub fn allot<T: AmxPrimitive>(&self, init_value: T) -> AmxResult<Ref<'_, T>>`
- `pub fn allot_buffer(&self, size: usize) -> AmxResult<Buffer<'_>>`
- `pub fn allot_array<T: AmxCell<'amx> + AmxPrimitive>(&self, array: &[T]) -> AmxResult<Buffer<'_>>`
- `pub fn allot_string(&self, string: &str) -> AmxResult<AmxString<'_>>` — uses configured encoding if feature on, raw bytes otherwise. Always writes terminator (allocates `bytes.len() + 1`).

### `impl Drop for Allocator<'_>`
- Calls `amx.release(release_addr)`.

### Internal macro
- `amx_try!($call)` — propagates `result > 0` as `Err(result.into())`.

### Imports / notes
- `exports::*` wildcard intentional (40+ marker types).
- `encoding` only imported with feature.
