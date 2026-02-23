# Funções Nativas

Funções nativas são funções Rust expostas ao PAWN. O atributo `#[native]` gera automaticamente o wrapper FFI `extern "C"` necessário.

## Sintaxe básica

```rust
impl MeuPlugin {
    #[native(name = "MinhaFuncao")]
    fn minha_funcao(&mut self, amx: &Amx, /* argumentos */) -> AmxResult</* tipo */> {
        // lógica
        Ok(valor)
    }
}
```

### Regras da assinatura

- Primeiro parâmetro: sempre `&mut self`
- Segundo parâmetro: sempre `&Amx` (pode usar `_` se não precisar)
- Demais parâmetros: argumentos da native, convertidos automaticamente
- Retorno: `AmxResult<T>` onde `T` implementa `AmxCell`

## Parâmetro name

O `name` define como a função aparece no PAWN:

```rust
#[native(name = "GetPlayerScore")]
fn get_score(&mut self, _amx: &Amx, player_id: i32) -> AmxResult<i32> {
    Ok(player_id * 100)
}
```

No PAWN:
```pawn
native GetPlayerScore(playerid);
```

## Tipos de argumentos

Os argumentos são convertidos automaticamente do AMX via a trait `AmxCell`:

| Tipo Rust | Equivalente PAWN | Descrição |
|-----------|-----------------|-----------|
| `i32` | `value` | Inteiro |
| `f32` | `Float:value` | Ponto flutuante |
| `bool` | `bool:value` | Booleano |
| `AmxString` | `const string[]` | String (leitura) |
| `Ref<i32>` | `&value` | Referência a inteiro |
| `Ref<f32>` | `&Float:value` | Referência a float |
| `UnsizedBuffer` | `buffer[]` | Array de saída |
| `usize` | `value` | Tamanho/índice |

### Exemplo com vários tipos

```rust
#[native(name = "FormatPlayerInfo")]
fn format_info(
    &mut self,
    _amx: &Amx,
    player_id: i32,
    name: AmxString,
    health: f32,
    buffer: UnsizedBuffer,
    size: usize,
) -> AmxResult<bool> {
    let name = name.to_string();
    let info = format!("[{}] {} - HP: {:.1}", player_id, name, health);

    let mut buffer = buffer.into_sized_buffer(size);
    let _ = samp::cell::string::put_in_buffer(&mut buffer, &info);

    Ok(true)
}
```

No PAWN:
```pawn
native FormatPlayerInfo(playerid, const name[], Float:health, buffer[], size = sizeof(buffer));
```

## Referências mutáveis (Ref)

Use `Ref<T>` para modificar variáveis do PAWN por referência:

```rust
#[native(name = "GetHealth")]
fn get_health(
    &mut self,
    _amx: &Amx,
    player_id: i32,
    mut health: Ref<f32>,
) -> AmxResult<bool> {
    *health = 100.0; // modifica a variável no PAWN
    Ok(true)
}
```

No PAWN:
```pawn
native GetHealth(playerid, &Float:health);

new Float:hp;
GetHealth(0, hp);
// hp agora é 100.0
```

## Modo raw

Para controle total sobre os argumentos, use `raw`:

```rust
use samp::args::Args;

#[native(name = "RawNative", raw)]
fn raw_native(&mut self, amx: &Amx, args: Args) -> AmxResult<bool> {
    let count = args.count();
    let first: Option<i32> = args.get(0);
    let second: Option<AmxString> = args.get(1);

    Ok(true)
}
```

O modo `raw` é útil quando:
- O número de argumentos é variável
- Você precisa de acesso posicional aos argumentos
- A conversão automática não atende seu caso

## Retorno

O tipo de retorno é convertido para `i32` via `as_cell()`:

| Tipo Rust | Valor no PAWN |
|-----------|--------------|
| `bool` | `true` → 1, `false` → 0 |
| `i32` | Valor direto |
| `f32` | Bits do float como inteiro |
| Tipo customizado | Implementação de `AmxCell::as_cell()` |

## Registrando natives

Todas as natives são listadas no `initialize_plugin!`:

```rust
initialize_plugin!(
    natives: [
        MeuPlugin::funcao_a,
        MeuPlugin::funcao_b,
        MeuPlugin::funcao_c,
    ],
    {
        return MeuPlugin;
    }
);
```

A ordem não importa. O nome no PAWN é definido pelo `name` do atributo, não pelo nome do método Rust.
