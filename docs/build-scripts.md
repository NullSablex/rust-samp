# Build scripts

The repository ships two helper scripts that produce both the Linux
`.so` and the Windows `.dll` in a single command, copying the
artefacts into `dist/`. They live under `scripts/` and are designed
for two host environments:

- `scripts/build-linux.sh` — run from Linux. Builds the `.so` natively
  and cross-compiles the `.dll` via `cargo-xwin`.
- `scripts/build-windows.sh` — run from Windows (Git Bash). Builds the
  `.dll` natively and produces the `.so` through WSL or Docker/cross.

Both scripts read the plugin name from the project's `Cargo.toml`.

## Common behavior

| Setting       | Default                                  | Override                         |
| ------------- | ---------------------------------------- | -------------------------------- |
| Profile       | `release`                                | `PROFILE=dev ./scripts/build-…`  |
| Mode          | SA-MP + native Open Multiplayer          | `--samp-only` flag               |
| Output folder | `dist/` at the workspace root            | not configurable                 |

The scripts:

- Install any missing Rust target via `rustup target add`.
- Install `cargo-xwin` (Linux script) or `cross` (Windows script,
  Docker fallback) automatically when not present.
- Abort if the expected build artefact is missing after the build.
- Log every step in colored output: green for results, yellow for
  steps, red for errors.

## `scripts/build-linux.sh`

Produces:

- `dist/<plugin>.so` — `i686-unknown-linux-gnu`.
- `dist/<plugin>.dll` — `i686-pc-windows-msvc` by default
  (cross-compiled via `cargo-xwin --xwin-arch x86`), or
  `i686-pc-windows-gnu` with `--samp-only`.

Usage:

```sh
./scripts/build-linux.sh                 # default — SA-MP + native Open Multiplayer
./scripts/build-linux.sh --samp-only     # SA-MP only — legacy Open Multiplayer
PROFILE=dev ./scripts/build-linux.sh     # dev profile
```

> `cargo-xwin` downloads the Windows SDK libraries for x86 the first
> time it runs. Without `--xwin-arch x86` the linker fails to locate
> `kernel32.lib`/`advapi32.lib` for the i686 target. The script
> already passes the right flag — there is nothing to configure.

## `scripts/build-windows.sh`

Produces:

- `dist/<plugin>.dll` — `i686-pc-windows-msvc` natively, or
  `i686-pc-windows-gnu` with `--samp-only`.
- `dist/<plugin>.so` — `i686-unknown-linux-gnu`, built through one of
  two paths:

| Path     | When picked                                          | Requirements                                            |
| -------- | ---------------------------------------------------- | ------------------------------------------------------- |
| **WSL**  | Detected first; pick explicitly with `--wsl`         | WSL installed; Rust toolchain available inside WSL.     |
| **Docker** | Fallback when WSL is unavailable; force with `--docker` | Docker Desktop running; `cross` (installed automatically). |

Usage:

```sh
./scripts/build-windows.sh                 # default — autodetect WSL → Docker
./scripts/build-windows.sh --samp-only     # SA-MP only — legacy Open Multiplayer
./scripts/build-windows.sh --wsl           # force WSL for the Linux build
./scripts/build-windows.sh --docker        # force Docker / cross
PROFILE=dev ./scripts/build-windows.sh     # dev profile
```

> The script translates the workspace root path from `/c/...` (Git
> Bash) to `/mnt/c/...` automatically when invoking `wsl`.

## Manual fallback

The scripts only wrap commands that can be run by hand:

```sh
# Linux (.so)
cargo build --release --target i686-unknown-linux-gnu

# Windows MSVC (.dll), cross from Linux
cargo xwin build --release --xwin-arch x86 --target i686-pc-windows-msvc

# Windows GNU (.dll) — SA-MP only
cargo build --release --target i686-pc-windows-gnu --features samp-only
```

Pick whichever workflow fits the host. The helper scripts exist
mainly to:

- Keep the `cargo-xwin` `--xwin-arch x86` flag from being forgotten.
- Produce a uniform `dist/` layout regardless of host.
- Auto-install missing tools the first time around.

## Troubleshooting

| Symptom                                            | Likely cause                                           | Fix                                                 |
| -------------------------------------------------- | ------------------------------------------------------ | --------------------------------------------------- |
| `lld-link: error: could not open 'kernel32.lib'`   | `cargo-xwin` cached the x86_64 SDK only.               | `rm -rf ~/.cache/cargo-xwin && ./scripts/build-linux.sh` |
| `error[E0080]: OmpComponent: invalid size for the Itanium ABI` | Build target is not i686.                              | Use `--target i686-…` or set it in `.cargo/config.toml`. |
| `Neither WSL nor Docker found`                     | Windows host missing both Linux build paths.           | Install WSL (`wsl --install`) or Docker Desktop, then re-run. |
| `Artifact not found: target/<…>/<plugin>.so/.dll`  | The build succeeded for a different binary name.       | Confirm `[package].name` in `Cargo.toml` (dashes become underscores). |
