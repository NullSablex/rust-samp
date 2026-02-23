# Encoding de Strings

O SA-MP usa encodings de texto legados (Windows codepages) em vez de UTF-8. O rust-samp oferece suporte transparente para converter entre esses encodings e strings Rust.

## Por que encoding importa?

Strings no AMX são sequências de bytes em um codepage específico:
- **Windows-1252** — Latim estendido (padrão em servidores ocidentais)
- **Windows-1251** — Cirílico (servidores russos/eslavos)

Rust usa UTF-8 internamente. Sem conversão adequada, caracteres acentuados ou cirílicos aparecem corrompidos.

## Habilitando o suporte

A feature `encoding` precisa estar habilitada no `Cargo.toml`:

```toml
[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git", features = ["encoding"] }
```

## Configurando o encoding

Defina o encoding padrão na inicialização do plugin:

```rust
initialize_plugin!(
    natives: [],
    {
        // Para servidores com texto em português/espanhol/francês:
        samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1252);

        // Para servidores com texto em russo/ucraniano:
        // samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1251);

        return MeuPlugin;
    }
);
```

Se você não chamar `set_default_encoding`, o padrão é **Windows-1252**.

## Como funciona

O encoding configurado é usado automaticamente em:

1. **`AmxString::to_string()`** — converte bytes AMX para `String` Rust (UTF-8)
2. **`Allocator::allot_string()`** — converte `&str` Rust para bytes AMX

```rust
#[native(name = "ProcessText")]
fn process_text(&mut self, _amx: &Amx, text: AmxString) -> AmxResult<bool> {
    // Converte de Windows-1252/1251 para UTF-8 automaticamente
    let rust_string = text.to_string();
    println!("{}", rust_string); // caracteres acentuados exibidos corretamente

    Ok(true)
}
```

## Encodings disponíveis

| Constante | Codepage | Uso |
|-----------|----------|-----|
| `WINDOWS_1252` | CP-1252 | Latim estendido (padrão) |
| `WINDOWS_1251` | CP-1251 | Cirílico |

## Detalhes técnicos

O encoding é armazenado em um `AtomicPtr`, o que garante segurança entre threads. A configuração é global — afeta todas as conversões de string do plugin.

```rust
// Internamente, set_default_encoding faz:
static DEFAULT_ENCODING: AtomicPtr<Encoding> = ...;

pub fn set_default_encoding(encoding: &'static Encoding) {
    DEFAULT_ENCODING.store(encoding as *const _ as *mut _, Ordering::Relaxed);
}
```

## Quando não usar encoding

Se seu servidor usa apenas ASCII (letras A-Z, números, símbolos básicos), o encoding é irrelevante — ASCII é idêntico nos três formatos (UTF-8, 1251, 1252).

Habilite encoding apenas se precisar de:
- Caracteres acentuados (á, é, ñ, ç)
- Caracteres cirílicos (а, б, в, г)
- Outros caracteres fora do ASCII básico
