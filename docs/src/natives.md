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
- Demais parâmetros: argumentos da native, convertidos automaticamente via `AmxCell`
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
| `AmxString` | `const string[]` | String de entrada (leitura) |
| `Ref<i32>` | `&value` | Referência a inteiro |
| `Ref<f32>` | `&Float:value` | Referência a float |
| `UnsizedBuffer` | `buffer[]` | Array de saída |
| `usize` | `value` | Tamanho/índice |

## Trabalhando com strings — AmxString

`AmxString` implementa `Deref<Target=str>`. Isso significa que você pode usar todos os métodos de `&str` diretamente, além de comparar com literais:

```rust
#[native(name = "ProcessarNome")]
fn processar_nome(&mut self, _amx: &Amx, nome: AmxString) -> AmxResult<bool> {
    // Comparação direta com &str — sem alocação
    if nome == "Admin" {
        println!("[ADMIN] Bem-vindo!");
    } else if nome.starts_with("VIP_") {
        println!("[VIP] Bem-vindo, {}!", &nome);
    } else {
        println!("Olá, {}! ({} chars)", &nome, nome.len());
    }
    Ok(true)
}
```

Quando precisar passar para funções que esperam `&str`, use `&nome` (auto-deref) ou `nome.as_str()` (explícito):

```rust
// Ambos equivalentes:
some_fn(&nome);          // auto-deref &AmxString → &str
some_fn(nome.as_str());  // explícito, mais legível para quem não conhece Deref
```

> [!NOTE]
> A decodificação de `AmxString` para `String` é **lazy**: a alocação só acontece no primeiro acesso via `Deref` ou `.as_str()`. Use `.to_string()` apenas quando precisar de uma `String` com lifetime independente.

## Escrevendo strings de saída — UnsizedBuffer

Para natives que retornam uma string ao PAWN via `buffer[]`:

```rust
#[native(name = "GetPlayerInfo")]
fn get_player_info(
    &mut self,
    _amx: &Amx,
    player_id: i32,
    buffer: UnsizedBuffer,
    size: usize,
) -> AmxResult<bool> {
    let info = format!("Jogador #{}", player_id);
    // write_str combina into_sized_buffer + put_in_buffer em um passo
    buffer.write_str(size, &info)?;
    Ok(true)
}
```

No PAWN:
```pawn
native GetPlayerInfo(playerid, buffer[], size = sizeof(buffer));
```

> [!TIP]
> `UnsizedBuffer::write_str(size, s)` é equivalente ao padrão manual:
> ```rust
> let mut buf = buffer.into_sized_buffer(size);
> samp::cell::string::put_in_buffer(&mut buf, &info)?;
> ```
> Prefira `write_str` — é mais conciso e propaga o erro com `?`.

## Arrays tipados — get_as / set_as

Para natives que recebem arrays (`Float:arr[]`, `bool:arr[]`), use `Buffer::iter_as`, `get_as` e `set_as`.
Não é necessário importar `CellConvert` — `use samp::prelude::*` é suficiente:

```rust
use samp::prelude::*;

#[native(name = "SomarFloats")]
fn somar_floats(
    &mut self,
    _amx: &Amx,
    array: UnsizedBuffer,
    len: usize,
) -> AmxResult<f32> {
    let buf = array.into_sized_buffer(len);
    // iter_as — itera todas as células já convertidas para f32
    Ok(buf.iter_as::<f32>().sum())
}
```

```rust
#[native(name = "EscalarArray")]
fn escalar_array(
    &mut self,
    _amx: &Amx,
    array: UnsizedBuffer,
    len: usize,
    fator: f32,
) -> AmxResult<bool> {
    let mut buf = array.into_sized_buffer(len);
    for i in 0..buf.len() {
        if let Some(v) = buf.get_as::<f32>(i) {
            buf.set_as(i, v * fator);
        }
    }
    Ok(true)
}
```

> [!NOTE]
> **`get_as`/`set_as` usam o trait `CellConvert` internamente, mas você não precisa saber disso para usá-los.**
> Basta chamar `buf.get_as::<f32>(i)` — o tipo no turbofish é suficiente.
>
> Se encontrar `CellConvert` na documentação da API e se perguntar quando usá-lo diretamente:
>
> | Trait | Use quando... |
> |-------|--------------|
> | `AmxCell` | Declarar o tipo de um argumento de `#[native]` (`AmxString`, `Ref<T>`, `i32`...) |
> | `CellConvert` | Implementar suporte a um tipo customizado em `get_as`/`set_as` |
>
> Para o uso diário, `get_as`/`set_as` no `Buffer` são a única interface necessária.

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
    type: MeuPlugin,
    natives: [
        MeuPlugin::funcao_a,
        MeuPlugin::funcao_b,
        MeuPlugin::funcao_c,
    ],
);
```

A ordem não importa. O nome no PAWN é definido pelo `name` do atributo, não pelo nome do método Rust.
