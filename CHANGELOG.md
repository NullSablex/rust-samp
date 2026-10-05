# Changelog

Current release only. Previous releases are split per major line under
[`changelog/`](changelog/) — see [`changelog/index.md`](changelog/index.md)
for the full directory.

## [v3.7.0-rc.4] — 2026/10/05

Speed, measured: the paths a server runs on every call got a benchmark that
loads a plugin the way a server does, and what it found was cut. Times are
per call on i686.

**Fix to note first:** a `#[event]` handler returning `EventReturn::Suppress`
broke the script after a few hundred suppressed calls — the gamemode's timers
and callbacks stopped running.

### `rust-samp` (lib `samp`) — 3.6.0-rc.4

#### Fixed

- **Suppressing a callback leaves the script's stack as the call would
  have.** The caller pushes a public's arguments and `amx_Exec` takes them off
  when it runs it; a suppressed public skipped `amx_Exec`, so its arguments
  stayed on the stack along with a stale argument count. After enough of them
  the script's stack ran into its heap and nothing in the script ran any more.
  Proven on all six server combinations: one call in three suppressed, the
  rest running, counts exact.

#### Added

- `mainthread::set_budget(Some(duration))` caps the time one drain spends
  running jobs; the rest wait, in order, for the next tick, so a burst of
  replies no longer freezes the server for one long tick. Off by default;
  `mainthread::budget()` reads it.

#### Changed

- **A public through the `#[event]` detour:** 266 → 42 ns with a handler, 67
  → 11 ns without — and every public goes through it once a plugin has an
  event. The handlers are found by script and public index instead of a
  hashed key, handed out without copying the list, and the callback's
  arguments are read into a buffer on the stack.
- **A native call:** the frame marker no longer does locked atomic operations
  (natives run on the main thread only); a native taking two `int`s went from
  27 to 17 ns.
- **The tick** skips the job queue's and the log backlog's locks when they
  are empty, as they almost always are: 111 → 53 ns.
- **Log lines** keep their timestamp for its second and read the local UTC
  offset once a minute, instead of asking for the local time and formatting
  it on every line.
- `mainthread::pending()` reads a counter instead of locking the queue.

### `rust-samp-sdk` (lib `samp_sdk`) — 3.6.0-rc.4

#### Changed

- **Decoding a string argument:** 616 → 155 ns for 127 characters. Unpacked
  strings are read in one pass the compiler vectorizes; packed ones a cell at
  a time instead of a byte at a time; and text that is already valid UTF-8
  becomes the `String` without being copied again.
- **Measuring a string argument** tests sixteen cells at a time: 143 → 80 ns
  for 127 characters.
- **`exec_public!` and `call_public`** resolve the public's name without
  allocating: 30–50% faster. The same for `find_native` and `find_pubvar`.

### `rust-samp-codegen` (lib `samp_codegen`) — 1.6.0-rc.4

Released with the other two; no change of its own.

### Tests

- New benchmark `samp/benches/hot_paths.rs`: natives with each argument kind,
  publics through the detour (passed, suppressed, unwatched), calling publics
  from Rust, the tick, the main-thread queue, a log line.
- The `samp-sdk` benchmarks were rewritten. Several measured nothing — the
  optimizer had removed the work, or a "first access" read a cached value —
  and the `f32` ones timed float addition, which cannot be vectorized, rather
  than the conversion.
- `bounded_strlen` is checked against a byte-by-byte search on five thousand
  generated inputs.

### Docs

- Threads: spreading a burst of jobs over several ticks with a budget.

## [v3.7.0-rc.3] — 2026/10/05

Fixes from feeding the SDK hostile input on real servers — scripts declaring
the natives with signatures they do not have, files that cannot be written,
threads logging in bulk, shutdowns in every order — and from fuzzing.

**Fixes to note first:** an open.mp server could crash on shutdown, and
`rust-samp-sdk` did not compile for `i686-pc-windows-gnu`.

### `rust-samp` (lib `samp`) — 3.6.0-rc.3

#### Fixed

- **open.mp shutdown no longer calls into freed components.** The server frees
  components in its own order, and `Timers` and `Pawn` often go before the
  plugin; the plugin's cleanup then killed its tick timer through a freed
  `ITimer`. In testing, about one shutdown in twenty crashed. The SDK now
  forgets what it holds from a component when the server announces it is
  freeing it (`onFree`), as the official components do.
- **Logging from another thread no longer calls the server from that thread.**
  Neither server's log is thread-safe — open.mp interleaved lines written by two
  threads at once. A line logged off the main thread now reaches the server log
  on the main thread, at the next tick or with the next line logged there; the
  plugin's own log file is still written at once. The backlog is capped at
  10 000 lines, and lines dropped past it are counted in the server log. Applies
  to `enable_logger!` and to the default logger alike.
- Lines logged while the plugin is built — the startup banner, a failure to
  open the log file — reach the server log. They used to go to stderr only,
  because the plugin is built before the server hands its log over.
- When the log file cannot be opened, the SDK says so in the server log
  instead of only returning the error, which plugins commonly discard; a later
  `install` can then try again.
- A panic payload whose `Drop` panics no longer escapes the boundary that
  caught the panic. Every entry point the server calls goes through one guard,
  `samp::panic_guard::catch`.
- `mainthread::post` warns about a backlog after releasing the queue's lock.

### `rust-samp-sdk` (lib `samp_sdk`) — 3.6.0-rc.3

#### Fixed

- **Strings are measured inside the AMX memory.** A string argument was
  measured with the server's `amx_StrLen`, which has no bound: given an address
  at the top of the stack, it read past the end of the script's memory. The SDK
  now measures the string itself, up to the end of the data region, and
  refuses one with no terminator before it.
- **A packed string starting with a byte of 0x80 or above decodes.** The
  packed/unpacked test compared signed, where the server compares unsigned, so
  `!"\233xyz"` read as garbage.
- **Text starting like a byte order mark keeps the configured encoding.**
  Decoding sniffed BOMs: on a Windows-1252 server, a string from a player
  beginning with `ÿþ` was decoded as UTF-16.
- A misaligned address from a script is refused (`MemoryAccess`) instead of
  backing a reference. In a debug build it used to fail an assertion outside
  the native's panic guard and abort the server.
- `UnsizedBuffer::into_sized_buffer` no longer asserts on a size above
  1 MiB in debug builds: the size is often a script argument, and the clamp is
  the defence.
- **`i686-pc-windows-gnu` compiles again.** The layout checks for Linux were
  gated on "not MSVC", which includes windows-gnu, where 64-bit fields align
  differently.

### `rust-samp-codegen` (lib `samp_codegen`) — 1.6.0-rc.3

#### Fixed

- The generated `#[native]` and `#[event]` wrappers and the component entry
  points catch panics through `samp::panic_guard::catch`.
- The component's `onFree` passes on the component being freed.

### Tests

- A fuzz target for AMX strings (`amx_string`): arbitrary cells and claimed
  lengths, writes into buffers of any size, read-back in three encodings.
- Stress test of the main-thread queue from eight threads while draining; it,
  and the rest of `samp` and `samp-sdk`, run clean under ThreadSanitizer.

### Tooling

- `cargo xtask gen-omp` refuses a spec that names a class or a module twice,
  a module or prefix that is not an identifier, an overload or a `skip` entry
  that matches no method. Each passed silently before.
- `cargo xtask check-abi` names the `.pdb` it failed to read.

### CI

- `i686-pc-windows-gnu` joins the build matrix, checked with clippy.

### Docs

- Logging: lines logged before `on_load` now wait for the server instead of
  being lost, and how lines from other threads reach the server log.
- Threads: `log::*` is safe from any thread.

## [v3.7.0-rc.2] — 2026/10/05

A candidate for the release workflow alone: the crates carry no code change
since `v3.7.0-rc.1` — `rust-samp` 3.6.0-rc.2, `rust-samp-sdk` 3.6.0-rc.2 and
`rust-samp-codegen` 1.6.0-rc.2 are the rc.1 crates under new versions.

### CI

- The release workflow's dry run checks the three crates together, with
  `cargo publish --workspace --dry-run`. It used to dry-run each crate on its
  own, so `rust-samp` looked on crates.io for the new `rust-samp-codegen`,
  which a dry run never publishes, and the run failed on every release that
  bumped both. The real publication was unaffected.

## [v3.7.0-rc.1] — 2026/10/05

The rest of the open.mp interfaces, and a Pawn include that stays in step with
the plugin.

**Fix to note first:** a plugin built with `samp-only` did not compile in
3.6.0 — the library side broke in 3.6.0-rc.1, and the code generator had never
honoured the feature for a plugin. Anyone relying on the opt-out wants this
release.

**New:** typed wrappers for nearly every open.mp interface, generated from the
SDK headers and proven against the official binaries and a running server; the
Pawn include generated from a template, checked in `cargo test`, documented in
Pawn's own format; main-thread jobs that reach the plugin's state.

**Hardening:** panics the server can reach no longer end its process; narrow
return values from C++ are read as the whole register.

The per-crate sections come first, then the ones belonging to the repository.

### `rust-samp` (lib `samp`) — 3.6.0-rc.1

#### Added

- **A main-thread job can reach the plugin.** `mainthread::post_with::<T>` hands
  the job `&mut T` when it runs, which is where the result of background work
  usually has to land. Until now the closure took no parameters and there was no
  route to plugin state from a worker thread, so a plugin doing I/O had to keep a
  channel of its own — the limitation `email_samp` ran into while adopting the
  queue.
- `mainthread::post_with_amx::<T, _>(script, job)` for the shape that shows up
  every time: record the result in the plugin, then tell the script. What the
  closure returns runs after the plugin borrow has ended, so calling a `public`
  from there is safe, and the job is dropped quietly when the script was
  unloaded meanwhile.
- `plugin::with_instance::<T, _>` reaches the plugin from anywhere on the main
  thread, which is what the hidden `plugin::get` never safely allowed. It returns
  `None`, naming the reason in the log, when `T` is not the plugin's type, when
  there is no plugin, when a borrow is already alive, and when called from inside
  a native — where `&mut self` is in scope and is what to use.
- `plugin::is_borrowed` reports whether a borrow is alive, for a plugin deciding
  between reaching for state and deferring.
- **`samp::pawn_include` — drift checking for a hand-written include.** The
  generated include from 3.6.0 cannot drift, but a plugin that keeps its `.inc`
  by hand — for the default values, `sizeof(dest)`, varargs and documentation a
  Rust signature cannot express — pays with silence when a native is renamed in
  Rust and forgotten in the include. `compare_file(path)` reports what the two
  sides disagree about: a native registered but not declared, declared but not
  registered, a different argument count, a different return tag, or an argument
  whose tag, `&` or `[]` changed. Setting `SAMP_PAWN_INCLUDE_CHECK` to a path
  runs it at load and logs the findings, which is the shape a CI job wants.
  Additions only the include can make are not divergences, and a declaration
  ending in `...` stops arity checking, being open-ended by design.
- The comparison understands three things a real include does. An **alias**
  (`native Email_Close(account = 0) = email_close;`) is matched by the native
  after the `=`, so an include presenting the whole surface under other names is
  not drift. A **`raw` native** is compared by presence only, since it parses
  its own arguments and the include is the only place its shape is written down.
  A **template** (`plugin.inc.in`) checks like its output, which is usually the
  file worth checking, being the one edited by hand.
- The check runs after `on_load` rather than from the entry point, so it reaches
  a logger the plugin installs there.
- **The whole check runs in `cargo test`, with no server.** `initialize_plugin!`
  now also emits `pawn_native_decls()`, the declarations `#[native]` derived,
  available without a loaded plugin. `pawn_include::compare_with` and
  `compare_file_with` take them, so a plugin guards its include in CI:
  `assert!(compare_file_with("include/p.inc", &pawn_native_decls())?.is_empty())`.
  `pawn_include::render` writes an include from the same place, for a plugin that
  generates rather than maintains one, replacing the server run with
  `SAMP_PAWN_INCLUDE` set.
- `pawn_include::registered` exposes the parsed registered side, for tooling that
  wants the declarations rather than the divergences.
- `samp::pawn_include::parse` reads `native` declarations out of any Pawn source,
  skipping commented-out ones and handling declarations spread over several
  lines; `compare_declarations` compares two parsed lists with no server behind
  either. Validated against four hand-written includes in the wild
  (`a_players`, `a_mysql`, `YSF`, `foreach`, ~550 declarations): every
  declaration read, and every one inside a comment block correctly left out.
- **Generating the include from a template.** A generated include has no room
  for documentation and a hand-written one drifts; a template is both.
  `pawn_include::Template` fills `{{NATIVES}}`, `{{NATIVE:Name}}` and
  `{{NATIVE:Name as Alias}}` from the derived declarations, while the prose, the
  sections, the constants and the callback documentation stay written by hand.
  `{{PLUGIN}}`, `{{GUARD}}` and `{{VERSION}}` come from the plugin, anything else
  from `.var(name, value)`. A registered native the template never places is an
  error, as are an unknown placeholder, a `{{NATIVE:…}}` that names nothing and an
  unclosed `{{` — all reported at once.
- **Documentation in Pawn's own format, written once.** Pawn documents a native
  with a `/** */` block of XML tags — what the open.mp includes carry and what
  `pawncc -r` reads into its report. `#[native]` now captures the Rust doc
  comment, `pawn_include::pawndoc` renders it in that format, and a template
  places it with `{{DOC:Name}}` or above every declaration with
  `Template::with_docs()`. `@param`, `@returns`, `@remarks` and `@seealso` map to
  the matching tags; a line already starting with `<` passes through, so the full
  format — `<library>`, nested markup — is available; everything else is escaped,
  so a doc mentioning `a < b` cannot break a tag.
- **Callbacks are declared once and forwarded by the include.** A script
  implements a plugin's callbacks as `public`, and nothing in the Rust code says
  what they are, since `exec_public!` takes the name at the call site.
  `initialize_plugin!` takes `callbacks: ["OnThing(a, b)"]`, `{{CALLBACKS}}` turns
  each into a `forward`, and `pawn_include::missing_forwards` reports one the
  include never forwards — a `public` for a callback the include forgot is never
  called, with nothing to say why. `pawn_include::parse_forwards` reads them back.
- Placing the same declaration twice in a template is an error: Pawn rejects the
  second as an already defined symbol, and the template is where it can still be
  caught. An alias beside its original is two names, so it is not a duplicate.
- `SAMP_PAWN_INCLUDE_TEMPLATE` renders a template at load with no code at all,
  with `SAMP_PAWN_VAR_<NAME>` supplying `{{<NAME>}}`. Because the environment
  belongs to the process and a server may load several Rust plugins, these
  variables now accept `plugin=path` entries, so two plugins pointed at one
  variable no longer overwrite each other's include; a bare path still applies to
  whichever plugin reads it.
- The include check renders a template before comparing, so it can be pointed at
  the `.inc.in` — and a template that will not render is reported there too.
- `#[native]` takes what a Rust signature cannot say: `default(account = 0)`,
  `sizeof(dest_len = dest)`, `varargs`, and `args = "…"` for the argument list
  written out — the only way to declare a `raw` native. Naming an argument the
  function does not have is a compile error, since it means a rename the
  attribute did not follow.
- `amx::loaded()` and `amx::count()` enumerate the loaded scripts. A plugin with
  something to announce to the gamemode and every filterscript had no way to
  reach them: `amx::get` only answers about an ident already in hand.

#### Fixed

- **A plugin built with `samp-only` did not compile — since `3.6.0-rc.1` for
  the library, and for every plugin since native Open Multiplayer became the
  default.** Two faults. In the library, the Pawn include's bookkeeping had been
  placed in the Open Multiplayer-only part of the runtime, and the SDK's log
  macros were compiled only with Open Multiplayer. In the code generator,
  `initialize_plugin!` decided whether to emit the Open Multiplayer entry point
  from `CARGO_FEATURE_SAMP_ONLY`, which Cargo sets only for build scripts — so
  the check was always false, the entry point was always emitted, and it
  referred to items `samp-only` removes. The decision now belongs to the `samp`
  crate: `initialize_plugin!` wraps the entry point in `samp::__omp_only!`,
  which `samp` defines once per case. A `samp-only` build of the `hello`
  example loads on SA-MP and, as a legacy plugin, on open.mp; CI now builds it,
  and runs clippy on the library crates with every feature combination — none
  of the builds before turned the feature on, which is how this shipped.
  With `samp-only`, the component UID is now written to `Cargo.toml` like in
  any other build; it is unused there, and harmless.
- **A native called outside the server's own ordering could end its process.**
  The `#[native]` wrapper resolved the script and the plugin with `expect`, before
  the `catch_unwind` that guards the native's body — so a panic there crossed
  into the server. That ordering is not guaranteed: another plugin can call a
  native directly, on a script this one never received or before this one has
  loaded. Both now answer `0`, with the reason logged once, through
  `interlayer::native_amx` and `plugin::try_get`.
- The `#[event]` hook no longer assumes the function table carries `amx_Exec`;
  without it, the SDK says `#[event]` handlers will not fire, instead of
  panicking. Its log lines also stop hardcoding the SDK prefix.
- **The turnkey logger lost the severity of every line on the server's side.**
  It routed through the plain log, which open.mp classifies as a message, so a
  `warn!` arrived as `[Info]`. It now maps the level, so warnings and errors
  arrive as warnings and errors; SA-MP's `logprintf` has no levels and is
  unaffected.

#### Tests

- The include parser was checked against the 38 includes the open.mp server
  ships: 1068 native declarations and 117 forwards read, with no divergence from a
  count of the declaration lines — the pawndoc `/** */` blocks and `///` lines
  around them are handled.
- 29 tests over the template engine and the documentation renderer: the prose kept, a native placed once
  whether by name or by `{{NATIVES}}`, aliasing, every error reported at once,
  the per-plugin env selector, and a template checked as what it renders to.
- The `counter` example is generated from a `counter.inc.in` that carries prose,
  sections, a generated `forward`, a hand-written `#define` shorthand and an
  alias; the committed `counter.inc` is asserted to
  be exactly what the template renders (`UPDATE_INCLUDE=1` rewrites it). The
  result compiles under the open.mp Pawn compiler with no warnings — and with
  `-r` the compiler reads the generated documentation into its XML report, next to
  the official includes' own, which is what proves the format is really pawndoc.
  A script
  implementing the callback through the template's shorthand, calling the alias
  and the defaulted `Counter_SetMax()`, runs on a live server. The file the server
  writes from the template at load is byte for byte the committed one.
- The plugin cannot be aliased: a nested borrow, a borrow from inside a native
  and a wrong `T` are each refused rather than served, the borrow is released
  even when the closure panics, and Miri sees no aliasing in any of it.
- Validated end to end on three servers — open.mp on Linux, open.mp on Windows
  under Wine (MSVC ABI), and SA-MP on Linux — with the `counter` example's async
  native rewritten onto `post_with_amx`: the job writes plugin state, the script
  hears back afterwards, and deliberately calling a `public` from inside the
  borrow produces the warning.
- 13 unit tests over the parser and the comparison, covering defaults,
  `sizeof(...)`, varargs, commented-out declarations and each kind of
  divergence, aliases and `raw` natives, plus the stable ordering the CI output
  depends on.
