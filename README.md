[![Build](https://github.com/NullSablex/rust-samp/actions/workflows/rust.yml/badge.svg)](https://github.com/NullSablex/rust-samp/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

# rust-samp

Toolkit em Rust para desenvolvimento de plugins de servidor [SA-MP](http://sa-mp.com). Escreva plugins seguros, rápidos e confiáveis usando Rust no lugar de C/C++.

> **Nota:** Este projeto é um fork de [samp-rs](https://github.com/Pycckue-Bnepeg/samp-rs), originalmente criado por [ZOTTCE](https://github.com/ZOTTCE). Foi atualizado com dependências modernas, Rust edition 2024 e práticas de segurança aprimoradas.

## Funcionalidades

- Derive de funções nativas SA-MP com o atributo `#[native]`
- Parsing automático de argumentos do AMX para tipos Rust
- `AmxString` com `Deref<Target=str>` — use diretamente como `&str`, sem `.to_string()`
- Arrays tipados com `Buffer::get_as::<f32>()` / `set_as::<bool>()` — sem manipulação de bits
- Criação de plugins simplificada com `#[derive(SampPlugin)]` e `initialize_plugin!(type: T, ...)`
- Suporte a encoding de strings (Windows-1251, Windows-1252)
- Logging integrado via `fern` e `log`
- Abstrações seguras sobre a API bruta do AMX

## Começando

1. Instale o [toolchain Rust](https://rustup.rs). Servidores SA-MP são 32-bit, então você precisa do target `i686`:
   ```sh
   rustup target add i686-unknown-linux-gnu   # Linux
   rustup target add i686-pc-windows-msvc     # Windows
   ```

2. Adicione ao seu `Cargo.toml`:
   ```toml
   [lib]
   crate-type = ["cdylib"]

   [dependencies]
   samp = { git = "https://github.com/NullSablex/rust-samp.git" }
   ```

3. Escreva seu plugin:

   **Forma simples** — para plugins sem lógica de inicialização:
   ```rust
   use samp::prelude::*;
   use samp::{native, initialize_plugin, SampPlugin};

   #[derive(SampPlugin, Default)]
   struct MeuPlugin;

   impl MeuPlugin {
       #[native(name = "RustSayHello")]
       fn say_hello(&mut self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
           // AmxString implementa Deref<Target=str> — use como &str diretamente
           println!("Olá, {}!", &*name);
           Ok(true)
       }
   }

   initialize_plugin!(
       type: MeuPlugin,
       natives: [MeuPlugin::say_hello],
   );
   ```

   **Forma completa** — quando há lógica de inicialização (logging, encoding, etc.):
   ```rust
   use samp::prelude::*;
   use samp::{native, initialize_plugin};

   struct MeuPlugin {
       contagem: u32,
   }

   impl SampPlugin for MeuPlugin {
       fn on_load(&mut self) {
           println!("Plugin carregado.");
       }
   }

   impl MeuPlugin {
       #[native(name = "Incrementar")]
       fn incrementar(&mut self, _amx: &Amx) -> AmxResult<i32> {
           self.contagem += 1;
           Ok(self.contagem as i32)
       }
   }

   initialize_plugin!(
       natives: [MeuPlugin::incrementar],
       {
           samp::plugin::enable_process_tick();
           return MeuPlugin { contagem: 0 };
       }
   );
   ```

> [!TIP]
> Use a forma `type: T` sempre que seu plugin não precisar de configuração no `on_load`. Ela elimina o bloco construtor e usa `Default::default()` automaticamente.

## Migração de Versões Anteriores

Veja o [guia de migração](migration.md) para atualizar plugins de versões anteriores.

## Exemplos

Um exemplo completo de plugin memcache está disponível no diretório [`plugin-example`](plugin-example/).

## Reconhecimentos

Este projeto é baseado no [samp-rs](https://github.com/Pycckue-Bnepeg/samp-rs) por [ZOTTCE](https://github.com/ZOTTCE) e colaboradores. O trabalho original é licenciado sob MIT.

## Licença

MIT
