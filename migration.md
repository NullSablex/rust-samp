# Guia de Migração

Para o guia completo de migração entre todas as versões, veja [docs/src/migracao.md](docs/src/migracao.md).

## Migração rápida: samp_sdk legado → v2.2.0

### O que mudou

| Antes | Agora |
|-------|-------|
| `samp_sdk = "*"` | `samp = { git = "https://github.com/NullSablex/rust-samp.git" }` |
| `new_plugin!(Plugin)` | `initialize_plugin!(type: T, natives: [...])` |
| `define_native!(name, args)` | `#[native(name = "Name")]` |
| `impl Default for Plugin` + `new_plugin!` | `#[derive(SampPlugin, Default)]` |
| `AMX` (raw) | `Amx` (wrapper seguro) |
| `Cell` | `i32`, `Ref<T>`, `AmxString` |
| `string.to_string()` | `&*string` (via `Deref<Target=str>`) |
| `f32::from_bits(buf[i] as u32)` | `buf.get_as::<f32>(i)` |

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