- The `counter` example ships a `counter.inc` and a test that compares it with
  its natives, so the derivation itself is covered in CI: an argument added to a
  native fails the example's test.
- Validated against `email_samp` both ways: in `cargo test` over its `.inc.in`
  template and its alias variant, and on a live open.mp server, over both of its
  hand-written includes (the original and the open.mp-styled alias variant, 34
  natives each, all `raw` or alias-declared): both report a match, and an
  include edited to rename one native and change another's arity and tag reports
  exactly those four divergences.

### `rust-samp-codegen` (lib `samp_codegen`) — 1.6.0-rc.1

#### Added

- `#[native]` registers its name as a C string literal. The name used to be
  built with `CString::new(...).unwrap()` and leaked on purpose so the server
  could keep the pointer; a literal lives in the binary for as long as the plugin
  is loaded, with nothing to allocate, leak or unwrap.
- `#[native]` takes `default(...)`, `sizeof(...)`, `varargs` and `args = "…"`,
  which shape the Pawn declaration it derives; an argument named that the
  function does not have is a compile error.
- `initialize_plugin!` records the plugin crate's version, for `{{VERSION}}` in
  an include template.
- `#[native]` marks its frame with `samp::plugin::NativeFrame`, so the SDK can
  tell that `&mut self` is out and refuse a second borrow from a main-thread job
  instead of aliasing it.
- `initialize_plugin!` emits `pawn_native_decls()` next to the entry points: the
  Pawn declaration of every registered native, as a plain function, available
  without a server. It is what lets `samp::pawn_include` compare a hand-written
  include with the natives behind it from a test.
- `initialize_plugin!` takes `callbacks: ["OnThing(a, b)"]`, the Pawn callbacks
  the plugin calls, and emits `pawn_callback_decls()`; `#[native]` captures the
  function's doc comment and `initialize_plugin!` emits `pawn_native_docs()`.
  Both feed the include template (`{{CALLBACKS}}`, `{{DOC:Name}}`).

#### Fixed

- **`initialize_plugin!` emitted the Open Multiplayer entry point even for a
  `samp-only` plugin**, which then did not compile. It decided from
  `CARGO_FEATURE_SAMP_ONLY`, which Cargo sets only for build scripts. The entry
  point is now wrapped in `samp::__omp_only!`, which the `samp` crate defines
  according to its own feature. See the `rust-samp` entry.
- The `#[native]` wrapper resolves the script and the plugin without `expect`,
  before its `catch_unwind` — through `samp::interlayer::native_amx` and
  `samp::plugin::try_get` — so a native called outside the server's ordering
  answers `0` instead of ending the process.

### `rust-samp-sdk` (lib `samp_sdk`) — 3.6.0-rc.1

#### Added

- **Wrappers for nearly every open.mp interface, generated from the SDK
  headers.** 762 functions over 53 interfaces, in `omp::generated` and
  re-exported at `omp::`: every entity (`object_*`, `pickup_*`, `textdraw_*`,
  `gangzone_*`, `actor_*`, `menu_*`, `class_*`, `vehicle_*`, `npc_*`,
  checkpoints, text labels, player objects and text draws), every component,
  the player pool, `ICore`, `IConfig`, the database connections and result sets,
  and each per-player data interface with its accessor (`player_dialogs(player)`,
  `player_objects(player)`, ...). Each component interface gets its UID and its
  `ComponentInterface` impl. What the generator will not state with certainty —
  `std::` types, references to structs the SDK does not mirror, C-style `...`,
  overloads not named in the TOML — is listed at the end of each module with the reason, never
  guessed.
- The generator covers three more shapes, which brought 67 methods in —
  among them every setter of the text draws, the checkpoints' positions and the
  vehicles' respawn delay. A `const Vector3 &` (or any mirrored value type
  taken by const reference) becomes a Rust reference, which is that pointer; a
  setter returning the object itself for chaining (`ITextDrawBase &`) drops the
  value, since the caller already holds the object; and `size_t` is `usize`.
- Overloads, named one by one in `xtask/omp-wrappers.toml` (`overloads`):
  `player_textdraws_create` and `_create_preview`, `player_textlabels_create`
  and `_create_on_player`/`_create_on_vehicle`,
  `player_objects_begin_editing` and `_begin_editing_player_object`,
  `player_attach_camera_to_object` and `_attach_camera_to_player_object`. The
  slot of each comes from clang's layout, which already places MSVC's reversed
  order; `cargo xtask check-abi` checks an overload on Linux by its parameters as
  well as its name, since the name alone would pass either one. A per-player
  text draw or text label can now be created, so their round trips run too.
- **Structs the headers pass by value, mirrored from them** — 16 in
  `omp::generated::structs`: `VehicleSpawnData`, `VehicleParams`,
  `PlayerClass`, `WeaponSlotData`, `ObjectMoveData`, `ObjectAttachmentData`,
  `TextLabelAttachmentData`, `PlayerKeyData`, `PlayerAimData`,
  `PlayerSurfingData`, `PlayerSpectateData`, `PlayerAnimationData`,
  `ActorSpawnData` and others. A struct is mirrored only when every field is
  something the SDK states with certainty and it travels by value under both
  ABIs (trivially copyable); each carries its C++ defaults as `Default` where
  they are literals, and compile-time assertions of its size and every field
  offset per ABI, taken from clang's layout. What was not mirrored is listed
  at the end of the file, with the reason. With them came 71 more methods:
  `vehicle_set_params`/`vehicle_params`, `vehicle_spawn_data`,
  `object_move`/`object_moving_data`, `class_class`, `player_key_data`,
  `player_aim_data`, `player_give_weapon`, `npc_rotation`, ... A method
  returning a `const T &` hands back a `*const T` into the server's copy.
- **Events of every component, generated.** `EventDispatcher<H>` with
  `add_event_handler`, `remove_event_handler`, `event_handler_count` and the
  `priority` constants, one generic wrapper for the template every component
  uses; an accessor per component (`objects_event_dispatcher`,
  `npcs_event_dispatcher`, `dialogs_event_dispatcher`, ...); and 12 handlers
  generated from the headers — actors, classes, the console, gang zones,
  menus, NPCs, objects, pickups, checkpoints, dialogs, custom models, text
  draws — each with `DEFAULT`, a vtable doing what the C++ bodies do, to
  override with struct update syntax. A narrow integer a handler receives
  arrives as a whole word, the value in its low bits: the rule return values
  already follow. The player groups and the vehicles' keep their hand-written
  handlers.
- **Nothing the headers declare is left without a wrapper.** The last gaps
  closed with what they needed:
  - `Pair<A, B>`, `Span<T>`, `HybridString<N>` and `FlatSet<T>` in
    `omp::containers`, laid out as the C++ types are. `flat_set_entries` reads
    a `robin_hood` set (`players_bots`, `vehicle_passengers`, ...) and fails
    closed, bounded by the table's own allocation — as the extension-map walk
    already did.
  - Structs with anonymous unions (`ObjectMaterialData`, `PeerAddress`,
    `ConsoleCommandSenderData`, the vehicle sync packets), with bit-fields
    (collected into one `bits_*` integer), with pointers, and with
    `HybridString`s (`BanEntry`, `AnimationData`) — the last ones by reference
    only, since the ABIs pass them by value differently. 36 types in all, each
    with its layout asserted per ABI.
  - Arrays (`StaticArray<T, N>`, `WeaponSlots`) as `[T; N]`, their bounds
    evaluated from the headers' constants and confirmed by clang before
    anything is written; raw pointers back (`config_int`); out-pointers
    (`const T *&`); enum out-parameters.
  - Interfaces the server calls back through — `HTTPResponseHandler`,
    `OptionEnumeratorCallback`, the core's `onTick`, the player pool's
    `PoolEventHandler<IPlayer>` — as handlers; a pure interface has no
    `DEFAULT`, since the plugin implements it whole.
  - `Microseconds`, `TimePoint` and `WorldTimePoint`; the unit of the last is
    the standard library's, hence `WORLD_TICKS_PER_SECOND`.
  - The overloaded `create` of text draws, text labels and vehicles under the
    generator's names too (`textdraws_create`, ...), beside the hand-written
    `create_*`.
- A getter returning a small struct can take arguments: `config_string`,
  `core_weapon_name`, `player_weapon_slot`, `players_default_colour`, the
  database rows' `field_string`/`field_name`, the menu's `cell` and
  `column_header`, the gang zones' colours for a player — 15 in all.
- The overloads that had no wrapper in any form: `textdraws_create_preview`,
  `textlabels_create_on_player`/`_on_vehicle`,
  `vehicles_create_from_spawn_data`, `npc_start_playback`/`_id` and
  `config_remove_ban_at`.
- `GTAQuat`, a rotation laid out as the server's `glm::quat`: `w` first, since
  the SDK is built with `GLM_FORCE_QUAT_DATA_WXYZ`. `entity_rotation` and
  `entity_set_rotation` read and set it for any entity, as `entity_id` does.
- Out-parameters (`T &` without `const`) are `&mut T`; typedefs of integers
  (`PickupType`) read as the integer; `long` is `i32` and `double` is `f64`.
  `GangZonePos` implements `Default`.
- `Milliseconds`, `Seconds`, `Minutes` and `Hours`, the `std::chrono`
  durations the headers take and return, laid out as the C++ classes are. The
  count of minutes and hours is an `int` under Microsoft's library and 64 bits
  under libstdc++, so it is `HoursRep`, which follows the target.
- Wrappers for `ITimersComponent` and `ITimer`: `timers_create_counted` (a
  first delay, an interval and a number of calls), `timers_count`, and the
  timer's `running`, `remaining`, `calls`, `interval`, `trigger` and `handler`.
  `ITimersComponent` implements `ComponentInterface`. The three-argument
  `create` and `kill` stay hand-written (`create_repeating_timer`,
  `kill_timer`).
- `IDatabasesComponent` implements `ComponentInterface`, so
  `omp_query::<Component<IDatabasesComponent>>()` finds it;
  `TextLabelAttachmentData` implements `Default` with the header's
  `INVALID_PLAYER_ID`/`INVALID_VEHICLE_ID`.
- `ComponentInterface::COMPONENT_OFFSET`, for a component whose `IComponent` is
  not its first base. `INPCComponent` puts its pool first, so the `IComponent*`
  the server hands out sits 4 bytes in (8 on MSVC); `Component::as_ptr` moves
  back by it. Without that, the first call into the NPC component crashed the
  server — found by running it.
- `Vector4::ZERO`, and `Default` for `Colour` and `Vector2`.
- **`omp::Component<I>` and `omp::ComponentInterface`.** An interface handle
  (`IObjectsComponent`, `IVehiclesComponent`, and the other seven) now declares
  its UID, and `samp::plugin::omp_query::<Component<IObjectsComponent>>()`
  returns it typed, with `Component::as_ptr` for the functions that take it.
- `StringView::of(&str)` for a view to hand the server for one call,
  `StringView::EMPTY`, and `StringView::to_owned_string` to copy one out;
  `StringView::from_static` is now `const`. `Vector3::ZERO`.
- Every public item of the `omp` submodules is re-exported at `omp::`, which
  adds `Component`, `ComponentInterface`, `ServerComponent` and `NUM_AMX_FUNCS`
  to what was already there.
- `encoding::current()` returns the encoding in force. A plugin that lets the
  server owner choose one could set it but not read it back, so it could not
  report which encoding it was using.

#### Changed

- **Narrow return values are read as the whole register.** A C++ function
  returning `bool`, `uint8_t` or a 16-bit integer sets only the low part of
  `EAX`: the official `IVehicle::isOccupied()` on Windows ORs two pointers into
  it and then `setne %al`, so `true` comes back as `0x????..01`. Declaring the
  foreign function as returning `bool` leaves Rust assuming a clean 0 or 1; the
  code rustc generates today happens to read only `AL`, but nothing guarantees
  that. `call_vtable!` now goes through `VirtualReturn`, which receives such
  values as `u32`/`i32` and narrows them in Rust. The two hand-rolled
  `add_*_handler` calls use `call_vtable!` now as well.
- The `omp` module lost most of its repetition without changing what it does.
  Per-ABI slot constants were a `#[cfg]` pair each (about 140 of them) and are
  now one line in `slots!`; opaque handles went through `opaque!`; getters and
  setters that were a signature around one `call_vtable!` are one line in
  `virtual_fns!`; the `StringView` return that differs per ABI is one helper
  instead of two hand-written copies. Two workarounds went with it: a helper
  that pushed `u32` values through an `i32` setter with `as`, and a slot
  constant with two names.
- That this changed nothing was checked, not assumed: every non-inlined
  function of the module was disassembled before and after, for both ABIs, and
  compared. All matched except the two position getters, where the neutral
  `Vector3` answer is now written only on the failure path instead of before the
  call — same slot, same call, same stack cleanup. The `counter` example then
  produced identical output on open.mp Linux and open.mp Windows.

#### Deprecated

- `Export::from_table`, in favour of `Export::try_from_table`, which returns
  `None` instead of panicking. The old method keeps its signature and behaviour,
  so nothing that calls it breaks.
- The nine `omp::as_*_component` casts (`as_objects_component`,
  `as_vehicles_component`, ...). Each took a component looked up by a UID
  constant and cast it, and nothing tied the two together: looking up one
  component and casting it to another compiled. `omp_query::<Component<I>>()`
  takes the UID from the interface type instead. The casts keep working.

#### Fixed

- **`player_extension` returned null for every component's per-player data.**
  It called the virtual `getExtension`, whose base implementation returns null;
  components file their data with `addExtension`, in the extension map. It now
  does what open.mp's own `queryExtension<T>()` does — the map first, then the
  virtual — and finds a player's dialog, checkpoint, menu and object data, as
  `examples/omp-showcase` confirms on both servers.
- `OmpComponent`'s documentation was attached to a constant declared between
  the doc comment and the struct, so the struct showed as undocumented. And
  `player_name` said both ABIs return its `StringView` through a hidden pointer,
  which is the MSVC half only — the code already did the right thing.
- **An AMX function table the server left partly empty ended the server's
  process.** Every `Amx` call resolved its function through `Export::from_table`,
  which asserted on an empty slot — a panic on the path of every native, and a
  panic there aborts the process. The calls now answer `Err(AmxError::NotFound)`,
  which a native reports like any other error. The first assertion, on a null
  table, was already unreachable: `Amx` checks for that before resolving.

### Dependencies

