# Changelog

## [Unreleased] - NullSablex (fork)

### Modernização
- Atualizado Rust edition de 2018 para **2024**
- Adicionado workspace `resolver = "3"`
- Substituído `static mut` por `AtomicPtr` em `runtime.rs` e `encoding.rs`
- Substituído `#[no_mangle]` por `#[unsafe(no_mangle)]` (exigido pela edition 2024)
- Adicionadas lifetimes explícitas onde a edition 2024 exige (`amx.rs`)
- Substituído `std::mem::transmute` por `f32::to_bits().cast_signed()` em `repr.rs`

### Dependências
- `syn` 0.15 → 2.0, `proc-macro2` 0.4 → 1.0, `quote` 0.6 → 1.0
- `bitflags` 1.0 → 2.10 (com derives obrigatórios)
- `fern` 0.5 → 0.7
- `memcache` 0.12 → 0.19
- Removida dependência morta `colored` (build-dep sem build.rs)

### Otimizações de código
- `.map().flatten()` → `.filter_map()` no codegen
- `match/Err(_) => ()` → `if let` em `runtime.rs`
- `unsafe { String::from_utf8_unchecked() }` → `String::from_utf8_lossy()` em `string.rs`
- `to_string()` que sombreava `Display` → `convert_to_string()` interno
- Adicionados parênteses de precedência em operação bitshift
- Renomeado `Args::next()` → `Args::next_arg()` para evitar confusão com `Iterator`
- Removidas lifetimes desnecessárias (`strlen`, `add`)
- Adicionada documentação `# Safety` em `AmxPrimitive` e `AmxString::new`

### Infraestrutura
- Removido `.travis.yml`, adicionado GitHub Actions (`.github/workflows/rust.yml`)
- Removido `.rustfmt.toml` obsoleto (`fn_args_density` descontinuado)
- Removida pasta `docs/` (build antigo de `cargo doc`)
- Adicionado `LICENSE` MIT na raiz
- README reescrito em português
- Adicionado `CHANGELOG.md`
- Removidas branches obsoletas (`potential-fix`, `pre-0.9.0`, `async-amx`)

---

## Histórico original (samp-rs por ZOTTCE e colaboradores)

### 0.9.x (2019)
- Nova API do SDK com `AmxString`, `AmxCell`, `Buffer`
- Macros procedurais `#[native]` e `initialize_plugin!` substituindo `define_native!` e `new_plugin!`
- Suporte a packed strings
- Argumentos raw para natives (`#[native(raw)]`)
- Feature `encoding` com suporte a Windows-1251/1252
- Logger integrado via `fern`
- Suporte a `process_tick`
- Macro `exec_public!` para chamar callbacks Pawn
- Migração para Rust edition 2018

### 0.1.x - 0.8.x (2018)
- Bindings iniciais para a SA-MP SDK (AMX)
- Macros `new_plugin!`, `define_native!`, `natives!`
- Funções AMX: `exec`, `find_native`, `find_public`, `push_string`, `push_array`, `allot`, `release`
- Macros utilitárias: `get_string!`, `set_string!`, `get_array!`, `exec_native!`
- Suporte a `ProcessTick`
- Documentação e exemplos

### Contribuições externas
- Kaperstone: exemplos de código melhorados
- povargek: correção de assinatura `Logprintf_t`
- xakdog: CI (Travis/AppVeyor), correção de chamadas nativas no Windows, doctests
- Southclaws: remoção da dependência `detour`
- Sreyas-Sreelal: correções em `push_string`, packed strings, `amxStrLen`, `amxGetAddr`
- Cheaterman: compatibilidade com GDK
