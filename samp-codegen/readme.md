# samp-codegen

Macros procedurais para o toolkit rust-samp. Gera as funções FFI `extern "C"` que o SA-MP espera.

> **Nota:** Em geral, você não precisa depender de `samp-codegen` diretamente. Use o crate `samp`, que re-exporta as macros via `use samp::{native, initialize_plugin, SampPlugin}`.

## Macros disponíveis

### `#[native]`

Transforma um método Rust em uma função nativa SA-MP. Gera o wrapper `extern "C"` e o parsing de argumentos automaticamente.

```rust
#[native(name = "MinhaFuncao")]
fn minha_funcao(&mut self, amx: &Amx, nome: AmxString, valor: i32) -> AmxResult<bool> {
    Ok(true)
}
```

### `initialize_plugin!`

Registra as natives e inicializa o plugin. Duas formas:

```rust
// Forma simples — usa Default::default()
initialize_plugin!(
    type: MeuPlugin,
    natives: [MeuPlugin::funcao_a],
);

// Forma completa — bloco de inicialização
initialize_plugin!(
    natives: [MeuPlugin::funcao_a],
    {
        samp::plugin::enable_process_tick();
        return MeuPlugin { /* ... */ };
    }
);
```

### `#[derive(SampPlugin)]`

Gera `impl SampPlugin for T {}` para structs que não precisam sobrescrever métodos do ciclo de vida.

```rust
#[derive(SampPlugin, Default)]
struct MeuPlugin;
```
