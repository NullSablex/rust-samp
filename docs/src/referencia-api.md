# Referência da API

Referência rápida dos tipos, traits e macros públicos do rust-samp.

## Prelude

```rust
use samp::prelude::*;
```

Exporta: `Amx`, `AmxExt`, `AmxCell`, `AmxString`, `Buffer`, `Ref`, `UnsizedBuffer`, `AmxResult`, `SampPlugin`.

## Traits

### SampPlugin

```rust
pub trait SampPlugin {
    fn on_load(&mut self) {}
    fn on_unload(&mut self) {}
    fn on_amx_load(&mut self, amx: &Amx) {}
    fn on_amx_unload(&mut self, amx: &Amx) {}
    fn process_tick(&mut self) {}
}
```

### AmxCell\<'amx\>

```rust
pub trait AmxCell<'amx>: Sized {
    fn from_raw(amx: &'amx Amx, cell: i32) -> AmxResult<Self>;
    fn as_cell(&self) -> i32;
}
```

Implementado para: `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `usize`, `isize`, `f32`, `bool`, `&T`, `&mut T`.

### AmxPrimitive

Marker trait para tipos que cabem em uma célula de 32 bits.

### AmxExt

```rust
pub trait AmxExt {
    fn ident(&self) -> AmxIdent;
}
```

## Structs

### Amx

| Método | Assinatura | Descrição |
|--------|-----------|-----------|
| `new` | `(ptr: *mut AMX, fn_table: usize) -> Amx` | Cria wrapper |
| `register` | `(&self, natives: &[AMX_NATIVE_INFO]) -> AmxResult<()>` | Registra natives |
| `exec` | `(&self, index: AmxExecIdx) -> AmxResult<i32>` | Executa função |
| `find_public` | `(&self, name: &str) -> AmxResult<AmxExecIdx>` | Busca public |
| `find_native` | `(&self, name: &str) -> AmxResult<i32>` | Busca native |
| `find_pubvar` | `(&self, name: &str) -> AmxResult<i32>` | Busca variável pública |
| `push` | `(&self, value: T) -> AmxResult<()>` | Empilha valor |
| `get_ref` | `(&self, address: i32) -> AmxResult<Ref<T>>` | Referência a célula |
| `allocator` | `(&self) -> Allocator` | Alocador de memória |
| `strlen` | `(&self, address: i32) -> AmxResult<usize>` | Tamanho de string |
| `flags` | `(&self) -> AmxResult<AmxFlags>` | Flags do AMX |

### Allocator\<'amx\>

| Método | Assinatura | Descrição |
|--------|-----------|-----------|
| `allot` | `(init_value: T) -> AmxResult<Ref<T>>` | Aloca primitivo |
| `allot_buffer` | `(size: usize) -> AmxResult<Buffer>` | Aloca buffer vazio |
| `allot_array` | `(array: &[T]) -> AmxResult<Buffer>` | Aloca e copia array |
| `allot_string` | `(string: &str) -> AmxResult<AmxString>` | Aloca e copia string |

### Ref\<'amx, T\>

Smart pointer para célula AMX. Implementa `Deref` e `DerefMut`.

| Método | Descrição |
|--------|-----------|
| `address()` | Endereço da célula |
| `as_ptr()` | Ponteiro de leitura |
| `as_mut_ptr()` | Ponteiro de escrita |

### AmxString\<'amx\>

| Método | Descrição |
|--------|-----------|
| `to_string()` | Converte para `String` (respeita encoding) |
| `to_bytes()` | Bytes brutos |
| `len()` | Tamanho em caracteres |
| `bytes_len()` | Tamanho em bytes |
| `is_empty()` | Verifica se vazia |

### Buffer\<'amx\>

| Método | Descrição |
|--------|-----------|
| `as_slice()` | `&[i32]` |
| `as_mut_slice()` | `&mut [i32]` |

### UnsizedBuffer\<'amx\>

| Método | Descrição |
|--------|-----------|
| `into_sized_buffer(size)` | Converte para `Buffer` |

### Args\<'a\>

| Método | Descrição |
|--------|-----------|
| `new(amx, args)` | Cria a partir de ponteiro |
| `next_arg::<T>()` | Próximo argumento |
| `get::<T>(offset)` | Argumento por posição |
| `count()` | Número de argumentos |
| `reset()` | Reinicia posição |

## Enums

### AmxError

28 variantes de erro + `Unknown`. Implementa `Display`, `Error`, `From<i32>`.

### AmxExecIdx

| Variante | Valor | Descrição |
|----------|-------|-----------|
| `Main` | -1 | Função principal |
| `Continue` | -2 | Continuação |
| `UserDef(i32)` | N | Função do usuário |

### ServerData

| Variante | Offset |
|----------|--------|
| `Logprintf` | 0 |
| `AmxExports` | 16 |
| `CallPublicFs` | 17 |
| `CallPublicGm` | 18 |

## Bitflags

### Supports

| Flag | Valor |
|------|-------|
| `VERSION` | 512 |
| `AMX_NATIVES` | 0x10000 |
| `PROCESS_TICK` | 0x20000 |

### AmxFlags

`DEBUG`, `COMPACT`, `BYTEOPC`, `NOCHECKS`, `NTVREG`, `JITC`, `BROWSE`, `RELOC`

## Macros

### #[native]

```rust
#[native(name = "NomePAWN")]          // native padrão
#[native(name = "NomePAWN", raw)]     // modo raw com Args
```

### initialize_plugin!

```rust
initialize_plugin!(
    natives: [Struct::metodo, ...],
    { /* inicialização */ return instancia; }
);
```

### exec_public!

```rust
exec_public!(amx, "PublicName");                    // sem argumentos
exec_public!(amx, "PublicName", arg1, arg2);        // com primitivos
exec_public!(amx, "PublicName", var => string);     // com string Rust
exec_public!(amx, "PublicName", &vec => array);     // com array Rust
```

## Módulos

| Caminho | Conteúdo |
|---------|----------|
| `samp::amx` | `Amx`, `AmxExt`, `AmxIdent`, `get()` |
| `samp::plugin` | `SampPlugin`, `enable_process_tick()`, `logger()` |
| `samp::cell` | `AmxCell`, `AmxString`, `Ref`, `Buffer`, `UnsizedBuffer` |
| `samp::error` | `AmxError`, `AmxResult` |
| `samp::args` | `Args` |
| `samp::consts` | `Supports`, `AmxFlags`, `AmxExecIdx`, `ServerData` |
| `samp::encoding` | `set_default_encoding()`, `WINDOWS_1251`, `WINDOWS_1252` |
| `samp::raw` | Tipos FFI brutos |

## Feature flags

| Feature | Descrição |
|---------|-----------|
| `encoding` | Habilita conversão de strings via `encoding_rs` |
