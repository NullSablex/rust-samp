# Changelog

## [v2.1.0]

### Segurança

- **[CRITICAL]** Corrigido off-by-one em `Args::get()` que permitia leitura fora dos limites (CWE-125)
- **[CRITICAL]** Adicionada validação de ponteiro nulo em `Amx::get_ref()` após chamada FFI (CWE-416)
- **[HIGH]** Proteção contra integer overflow em `Amx::allot()` com validação de `cells > i32::MAX` (CWE-190)
- **[HIGH]** Bounds checking em packed string parsing para evitar leitura além do buffer (CWE-125)
- **[HIGH]** Corrigido `AtomicPtr` ordering de `Relaxed` para `Release/Acquire` em `encoding.rs` (CWE-362)
- **[MEDIUM]** Adicionado `debug_assert` de ponteiro nulo e alinhamento em `Ref::new()` (CWE-843)
- **[MEDIUM]** Validação de endereço em `Amx::release()` antes de modificar heap (CWE-763)
- **[MEDIUM]** Validação de ponteiro de função em `from_table()` do exports (CWE-476)
- **[MEDIUM]** Eliminado undefined behavior em `Runtime` via `UnsafeCell<RuntimeInner>` (CWE-362)
- **[MEDIUM]** Validação de tamanho em `UnsizedBuffer::into_sized_buffer()` com limite de 1MB (CWE-120)
- **[LOW]** Proteção contra overflow em `Args::count()` com valores negativos (CWE-190)
- **[LOW]** Corrigido `AmxString::new()` usando `bytes.len()` ao invés de `buffer.len()` (CWE-170)
- **[LOW]** Proteção contra OOM em alocação de strings com `MAX_STRING_LEN` de 1MB (CWE-20)
- **[LOW]** Memory leak de nomes de natives tornado explícito via `Box::leak()` (CWE-401)

### Testes

- Adicionados testes para `Args`: `count_with_zero`, `count_with_negative`, `count_with_valid_args`, `get_out_of_bounds`, `reset`
- Adicionados testes para `encoding`: `default_encoding_is_windows_1252`, `set_and_get_encoding`
- Adicionados testes para `AmxError`: `memory_access_display`, `memory_error_display`, `amx_result_ok`, `amx_result_err`
- Total: 32 testes unitários + 27 doctests

### CI/CD

- Adicionado `cargo audit` ao workflow para detecção de CVEs em dependências

### Interno

- `Runtime` refatorado com `UnsafeCell<RuntimeInner>` para interior mutability segura
- `Runtime::get()` agora retorna `&'static Runtime` (imutável) ao invés de `&'static mut`
- Métodos mutáveis do `Runtime` agora usam `&self` com interior mutability
- `CString::into_raw()` em natives substituído por `Box::leak(CString.into_boxed_c_str())`

### Versões

- `samp-sdk`: 2.0.0 → 2.1.0
- `samp`: 2.0.0 → 2.1.0

---

## [v2.0.0]

### CI/CD

- Adicionado build cross-platform i686 no GitHub Actions (Linux + Windows)
- Configurado `Swatinem/rust-cache` para cache de dependências
- Adicionado `cargo clippy` com `-D warnings` ao workflow
- Upload de artefatos (`.so`/`.dll`) em pushes para master

### Tratamento de Erros

- Validação de ponteiros em blocos `unsafe` do SDK
- Mensagens de erro em português nas macros procedurais
- Uso de `Result` em mais pontos da API pública

### Testes

- Testes unitários para `AmxPrimitive`, `AmxError`, `consts`, `Exports`
- Correção de doctests (`compile_fail` → `no_run` onde necessário)
- 21 testes unitários + 27 doctests

### Documentação

- Documentação completa com mdBook (13 capítulos em português)
- Guias: introdução, primeiro plugin, anatomia, natives, tipos Amx, cells/memória, encoding, erros, logging, exemplos avançados, migração, referência API

---

## [v1.0.1]

- Atualizado repositório, versões e autoria nos Cargo.toml de todos os crates

---

## [v1.0.0]

### Correções pós-release
- Corrigido nome do projeto de "Rust-SAMP" para "rust-samp" no README e LICENSE
- Corrigido doctests marcados como `compile_fail` que passaram a compilar com as dependências atualizadas (`compile_fail` → `no_run`)
- Corrigido `#[no_mangle]` → `#[unsafe(no_mangle)]` nos doctests do samp-sdk (edition 2024)

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
