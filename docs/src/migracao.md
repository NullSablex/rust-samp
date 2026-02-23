# Guia de Migração

Este guia ajuda a migrar de versões anteriores do `samp_sdk` para o `samp` atual.

## Mudanças principais

| Antes | Agora |
|-------|-------|
| `samp_sdk = "*"` | `samp = { git = "..." }` |
| `new_plugin!(Plugin)` | `initialize_plugin!({ ... })` |
| `define_native!(name, args)` | `#[native(name = "Name")]` |
| `impl Default for Plugin` | Bloco de inicialização no macro |
| `AMX` (raw) | `Amx` (wrapper seguro) |
| `Cell` | `i32` ou tipos tipados (`Ref<T>`, `AmxString`) |
| Registro manual de natives | Automático via `initialize_plugin!` |

## Passo a passo

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
+ use samp::{native, initialize_plugin};
```

### 3. Substituir define_native! por #[native]

Antes:
```rust
define_native!(my_native, string: String);
define_native!(raw_native as raw);
```

Agora:
```rust
#[native(name = "MyNative")]
fn my_native(&mut self, amx: &Amx, string: AmxString) -> AmxResult<bool> {
    let string = string.to_string();
    // ...
    Ok(true)
}

#[native(name = "RawNative", raw)]
fn raw_native(&mut self, amx: &Amx, args: Args) -> AmxResult<f32> {
    // ...
    Ok(1.0)
}
```

### 4. Substituir new_plugin! por initialize_plugin!

Antes:
```rust
impl Default for Plugin {
    fn default() -> Plugin {
        Plugin { /* ... */ }
    }
}

new_plugin!(Plugin);
```

Agora:
```rust
initialize_plugin!(
    natives: [
        Plugin::my_native,
        Plugin::raw_native,
    ],
    {
        return Plugin { /* ... */ };
    }
);
```

### 5. Atualizar o trait do plugin

Antes:
```rust
impl Plugin {
    fn load(&self) {
        let natives = natives! { "MyNative" => my_native };
        amx.register(&natives);
    }
    fn unload(&self) { }
}
```

Agora:
```rust
impl SampPlugin for Plugin {
    fn on_load(&mut self) {
        // registro de natives é automático
    }
    fn on_unload(&mut self) { }
}
```

### 6. Atualizar tipos de retorno

Antes:
```rust
fn my_native(&self, amx: &AMX, string: String) -> AmxResult<Cell> {
    // ...
}
```

Agora:
```rust
fn my_native(&mut self, amx: &Amx, string: AmxString) -> AmxResult<bool> {
    // ...
}
```

Tipos de retorno suportados: `bool`, `i32`, `f32`, ou qualquer tipo que implemente `AmxCell`.

## Exemplo completo

### Antes
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

### Agora
```rust
use samp::prelude::*;
use samp::{native, initialize_plugin};

struct Plugin;

impl SampPlugin for Plugin {
    fn on_load(&mut self) {
        println!("Plugin carregado.");
    }
}

impl Plugin {
    #[native(name = "MyNative")]
    fn my_native(&mut self, _amx: &Amx, string: AmxString) -> AmxResult<bool> {
        println!("{}", string.to_string());
        Ok(true)
    }
}

initialize_plugin!(
    natives: [Plugin::my_native],
    {
        return Plugin;
    }
);
```
