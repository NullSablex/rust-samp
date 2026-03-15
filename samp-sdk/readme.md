# samp-sdk

Bindings de baixo nível para a máquina virtual AMX do SA-MP. Este crate expõe os tipos e abstrações usados pelo crate `samp`.

> **Nota:** Em geral, você não precisa depender de `samp-sdk` diretamente. Use o crate `samp`, que re-exporta tudo que você precisa via `use samp::prelude::*`.

## Tipos principais

| Tipo | Descrição |
|------|-----------|
| `AmxString` | String vinda do AMX. Implementa `Deref<Target=str>` — use como `&str` diretamente |
| `Buffer` | Array de células AMX com tamanho conhecido. Suporta `get_as::<T>` e `set_as::<T>` |
| `UnsizedBuffer` | Array sem tamanho — converta com `into_sized_buffer(len)` ou `write_str(len, s)` |
| `Ref<T>` | Referência mutável a uma célula AMX |
| `AmxCell` | Trait para converter entre tipos Rust e células AMX (argumentos de natives) |
| `CellConvert` | Trait para converter elementos individuais de arrays sem precisar de `&Amx` |
| `AmxPrimitive` | Marker trait para tipos que cabem em uma célula de 32 bits |

## CellConvert vs AmxCell

Duas traits de conversão com propósitos distintos:

- **`AmxCell`** — converte argumentos recebidos de uma native. Precisa de `&Amx` para tipos complexos como `AmxString` e `Ref<T>`.
- **`CellConvert`** — converte elementos de arrays (`Buffer::get_as`, `Buffer::set_as`). Não precisa de contexto AMX.

```rust
// AmxCell: argumento de native (automático via #[native])
fn minha_native(&mut self, _amx: &Amx, valor: f32) -> AmxResult<bool> { ... }

// CellConvert: elemento de array
let velocidade: f32 = buffer.get_as(0).unwrap();
buffer.set_as(0, velocidade * 2.0);
```
