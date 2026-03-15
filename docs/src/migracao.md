# Guia de Migração

## v2.1.0 → v2.2.0

Esta versão traz melhorias de ergonomia sem breaking changes. O código antigo continua compilando, mas os padrões novos são mais simples e devem ser preferidos.

### 1. Criação de plugins — forma simplificada

**Antes:**
```rust
struct MeuPlugin;

impl SampPlugin for MeuPlugin {}

initialize_plugin!(
    natives: [MeuPlugin::my_native],
    {
        return MeuPlugin;
    }
);
```

**Agora:**
```rust
#[derive(SampPlugin, Default)]
struct MeuPlugin;

initialize_plugin!(
    type: MeuPlugin,
    natives: [MeuPlugin::my_native],
);
```

> [!NOTE]
> `#[derive(SampPlugin)]` gera `impl SampPlugin for T {}` apenas para structs **sem overrides**. Se você precisar de `on_load`, `on_amx_load`, ou qualquer outro método, continue usando `impl SampPlugin for T` manualmente — e nesse caso use a forma completa do `initialize_plugin!`.

**Quando usar cada forma:**

| Situação | Forma recomendada |
|----------|------------------|
| Sem lógica de inicialização | `initialize_plugin!(type: T, natives: [...])` |
| Com `on_load`, logging, encoding | `initialize_plugin!(natives: [...], { ... })` |
| Com estado inicial no struct | `initialize_plugin!(natives: [...], { return T { ... }; })` |

---

### 2. AmxString — use Deref em vez de to_string()

`AmxString` agora implementa `Deref<Target=str>`. Todos os métodos de `&str` estão disponíveis diretamente.

**Antes:**
```rust
fn say_hello(&mut self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
    let name = name.to_string();
    println!("Olá, {}!", name);
    Ok(true)
}
```

**Agora:**
```rust
fn say_hello(&mut self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
    println!("Olá, {}!", &*name);
    Ok(true)
}
```

Mais exemplos de uso via Deref:
```rust
// Comparação
if name.starts_with("Admin") { }
if name.contains("vip") { }

// Formatação
let msg = format!("Bem-vindo, {}!", &*name);

// Passar para funções que esperam &str
connect_to_server(&*name);
```

> [!NOTE]
> A decodificação é **lazy**: a `String` só é alocada no primeiro acesso via `Deref`. Se você nunca usar `&*name`, não há alocação. Use `.to_string()` apenas quando precisar de uma `String` com ownership independente.

> [!TIP]
> `println!("{}", &*name)` e `format!("{name}")` funcionam porque `AmxString` implementa `Display` via Deref. Não é necessário converter para `String` para imprimir.

---

### 3. Escrita de strings — write_str

**Antes:**
```rust
fn get_value(_amx: &Amx, buffer: UnsizedBuffer, size: usize) -> AmxResult<bool> {
    let mut buf = buffer.into_sized_buffer(size);
    let _ = samp::cell::string::put_in_buffer(&mut buf, "valor");
    Ok(true)
}
```

**Agora:**
```rust
fn get_value(_amx: &Amx, buffer: UnsizedBuffer, size: usize) -> AmxResult<bool> {
    buffer.write_str(size, "valor")?;
    Ok(true)
}
```

> [!IMPORTANT]
> O padrão antigo silenciava erros com `let _ = ...`. O `write_str` com `?` propaga o erro corretamente — se a string for grande demais para o buffer, o erro chega ao caller.

Também disponível para `Buffer` já dimensionado:
```rust
let mut buf = allocator.allot_buffer(32)?;
buf.write_str("Olá, mundo!")?;
```

---

### 4. Arrays tipados — get_as / set_as

Para ler `Float:arr[]` ou `bool:arr[]` do PAWN, não é mais necessário manipular bits manualmente.

**Antes:**
```rust
fn processar_floats(_amx: &Amx, array: UnsizedBuffer, len: usize) -> AmxResult<bool> {
    let buf = array.into_sized_buffer(len);
    for i in 0..buf.len() {
        let valor = f32::from_bits(buf[i] as u32); // manipulação manual
        println!("{}", valor);
    }
    Ok(true)
}
```

**Agora:**
```rust
fn processar_floats(_amx: &Amx, array: UnsizedBuffer, len: usize) -> AmxResult<bool> {
    let buf = array.into_sized_buffer(len);
    for i in 0..buf.len() {
        if let Some(valor) = buf.get_as::<f32>(i) {
            println!("{}", valor);
        }
    }
    Ok(true)
}
```