- Dependabot: Cargo updates in two batches (13 in #69, 12 in #70 — the one the
  SDK itself uses is `encoding_rs` 0.8.42, behind the `encoding` feature),
  `quinn-udp` 0.5.16 (#71), 3 GitHub Actions updates
  (#68), and for the documentation site `pymdown-extensions` (#67) and
  `urllib3` 2.8.0 (#72).

### CI

- Clippy on the library crates with every feature combination, and a build of
  the `hello` example with `samp-only`. No build turned that feature on before,
  which is how its breakage reached a release.
- `cargo deny check` against `deny.toml`: licenses, sources, yanked crates.
- An `xtask` job: formatting, clippy and the generator's own tests, with no
  clang installed. It is part of the `ci-status` gate.
- The cross-only `aarch64` check caught a log import in `samp/src/events.rs`
  left unconditional during the panic audit: only the `amx_Exec` hook uses it,
  and the hook exists only on x86, so `-D warnings` failed everywhere else. The
  import now carries the hook's own `cfg`. It never reached a published
  version.

### Docs

- `natives.md`: checking and generating the Pawn include — templates,
  placeholders, callbacks, documentation in Pawn's format, what `#[native]` can
  add to a declaration, and the per-plugin environment variables.
- `omp-interfaces.md`: the typed component lookup, the generated wrappers and
  how they are proven, per-player extensions, and adding an interface.
- `threads.md`: reaching the plugin's state from a main-thread job, and the one
  `&mut` rule. `encoding.md`: reading the encoding in force.
- `migration.md`, `plugin-anatomy.md` and the README: the plugin declares its
  own `samp-only` feature, which the documented `#[cfg]` guard needs.
- `migration.md` gains a v3.6.0 → v3.7.0 section: the typed component lookup
  replacing the deprecated casts, `try_from_table`, and what `player_extension`
  finds now.

- The README shows the repository release and each crate's version as separate
  badges, and says that the crates are versioned on their own: one crates.io
  badge next to a different GitHub release number read as a mismatch.
- `examples/omp-showcase` gained a README, and is listed in the root README,
  `examples/README.md` (which also lists `sink-demo`, missing until now) and
  `advanced-examples.md`.
- `api-reference.md` lists what this release added: `amx::loaded`/`count`,
  `plugin::with_instance`/`is_borrowed`, `samp::mainthread`,
  `samp::pawn_include`, `encoding::current`, `omp::Component<I>` and the
  generated wrappers. The `rust-samp-sdk` README names the generated wrappers.
- Removed `samp-sdk/readme.md` and `samp-codegen/readme.md`, copies left from
  v2 that no manifest pointed at, and that clash with `README.md` on a
  case-insensitive file system.

### Tooling

- `deny.toml`, the dependency policy the new CI step checks: licenses, sources
  and yanked crates. Everything that ships with the MIT crates is permissive today; the
  policy makes a dependency under anything else fail instead of arriving with an
  update.
- The generators pass their output through `rustfmt`, so it is what `cargo fmt`
  leaves: written unformatted, the next `cargo fmt` rewrote it and `--check`
  reported the files stale forever after.
- `fuzz/` gained a `pawn_include` target: the include parser and the template
  renderer read files from outside the plugin, inside the server, at load.
- **The repository's tooling is Rust: `cargo xtask`.** It lives in `xtask/`, a
  host crate with its own workspace, and no Python is left in the repository.
  libclang is loaded at run time, so building, linting and testing the crate
  need no clang; only running the generator does. Each command was proven
  against the Python script it replaces before that script was removed.
  - **`cargo xtask gen-omp`** generates `samp-sdk/src/omp/generated/` from
    `xtask/omp-wrappers.toml` and the open.mp SDK headers; `--check` reports
    stale files. The Python generator this release first wrote asked clang for
    a full JSON AST once per interface — about 30 minutes and up to 4 GB of
    memory. This one reads every header in one libclang parse per ABI, takes
    sizes, offsets and constants from it, and runs `clang++` once per ABI for
    the vtables and once for the subobject offsets, beside the parse: about
    15 seconds and 280 MB. Types are read from libclang's structure — typedef
    names, template arguments, the canonical record — instead of their
    spelling, and what the SDK writes by hand is read with `syn`. Its output
    matched the Python generator's line for line except where it knew better:
    the `IDatabasesComponent` UID and `ComponentInterface` impl, which the
    script's pattern missed; the header of a nested struct (`network.hpp` for
    `NetworkID`); and a `Default` for `TextLabelAttachmentData`, whose C++
    defaults are named constants rather than literals. clang is given the
    definitions the SDK's `CMakeLists.txt` gives every consumer
    (`GLM_FORCE_QUAT_DATA_WXYZ` and the rest): without them the quaternion's
    field order was the opposite of the server's.
  - **`cargo xtask check-abi`** replaces `scripts/check-abi-slots.py`: every
    slot against the official binaries, by name on Linux and by the bytes each
    method pops on Windows, with `--generated` for the generated wrappers too.
    ELF and PE are read with `object`, names demangled with `cpp_demangle`, and
    `ret N` found with `iced-x86` — no `nm`, `readelf`, `c++filt` or `objdump`.
    It runs in under a second, and gives the script's verdict on every slot
    plus seven more: the script passed an address's bytes to `re.finditer` as
    a pattern, and the `\` in one (`0x5C`) made it find no vtable for
    `CheckpointsComponent` and `CustomModelsComponent`. Two fixes carried over
    from the script's last revisions: slots declared through `slots!` are
    read, and a tail call (`jmp` through a pointer) ends the search for a
    method's `ret` instead of reading the next function's. It then went
    further than the script: the official Windows server ships a `.pdb`
    beside every binary, `omp-server.exe` included, and `check-abi` reads them
    (`pdb`, `msvc-demangler`) to check every MSVC slot by the name of the
    method it holds — the way Linux is checked — on top of `ret N`, which
    cannot tell apart methods without arguments. The vtable is still found
    through RTTI where the binary keeps it, whose locator records the
    subobject offset; the PDB stands in only for the server's own classes,
    which have none. A function MSVC folded with others (`/OPT:ICF`) is
    matched by any of the names it carries.
  - **`cargo xtask roundtrip`** derives the showcase's set/get round trips
    from the generated code, so nobody chooses by hand what gets tested. It
    reads the wrappers with `syn`, and found one round trip the script's
    pattern could not: a getter whose signature `rustfmt` wraps over lines.
  - **`cargo xtask vtable <Class>`** replaces `scripts/omp-vtable.py`: a
    class's slots for both ABIs, `--rust` printing `slots!` entries. Its stub
    only defines a constructor, which makes clang lay the vtable out without
    overriding anything — no list of pure methods to build, and no host
    `va_list` leaking into the i686 layout. It says where MSVC keeps an
    override in a secondary base's vtable (`getUID` at offset 56), which the
    script reported as a meaningless slot, and marks overloads that need a
    name each.

### Validation

- `omp-showcase` now also proves what the first run listed as ignored. Every
  setter the server rejected was traced to the server's own code and exercised
  under the conditions it sets: a valid fighting style, an armed NPC for its
  clip, the NPC seated as a driver for its vehicle state and for angular
  velocity, and the NPC's own weapon and special action for the player-side
  getters. Four setters remain unproven by design — they only send an RPC to
  the client (`player_set_velocity`, `vehicle_set_velocity`, and the player's
  `set_action`/`set_armed_weapon`, which an NPC ignores) — and the report says
  so. Round trips also cover `Vector3`, `Vector4`, `Colour` and text now, the
  return conventions most likely to differ between the ABIs, and the durations
  and const-reference setters the generator gained later, the global text
  draw's included, and the per-player text draws and text labels created
  through the new overloads, and one check per mirrored struct in the shape
  it travels in — by value, by `const &`, as a pointer into the server's copy.
  The quaternion's order was proven against an angle the server keeps apart:
  a vehicle turned 90 degrees reads back with only `w` and `z` set. Generated
  handlers were registered and fired by the server with no client — an NPC's
  create [1], destroy [2], spawn [3] and death [8] with its reason, an
  object's `onMoved` [0] — and `config_string`/`core_weapon_name` read a
  `StringView` back past an argument. The containers were proven the same
  way: `config_strings` filling a `Span`, `config_enum_options` calling a
  callback object, a `BanEntry` added, found and removed, `vehicle_colour`
  and `player_time` as `Pair`s, the NPC found in `players_bots` through the
  hash-table walk, `vehicles_models` as an array, and the core's `onTick`
  receiving sane `Microseconds` and a steady `TimePoint` on every tick; the
  database component found through its new `ComponentInterface` impl, and the
  text draw's background colour, a round trip only the Rust reader finds; and a
  counted timer that reports its interval, calls and handler and fires exactly
  three times. 152 passed, 0 wrong, identically on open.mp Linux and on
  Windows under Wine (WineHQ 11).
- New tools in the loop: `cargo-careful` (the standard library's own debug and
  UB checks) and AddressSanitizer with leak detection on the host target both
  run the SDK's tests clean; `cargo-semver-checks` finds no breaking change in
  `samp` and only the intended deprecation in `samp-sdk`; a new fuzz target for
  `pawn_include` ran 214,617 inputs without a panic, `parse_debug` 9.7 million.
  The four plugins built on the SDK (`email-samp`, `mysql_samp`, `json-samp`,
  `env-samp`, the last two written for 3.0) compile against it unchanged.
- The SDK ran on every combination it supports: SA-MP on Linux and on
  Windows (plugin), and open.mp on Linux and on Windows as a legacy plugin and
  as a native component — Windows under Wine. On each, the `counter` example
  under a Pawn script gave the same answers: default and by-reference
  arguments, an aliased native, a public called back with a string and a float
  whose return reached the native, work finished on another thread and handed
  back through `post_with_amx`, the tick, and the extended native table — read
  as a component, reported unavailable everywhere else.
- The four plugins built on the SDK compile against this tree with no error
  or warning, each checked to resolve `rust-samp` to it rather than to a
  locked older version.
- The generated wrappers were proven three ways. **Against the binaries**:
  `cargo xtask check-abi --generated` passes 2276 checks — every generated
  slot by method name on both ABIs (776 of them on Windows through the PDBs,
  the server's own `Player` and `PlayerPool` included, which used to be left
  to a server run) and by the bytes each method pops. The 35 it declares
  underivable are all `ret N` checks of methods that end in a tail call; each
  of those slots passes its name check.
  **Against a running server**: the new `examples/omp-showcase` creates one of
  every entity, reads back what it created, and round-trips every setter that
  has a matching getter — including the player's, through an NPC. On open.mp
  Linux and on open.mp Windows under Wine it reports the same thing: 53 passed,
  0 wrong, and 9 setters the server ignores by its own rules (a fighting style
  of 3 is not one, ammunition without a weapon), listed rather than hidden.
  That run also proved the extension-map walk against a real player's data for
  the first time.
- The panic audit: every `unwrap`, `expect`, `assert!` and `panic!` outside test
  code was classified by whether the server can reach it. Four were, and are
  fixed above; the rest are compile-time layout checks, `debug_assert!`s, or
  invariants the servers' call order guarantees — the runtime and the plugin are
  created by `Supports`/`ComponentEntryPoint`, the first call each server makes.
  An empty function-table slot is now pinned by a test that expects an error, and
  the `counter` example, whose every native went through the changed wrapper,
  was run on open.mp Linux, open.mp Windows under Wine and SA-MP Linux.

## [v3.6.0] — 2026/09/25

Correctness release, in two parts, plus the whole open.mp interface layer.

**Bug fixes:** several vtable layouts were wrong for native open.mp components.
`SampPlugin::on_tick` never fired on Linux, `getUID()` returned a different value
on every run, and on Windows the timer call and `removeExtension` corrupted the
stack. Every index is now verified against the official server binaries. Anyone
shipping a Rust component under open.mp wants this release.

**Soundness:** the SDK runs clean under
[Miri](https://github.com/rust-lang/miri) on `i686-unknown-linux-gnu`, with the
default (strict) provenance and Stacked Borrows checks.

**New:** the generated Pawn include, a queue back to the main thread, typed
calls into Pawn publics, encodings that stop losing characters quietly, and
`samp::omp` — player and vehicle events delivered by the server itself, entities
created and read, players and vehicles found by id.

It went through two release candidates, `v3.6.0-rc.1` and `v3.6.0-rc.2`, both
published here and on crates.io. Their entries are kept below as they were
written, so anyone still on `3.5.0-rc.1` or `3.5.0-rc.2` can see what their
version carried and what the next candidate changed. What follows here is
everything both carried, in one place. The open.mp interface layer is the newest
part of it and has been exercised against real servers on both platforms, but
not yet on a public server with players — expect its shape to be refined in
3.7 as it meets real use.

The per-crate sections come first, then the ones belonging to the repository
rather than to any published crate.

### `rust-samp-sdk` (lib `samp_sdk`) — 3.5.0

Additive public API plus two deprecations.

#### Fixed

- **`getUID()` returned garbage on Linux.** The secondary `IUIDProvider` vtable
  the SDK hands the server carried two destructor thunks before `getUID`. It has
  none: `IUIDProvider` declares no virtual destructor, so the secondary vtable
  holds a single slot, on Itanium exactly as on MSVC. The server called slot [0]
  and got the no-op thunk, so every component reported whatever happened to be
  in the return registers — a different UID on each run. The primary vtable also
  gains the `getUID` override at slot [17], where Itanium places it. Verified on
  a live server: the component now reports the UID from `Cargo.toml` instead of
  a value that changed every start.
- **`ITimersComponent::create` used the wrong overload on Windows.** MSVC emits
  an overload set in **reverse** declaration order, so slot [16] is
  `create(handler, initial, interval, count)` and the three-argument overload
  the SDK calls is [17]. Calling the wrong one passed four arguments' worth of
  cleanup against three arguments pushed, corrupting the stack. Confirmed by
  disassembly: slot [16] ends in `ret 0x18`, slot [17] in `ret 0x10`.
- **`IExtensible::removeExtension` overloads were swapped on Windows**, by the
  same rule. In the vtable the SDK hands the server, slot [2] must be the `UID`
  overload and [3] the pointer one. Under `thiscall` the callee pops the
  arguments, so the previous order popped 4 bytes where the server had pushed 8.
- **`on_tick` never fired for native open.mp components on Linux.** The
  `ITimersComponent` and `ITimer` slot indices were the MSVC ones, used on both
  ABIs. On Itanium every method shifts: the destructor takes two slots (D1 + D0)
  instead of one, and the `getUID()` override from `PROVIDE_UID` sits in the
  primary vtable. So `create(handler, interval, repeating)` is slot **18**, not
  16, and `ITimer::kill()` is **11**, not 10. The SDK was calling
  `TimersComponent::reset()`, which returns non-null, so no warning was logged
  and the plugin believed the timer existed. Slots now confirmed against the
  official `Timers.so` and `Timers.dll` of open.mp 1.5.8.3079, pinned by a
  regression test, and verified on a live server: a component that logged
  nothing before now delivers its callbacks to Pawn. SA-MP (`ProcessTick`) and
  MSVC builds were never affected.
- **`component_name()` / `component_version()` read the wrong slots on Linux**,
  for the same reason: `componentName()` is `[7]` and `componentVersion()` is
  `[9]` on Itanium, against `[6]` and `[8]` on MSVC. Both returned `None`
  instead of the component's data. The correct per-ABI numbers were already in
  `docs/internals/omp-abi.md`; the code disagreed with its own documentation.
- **Calls through server vtables** (`core_print_ln`, `core_log_ln`, the `_u8`
  variants, `component_name`, `component_version`, the repeating timer helpers)
  rebuilt the function pointer from a `usize`, which carries no provenance.
  They now use the `_ptr` helpers below.
- **A `data_only` view panicked instead of failing.** `Amx::data_only` builds a
  view for reading VM memory with no function table, and its documentation says
  that calls needing one "will fail" — they asserted instead, which on an FFI
  boundary means aborting the server. Every such call now returns
  `AmxError::NotFound`. The scenario is a debugger or a paused VM, where a
  plugin holds the `AMX*` without a call context.
- **`component_name()` / `component_version()` used the wrong return
  convention on Linux.** Both call functions returning a small struct, and the
  caller was written for MSVC's hidden pointer on both ABIs. The Itanium ABI
  hands a struct that size back in `EAX:EDX`, so the reads came back empty —
  and the same mistake applied to `IPlayer::getName` crashed a live server,
  which is how it surfaced. Now split per ABI, and verified against a running
  server: the Timers component reports `name="Timers" version=(1, 5, 8)` on
  Linux and on Windows.
- **`Amx::call_native`** rebuilt the native's function pointer by
  `transmute`-ing the `u32` address read from the AMX header. It now goes
  through `std::ptr::with_exposed_provenance`, the sanctioned int-to-pointer
  conversion.

- **`Amx::opcode_table` read the VM's flags before checking it could call into
  it**, so a `data_only` view — which has no function table — flipped a flag on
  the VM and then gave up. Miri caught it in CI as an uninitialized read; on a
  real VM it was a pointless write. The table is resolved first now.

#### Added

- **Encoding losses are reportable.** `encode_checked` returns the encoded bytes
  plus whether the encoding had to substitute characters it cannot represent,
  and `unmappable_chars` names them for the log. `Buffer::write_str_checked` and
  `UnsizedBuffer::write_str_checked` do the same for the write path. Until now a
  Cyrillic name on a Windows-1252 server became `?????` with nothing said
  anywhere — the conversion is lossy by nature, but staying quiet about it was a
  choice the SDK made for the plugin.
- **Encodings beyond the two obvious ones.** `set_default_encoding` always
  accepted any `encoding_rs` encoding, but the module re-exported only
  `WINDOWS_1251` and `WINDOWS_1252`, so the rest were invisible. It now
  re-exports the ones servers actually run — 1250, 1253, 1254, 1256, 1257,
  `ISO_8859_2` and `UTF_8` — with a table naming the communities behind each.
- **`set_default_encoding_by_label("windows-1254")`.** Resolves an encoding from
  the name a configuration file carries, following the WHATWG label rules, so a
  multi-region plugin can read it from the server config instead of compiling it
  in. An unknown label returns `None` and leaves the current encoding alone.
- **`samp_sdk::exports` covers the open.mp-only slots `[44..=51]`**:
  `PushStringLen`, `SetStringLen`, `Swap16/32/64`, `GetNativeByIndex`,
  `MakeAddr` and `StrSize`, with their raw signatures in `samp_sdk::raw`. The
  table SA-MP provides stops at `[43]`, so these are documented as open.mp only
  and are meant to be reached through the gated wrapper in `rust-samp`.
- **`Amx::call_public` — typed calls into Pawn publics.** `amx.call_public("OnPlayerScored", (7, "headshot", 1.5))`,
  with the arguments in a tuple in the same order as the Pawn signature. The
  types carry what `exec_public!` makes the caller mark by hand: `&str` and
  `String` are copied into the AMX heap and arrive as `const arg[]`, `&[i32]` as
  `arg[]`, cell-sized values go straight in. Being a method rather than a macro,
  it works in generic code and behind abstractions. The new `samp_sdk::call`
  module holds the `PublicArg` / `PublicArgs` traits; `exec_public!` stays for
  argument types outside that list.
- **`omp::vtable::vtable_slot_ptr` and `secondary_call_target_ptr`.** Same
  contract as `vtable_slot` / `secondary_call_target`, but the slot is read and
  returned as `*const ()` instead of `usize`, so the function pointer keeps its
  provenance before the caller `transmute`s it to a function type.

- **Text draws, gang zones and actors** join the world module: query the
  component, create one, read its id. The text draw `create` is overloaded, so
  MSVC emits it at [18] against [19]; the other two lose only the destructor
  slot. Created and read back on Linux and Windows.
- **Text labels, menus and spawn classes**, finishing the sweep of components
  that expose a `create`. Text labels overload it three ways (global, per
  player, per vehicle) and MSVC emits the set reversed, so the global one lands
  on [18] in both ABIs — a coincidence of the reversal, not a rule. Spawn
  classes take the thirteen weapon slots the server expects, exposed as
  `WeaponSlot`. All created and read back on both platforms.
- **Nine more `IPlayer` accessors**: money, skin, wanted level, interior,
  weather, drunk level and `setControllable`. Written and read back on both
  platforms — wanted=4, money=250 and skin=46 come back as set; interior stays 0
  because a bot resends its own, the same client-sync story as health and
  armour.
- **Pool iteration, without the hash set.** `pool_bounds` calls
  `IReadOnlyPool<T>::bounds()` and `all_players` walks that id range, asking the
  pool for each one — so listing who is connected costs a call per id and never
  touches `entries()`, whose `robin_hood` layout this SDK refuses to mirror.
  `bounds()` returns a `Pair<size_t, size_t>`: GCC hands those eight bytes back
  in registers, MSVC sees a type with a constructor and uses a hidden pointer,
  and Rust cannot tell the two apart from a `#[repr(C)]` struct — so the MSVC
  side is spelled out. Verified on both platforms with an NPC connected.
- **`samp::omp::extension` reads the per-player extension map.** Components
  attach their data with `addExtension`, which files it in a `robin_hood` flat
  map that the virtual getter never consults — so checkpoints, dialogs, a
  player's objects were unreachable through vtables alone. This walks the map:
  the field offsets come from clang's record layout and are pinned per ABI by
  tests, the hash is `robin_hood`'s own (it specializes for integral keys, so
  `std::hash` and its per-standard-library differences never enter), and the
  reimplementation is checked against values printed by the real thing.
  The lookup fails closed: the probe is bounded by the table size, and a
  candidate is only returned once `getExtensionID()` on it answers with the UID
  that was asked for. Verified live on both platforms — the checkpoint data of a
  connected player is found, and the dialog data correctly is not, because the
  server had not attached it.

  It is the one place in this SDK whose correctness rests on a vendored
  library's internals rather than an ABI, and `check-abi-slots.py` cannot cover
  it. The Pawn natives remain the route that does not depend on any of this.
- **`player_extension`** exposes `IExtensible::getExtension(UID)`, with the
  caveat measured rather than assumed: the stock components attach their
  per-player data with `addExtension`, which files it in a map the virtual
  getter does not consult, so dialogs and checkpoints come back null there. The
  Pawn natives remain the working route for those, through
  `Amx::call_native`. Wrappers that would always fail were dropped rather than
  shipped.
- **What is still out of reach**: reading a pool's `entries()` set directly, and
  the handful of interfaces that only exist per player and expose no `create`.
  Both were reachable enough by other means — `bounds()` for iteration, the
  extension map for per-player data — that nothing needs the `robin_hood`
  container layout mirrored.

#### Changed

- **The open.mp wrappers lost a third of their code.** Every accessor repeated
  the same twelve lines: name the function type per calling convention, adjust
  `this`, read the slot, give up when either is missing. That is now one macro,
  `call_vtable!`, used by about forty wrappers — 577 lines deleted against 265
  added. The behaviour is unchanged and the same servers were run again to say
  so, but "fails closed" is now implemented once instead of forty times, which
  is what makes the null-safety tests below meaningful.

#### Deprecated

- **`omp::vtable::vtable_slot` and `secondary_call_target`.** A function pointer
  rebuilt from a `usize` carries no provenance, and calling it is undefined
  behavior. Both remain as wrappers over the `_ptr` variants and return the same
  address; switch to `vtable_slot_ptr` / `secondary_call_target_ptr`.

#### Tests

- Timer and component slot indices are pinned per ABI, with the dump of the
  official binary named as the source.
- Mock vtables in the `omp::core`, `omp::component_api` and `omp::vtable` tests
  store pointers instead of integers, so Miri can follow them.
- `uid_get_uid_recovers_from_subobject_pointer` derived the `IUIDProvider`
  pointer from the `uid_vtable` field alone, then stepped back to the whole
  object — a Stacked Borrows violation in the test, not in `uid_get_uid`. It now
  derives the pointer from the whole object, as the server does.
- The `AmxString` test helper leaked its backing buffer on purpose; it now hands
  the buffer to the caller, which keeps it alive for the test.

- **A null-safety sweep over the whole open.mp layer**: every public entry point
  is called with a null handle and has to answer with its documented default —
  `None`, zero, a null pointer, `false`, or an empty range. Sixty-odd entry
  points, six tests. A plugin asking about a player who just disconnected is the
  ordinary case for this, not an edge one.

### `rust-samp` (lib `samp`) — 3.5.0

#### Added

- **Native player events, no Pawn in the middle.** `samp::omp::players` reaches
  `ICore::getPlayers()`, then the pool's `IEventDispatcher<PlayerConnectEventHandler>`,
  and registers a handler on it: the server calls the plugin directly for
  connect, disconnect, incoming connection and client init. Until now the only
  way to see a player connect was the `#[event]` detour over `amx_Exec`, which
  sees what the gamemode is told and costs a hook. Slots verified against
  `omp-server` (`ICore::getPlayers` at 8/7, `getPlayerConnectDispatcher` at
  10/9) and the whole path exercised with an NPC on Linux and on Windows.
  `DisconnectReason::from_raw` maps an unknown value to `Custom` rather than
  transmuting it into a variant that does not exist.
- **Spawn, text and damage events**, through the same mechanism:
  `player_spawn_dispatcher`, `player_text_dispatcher` and
  `player_damage_dispatcher` with their handler vtables. `onPlayerRequestSpawn`
  returning `false` denies a spawn and `onPlayerText` returning `false` blocks a
  message, as the server defines. Spawn was exercised with an NPC on both
  platforms; text and damage have their registration verified, and their slots
  pinned by tests and by `scripts/check-abi-slots.py`, but no NPC triggers them.
- **Objects and pickups** (`samp::omp::world`): query the component, create
  one, read its id. Both `create` calls sit at indices the two ABIs disagree
  about (21/19 and 19/17), which is the rule rather than the exception once a
  component overloads anything. `entity_id` reads the id of any entity carrying
  the `IEntity` subobject — object, pickup, vehicle or player. Created and read
  back on both platforms.
- **Entities by id, without touching the pools' hash sets.** `player_by_id` and
  `vehicle_by_id` go through `IReadOnlyPool<T>::get`, a secondary base of each
  pool (offset 40/56 for players, 44/64 for vehicles — the vehicle component
  reaches it through `IComponent`, which carries a `IUIDProvider` of its own).
  Iterating still means mirroring a `robin_hood` hash set, which this does not
  do; answering "who is player 7?" no longer requires it. `player_id` and
  `vehicle_id` come from the shared `IEntity`. Proven live: looking the created
  vehicle and the connected player up by id returns the very pointers the server
  handed over.
- **Vehicle events**: `vehicle_event_dispatcher` plus a fourteen-slot
  `VehicleHandlerVTable` — stream in and out, death, enter and exit, damage,
  paint job, mods, respray, mod shop, spawn, unoccupied updates, trailers and
  sirens. Registration verified on both platforms.
- **More of `IPlayer`**: money, armour, team and skin.
- **`samp::omp::vehicles` — the vehicle component.** Query it by UID, spawn a
  vehicle with `create_vehicle`, then read or set model, health, colours and
  position. `create` is overloaded, so MSVC emits the pair reversed and the
  eight-argument overload lands at [18] there against [19] under Itanium — the
  same shape as the timer defect this release opened with, caught this time by
  `scripts/omp-vtable.py` before a line was written. A vehicle spawned on both
  platforms reports `model=411 health=750 pos=(10.0,20.0,3.0)`.
- **Position and virtual world**, through the `IEntity` subobject the player
  carries: `player_position`, `player_set_position`, `player_virtual_world` and
  `player_set_virtual_world`. `IEntity` is a secondary base, so `this` is
  adjusted by 40 bytes on Itanium and 56 on MSVC — clang's record layout gives
  both — before indexing its own vtable, where the four methods sit at the same
  slots on either ABI. Read back live on Linux and Windows.
- **Reading and acting on an `IPlayer`**: `player_name`, `player_is_bot`,
  `player_kick`, `player_health` / `player_set_health`, `player_score` /
  `player_set_score` and `player_send_message`. Slots derived with
  `scripts/omp-vtable.py` and proven live: writing a score of 1337 reads back
  as 1337 on Linux and on Windows. Health is read-only in practice for a bot —
  the client sends its own on the next sync packet — which is why the
  round-trip test uses the score, a value the server owns.
  Validated live on both platforms — the NPC reports `name="TesteDetour"
  bot=true`. Unlike the component classes, the Windows server carries no RTTI
  for `Player`, so these indices cannot be re-derived from the binary; they are
  pinned by tests and proven by running a server.
- **The remaining player dispatchers**: stream, shot, change, click, check and
  update, closing the eleven `IPlayerPool` exposes. `onPlayerUpdate` fires for
  every player on every tick and was validated on both platforms;
  `onPlayerShot*` and `onPlayerClickMap` carry types the server passes by value
  or by const reference (`Vector3`, `PlayerBulletData`), declared accordingly.
  Every slot is re-derived by `scripts/check-abi-slots.py`, now at 22 checks.
- **`samp::omp_amx::AmxOmpExt` — the AMX functions open.mp adds.**
  `native_by_index`, `make_addr`, `str_size` and the byte-swap helpers, as
  methods on `Amx`. Each one first checks that the SDK read the function table
  from `getAmxFunctions()`, and returns `AmxError::NotFound` otherwise:
  resolving entry 44 of SA-MP's 44-entry table would read past its end and call
  an arbitrary address. A legacy plugin under open.mp is refused for the same
  reason — its table arrives through SA-MP's `Load()` with no stated size.
  `extended_table_available()` reports availability up front.
- **`samp::mainthread` — handing work back to the main thread.** A worker
  thread calls `post(closure)`; the closure runs on the main thread at the next
  tick, which is the only place the AMX VM may be touched. Jobs run in order, a
  job posted during a drain waits for the next tick (so self-posting work cannot
  spin), and a panic inside a job is caught and logged instead of unwinding into
  the server. `pending()` reports the backlog and `run_pending()` drives the
  queue by hand for plugins that do not enable the tick; without a tick and
  without that call, the SDK warns once the backlog passes 10,000. Until now
  every plugin doing HTTP, SMTP or database work had to hand-roll this hop, and
  getting it wrong means touching the VM off-thread.
- **Generated Pawn include.** `#[native]` now derives each native's Pawn
  declaration from its Rust signature, and the plugin can write the `.inc` the
  script side includes: start the server once with `SAMP_PAWN_INCLUDE=path`, or
  call `samp::plugin::pawn_include()` / `write_pawn_include(path)`. Works on
  either server and in either mode. `f32` maps to `Float:`, `bool` to `bool:`,
  `AmxString` to `const arg[]`, `Ref<T>` to `&arg` keeping the tag, and buffers
  to `arg[]`; an unrecognized type falls back to a plain cell, and a `raw`
  native comes back commented out. The include no longer has to be maintained by
  hand, so it cannot drift from the code.

It also re-exports `samp::omp::vtable`, so the new helpers and the two
deprecations reach plugin authors through it, and it now requires
`rust-samp-sdk` 3.5.0 — which is where the `on_tick` fix lives.

### `rust-samp-codegen` (lib `samp_codegen`) — 1.5.0

#### Added

- `#[native]` also emits a hidden accessor with the native's Pawn declaration,
  which `initialize_plugin!` collects for the generated include described under
  `rust-samp`.

### Security

- **`rustls` 0.23.44 → 0.23.45** ([RUSTSEC-2026-0285], via #66). Reaches the
  lockfile through the `sink-demo` example only — `sentry` → `reqwest` →
  `hyper-rustls`. No shipped crate depends on it.

[RUSTSEC-2026-0285]: https://rustsec.org/advisories/RUSTSEC-2026-0285

### Dependencies

- Weekly lockfile refresh across **70 packages** (#64), plus `flate2`
  1.1.9 → 1.1.10 (#57). All transitive or build-time; no manifest requirement
  and no public API changed.
- Docs toolchain: `pymdown-extensions` 11.0.2 → 12.0.1 (#62).
- GitHub Actions: bumps to the `codeql`, `scorecard`, `docs` and `release`
  workflows (#58, #60).

### Internal

- `Runtime::logger()`, which asserted when the server passed no `logprintf` —
  every native Open Multiplayer run — is replaced by `try_logger()`, returning
  `Option`. Internal to the crate (`Runtime` is `pub(crate)`), so no plugin sees
  the change.

### Tooling

- **`scripts/check-abi-slots.py`** re-derives every vtable slot index from the
  official server binaries and fails when the source disagrees. On Linux the
  `.so` files keep their symbols, so each slot is identified by name; on Windows
  it locates vtables through RTTI and identifies methods by the `ret N` of each
  slot — which is exactly what pins the `create` overload pair MSVC emits in
  reverse. It would have caught all four defects above on its own. Not part of
  CI, since it needs servers that cannot be redistributed.

- **Both scripts find their inputs instead of hardcoding them.** The SDK
  checkout comes from `--sdk`, `$OPENMP_SDK` or the usual locations, and the
  servers from `--linux`/`--win`, `$OPENMP_LINUX_SERVER`/`$OPENMP_WIN_SERVER` or
  likewise; a checkout missing its submodules is reported as such rather than
  failing three steps later. The fuzzing seed was regenerated: the previous one
  was compiled from an absolute path, which the AMX debug block stores verbatim.
- **`scripts/omp-vtable.py --rust`** emits the cfg-gated slot constants ready to
  paste, with `--filter` to pick methods. Wrapping the next interface is then
  mechanical: generate the constants, write the call, run a server.
- **`scripts/omp-vtable.py`** asks clang where an interface's methods land, in
  both ABIs at once, by generating a stub and dumping the vtable layout. Until
  now MSVC indices were derived by hand from the ABI rules, because the Windows
  server carries RTTI for three classes only. Run against `IPlayerPool`, it
  reproduces every index this release ships.

### Validation

- **The `#[event]` detour was exercised end to end for the first time.** It
  rewrites `amx_Exec` at runtime through `retour`, which is pinned to an alpha,
  and no test had ever seen a callback actually fire through it. An NPC
  connecting now proves it on all four combinations — SA-MP and open.mp, Linux
  and Windows — with the handler running before the gamemode's public, and
  `EventReturn::Suppress` skipping the public as documented.
- The open.mp interface layer was exercised the same way: player and vehicle
  events firing, entities created and read back, lookups by id returning the
  pointers the server handed over, on both platforms.

### Fuzzing

- **Harness for `AmxDbg::parse`** (`fuzz/`, driven by `cargo-fuzz`). The
  debug block comes from a `.amx` file on the server's disk, which the plugin
  did not produce, so the parser's contract is that no input panics or hangs it.
  A first run of 6.3 million cases over two minutes found nothing — the
  allocation caps already in the parser hold. Seed inputs are versioned;
  `CONTRIBUTING.md` documents the workflow. Out of the workspace: it needs
  nightly and links libFuzzer.

### CI

- **Dependabot now watches transitive Cargo dependencies** (#63), with
  `dependency-type: all` and security updates grouped into a single weekly pull
  request. Before this, a patched version of a crate nobody declares would only
  surface when `cargo audit` failed an unrelated pull request.
- New advisory `miri (i686)` job running `cargo miri test` over the library
  crates on `i686-unknown-linux-gnu`. It reports undefined behaviour — the class
  of bug the FFI layer is exposed to and the regular test run cannot see. Being
  nightly-only, it does not gate the merge.

- The Miri job installs the 32-bit headers its dependencies' build scripts need.
  Without them it failed on `bits/libc-header-start.h`, which reads like a Miri
  problem and is an apt package.

### Docs

- `docs/encoding.md` gains the encoding table, selecting one by label at
  runtime, reporting a lossy conversion, and a section on multi-byte encodings —
  Pawn counts cells, so `strlen` and indexing mean something different there.
- `docs/natives.md` gains the generated-include workflow and the Rust-to-Pawn
  type table.
- `docs/exec-public.md` is reorganized around `Amx::call_public`, with
  `exec_public!` kept for the argument types the method does not take.
- `docs/omp-native.md` gains a section on the extended AMX function table: what
  the eight extra entries are and why the wrapper refuses them outside a native
  component.
- New page **Talking to the Server Directly** (`docs/omp-interfaces.md`): when
  to prefer the direct route over Pawn and when not to, registering event
  handlers, entities, finding players, the extension map and its caveat, and
  what "fails closed" covers.
- The README says the toolkit can talk to the server directly, and its feature
  list no longer claims `encoding` is limited to two code pages.
- New page **Background Work and the Main Thread** (`docs/threads.md`) with the
  worker-thread pattern, the draining rules and the guarantees.
- `docs/internals/omp-abi.md` gains the per-ABI slot tables for
  `ITimersComponent` and `ITimer`, and states that a wrong slot fails silently.
  The `vtable` helper table now lists the `_ptr` variants.

## [v3.6.0-rc.2] — 2026/09/25

> Kept as published: this entry lists only what the second candidate added on
> top of the first. The whole release is consolidated under `v3.6.0`.

Second candidate. It finishes the two categories rc.1 could not reach —
iterating a pool and reading a player's extensions — adds the remaining
components, and then spends the rest of its time on assurance rather than
surface: a null-safety sweep over the whole open.mp layer, and a refactor that
removed a third of it.

### `rust-samp-sdk` (lib `samp_sdk`) — 3.5.0-rc.2

#### Changed

- **The open.mp wrappers lost a third of their code.** Every accessor repeated
  the same twelve lines: name the function type per calling convention, adjust
  `this`, read the slot, give up when either is missing. That is now one macro,
  `call_vtable!`, used by about forty wrappers — 577 lines deleted against 265
  added. The behaviour is unchanged and the same servers were run again to say
  so, but "fails closed" is now implemented once instead of forty times, which
  is what makes the null-safety tests below meaningful.

#### Tests

- **A null-safety sweep over the whole open.mp layer**: every public entry point
  is called with a null handle and has to answer with its documented default —
  `None`, zero, a null pointer, `false`, or an empty range. Sixty-odd entry
  points, six tests. A plugin asking about a player who just disconnected is the
  ordinary case for this, not an edge one.

#### Fixed

- **`Amx::opcode_table` read the VM's flags before checking it could call into
  it**, so a `data_only` view — which has no function table — flipped a flag on
  the VM and then gave up. Miri caught it in CI as an uninitialized read; on a
  real VM it was a pointless write. The table is resolved first now.

#### Added

- **Text draws, gang zones and actors** join the world module: query the
  component, create one, read its id. The text draw `create` is overloaded, so
  MSVC emits it at [18] against [19]; the other two lose only the destructor
  slot. Created and read back on Linux and Windows.
- **Text labels, menus and spawn classes**, finishing the sweep of components
  that expose a `create`. Text labels overload it three ways (global, per
  player, per vehicle) and MSVC emits the set reversed, so the global one lands
  on [18] in both ABIs — a coincidence of the reversal, not a rule. Spawn
  classes take the thirteen weapon slots the server expects, exposed as
  `WeaponSlot`. All created and read back on both platforms.
- **Nine more `IPlayer` accessors**: money, skin, wanted level, interior,
  weather, drunk level and `setControllable`. Written and read back on both
  platforms — wanted=4, money=250 and skin=46 come back as set; interior stays 0
  because a bot resends its own, the same client-sync story as health and
  armour.
- **Pool iteration, without the hash set.** `pool_bounds` calls
  `IReadOnlyPool<T>::bounds()` and `all_players` walks that id range, asking the
  pool for each one — so listing who is connected costs a call per id and never
  touches `entries()`, whose `robin_hood` layout this SDK refuses to mirror.
  `bounds()` returns a `Pair<size_t, size_t>`: GCC hands those eight bytes back
  in registers, MSVC sees a type with a constructor and uses a hidden pointer,
  and Rust cannot tell the two apart from a `#[repr(C)]` struct — so the MSVC
  side is spelled out. Verified on both platforms with an NPC connected.
- **`samp::omp::extension` reads the per-player extension map.** Components
  attach their data with `addExtension`, which files it in a `robin_hood` flat
  map that the virtual getter never consults — so checkpoints, dialogs, a
  player's objects were unreachable through vtables alone. This walks the map:
  the field offsets come from clang's record layout and are pinned per ABI by
  tests, the hash is `robin_hood`'s own (it specializes for integral keys, so
  `std::hash` and its per-standard-library differences never enter), and the
  reimplementation is checked against values printed by the real thing.
  The lookup fails closed: the probe is bounded by the table size, and a
  candidate is only returned once `getExtensionID()` on it answers with the UID
  that was asked for. Verified live on both platforms — the checkpoint data of a
  connected player is found, and the dialog data correctly is not, because the
  server had not attached it.

  It is the one place in this SDK whose correctness rests on a vendored
  library's internals rather than an ABI, and `check-abi-slots.py` cannot cover
  it. The Pawn natives remain the route that does not depend on any of this.
- **`player_extension`** exposes `IExtensible::getExtension(UID)`, with the
  caveat measured rather than assumed: the stock components attach their
  per-player data with `addExtension`, which files it in a map the virtual
  getter does not consult, so dialogs and checkpoints come back null there. The
  Pawn natives remain the working route for those, through
  `Amx::call_native`. Wrappers that would always fail were dropped rather than
  shipped.
- **What is deliberately not wrapped**: the per-player interfaces (dialogs,
  checkpoints, menus shown to a player) are not components with a `create` —
  they are extensions queried off an `IPlayer`, a different shape that deserves
  its own pass. Pool iteration likewise stays out: it means mirroring a
  `robin_hood` hash set, while `player_by_id` and `vehicle_by_id` cover the
  question that iteration was usually asked for.

### CI

- The Miri job installs the 32-bit headers its dependencies' build scripts need.
  Without them it failed on `bits/libc-header-start.h`, which reads like a Miri
  problem and is an apt package.

## [v3.6.0-rc.1] — 2026/09/25

> Kept as published. The content also appears, consolidated, under `v3.6.0`.

**Release candidate.** Everything here is implemented, tested and exercised
against real servers on Linux and Windows, but none of it has run on a public
server with players. The release is large — it fixes four ABI defects and adds
the whole open.mp interface layer — so it goes out as a candidate first, to be
validated in the field before it becomes v3.6.0.

Cargo does not resolve a pre-release from an ordinary requirement, so
`rust-samp = "3"` keeps pointing at 3.4.0. Trying the candidate is explicit:

```toml
samp = { package = "rust-samp", version = "3.5.0-rc.1" }
```

What would make it final: the open.mp interface wrappers used by a plugin that
ships, and no ABI correction needed in the process. Report anything that
misbehaves — a wrong vtable slot shows up as a value that makes no sense on
Linux and as a crash on Windows.

Correctness release, in two parts.

**Bug fixes:** several vtable layouts were wrong for native open.mp components.
`SampPlugin::on_tick` never fired on Linux, `getUID()` returned a different value
on every run, and on Windows the timer call and `removeExtension` corrupted the
stack. Every index is now verified against the official server binaries. Anyone
shipping a Rust component under open.mp wants this release.

**Soundness:** the SDK now runs clean under
[Miri](https://github.com/rust-lang/miri) on `i686-unknown-linux-gnu`, with the
default (strict) provenance and Stacked Borrows checks. That part changes no
behavior on a real server — it removes undefined behavior the compiler was free
to exploit, not crashes observed in the field.

Beyond the fixes, the release adds what plugin authors had to hand-roll: the
generated Pawn include, a queue back to the main thread, typed calls into Pawn
publics, and encodings that stop losing characters quietly.

The per-crate sections come first, then the ones belonging to the repository
rather than to any published crate.

### `rust-samp-sdk` (lib `samp_sdk`) — 3.5.0-rc.1

Additive public API plus two deprecations.

#### Fixed

- **`getUID()` returned garbage on Linux.** The secondary `IUIDProvider` vtable
  the SDK hands the server carried two destructor thunks before `getUID`. It has
  none: `IUIDProvider` declares no virtual destructor, so the secondary vtable
  holds a single slot, on Itanium exactly as on MSVC. The server called slot [0]
  and got the no-op thunk, so every component reported whatever happened to be
  in the return registers — a different UID on each run. The primary vtable also
  gains the `getUID` override at slot [17], where Itanium places it. Verified on
  a live server: the component now reports the UID from `Cargo.toml` instead of
  a value that changed every start.
- **`ITimersComponent::create` used the wrong overload on Windows.** MSVC emits
  an overload set in **reverse** declaration order, so slot [16] is
  `create(handler, initial, interval, count)` and the three-argument overload
  the SDK calls is [17]. Calling the wrong one passed four arguments' worth of
  cleanup against three arguments pushed, corrupting the stack. Confirmed by
  disassembly: slot [16] ends in `ret 0x18`, slot [17] in `ret 0x10`.
- **`IExtensible::removeExtension` overloads were swapped on Windows**, by the
  same rule. In the vtable the SDK hands the server, slot [2] must be the `UID`
  overload and [3] the pointer one. Under `thiscall` the callee pops the
  arguments, so the previous order popped 4 bytes where the server had pushed 8.
- **`on_tick` never fired for native open.mp components on Linux.** The
  `ITimersComponent` and `ITimer` slot indices were the MSVC ones, used on both
  ABIs. On Itanium every method shifts: the destructor takes two slots (D1 + D0)
  instead of one, and the `getUID()` override from `PROVIDE_UID` sits in the
  primary vtable. So `create(handler, interval, repeating)` is slot **18**, not
  16, and `ITimer::kill()` is **11**, not 10. The SDK was calling
  `TimersComponent::reset()`, which returns non-null, so no warning was logged
  and the plugin believed the timer existed. Slots now confirmed against the
  official `Timers.so` and `Timers.dll` of open.mp 1.5.8.3079, pinned by a
  regression test, and verified on a live server: a component that logged
  nothing before now delivers its callbacks to Pawn. SA-MP (`ProcessTick`) and
  MSVC builds were never affected.
- **`component_name()` / `component_version()` read the wrong slots on Linux**,
  for the same reason: `componentName()` is `[7]` and `componentVersion()` is
  `[9]` on Itanium, against `[6]` and `[8]` on MSVC. Both returned `None`
  instead of the component's data. The correct per-ABI numbers were already in
  `docs/internals/omp-abi.md`; the code disagreed with its own documentation.
- **Calls through server vtables** (`core_print_ln`, `core_log_ln`, the `_u8`
  variants, `component_name`, `component_version`, the repeating timer helpers)
  rebuilt the function pointer from a `usize`, which carries no provenance.
  They now use the `_ptr` helpers below.
- **A `data_only` view panicked instead of failing.** `Amx::data_only` builds a
  view for reading VM memory with no function table, and its documentation says
  that calls needing one "will fail" — they asserted instead, which on an FFI
  boundary means aborting the server. Every such call now returns
  `AmxError::NotFound`. The scenario is a debugger or a paused VM, where a
  plugin holds the `AMX*` without a call context.
- **`component_name()` / `component_version()` used the wrong return
  convention on Linux.** Both call functions returning a small struct, and the
  caller was written for MSVC's hidden pointer on both ABIs. The Itanium ABI
  hands a struct that size back in `EAX:EDX`, so the reads came back empty —
  and the same mistake applied to `IPlayer::getName` crashed a live server,
  which is how it surfaced. Now split per ABI, and verified against a running
  server: the Timers component reports `name="Timers" version=(1, 5, 8)` on
  Linux and on Windows.
- **`Amx::call_native`** rebuilt the native's function pointer by
  `transmute`-ing the `u32` address read from the AMX header. It now goes
  through `std::ptr::with_exposed_provenance`, the sanctioned int-to-pointer
  conversion.

#### Added

- **Encoding losses are reportable.** `encode_checked` returns the encoded bytes
  plus whether the encoding had to substitute characters it cannot represent,
  and `unmappable_chars` names them for the log. `Buffer::write_str_checked` and
  `UnsizedBuffer::write_str_checked` do the same for the write path. Until now a
  Cyrillic name on a Windows-1252 server became `?????` with nothing said
  anywhere — the conversion is lossy by nature, but staying quiet about it was a
  choice the SDK made for the plugin.
- **Encodings beyond the two obvious ones.** `set_default_encoding` always
  accepted any `encoding_rs` encoding, but the module re-exported only
  `WINDOWS_1251` and `WINDOWS_1252`, so the rest were invisible. It now
  re-exports the ones servers actually run — 1250, 1253, 1254, 1256, 1257,
  `ISO_8859_2` and `UTF_8` — with a table naming the communities behind each.
- **`set_default_encoding_by_label("windows-1254")`.** Resolves an encoding from
  the name a configuration file carries, following the WHATWG label rules, so a
  multi-region plugin can read it from the server config instead of compiling it
  in. An unknown label returns `None` and leaves the current encoding alone.
- **`samp_sdk::exports` covers the open.mp-only slots `[44..=51]`**:
  `PushStringLen`, `SetStringLen`, `Swap16/32/64`, `GetNativeByIndex`,
  `MakeAddr` and `StrSize`, with their raw signatures in `samp_sdk::raw`. The
  table SA-MP provides stops at `[43]`, so these are documented as open.mp only
  and are meant to be reached through the gated wrapper in `rust-samp`.
- **`Amx::call_public` — typed calls into Pawn publics.** `amx.call_public("OnPlayerScored", (7, "headshot", 1.5))`,
  with the arguments in a tuple in the same order as the Pawn signature. The
  types carry what `exec_public!` makes the caller mark by hand: `&str` and
  `String` are copied into the AMX heap and arrive as `const arg[]`, `&[i32]` as
  `arg[]`, cell-sized values go straight in. Being a method rather than a macro,
  it works in generic code and behind abstractions. The new `samp_sdk::call`
  module holds the `PublicArg` / `PublicArgs` traits; `exec_public!` stays for
  argument types outside that list.
- **`omp::vtable::vtable_slot_ptr` and `secondary_call_target_ptr`.** Same
  contract as `vtable_slot` / `secondary_call_target`, but the slot is read and
  returned as `*const ()` instead of `usize`, so the function pointer keeps its
  provenance before the caller `transmute`s it to a function type.

#### Deprecated

- **`omp::vtable::vtable_slot` and `secondary_call_target`.** A function pointer
  rebuilt from a `usize` carries no provenance, and calling it is undefined
  behavior. Both remain as wrappers over the `_ptr` variants and return the same
  address; switch to `vtable_slot_ptr` / `secondary_call_target_ptr`.

#### Tests

- Timer and component slot indices are pinned per ABI, with the dump of the
  official binary named as the source.
- Mock vtables in the `omp::core`, `omp::component_api` and `omp::vtable` tests
  store pointers instead of integers, so Miri can follow them.
- `uid_get_uid_recovers_from_subobject_pointer` derived the `IUIDProvider`
  pointer from the `uid_vtable` field alone, then stepped back to the whole
  object — a Stacked Borrows violation in the test, not in `uid_get_uid`. It now
  derives the pointer from the whole object, as the server does.
- The `AmxString` test helper leaked its backing buffer on purpose; it now hands
  the buffer to the caller, which keeps it alive for the test.

### `rust-samp` (lib `samp`) — 3.5.0-rc.1

#### Added

- **Native player events, no Pawn in the middle.** `samp::omp::players` reaches
  `ICore::getPlayers()`, then the pool's `IEventDispatcher<PlayerConnectEventHandler>`,
  and registers a handler on it: the server calls the plugin directly for
  connect, disconnect, incoming connection and client init. Until now the only
  way to see a player connect was the `#[event]` detour over `amx_Exec`, which
  sees what the gamemode is told and costs a hook. Slots verified against
  `omp-server` (`ICore::getPlayers` at 8/7, `getPlayerConnectDispatcher` at
  10/9) and the whole path exercised with an NPC on Linux and on Windows.
  `DisconnectReason::from_raw` maps an unknown value to `Custom` rather than
  transmuting it into a variant that does not exist.
- **Spawn, text and damage events**, through the same mechanism:
  `player_spawn_dispatcher`, `player_text_dispatcher` and
  `player_damage_dispatcher` with their handler vtables. `onPlayerRequestSpawn`
  returning `false` denies a spawn and `onPlayerText` returning `false` blocks a
  message, as the server defines. Spawn was exercised with an NPC on both
  platforms; text and damage have their registration verified, and their slots
  pinned by tests and by `scripts/check-abi-slots.py`, but no NPC triggers them.
- **Objects and pickups** (`samp::omp::world`): query the component, create
  one, read its id. Both `create` calls sit at indices the two ABIs disagree
  about (21/19 and 19/17), which is the rule rather than the exception once a
  component overloads anything. `entity_id` reads the id of any entity carrying
  the `IEntity` subobject — object, pickup, vehicle or player. Created and read
  back on both platforms.
- **Entities by id, without touching the pools' hash sets.** `player_by_id` and
  `vehicle_by_id` go through `IReadOnlyPool<T>::get`, a secondary base of each
  pool (offset 40/56 for players, 44/64 for vehicles — the vehicle component
  reaches it through `IComponent`, which carries a `IUIDProvider` of its own).
  Iterating still means mirroring a `robin_hood` hash set, which this does not
  do; answering "who is player 7?" no longer requires it. `player_id` and
  `vehicle_id` come from the shared `IEntity`. Proven live: looking the created
  vehicle and the connected player up by id returns the very pointers the server
  handed over.
- **Vehicle events**: `vehicle_event_dispatcher` plus a fourteen-slot
  `VehicleHandlerVTable` — stream in and out, death, enter and exit, damage,
  paint job, mods, respray, mod shop, spawn, unoccupied updates, trailers and
  sirens. Registration verified on both platforms.
- **More of `IPlayer`**: money, armour, team and skin.
- **`samp::omp::vehicles` — the vehicle component.** Query it by UID, spawn a
  vehicle with `create_vehicle`, then read or set model, health, colours and
  position. `create` is overloaded, so MSVC emits the pair reversed and the
  eight-argument overload lands at [18] there against [19] under Itanium — the
  same shape as the timer defect this release opened with, caught this time by
  `scripts/omp-vtable.py` before a line was written. A vehicle spawned on both
  platforms reports `model=411 health=750 pos=(10.0,20.0,3.0)`.
- **Position and virtual world**, through the `IEntity` subobject the player
  carries: `player_position`, `player_set_position`, `player_virtual_world` and
  `player_set_virtual_world`. `IEntity` is a secondary base, so `this` is
  adjusted by 40 bytes on Itanium and 56 on MSVC — clang's record layout gives
  both — before indexing its own vtable, where the four methods sit at the same
  slots on either ABI. Read back live on Linux and Windows.
- **Reading and acting on an `IPlayer`**: `player_name`, `player_is_bot`,
  `player_kick`, `player_health` / `player_set_health`, `player_score` /
  `player_set_score` and `player_send_message`. Slots derived with
  `scripts/omp-vtable.py` and proven live: writing a score of 1337 reads back
  as 1337 on Linux and on Windows. Health is read-only in practice for a bot —
  the client sends its own on the next sync packet — which is why the
  round-trip test uses the score, a value the server owns.
  Validated live on both platforms — the NPC reports `name="TesteDetour"
  bot=true`. Unlike the component classes, the Windows server carries no RTTI
  for `Player`, so these indices cannot be re-derived from the binary; they are
  pinned by tests and proven by running a server.
- **The remaining player dispatchers**: stream, shot, change, click, check and
  update, closing the eleven `IPlayerPool` exposes. `onPlayerUpdate` fires for
  every player on every tick and was validated on both platforms;
  `onPlayerShot*` and `onPlayerClickMap` carry types the server passes by value
  or by const reference (`Vector3`, `PlayerBulletData`), declared accordingly.
  Every slot is re-derived by `scripts/check-abi-slots.py`, now at 22 checks.
- **`samp::omp_amx::AmxOmpExt` — the AMX functions open.mp adds.**
  `native_by_index`, `make_addr`, `str_size` and the byte-swap helpers, as
  methods on `Amx`. Each one first checks that the SDK read the function table
  from `getAmxFunctions()`, and returns `AmxError::NotFound` otherwise:
  resolving entry 44 of SA-MP's 44-entry table would read past its end and call
  an arbitrary address. A legacy plugin under open.mp is refused for the same
  reason — its table arrives through SA-MP's `Load()` with no stated size.
  `extended_table_available()` reports availability up front.
- **`samp::mainthread` — handing work back to the main thread.** A worker
  thread calls `post(closure)`; the closure runs on the main thread at the next
  tick, which is the only place the AMX VM may be touched. Jobs run in order, a
  job posted during a drain waits for the next tick (so self-posting work cannot
  spin), and a panic inside a job is caught and logged instead of unwinding into
  the server. `pending()` reports the backlog and `run_pending()` drives the
  queue by hand for plugins that do not enable the tick; without a tick and
  without that call, the SDK warns once the backlog passes 10,000. Until now
  every plugin doing HTTP, SMTP or database work had to hand-roll this hop, and
  getting it wrong means touching the VM off-thread.
- **Generated Pawn include.** `#[native]` now derives each native's Pawn
  declaration from its Rust signature, and the plugin can write the `.inc` the
  script side includes: start the server once with `SAMP_PAWN_INCLUDE=path`, or
  call `samp::plugin::pawn_include()` / `write_pawn_include(path)`. Works on
  either server and in either mode. `f32` maps to `Float:`, `bool` to `bool:`,
  `AmxString` to `const arg[]`, `Ref<T>` to `&arg` keeping the tag, and buffers
  to `arg[]`; an unrecognized type falls back to a plain cell, and a `raw`
  native comes back commented out. The include no longer has to be maintained by
  hand, so it cannot drift from the code.

It also re-exports `samp::omp::vtable`, so the new helpers and the two
deprecations reach plugin authors through it, and it now requires
`rust-samp-sdk` 3.5.0-rc.1 — which is where the `on_tick` fix lives.

### `rust-samp-codegen` (lib `samp_codegen`) — 1.5.0-rc.1

#### Added

- `#[native]` also emits a hidden accessor with the native's Pawn declaration,
  which `initialize_plugin!` collects for the generated include described under
  `rust-samp`.

### Security

- **`rustls` 0.23.44 → 0.23.45** ([RUSTSEC-2026-0285], via #66). Reaches the
  lockfile through the `sink-demo` example only — `sentry` → `reqwest` →
  `hyper-rustls`. No shipped crate depends on it.

[RUSTSEC-2026-0285]: https://rustsec.org/advisories/RUSTSEC-2026-0285

### Dependencies

- Weekly lockfile refresh across **70 packages** (#64), plus `flate2`
  1.1.9 → 1.1.10 (#57). All transitive or build-time; no manifest requirement
  and no public API changed.
- Docs toolchain: `pymdown-extensions` 11.0.2 → 12.0.1 (#62).
- GitHub Actions: bumps to the `codeql`, `scorecard`, `docs` and `release`
  workflows (#58, #60).

### Internal

- `Runtime::logger()`, which asserted when the server passed no `logprintf` —
  every native Open Multiplayer run — is replaced by `try_logger()`, returning
  `Option`. Internal to the crate (`Runtime` is `pub(crate)`), so no plugin sees
  the change.

### Tooling

- **`scripts/check-abi-slots.py`** re-derives every vtable slot index from the
  official server binaries and fails when the source disagrees. On Linux the
  `.so` files keep their symbols, so each slot is identified by name; on Windows
  it locates vtables through RTTI and identifies methods by the `ret N` of each
  slot — which is exactly what pins the `create` overload pair MSVC emits in
  reverse. It would have caught all four defects above on its own. Not part of
  CI, since it needs servers that cannot be redistributed.

- **Both scripts find their inputs instead of hardcoding them.** The SDK
  checkout comes from `--sdk`, `$OPENMP_SDK` or the usual locations, and the
  servers from `--linux`/`--win`, `$OPENMP_LINUX_SERVER`/`$OPENMP_WIN_SERVER` or
  likewise; a checkout missing its submodules is reported as such rather than
  failing three steps later. The fuzzing seed was regenerated: the previous one
  was compiled from an absolute path, which the AMX debug block stores verbatim.
- **`scripts/omp-vtable.py --rust`** emits the cfg-gated slot constants ready to
  paste, with `--filter` to pick methods. Wrapping the next interface is then
  mechanical: generate the constants, write the call, run a server.
- **`scripts/omp-vtable.py`** asks clang where an interface's methods land, in
  both ABIs at once, by generating a stub and dumping the vtable layout. Until
  now MSVC indices were derived by hand from the ABI rules, because the Windows
  server carries RTTI for three classes only. Run against `IPlayerPool`, it
  reproduces every index this release ships.

### Validation

- **The `#[event]` detour was exercised end to end for the first time.** It
  rewrites `amx_Exec` at runtime through `retour`, which is pinned to an alpha,
  and no test had ever seen a callback actually fire through it. An NPC
  connecting now proves it on all four combinations — SA-MP and open.mp, Linux
  and Windows — with the handler running before the gamemode's public, and
  `EventReturn::Suppress` skipping the public as documented.
- The open.mp interface layer was exercised the same way: player and vehicle
  events firing, entities created and read back, lookups by id returning the
  pointers the server handed over, on both platforms.

### Fuzzing

- **Harness for `AmxDbg::parse`** (`fuzz/`, driven by `cargo-fuzz`). The
  debug block comes from a `.amx` file on the server's disk, which the plugin
  did not produce, so the parser's contract is that no input panics or hangs it.
  A first run of 6.3 million cases over two minutes found nothing — the
  allocation caps already in the parser hold. Seed inputs are versioned;
  `CONTRIBUTING.md` documents the workflow. Out of the workspace: it needs
  nightly and links libFuzzer.

### CI

- **Dependabot now watches transitive Cargo dependencies** (#63), with
  `dependency-type: all` and security updates grouped into a single weekly pull
  request. Before this, a patched version of a crate nobody declares would only
  surface when `cargo audit` failed an unrelated pull request.
- New advisory `miri (i686)` job running `cargo miri test` over the library
  crates on `i686-unknown-linux-gnu`. It reports undefined behaviour — the class
  of bug the FFI layer is exposed to and the regular test run cannot see. Being
  nightly-only, it does not gate the merge.

### Docs

- `docs/encoding.md` gains the encoding table, selecting one by label at
  runtime, reporting a lossy conversion, and a section on multi-byte encodings —
  Pawn counts cells, so `strlen` and indexing mean something different there.
- `docs/natives.md` gains the generated-include workflow and the Rust-to-Pawn
  type table.
- `docs/exec-public.md` is reorganized around `Amx::call_public`, with
  `exec_public!` kept for the argument types the method does not take.
- `docs/omp-native.md` gains a section on the extended AMX function table: what
  the eight extra entries are and why the wrapper refuses them outside a native
  component.
- New page **Background Work and the Main Thread** (`docs/threads.md`) with the
  worker-thread pattern, the draining rules and the guarantees.
- `docs/internals/omp-abi.md` gains the per-ABI slot tables for
  `ITimersComponent` and `ITimer`, and states that a wrong slot fails silently.
  The `vtable` helper table now lists the `_ptr` variants.

## [v3.5.0] — 2026/09/01

Feature release for **debugger tooling**: the SDK now carries the AMX facts a
debugger had to reimplement — opcode numbering and the computed-goto decoder,
call-stack walking, and range reads of the data segment. Extracted from the
[PawnPro Debugger](https://github.com/NullSablex/PawnPro-Debugger), where each
piece was already running against live SA-MP and open.mp servers.

### Added

- **`samp::debug::opcode` — AMX opcode numbering and instruction sizes.** The
  opcode constants (`OP_SDIV`, `OP_BOUNDS`, `OP_BREAK`, `OP_PROC`, `OP_CALL`,
  the load/store and stack/heap opcodes…), `OP_NUM_OPCODES`, the VM's
  `STK_MARGIN`, and `operand_cells(op)` — how many inline operand cells an
  instruction carries, so a scanner can step to the next one. It returns `None`
  for a variable-length instruction (`casetbl`) or an out-of-range opcode: the
  signal to stop scanning rather than guess. Numbering follows the opcode enum
  in `amx.c`, identical on SA-MP and open.mp.
- **`OpcodeMap` — decoding a relocated code segment.** Inverts the VM's dispatch
  table (label address → opcode) to undo the computed-goto rewrite the loader
  applies on GCC/Clang builds, so `read_code` values become real opcodes.
  `is_identity()` reports a non-relocated image, where code values are already
  opcode numbers. `Amx::opcode_map()` builds one straight from a VM; consumers
  no longer hand-roll the `HashMap` the docs used to show.
- **`samp::debug::stack::walk` — call-stack walking.** Follows the AMX frame
  chain (`[frm]` = the caller's FRM, `[frm + CELL]` = the return address) and
  returns the `(cip, frm)` of every frame, top first. It takes an injected cell
  reader, so it is unit-testable against a fake memory map and usable
  host-side; `MAX_DEPTH` caps it so a corrupted stack cannot spin a debug hook.
  `Amx::call_stack(top_cip)` is the wired-up version.
- **`Amx::read_cells` / `Amx::read_bytes` — range reads.** Read consecutive
  cells, or raw bytes for a hex view, with the same `amx_GetAddr` bounds
  checking as `read_cell`. Both stop early at the first inaccessible address
  and return what they read — the natural case at the end of the data segment —
  returning `None` only when the start itself is inaccessible. `read_bytes`
  needs no alignment: it starts at the enclosing cell and trims. Unlike the
  `get_ref`-based `Buffer`/`AmxString` path, they need no function table, so
  they work inside a debug hook.
- **`Amx::data_only(ptr)`.** Wraps a raw `*mut AMX` for data-side access only —
  registers, cell reads/writes, code reads — stating the intent instead of
  passing a bare `0` as the function table, the usual situation while a VM is
  paused.
- **`Amx::hlw()` — heap low-water mark** (#54). The last VM register the accessors
  were missing (`hlw` is where the heap starts, below which a release is a
  `AMX_ERR_HEAPLOW` underflow). Reading it is what lets a debugger predict that
  error the way `amx.c`'s `CHKHEAP` raises it.
- **`AmxDbg::function_address`.** Resolves a function name to its entry address
  in the code segment, the missing half of `lookup_function` and what a
  debugger needs to place a breakpoint on a function by name.

### Docs

- [VM Debugging](docs/vm-debugging.md) rewritten around the new API: the
  hand-rolled inverse-`HashMap` example is replaced by `opcode_map()`, and the
  page gains sections on opcode numbers/instruction sizes and on walking the
  call stack.
- [API reference](docs/api-reference.md) updated with the new `Amx` methods and
  the widened `samp::debug` surface.
- README: the `debug` feature is no longer described as only a parser, and the
  badge row gains crates.io version and downloads, docs.rs, MSRV and stars.
- The v3.4.0 entry dropped an `@`-mention that could read as crediting a
  contribution that was not one (#36).

### Changed

- **CI: CodeQL migrated from default to advanced setup** (#52). The default
  setup only analysed a pull request that touched files relevant to the
  configured languages, so a docs-only PR produced no analysis at all while
  `master` still carried one per language — leaving the `code_scanning` branch
  rule unable to diff the two sides. The workflow runs with no path filter, so
  both configurations (`actions`, `rust`) exist on every PR.
- **CI: a stable `ci-status` gate** (#51) is now the required status check, with
  a companion skip workflow, so the required check reports on every PR
  regardless of which jobs the path filters select.
- **Dependabot updates grouped into a single PR per ecosystem** (#44), across
  `cargo`, `github-actions` and `pip`, cutting the PR noise from one per
  dependency.
- **Dependency updates.** `time` 0.3.54 → 0.3.55 (#37), `sentry` 0.49.0 →
  0.49.1 (#38) then 0.49.2 with `log` and `syn` in the cargo group (#48),
  `memcache` 0.20.0 → 0.21.0 (#49); GitHub Actions bumps for
  `codeql-action/upload-sarif` (#39, #42, #46), `Swatinem/rust-cache` (#41,
  #43) and the actions group (#53); docs toolchain bumps for
  `mkdocs-material` (#45) and `pymdown-extensions` (#47). None of these touch
  the shipped public API — `sentry` and `memcache` only reach the examples.

### Crate versions

- `rust-samp` (lib `samp`): 3.3.0 → 3.4.0 (re-exports the widened `samp::debug`
  surface and the new `Amx` methods; requires `rust-samp-sdk` 3.4.0).
- `rust-samp-sdk` (lib `samp_sdk`): 3.3.0 → 3.4.0 (additive public API: the
  `opcode`/`stack` modules, the range readers, `data_only`, `function_address`).
- `rust-samp-codegen` (lib `samp_codegen`): 1.4.0 — unchanged (no macro
  changes).

## [v3.4.0] — 2026/08/05

Feature release: **`#[event]`** — write Pawn callback handlers (observers, or
handlers that cancel the callback) directly in Rust, the missing half for
building gamemodes rather than only plugins. Ships alongside
`Amx::exec_public_scope` for output-array callbacks and a round of buffer/stack
hardening across the FFI boundary. Event delivery and the hardening were verified
end-to-end on a live SA-MP server.

### Added

- **`#[event]` — Pawn callback handlers.** Observe gamemode callbacks
  (`OnPlayerConnect`, `OnPlayerSpawn`, …) directly in Rust, the missing half for
  writing gamemodes rather than only plugins. Mark a method
  `#[event(name = "OnPlayerConnect")]` and register it via the new
  `initialize_plugin!(events: [...])` list; arguments are marshalled exactly like
  `#[native]`. Under the hood the SDK detours the VM's `amx_Exec` (via `retour`)
  and dispatches each public into the matching handlers before the gamemode's own
  public runs. The detour is installed lazily — plugins with no events never
  touch `amx_Exec`.
  - Handlers are **observers** by default (return `AmxResult<T>` / `T`, value
    ignored, the public runs). A handler that returns `EventReturn` can instead
    **cancel** the callback (`EventReturn::Suppress(value)` skips the gamemode's
    public and returns `value`; `EventReturn::suppress(v)` encodes a typed
    `f32`/`bool`/int for `Float:`/`bool:` callbacks). Dispatch is O(1) per public
    (keyed by `(amx, index)`), reentrancy-guarded (a handler re-entering the same
    public runs it directly instead of recursing), and de-duplicated per AMX.
  - **`#[event(name = "…", raw)]`** hands the handler the `Args` cursor
    (`fn(&mut self, amx: &Amx, args: &mut Args) -> EventReturn`) for variadic or
    protocol-specific callbacks, mirroring `#[native(raw)]`.
  - Verified end-to-end on a live SA-MP server (arg order for int/multi-arg/
    string, observer vs suppression, reentrancy, panic isolation). The same
    detour drives native open.mp, but that path has **not** been validated on a
    live open.mp server yet.
  - The detour is **x86/x86_64 only** (the arches SA-MP/open.mp run on); the
    `retour` dependency and dispatch code are scoped accordingly, so the aarch64
    check job still builds with events as a no-op.
  - `retour` is pinned to `=0.4.0-alpha.4` — the only release line that compiles
    on the stable channel (0.3.x needs nightly). Revisit when a stable `0.4` ships.
  - Inspired by an API proposal in the upstream `samp-rs` project
    ([PR #29](https://github.com/zottce/samp-rs/pull/29), issue #3). The macro
    surface follows that proposal; the implementation — including the `amx_Exec`
    detour and stack marshalling the proposal left untested — was written and
    validated here from scratch. No code from that proposal is used, and it is
    not a contribution to this repository.
- **`Amx::exec_public_scope`** — calls a public inside a managed `Allocator`
  scope, the escape hatch for callbacks with **output arrays** (which the
  input-only `exec_public!` macro cannot express): allocate buffers, push args,
  `exec`, and read outputs back before the scope frees them. Validated on a live
  SA-MP server.

### Tests

- **Property/fuzz tests for the marshalling boundary** (dependency-free,
  deterministic): `AmxString` decoding is total (no panic/overrun) for random
  cells, corrupted lengths and non-UTF8 bytes; `Buffer::get_as`/`set_as` stay in
  bounds for any index; `into_sized_buffer` length is exactly
  `min(requested, segment, 1 MiB)`; `Args::count` is never negative/absurd.

### Hardened

- **`UnsizedBuffer::into_sized_buffer` clamps the requested size to the VM data
  region `[0, stp)`.** A native that passes a `size` larger than the real Pawn
  array (e.g. a corrupted or attacker-influenced length) can no longer produce a
  slice that reads or writes past the AMX allocation and segfaults; the size is
  bounded to the script's own memory (and the existing 1 MiB ceiling). It still
  cannot detect a size that overruns the array but stays inside the segment —
  always pass the real `sizeof(arr)`. Verified on a live SA-MP server.
- **`AmxString::to_bytes` no longer indexes an empty backing buffer**, returning
  an empty string instead of panicking on a corrupted length.
- **`Allocator` now rewinds the VM stack as well as the heap on drop.** If a
  `push` sequence inside `exec_public!` fails part-way (VM stack exhausted), the
  already-pushed cells are restored so the stack stays balanced instead of
  drifting. On the normal balanced path it is a no-op — verified with 1000
  `exec_public!` calls on a live SA-MP server (all succeeded, stack healthy).
- **`Allocator::new` no longer panics on a null VM pointer** (only reachable from
  tests); it captures `(0, 0)` and every `allot*` then fails gracefully via
  `amx_Allot`.

### Security

- **RUSTSEC-2026-0204** (`crossbeam-epoch` invalid pointer dereference in the
  `fmt::Pointer` impl for `Atomic`/`Shared`) — updated to 0.9.20. The advisory
  reached the SDK only through a dev-dependency (`criterion` → `rayon` →
  `crossbeam-deque`), so no shipped plugin was affected, but it was failing the
  `cargo audit` CI step on `master`.
- **CVE-2026-61632 / GHSA-9xwg-3r6f-jcx2** (`pymdown-extensions` b64 path
  traversal) — bumped `10.21.3` → `11.0.1` in `docs/requirements.txt`. This is a
  **docs-build-only** dependency (MkDocs) over the project's own trusted
  markdown, shipped in no crate; the fix just clears the Dependabot alert.

### Changed

- **CI: benchmarks no longer run on GitHub.** The `bench` job (and its `changes`
  gate) and the `bench-release.yml` workflow were removed — benches were noisy on
  shared runners and added little signal on every push/PR/release. They are now
  **dev-local only**: run `scripts/bench.sh` (criterion on i686, extra args
  forwarded to `cargo bench`). The CI-only reporting scripts
  (`extract-bench`/`render-bench-entry`/`build-bench-comment`/`append-bench-history`)
  were dropped with it.
- **Docs: added a "Not affiliated" disclaimer** to the READMEs (root + published
  crates) and the docs home, making explicit that this is an independent fork
  with no affiliation to SA-MP, open.mp, or the upstream `samp-rs` project.
- **CI: the cross-only (aarch64) job now runs `clippy -D warnings`** instead of a
  bare `cargo check`, so warnings that surface only on that arch — e.g.
  `dead_code` from target-gated code — fail the build like on every other target.
- **Dependency maintenance since v3.3.1.** Notable library bumps: `syn` 2 → 3
  (`samp-codegen`), `time` 0.3.53 → 0.3.54 (`samp`), `quote` → 1.0.47,
  `proc-macro2` → 1.0.107, `memcache` 0.19 → 0.20 (`advanced` example), and
  `sentry` 0.48.5 → 0.49.0 (`sink-demo` example — its `ClientOptions` became
  `#[non_exhaustive]`, now built via `ClientOptions::default()`). Routine GitHub
  Actions bumps as well (`codeql-action/upload-sarif`, `dorny/paths-filter`,
  `actions/checkout`, `actions/setup-python`, `ossf/scorecard-action`,
  `softprops/action-gh-release`, `marocchino/sticky-pull-request-comment`). None
  of these touch the shipped public API.

### Crate versions

- `rust-samp` (lib `samp`): 3.2.0 → 3.3.0 (new `#[event]`/`EventReturn` surface;
  requires `rust-samp-sdk` 3.3.0).
- `rust-samp-sdk` (lib `samp_sdk`): 3.2.1 → 3.3.0 (new `Amx::exec_public_scope`;
  buffer/stack hardening).
- `rust-samp-codegen` (lib `samp_codegen`): 1.3.0 → 1.4.0 (new `#[event]` macro,
  including `raw` mode).

## [v3.3.1] — 2026/07/04

Bug-fix release: the v3.3.0 changes did not build for 64-bit targets. Both fixes
matter to any consumer building the SDK for a 64-bit host (e.g. a DAP adapter
that only needs `samp::debug`).

### Fixed

- **`omp` module failed to compile on 64-bit MSVC** (E0570) — the open.mp
  component ABI uses `extern "thiscall"`, which only exists on 32-bit x86 (the
  arch SA-MP/open.mp servers run on). The module is now skipped on
  `windows + msvc + 64-bit`, where that ABI is invalid anyway; 32-bit and
  non-MSVC targets are unchanged.
- **FFI signatures used `i8` instead of `c_char`** (E0308 on aarch64) — the
  `amx_Exports` function-pointer types hardcoded `i8` for C-string/char
  arguments. `c_char` is `i8` on x86 but `u8` on aarch64, so `CString::as_ptr()`
  mismatched the signatures when building for a 64-bit target. The C-string/char
  pointers in `raw::functions` now use `c_char`.

### Changed

- **CI: added an `aarch64-unknown-linux-gnu` check job** (a 64-bit target where
  `c_char == u8`) so this class of ABI/type mismatch is caught going forward —
  the previous 32-bit-only matrix never exercised it.

### Crate versions

- `rust-samp-sdk` (lib `samp_sdk`): 3.2.0 → 3.2.1 (bug fixes only)
- `rust-samp` (lib `samp`): 3.2.0 — unchanged
- `rust-samp-codegen` (lib `samp_codegen`): 1.3.0 — unchanged

## [v3.3.0] — 2026/07/04

### Added

- **`Amx::pri()` / `Amx::alt()`** — safe reads of the VM's primary and
  alternate accumulator registers, completing the register set alongside the
  existing `cip`/`frame`/`stack`/`heap`/`stp`. Read directly from the
  `#[repr(C, packed)]` `AMX` struct with `read_unaligned` (taking a reference
  to a packed field is UB), returning `None` when the VM pointer is null.
- **`Amx::read_code(offset)`** — bounds-checked read of a 32-bit cell from the
  **code** segment, the instruction-side counterpart of `read_cell`. Resolves
  `base + header.cod + offset` and validates `offset` against the code segment
  `[0, header.dat - header.cod)`, reading byte-wise (the header is packed). The
  SDK exposes the raw bytes only; decoding the instruction is up to the consumer.
- **`Amx::opcode_table(count)`** — returns the VM's `amx_opcodelist` (the opcode
  → handler-address dispatch table) as `count` raw addresses, fetched the way the
  loader does it (set the `BROWSE` flag, call `amx_Exec` with index 0, restore the
  flags). On computed-goto builds (GCC/Clang — SA-MP and open.mp) the loader
  rewrites code-segment opcodes to these addresses, so a consumer can invert the
  table to recover the real opcode behind a `read_code` value. Returns `None` for
  a non-relocated image (`AMX_FLAG_RELOC` unset), where opcodes are stored raw.

### Crate versions

- `rust-samp-sdk` (lib `samp_sdk`): 3.1.0 → 3.2.0 (additive public API)
- `rust-samp` (lib `samp`): 3.2.0 — unchanged
- `rust-samp-codegen` (lib `samp_codegen`): 1.3.0 — unchanged

## [v3.2.0] — 2026/06/30

### New features

- **VM debugging primitives on `Amx`** — safe accessors that previously had to
  be hand-written by tooling poking the `#[repr(C, packed)]` `AMX` struct:
  register reads (`cip`, `frame`, `stack`, `heap`, `stp`), bounds-checked
  data-segment cell access (`read_cell`/`write_cell`, mirroring `amx_GetAddr`
  and usable inside a debug hook where no native context exists), and debug
  hook management (`install_debug_hook`/`remove_debug_hook`, the equivalent of
  `amx_SetDebugHook`). Always available, no feature gate.
- **`samp::debug` — AMX_DBG debug-info parser (feature `debug`)** — pure-logic
  decoder for the debug block `pawncc -d2`/`-d3` appends to the `.amx`. Maps a
  code address ↔ source line ↔ symbol ↔ function (`AmxDbg::from_amx`/`parse`,
  `lookup_line`, `lookup_file`, `lookup_function`, `line_to_address`,
  `symbols_in_scope`, `tag_name`), handling the 16-bit line-count overflow of
  large gamemodes and corrupted-count sanity ceilings. No extra dependencies;
  opt-in via the `debug` feature. `DbgSymbol::effective_address(frm)` and
  `DbgSymbol::is_array()` remove the global-vs-frame address boilerplate when
  pairing the parser with `Amx::read_cell`/`write_cell`.

- **External sinks (`samp::logger::Sink` trait + `LoggerConfig::add_sink`)** —
  extension point for forwarding accepted log records to a destination
  chosen by the plugin author (Sentry, an OTLP collector, an in-house
  HTTP endpoint, anything). **No telemetry is built into the SDK.** No
  dependency on `sentry` / `opentelemetry` is added; `rust-samp`
  ships exactly the same dependency graph as before. The trait is an
  opt-in surface only — implementing it is the plugin author's call,
  and instances become active only through an explicit
  `LoggerConfig::add_sink(Box::new(...))` in the plugin's own source.
  The SDK contains zero `add_sink` invocations of its own; server
  operators auditing what a `rust-samp` plugin can export only need
  to grep its source for `add_sink(`. Zero hits means zero external
  traffic from the logger. There is no hidden flag, no environment
  override, and no default destination — this is not Microsoft-style
  always-on telemetry, it is a hook for plugin authors who already
  run their own observability stack to integrate with it on their own
  terms.
- **`samp::version()`** — free function returning the `CARGO_PKG_VERSION`
  of the `rust-samp` (`samp`) crate. Pair it with a Pawn-side native
  (e.g. `MyPlugin_GetSdkVersion()`) to surface the active SDK build in
  bug reports and diagnostic dashboards.
- **`samp::logger::flush()`** — public free function that flushes the
  active log file directly through the live `LoggerImpl`. Going through
  `log::logger().flush()` did not guarantee a sync of the SDK's own
  file handle; calling `samp::logger::flush()` does. Safe no-op when
  the logger has not been installed — meant for panic hooks and
  custom shutdown paths.
- **`LoggerConfig::from_env()`** — applies runtime overrides from
  environment variables, so server operators can flip the log level,
  redirect the directory, change the rotation threshold etc. **without
  recompiling the plugin**. The prefix is derived from the plugin's
  crate name uppercased with non-alphanumeric characters replaced by
  `_` (`streamer-rs` → `STREAMER_RS_LOG_*`). Recognised keys:
  `LEVEL`, `DIR`, `FILE`, `ROTATION_MB`, `ROTATION_KEEP`,
  `NO_ROTATION`, `NO_BANNER`, `SERVER`, and `COMPRESS` (the last only
  effective when the `compression` feature is enabled). Missing vars
  leave the existing value untouched; invalid values are reported to
  the server console and the previous value is kept. Pairs with
  `Runtime::try_get()` (also new) so the parser can warn gracefully
  even when called before the runtime is initialised (e.g. from a
  unit test).
- **`LoggerConfig::compress_archives(bool)`** — opt-in gzip of rotated
  archives. When enabled, every rotation produces
  `{filename}.{N}.gz` instead of `{filename}.{N}` and removes the
  uncompressed file. Works with both rotation strategies (append-style
  and `rotation_keep(N)` shift-style). Gated by the new `compression`
  Cargo feature, which pulls in `flate2` with the pure-Rust backend —
  not enabled by default, so plugins that do not need it pay no extra
  dependency cost. The next-archive scan also recognizes `.gz`
  variants so an index is never reused across restarts.
- **`Amx::call_native()`** — invoke a native registered by **another
  plugin** in the same AMX, straight from Rust. Resolves the host
  function pointer through `amx_FindNative` + the natives table in the
  `AMX_HEADER`, builds the `params` block in the AMX convention
  (`[argc * sizeof(cell), arg0, ...]`) and surfaces VM-side errors back
  via `amx.error`. Unblocks integration with the entire existing C++
  plugin ecosystem (Streamer, MySQL, sscanf, …) without dropping down
  to `samp_sdk::raw`. Originally surfaced by
  [@Day-OS](https://github.com/Day-OS) (Discord `@daytheipc`), who
  found `rust-samp` on crates.io while trying to drive the Streamer
  plugin from Rust for an in-game PNG / video / YouTube-live 3D panel
  and hit the gap that this API closes. May or may not have been
  exactly what she needed — but it should help.

### Examples

- **New `examples/sink-demo/`** — complete, working **Sentry
  integration** for the new `Sink` trait. Uses the real `sentry`
  crate (`sentry = "0.43"` with `reqwest` + `rustls` + `contexts`,
  `default-features = false`), with `sentry::init` and
  `sentry::capture_event` wired up end-to-end — every `log!` call
  becomes a real Sentry event. **DSN is read from the env var
  `SINK_DEMO_SENTRY_DSN` at plugin load — never hardcoded.** Source
  code stays clean, the DSN stays in the operator's environment
  (systemd `Environment=`, Docker secret, vault sidecar, `.env`
  outside the repo, …). Implements the full backpressure pattern
  (`mpsc::sync_channel` between the logger lock and Sentry +
  dedicated background drainer thread that owns the
  `ClientInitGuard`, so its `Drop` flushes pending events at plugin
  unload). When the env var is missing the example falls back to a
  fake local DSN (`http://fake@127.0.0.1:9999/1`) — the Sentry
  client still initializes but its HTTP transport refuses fast, so
  no event ever reaches a real Sentry server. Going to production
  is one `export` statement. When a real DSN is configured, the
  plugin also emits a startup smoke test (one `info` + one
  `warning` + one `error`) on `on_load` so the operator immediately
  sees the wiring working on the Sentry dashboard. The heavy
  `sentry` dep (pinned to 0.48.3) is paid by this example crate,
  not by the SDK. Pawn natives: `SinkDemo_GetExportedCount`,
  `SinkDemo_GetDroppedCount` for pipeline observability;
  `SinkDemo_EmitInfo`, `SinkDemo_EmitWarn`, `SinkDemo_EmitError`
  for firing test events at each severity from the gamemode.

### Build

- **New Cargo feature `compression`** on the `rust-samp` crate. Opt-in;
  pulls in `flate2 = "1"` with the pure-Rust backend
  (`default-features = false`, `features = ["rust_backend"]`) so plugins
  that do not enable it remain dependency-free on this axis.
- **`time` bumped to `>= 0.3.47`** (also pulls in `time-core 0.1.8`
  and `time-macros 0.2.27`) — this is the floor that drove the MSRV bump;
  later refreshed to 0.3.51 by Dependabot (see Dependencies, #16).
- **MSRV bumped to Rust 1.88** (was 1.87) to satisfy those versions.
  Declared via `[workspace.package].rust-version = "1.88"`.

### Security & governance

- **OpenSSF Scorecard** — new `.github/workflows/scorecard.yml` that runs
  the OpenSSF Scorecard analysis, uploads the SARIF to code-scanning and
  publishes the result. Scorecard badge added to the README.
- **All GitHub Actions pinned by commit SHA** — every `uses:` across all
  seven workflows is pinned to a full commit SHA (with a `# vX` comment),
  satisfying the Scorecard *Pinned-Dependencies* check.
- **Least-privilege token permissions** — every workflow declares a
  top-level minimal `permissions: contents: read`, with jobs escalating
  explicitly only where needed (Scorecard *Token-Permissions*).
- **No script injection from untrusted PR fields** — `rust.yml` now passes
  `github.event.pull_request.*` values (e.g. `head.ref`) through `env`
  instead of interpolating them into `run:` scripts (Scorecard
  *Dangerous-Workflow*).
- **`docs/requirements.txt` pinned by hash** — the MkDocs Material build
  dependencies are now a fully hashed lockfile (`pip-compile
  --generate-hashes` from the new `docs/requirements.in`), installed with
  `pip install --require-hashes` in the docs workflow.
- **`.github/dependabot.yml`** — weekly version updates for the
  `github-actions` and `cargo` ecosystems, keeping the pinned SHAs and
  crate dependencies fresh (Scorecard *Dependency-Update-Tool*).
- **CI tweaks** — the Scorecard workflow gained `workflow_dispatch` for
  on-demand re-scans; the Rust workflow ignores docs-only changes via
  `paths-ignore` (`**.md`, `docs/**`, `mkdocs.yml`, `LICENSE`); and the
  benchmark job now runs only when benchmark-relevant code actually changed
  (a `changes` path-filter job gates it), never for `dependabot[bot]`
  (dependency bumps don't need a bench run, and Dependabot's read-only
  token cannot post the PR comment).
- **Release Drafter removed** — `.github/release-drafter.yml` and
  `.github/workflows/release-drafter.yml` dropped. Releases are cut
  directly rather than drafted, so the workflow was dead weight.
- **`SECURITY.md`** — security policy and private vulnerability reporting
  via GitHub Security Advisory.
- **`CODE_OF_CONDUCT.md`** — Contributor Covenant 2.1.
- **`CONTRIBUTING.md`** — build/test/lint workflow for the i686 targets,
  project structure and code rules.

### Documentation

- **Single source of truth for crate versions** — the per-crate version
  columns were removed from the `README.md` and `docs/index.md` workspace
  tables (they duplicated `Cargo.toml` and had already drifted out of date).
  Version-specific git tags in install snippets across the docs were replaced
  with a `tag = "vX.Y.Z"` placeholder, and the dependency examples already use
  the major-only `version = "3"`. Bumping a crate now means editing its
  `Cargo.toml` and adding a CHANGELOG entry — no docs need touching.

### Dependencies

Automated bumps opened and merged via [@dependabot](https://github.com/apps/dependabot)
after the new `dependabot.yml` went live:

- Bump `actions/cache/save` from 5.1.0 to 6.1.0 (#8)
- Bump `actions/cache/restore` from 5.1.0 to 6.1.0 (#9)
- Bump `actions/checkout` from 4.2.2 to 7.0.0 (#10)
- Bump `marocchino/sticky-pull-request-comment` from 2.9.4 to 3.0.4 (#12)
- Bump `log` from 0.4.29 to 0.4.33 (#11)
- Bump `bitflags` from 2.11.0 to 2.13.0 (#13)
- Bump `quote` from 1.0.44 to 1.0.46 (#14)
- Bump `syn` from 2.0.116 to 2.0.118 (#15)
- Bump `time` from 0.3.47 to 0.3.51 (#16)

The cargo bumps are lock-file only (`Cargo.lock`); the version
requirements in the manifests are unchanged.

### Crate versions

- `rust-samp` (lib `samp`): 3.1.0 → 3.2.0
- `rust-samp-sdk` (lib `samp_sdk`): 3.0.0 → 3.1.0 (new VM debugging
  primitives on `Amx` and the `samp::debug` parser are additive public API)
- `rust-samp-codegen` (lib `samp_codegen`): 1.3.0 — unchanged

### CHANGELOG correction (v3.1.0)

The v3.1.0 entry below described the `CNAME` removal as a switch to
the default GitHub Pages URL. That was wrong: the file was simply
unnecessary and the documentation URL is unchanged. Recorded here;
the v3.1.0 section below is left as-published.

## [v3.1.0] — 2026/06/09

Headline: turnkey logger — `samp::enable_logger!()` installs a complete
per-plugin logging pipeline (file under `logs/`, size-based rotation
into `logs/archive/`, prefix derived from `CARGO_PKG_NAME`, startup
banner, runtime-adjustable level) in a single call. The previous
`samp::plugin::logger()` DIY path stays unchanged for advanced cases.

v3.1.0 is also the **first version available on crates.io** —
[`rust-samp`](https://crates.io/crates/rust-samp),
[`rust-samp-sdk`](https://crates.io/crates/rust-samp-sdk) and
[`rust-samp-codegen`](https://crates.io/crates/rust-samp-codegen).
Earlier releases (v3.0.0 and the entire v2.x line) are not published to
the registry; plugins targeting those versions must keep using a git
dependency. The **library** names (`samp`, `samp_sdk`, `samp_codegen`)
are unchanged; only the **package** names differ on the registry to
avoid colliding with the upstream `samp-rs` fork.

### Crate versions

- `rust-samp` (lib `samp`): 3.0.0 → 3.1.0
- `rust-samp-sdk` (lib `samp_sdk`): 3.0.0 — unchanged (metadata only)
- `rust-samp-codegen` (lib `samp_codegen`): 1.3.0 — unchanged (metadata only)

### Installation

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
samp = { package = "rust-samp", version = "3" }
log  = "0.4"
```

The package is published as `rust-samp`; the alias keeps the
source-level `use samp::prelude::*;` imports unchanged. Git consumers
do not need to update anything — both names continue to resolve to the
same library.

### New features

- **Turnkey logger** — new module `samp::logger` plus the
  `samp::enable_logger!()` and `samp::enable_logger_with!(cfg)` macros.
  The macros capture the caller's `CARGO_PKG_*` at compile time and
  install a `log::Log` implementation that routes through both the
  server's log sink and a per-plugin file.
- **`LoggerConfig` builder** — every aspect of the pipeline is
  configurable through fluent setters: `directory`, `filename`,
  `prefix`, `level`, `also_to_server`, `banner`, `file_format`,
  `server_format`, `rotation_size_mb`, `rotation_keep`,
  `rotation_no_cleanup`, `no_rotation`, `no_banner`, `banner_with`.
- **Format templates** — `file_format` and `server_format` accept
  `{timestamp}`, `{level}`, `{message}` and (server-only) `{prefix}`
  placeholders with optional alignment specifiers (`{level:>5}`,
  `{level:<5}`, `{level:^5}`). Unknown placeholders pass through
  verbatim so typos are visible.
- **Banner modes** — `BannerMode::Default` (5-line banner from
  `CARGO_PKG_*`), `BannerMode::Off`, and `BannerMode::Custom` (closure
  receiving `BannerMetadata` and returning the lines to render).
- **Size-based rotation** — when the active file passes
  `rotation_size_mb` (default 50 MB), it is renamed into
  `{directory}/archive/{filename}.{N}` and a fresh active file is
  opened. Two strategies are available:
  - **Append-style** (default): every rotation uses the next free
    index, archives are **never deleted** by the SDK, the dev keeps
    full control over cleanup. The archive folder is created lazily on
    the first rotation; the next index survives restarts (rescanned at
    install time).
  - **Shift-style** — opt-in via `rotation_keep(N)`: `.log.N` is
    deleted, every other archive shifts down, active becomes `.log.1`.
    Disk footprint becomes `(keep + 1) * rotation_size_mb`.
- **Runtime level adjustment** — `samp::logger::set_level(...)` and
  `samp::logger::level()` let plugins expose a Pawn-side knob for log
  verbosity (e.g. a `MyPlugin_SetLogLevel(level)` native).
- **`InstallError`** — `Display` + `Error` with `source()` exposing
  the inner `std::io::Error` for the `Io` variant.

### Migrating to the turnkey logger

The previous handcrafted `fern::Dispatch` pattern keeps working —
adoption is optional.

| Before (v3.0.0)                                                                                                       | After (v3.1.0)                                |
| --------------------------------------------------------------------------------------------------------------------- | --------------------------------------------- |
| `fern::Dispatch::new().level(...).chain(samp::plugin::logger()).chain(fern::log_file(...)?).format(...).apply()?;`    | `samp::enable_logger!()` inside `on_load`     |
| Hand-built prefix string `format_args!("[my-plugin][{}]: {}", record.level(), message)`                               | Automatic `[CARGO_PKG_NAME]` prefix           |
| Manually managing the log filename                                                                                    | Default `logs/{CARGO_PKG_NAME}.log`           |
| External `logrotate` setup for file growth                                                                            | Built-in 50 MB rotation into `logs/archive/`  |
| Pawn-side `SetLogLevel(level)` native backed by a `static AtomicI32`                                                  | `samp::logger::set_level(LevelFilter)` direct |

See [`docs/logging.md`](docs/logging.md) for the full reference,
including the three layers (turnkey / `LoggerConfig` / DIY with fern),
format placeholders, rotation modes, and runtime tuning.

### Packaging (crates.io)

The three workspace crates now have crates.io-ready metadata
(`description`, `keywords`, `categories`, `rust-version`, per-crate
`README.md`) and centralized shared fields in `[workspace.package]`.

- **Package names**: `rust-samp`, `rust-samp-sdk`, `rust-samp-codegen`.
  The upstream `samp` / `samp-sdk` / `samp-codegen` names on crates.io
  belong to the original `samp-rs` author and are not the publication
  target of this fork.
- **Library names**: unchanged — `samp`, `samp_sdk`, `samp_codegen`.
  Existing `use samp::prelude::*;` keeps compiling.
- **Crates.io consumers** add the package alias to their
  `Cargo.toml`:

  ```toml
  [dependencies]
  samp = { package = "rust-samp", version = "3" }
  ```

- **Git consumers** (`samp = { git = "..." }`) need no change — the
  workspace exposes both names.

### Build

- **MSRV bumped to Rust 1.88** (was 1.85). Required by stable
  `i32::cast_unsigned` / `u32::cast_signed` (used internally for AMX
  cell bit conversions) and by the patched `time 0.3.47` / `time-core
  0.1.8` / `time-macros 0.2.27`. Declared via
  `[workspace.package].rust-version = "1.88"`.
- **New transitive dependency** — `time = "0.3.47"` (features
  `local-offset`, `formatting`, `macros`) is pulled in by the turnkey
  logger for timestamp formatting. The `chrono` crate is **not** added.
  The minimum is pinned to `0.3.47` to pick up the fix for
  [RUSTSEC-2026-0009](https://rustsec.org/advisories/RUSTSEC-2026-0009)
  (DoS via stack exhaustion, medium severity).

### CI / release infrastructure

- **Release notes are now auto-assembled** — workflow `.github/workflows/release.yml`
  combines the curated CHANGELOG section with the GitHub-native
  `releases/generate-notes` API output, keeping the "New Contributors"
  block and the "Full Changelog" comparison link while dropping the
  redundant `## What's Changed` header.
- **Crates.io publication is wired into the release workflow** — on
  `v*` tag push (and manual `workflow_dispatch` with a `dry_run`
  input), the workflow validates the workspace, then publishes
  `rust-samp-sdk` → `rust-samp-codegen` → `rust-samp` in dependency
  order with a 30 s sleep between steps. Each `cargo publish` step
  gracefully skips when the version is already on crates.io, so a
  patch release that bumps only one crate goes through unattended.
- **Bench jobs were updated** to reference `rust-samp-sdk` instead of
  the pre-rename `samp-sdk` package id.
- **Release-drafter template** no longer emits the duplicated
  `## What's Changed` heading at the top of release notes.

### Documentation

- **`docs/logging.md`** — rewritten end-to-end (139 → 413 lines).
  Covers the three layers, format placeholders and alignment specs,
  banner modes, rotation strategies, runtime level adjustment, and a
  pitfalls section.
- **`docs/api-reference.md`** — new `samp::logger` section with the
  full builder signature, error type, and placeholder reference.
- **`docs/migration.md`** — new v3.0.0 → v3.1.0 section with the
  before/after migration table and crates.io adoption notes.
- **`docs/first-plugin.md`** — new "Enabling logging" section showing
  the one-liner inside `on_load`.
- **`docs/plugin-anatomy.md`** — explicit note that `enable_logger!`
  belongs in `on_load`, not the constructor block (server's log sink
  is not connected yet during construction).
- **`docs/index.md`** — the integrated-logging bullet now describes
  the turnkey path; workspace table reflects the new `samp` version.
- **`docs/advanced-examples.md`** — the `examples/counter` snippet
  matches the source change (uses `samp::enable_logger!()` instead of
  the handcrafted `fern::Dispatch`).
- **Dual-availability sweep** — `README.md`, `migration.md`,
  `docs/setup.md`, `docs/encoding.md` and `docs/migration.md` now show
  both installation paths side-by-side (crates.io for v3.1.0+, git for
  any version including v3.0.0 and earlier) with a consistent
  `package = "rust-samp"` snippet. The workspace version table in
  `README.md` was bumped to reflect the new `samp` 3.1.0.

### Examples

- **`examples/counter` (1.0.0 → 1.1.0)** — `on_load` now calls
  `samp::enable_logger!()`; the `fern` dependency was dropped.
- **`examples/hello` (1.0.0 → 1.0.1)**, **`examples/advanced` (1.1.0 →
  1.1.1)** — switched to the `package = "rust-samp"` alias so the path
  dependency matches the published name. No source changes.

### Repository housekeeping

- `ROADMAP.md` moved out of version control (now under `.gitignore`)
  — it stays as a local working document for the maintainer and is no
  longer shipped to crates.io tarballs or GitHub.
- `.github/CODEOWNERS` added.
- `CNAME` removed (project pages are served from the default
  `nullsablex.github.io/rust-samp/` URL).

### Code quality

- Clippy `-D warnings` and `-W clippy::pedantic` both report **zero**
  warnings.
- 232 tests pass (was 219 in v3.0.0; +13 covering the new logger
  config, format substitution, width specifiers, rotation modes, error
  source, and macro-driven metadata capture).
- `cargo fmt --check` green; `cargo machete` reports no unused
  dependencies.

## [v3.0.0] — 2026/05/17

Compared to **v2.2.0** (2026/03/15).

Headline: native Open Multiplayer component ABI implemented in pure
Rust (Itanium **and** MSVC); a single binary now works as a SA-MP
plugin and as a first-class Open Multiplayer component, with no extra
configuration.

### Migrating from v2.x (or from the upstream `samp-rs` fork)

This release contains **breaking changes**. A plugin written against
v2.x will not compile against v3.0.0 without edits. Minimum diff:

| Before (v2.x / upstream)              | After (v3.0.0)                                                          |
| ------------------------------------- | ----------------------------------------------------------------------- |
| `fn process_tick(&mut self) { … }`    | `fn on_tick(&mut self, _ctx: TickContext) { … }`                        |
| `samp::plugin::enable_process_tick()` | `samp::plugin::enable_tick()` (or `enable_tick_with(TickConfig)`)       |
| `samp::cell::string::put_in_buffer(buf, s)?` | `buf.write_str(s)?` / `unsized.write_str(size, s)?` (`put_in_buffer` is `pub(crate)` now) |
| `samp::raw::functions::Logprintf` (variadic) | Same path, now `extern "C" fn(*const i8)` — the SDK formats in Rust and passes a single C string |
| `example-hello/` / `example-counter/` / `plugin-example/` | `examples/hello/` / `examples/counter/` / `examples/advanced/` |

Open Multiplayer support comes turned on by default — the build
produces both the SA-MP exports **and** the `ComponentEntryPoint`. To
keep the v2.x behavior unchanged, enable the new `samp-only` feature:

```toml
samp = { git = "...", tag = "v3.0.0", features = ["samp-only"] }
```

Two new requirements that affect builds:

- The workspace's `[profile.release]` adds `lto = "thin"`,
  `codegen-units = 1`, `strip = true`. Override in your own
  `Cargo.toml` if you need otherwise.
- The i686 target is now strictly required at compile time
  (`OmpComponent` has `const _` layout asserts). Set
  `target = "i686-unknown-linux-gnu"` in `.cargo/config.toml` if you
  were relying on the host target picking up automatically.

The full step-by-step walkthrough lives in
[`docs/migration.md`](docs/migration.md) — including the new
`TickConfig` knobs (`sa_mp_only()`, `omp_only(Duration)`, custom
`omp_interval`) and how to choose between them.

### Crate versions

- `samp`: 2.2.0 → 3.0.0
- `samp-sdk`: 2.2.0 → 3.0.0
- `samp-codegen`: 1.2.0 → 1.3.0

### Breaking changes

- **`SampPlugin::process_tick`** replaced by
  **`SampPlugin::on_tick(&mut self, ctx: TickContext)`** — unified
  callback that fires on both servers. Cadence is the server's main
  loop on SA-MP and the SDK-owned `ITimersComponent` timer on native
  Open Multiplayer (interval configurable). `TickContext::source`
  reports the origin (`TickSource::SaMp` /
  `TickSource::OmpTimer`); `TickContext::elapsed` is the
  wall-clock interval since the previous dispatch.
- **`samp::plugin::enable_process_tick`** replaced by
  **`samp::plugin::enable_tick()`** (default config) and
  **`samp::plugin::enable_tick_with(config: TickConfig)`** (explicit
  per-server toggle + Open Multiplayer interval).
- **`samp::cell::string::put_in_buffer`** is now `pub(crate)` (was
  `pub`). The public API for writing strings is `Buffer::write_str`
  and `UnsizedBuffer::write_str`.
- **`samp_sdk::raw::functions::Logprintf`** signature changed from
  variadic `extern "C" fn(*const i8, ...)` to fixed-arity
  `extern "C" fn(*const i8)` — the SDK formats the message in Rust
  and passes a single C string, matching what `logprintf("%s", msg)`
  does at the ABI level.
- Example crates renamed: `example-hello/` → `examples/hello/`,
  `example-counter/` → `examples/counter/`,
  `plugin-example/` → `examples/advanced/`. Workspace members
  updated accordingly.

### Added — native Open Multiplayer support

- **`samp_sdk::omp`** — new top-level module with eight submodules:
  - `component` — `OmpComponent`, `IComponentVTable`,
    `IUIDProviderVTable`, opaque types (`ICore`, `IComponentList`,
    `ILogger`, `IEarlyConfig`), default vtable implementations for
    both ABIs.
  - `component_api` — `OmpComponentHandle` trait, generic
    `component_name<T>()` / `component_version<T>()` helpers.
  - `core` — `LogLevel`, `core_print_ln`, `core_log_ln`,
    `core_print_ln_u8`, `core_log_ln_u8`.
  - `events` — `PawnEventHandler`, `PawnEventHandlerVTable`.
  - `server` — `PAWN_COMPONENT_UID`, `NUM_AMX_FUNCS`,
    `PawnComponent`, `ServerComponentList`, `ServerComponent`,
    `ServerPawnComponent`, `IEventDispatcherPawn`, `IPawnScript`,
    `AmxFunctionTable`, plus the free functions
    `query_component`, `add_pawn_event_handler`,
    `remove_pawn_event_handler`, `get_pawn_event_dispatcher`,
    `get_amx_from_script`, `get_amx_functions`.
  - `timers` — `TIMERS_COMPONENT_UID`, `TimersComponent`,
    `ITimersComponent`, `ITimer`, `TimerHandlerVTable`,
    `TimerTimeOutHandler`, `create_repeating_timer`, `kill_timer`,
    `query_timers_component`.
  - `types` — `UID`, `SemanticVersion` (with `new` and
    `with_prerel`), `StringView` (with `as_str`, `try_as_str`,
    `from_static`), `Colour` (with `rgb`, `rgba`, `from_rgba_u32`,
    `to_rgba_u32` and the `WHITE`, `BLACK`, `NONE` constants),
    `Vector2`, `Vector3`, `Vector4`, `ComponentType`.
  - `vtable` — `subobject_ptr`, `vtable_slot`,
    `secondary_call_target` helpers for safe access to secondary
    vtables.
- **`samp::omp`** re-exports the module above.
- **`samp::plugin`** new functions: `enable_tick`,
  `enable_tick_with`, `omp_core`, `omp_query_component`,
  `omp_query::<T>` (typed wrapper for `OmpComponentHandle`
  implementors).
- **`samp::plugin`** new types: `TickConfig`, `TickContext`,
  `TickSource`.
- **`SampPlugin`** new hooks: `on_tick(ctx)`,
  `on_omp_ready` (gated by `not(feature = "samp-only")`),
  `on_component_free` (same gating).
- **`samp::log`** re-export — `#[native]`-expanded code now uses
  `samp::log::error!`, so user crates no longer need to declare
  `log` as a direct dependency just to satisfy the macro.

### Added — `initialize_plugin!` extensions

- Optional metadata fields: `uid: <u64 expression>`,
  `component_name: "..."`, `component_version: (x, y, z)`.
- `samp-codegen` reads `[package.metadata.samp]` from the project's
  `Cargo.toml`. Resolution order per field:
  **macro argument > `[package.metadata.samp]` > derived value**
  (`CARGO_PKG_NAME`, parsed `CARGO_PKG_VERSION`, FNV-1a 64 of
  `CARGO_PKG_NAME@CARGO_PKG_VERSION` for the UID).
- When the UID is missing from both sources, the generated value is
  **persisted back** into `Cargo.toml` under `[package.metadata.samp]`
  so subsequent builds reuse the same identifier.
- Generates the SA-MP exports (`Load`, `Unload`, `Supports`,
  `AmxLoad`, `AmxUnload`, `ProcessTick`) **and** the Open Multiplayer
  `ComponentEntryPoint` by default. Opt out with the `samp-only`
  feature.

### Added — `#[native]` extensions

- Accepts **associated functions** (no `self`), in addition to
  methods.
- Return type detection: `Result` / `AmxResult` is matched against
  `Ok`/`Err`; any other type implementing `AmxCell` is used as the
  return cell directly (no spurious `Ok(...)` wrapping).
- Accepts `&AmxString` (and any other `&T`) parameters — the macro
  materializes the owned value from `args.next_arg()` and injects
  `&local` at the call site.
- Validates the `name = "..."` literal at proc-macro time:
  interior `\0` bytes now produce a compile error instead of
  panicking at `CString::new` during server load.
- Wraps every invocation in `std::panic::catch_unwind`. Panics that
  would otherwise cross the `extern "C"` boundary (process abort on
  Rust 1.71+) are caught, logged as
  `[<NativeName>] panic in native: <payload>`, and converted to a
  `0` return.
- Argument parsing failures now log
  `[<NativeName>] failed to parse argument #<i> '<name>' (expected type: <Type>)`
  — both the positional index and the expected type are included.

### Added — features and build infrastructure

- New `samp-only` feature on both `samp` and `samp-sdk`: removes
  every Open Multiplayer code path. The plugin still loads on Open
  Multiplayer, but in legacy mode (no component API).
- New workspace `[profile.release]`: `lto = "thin"`,
  `codegen-units = 1`, `strip = true`.
- `Cargo.lock` is now committed (removed from `.gitignore`).
- `Cargo.toml` per crate exposes
  `package.metadata.docs.rs.default-target = "i686-pc-windows-msvc"`
  + `features = ["encoding"]` so docs.rs builds with the right
  target.
- New build scripts:
  - `scripts/build-linux.sh` — produces `.so`
    (`i686-unknown-linux-gnu`) and `.dll`
    (`i686-pc-windows-msvc` via `cargo-xwin --xwin-arch x86`, or
    `i686-pc-windows-gnu` with `--samp-only`).
  - `scripts/build-windows.sh` — produces `.dll` natively and `.so`
    through WSL or Docker/cross (autodetected; forceable with
    `--wsl` / `--docker`).
- New helper scripts used by the benchmark workflow:
  `scripts/append-bench-history.py`, `scripts/extract-bench.py`,
  `scripts/render-bench-entry.py`.
- New GitHub workflows: `docs.yml` (publishes the MkDocs site),
  `release.yml` (creates releases on `v*` tags, attaches a source
  tarball with only the essential crates), `release-drafter.yml`,
  `labels.yml`, `bench-release.yml` (per-release benchmark history
  on the `bench-data` branch).
- `.github/labels.yml` and `.github/release-drafter.yml` for the
  workflows above.
- `rust.yml` workflow: action versions bumped to
  `actions/checkout@v6`, `actions/upload-artifact@v7`,
  `actions/cache/restore@v5`, `actions/cache/save@v5` (Node 24
  baseline); benchmark job restricted to `-p samp-sdk` to avoid
  `Unrecognized option: 'save-baseline'`; artefact retention now
  capped at 14 days.

### Added — tests and benchmarks

- Unit tests grew from 80 (v2.2.0) to 207 (this release) — **+127
  tests**.
- New per-module test files:
  - `samp-sdk/src/tests/amx_cell.rs` (10 tests).
  - `samp-sdk/src/tests/amx_string.rs` (8 tests).
  - `samp-sdk/src/tests/buffer.rs` (12 tests).
  - `samp-sdk/src/tests/omp_lifecycle.rs` (4 tests).
- Inline coverage added to every new `omp` submodule
  (`component`, `component_api`, `core`, `events`, `server`,
  `timers`, `types`, `vtable`) — 62 tests across them.
- `samp-codegen` gained 26 unit tests in `plugin.rs` covering
  `fnv1a_64`, `parse_uid_str`, `parse_version_str`, and
  `read_samp_metadata_from_content`.
- New `samp-sdk/benches/buffer_bench.rs` (Criterion): `get_as::<f32>`,
  `set_as::<bool>`, `iter_as::<i32>`, `iter_as::<f32>` at sizes
  8 / 64 / 256 / 1024.
- `samp-sdk/benches/string_bench.rs` reworked: uses
  `std::hint::black_box` (prevents LLVM DCE) and exercises
  `Buffer::write_str` / `UnsizedBuffer::write_str` alongside the
  existing baselines.

### Added — examples

- `examples/hello/src/lib.rs` and `examples/counter/src/lib.rs`
  ship with full source (previously only had `Cargo.toml`
  placeholders).
- `examples/README.md` plus one `README.md` per example
  (`hello`, `counter`, `advanced`) documenting the natives, the
  patterns demonstrated, and how to build each one in isolation.

### Changed

- All SDK diagnostic warnings are routed through the standard `log`
  facade with the `[rust-samp]` prefix. New warnings cover the Open
  Multiplayer lifecycle (null `ICore*` in `on_load`, missing
  `IPawnComponent` in `on_init`, null `IEventDispatcher`,
  `getAmxFunctions()` returning 0 in `on_ready`, missing
  `ITimersComponent` when the tick is enabled).
- Default log routing now writes via `ICore::logLnU8` when the
  plugin runs on native Open Multiplayer, mapping `log::Level` to
  `samp_sdk::omp::LogLevel` automatically; SA-MP behavior is
  unchanged (`logprintf`).
- `f32::as_cell` uses `f32::to_bits(*self).cast_signed()` (Rust
  1.87+ helper) instead of an `as i32` round trip.
- `Allocator::string_bytes` lifetime simplified to `&str → Cow<'_, [u8]>`.
- `Args::new` parameter renamed `args` → `params` (positional API
  unchanged).
- Compile-time layout assertions for `OmpComponent`:
  - Linux i686 (gated by `target_os = "linux"`):
    `offset_of!(uid_vtable) == 40`, `size_of == 56`.
  - Windows MSVC i686 (gated by `target_env = "msvc"`):
    `offset_of!(uid_vtable) == 56`.
  - Both with explanatory error messages on mismatch.

### Fixed

- Open Multiplayer adaptive bootstrap: `getAmxFunctions()` is tried
  in `on_init` and the pointer is stored if non-zero; otherwise the
  SDK retries in `on_ready`. This works for the current Open
  Multiplayer release (1.5.x — returns 0 in `on_init`) **and** any
  future release that populates the table earlier, without code
  changes.
- AMX scripts that arrive via `on_amx_load` **before** the AMX
  function table is available are now queued and processed in
  `on_ready` instead of being dropped.
- `omp_cleanup` correctly kills the tick timer (if any) and
  removes the `PawnEventHandler` from the dispatcher before the
  component is unloaded, preventing use-after-free if the server
  fires Pawn events during shutdown.
- The native-name `CString` allocation is leaked through
  `Box::leak(CString::into_boxed_c_str())` and the leak is now
  documented at the leak site (was previously implicit via
  `CString::into_raw()`).
- All compiler-emitted error messages from `samp-codegen` are now
  in English (were a mix of English and Portuguese).

### Documentation

- README, `samp-sdk/readme.md`, `samp-codegen/readme.md`, and root
  `migration.md` rewritten in English.
- mdBook removed (`docs/book.toml`, every `docs/src/*` page) and
  replaced by a MkDocs Material site under `docs/`. New pages:
  `introduction`, `setup`, `first-plugin`, `plugin-anatomy`,
  `natives`, `amx-types`, `cells-and-memory`, `encoding`,
  `error-handling`, `logging`, `advanced-examples`,
  `api-reference`, `omp-native`, `migration`, plus the new
  `exec-public`, `build-scripts`, `diagnostics`, and
  `internals/omp-abi`.
- All source-code docstrings translated from Portuguese to English
  (in-tree only — user-facing release notes and prose stay in
  English as well).
- This `CHANGELOG.md` now only carries the current release; older
  releases moved to `changelog/v1.x.md`, `changelog/v2.x.md`, and
  `changelog/historical.md` (pre-fork `samp-rs`).

### Dependencies

- Dev dependency `criterion` bumped 0.5 → 0.8.
- No other runtime dependency changes.

### Platform support

| Target                    | SA-MP | Native Open Multiplayer | Notes                                  |
| ------------------------- | :---: | :---------------------: | -------------------------------------- |
| `i686-unknown-linux-gnu`  |  ✅   |   ✅ (Itanium ABI)       | Default on Linux.                      |
| `i686-pc-windows-msvc`    |  ✅   |   ✅ (MSVC ABI)          | **New in 3.0.0.** Cross-compile from Linux via `cargo xwin build --xwin-arch x86`. |
| `i686-pc-windows-gnu`     |  ✅   |   ❌                    | Use with `--features samp-only`.       |

### Repository hygiene

- `.gitignore`: `Cargo.lock` removed (now committed); added
  `dist/` (build-script artefacts), `site/` (MkDocs build),
  `bench-entry.json`, `bench-history.json`, `bench_report.md`,
  `bench_results.txt`, `bench_comparison.txt`, `__pycache__/`,
  `*.pyc`, `release_body.md` (release workflow tempfile),
  `rust-samp-*-src.tar.gz` (local release tarball name), and
  common editor / OS junk (`*.swp`, `*.swo`, `.DS_Store`,
  `Thumbs.db`); consolidated `target` ignore.
- New `.gitattributes` with `export-ignore` rules so docs,
  examples, scripts, `.github/`, `changelog/`, `notes/`,
  `mkdocs.yml`, `ROADMAP.md`, `CHANGELOG.md`, and `migration.md`
  are excluded from the auto-generated "Source code (zip/tar.gz)"
  archives GitHub attaches to every release/tag. Also normalizes
  line endings (`text=auto eol=lf`) so shell scripts survive
  Windows checkouts and tags common binary extensions explicitly.
- `release.yml` hardened: the `tar` step adds defensive
  `--exclude` flags (`target`, `site`, `dist`, `__pycache__`,
  `*.pyc`, `*.rs.bk`, `.DS_Store`, `Thumbs.db`, `.git*`, `*.swp`,
  `*.swo`) and a new verification step fails the release if any
  forbidden path slips into the SDK source tarball.
- Workspace members updated to the renamed example crates.
- Per-crate `authors` field normalized to
  `"ZOTTCE <zottce@gmail.com>", "NullSablex <https://github.com/NullSablex>"`.
