# Células e Memória

A máquina virtual AMX trabalha com **células de 32 bits**. Todo valor no AMX — inteiros, floats, booleanos, endereços — é uma célula `i32`. O rust-samp fornece abstrações para trabalhar com essas células de forma segura.

## AmxCell

`AmxCell` é a trait central do SDK. Ela define como converter valores entre Rust e AMX:

```rust
pub trait AmxCell<'amx> {
    /// Converte um valor AMX bruto para o tipo Rust.
    fn from_raw(amx: &'amx Amx, cell: i32) -> AmxResult<Self>;

    /// Converte o valor Rust para uma célula AMX.
    fn as_cell(&self) -> i32;
}
```

### Tipos primitivos implementados

| Tipo | `as_cell()` | `from_raw()` |
|------|------------|-------------|
| `i32` | Identidade | Identidade |
| `f32` | `to_bits()` | `from_bits()` |
| `bool` | `true` → 1, `false` → 0 | `!= 0` → `true` |
| `i8`, `u8`, `i16`, `u16` | Cast para `i32` | Cast do `i32` |
| `u32`, `usize`, `isize` | Cast para `i32` | Cast do `i32` |

### Implementação customizada

Você pode implementar `AmxCell` para seus próprios tipos:

```rust
#[derive(Debug, Clone, Copy)]
enum Resultado {
    Sucesso(i32),
    Erro,
}

impl AmxCell<'_> for Resultado {
    fn as_cell(&self) -> i32 {
        match self {
            Resultado::Sucesso(v) => *v,
            Resultado::Erro => -1,
        }
    }
}
```

Isso permite retornar `AmxResult<Resultado>` em suas natives.

## AmxPrimitive

`AmxPrimitive` é um marker trait para tipos que cabem em uma única célula de 32 bits. É usado como constraint em genéricos que precisam trabalhar com valores no stack/heap do AMX.

Tipos que implementam: `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `usize`, `isize`, `f32`, `bool`.

## Ref\<T\>

`Ref<T>` é um smart pointer para uma célula no AMX. Permite ler e modificar valores de variáveis PAWN por referência:

```rust
#[native(name = "GetValues")]
fn get_values(
    &mut self,
    _amx: &Amx,
    mut health: Ref<f32>,
    mut armor: Ref<f32>,
) -> AmxResult<bool> {
    *health = 100.0;
    *armor = 50.0;
    Ok(true)
}
```

`Ref<T>` implementa `Deref` e `DerefMut`, então você acessa o valor diretamente com `*`.

### Métodos

| Método | Descrição |
|--------|-----------|
| `address()` | Retorna o endereço da célula no AMX |
| `as_ptr()` | Ponteiro bruto (leitura) |
| `as_mut_ptr()` | Ponteiro bruto (escrita) |

## AmxString

`AmxString` representa uma string vinda do AMX. Suporta strings packed e unpacked:

```rust
#[native(name = "PrintString")]
fn print_string(&mut self, _amx: &Amx, text: AmxString) -> AmxResult<bool> {
    let rust_string = text.to_string(); // converte para String Rust
    let bytes = text.to_bytes();        // bytes brutos
    let len = text.len();               // tamanho em caracteres
    let is_empty = text.is_empty();     // verifica se vazia

    println!("{}", rust_string);
    Ok(true)
}
```

A conversão `.to_string()` respeita o encoding configurado (veja [Encoding](./encoding.md)).

## Buffer e UnsizedBuffer

### UnsizedBuffer

`UnsizedBuffer` é um buffer temporário vindo do AMX. Precisa ser convertido para `Buffer` com o tamanho:

```rust
#[native(name = "FillBuffer")]
fn fill_buffer(
    &mut self,
    _amx: &Amx,
    buffer: UnsizedBuffer,
    size: usize,
) -> AmxResult<bool> {
    let mut buffer = buffer.into_sized_buffer(size);
    // agora buffer é um Buffer com tamanho conhecido
    Ok(true)
}
```

### Buffer

`Buffer` é um array de células com tamanho conhecido:

```rust
let slice = buffer.as_slice();       // &[i32]
let mut_slice = buffer.as_mut_slice(); // &mut [i32]
```

### Escrevendo strings em buffers

Use `put_in_buffer` para copiar strings Rust para um buffer AMX:

```rust
use samp::cell::string::put_in_buffer;

let mut buffer = unsized_buffer.into_sized_buffer(size);
let _ = put_in_buffer(&mut buffer, "texto para o PAWN");
```

## Allocator

O `Allocator` aloca memória no heap do AMX. Útil ao chamar funções public que esperam referências:

```rust
let allocator = amx.allocator();

// Alocar um primitivo
let cell = allocator.allot(42i32)?;

// Alocar um array
let buf = allocator.allot_array(&[1, 2, 3])?;

// Alocar uma string
let string = allocator.allot_string("olá")?;

// Alocar buffer vazio
let empty_buf = allocator.allot_buffer(256)?;
```

A memória alocada é automaticamente liberada quando o `Allocator` sai de escopo.