Tipos suportados por `get_as`/`set_as`: `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `usize`, `isize`, `f32`, `bool`.

> [!NOTE]
> `CellConvert` (usado por `get_as`/`set_as`) é diferente de `AmxCell` (usado para argumentos de natives):
>
> - **`AmxCell`**: converte argumentos recebidos do AMX — precisa de contexto `&Amx` para tipos complexos (`AmxString`, `Ref<T>`)
> - **`CellConvert`**: converte células individuais de arrays — não precisa de `&Amx`, opera diretamente em `i32`
>
> Use `CellConvert` para operar em elementos de `Buffer`. Use `AmxCell` para tipar argumentos de natives.

---

## samp_sdk legado → v2.x

Este guia cobre a migração do `samp_sdk` original (pré-v1) para a API atual.

### Resumo das mudanças

| Antes | Agora |
|-------|-------|
| `samp_sdk = "*"` | `samp = { git = "..." }` |
| `new_plugin!(Plugin)` | `initialize_plugin!(type: T, ...)` ou bloco |
| `define_native!(name, args)` | `#[native(name = "Name")]` |
| `impl Default for Plugin` | `#[derive(Default)]` ou bloco construtor |
| `AMX` (raw) | `Amx` (wrapper seguro) |
| `Cell` | `i32` ou tipos tipados (`Ref<T>`, `AmxString`) |
| Registro manual de natives | Automático via `initialize_plugin!` |
| `string.to_string()` | `&*string` via Deref |

### 1. Atualizar Cargo.toml

```diff
- [dependencies]
- samp_sdk = "*"

+ [dependencies]
+ samp = { git = "https://github.com/NullSablex/rust-samp.git" }
```

### 2. Atualizar imports

```diff
- use samp_sdk::new_plugin;
- use samp_sdk::...;

+ use samp::prelude::*;
+ use samp::{native, initialize_plugin, SampPlugin};
```

### 3. Substituir define_native! por #[native]

**Antes:**
```rust
define_native!(my_native, string: String);
define_native!(raw_native as raw);
```

**Agora:**
```rust
#[native(name = "MyNative")]
fn my_native(&mut self, _amx: &Amx, string: AmxString) -> AmxResult<bool> {
    println!("{}", &*string);
    Ok(true)
}

#[native(name = "RawNative", raw)]
fn raw_native(&mut self, amx: &Amx, args: Args) -> AmxResult<f32> {
    Ok(1.0)
}
```

### 4. Substituir new_plugin! por initialize_plugin!

**Antes:**
```rust
impl Default for Plugin {
    fn default() -> Plugin { Plugin { /* ... */ } }
}
new_plugin!(Plugin);
```

**Agora (forma simples):**
```rust
#[derive(SampPlugin, Default)]
struct Plugin;

initialize_plugin!(
    type: Plugin,
    natives: [Plugin::my_native],
);
```

**Agora (com inicialização):**
```rust
initialize_plugin!(
    natives: [Plugin::my_native],
    {
        return Plugin { /* ... */ };
    }
);
```

### 5. Atualizar o trait do plugin

**Antes:**
```rust
impl Plugin {
    fn load(&self) {
        let natives = natives! { "MyNative" => my_native };
        amx.register(&natives);
    }
    fn unload(&self) { }
}
```

**Agora:**
```rust
// Sem overrides: use o derive
#[derive(SampPlugin)]
struct Plugin;

// Com overrides: impl manual
impl SampPlugin for Plugin {
    fn on_load(&mut self) {
        // registro de natives é automático
    }
    fn on_unload(&mut self) { }
}
```

### Exemplo completo

**Antes:**
```rust
use samp_sdk::new_plugin;

define_native!(my_native, string: String);

pub struct Plugin;

impl Plugin {
    fn load(&self) {
        let natives = natives! { "MyNative" => my_native };
        amx.register(&natives);
    }

    fn my_native(&self, amx: &AMX, string: String) -> AmxResult<Cell> {
        println!("{}", string);
        Ok(1)
    }
}

impl Default for Plugin {
    fn default() -> Plugin { Plugin }
}

new_plugin!(Plugin);
```

**Agora:**
```rust
use samp::prelude::*;
use samp::{native, initialize_plugin, SampPlugin};

#[derive(SampPlugin, Default)]
struct Plugin;

impl Plugin {
    #[native(name = "MyNative")]
    fn my_native(&mut self, _amx: &Amx, string: AmxString) -> AmxResult<bool> {
        println!("{}", &*string);
        Ok(true)
    }
}

initialize_plugin!(
    type: Plugin,
    natives: [Plugin::my_native],
);
```
