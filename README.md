[![Build](https://github.com/NullSablex/rust-samp/actions/workflows/rust.yml/badge.svg)](https://github.com/NullSablex/rust-samp/actions)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

# rust-samp

Toolkit em Rust para desenvolvimento de plugins de servidor [SA-MP](http://sa-mp.com). Escreva plugins seguros, rápidos e confiáveis usando Rust no lugar de C/C++.

> **Nota:** Este projeto é um fork de [samp-rs](https://github.com/Pycckue-Bnepeg/samp-rs), originalmente criado por [ZOTTCE](https://github.com/ZOTTCE). Foi atualizado com dependências modernas, Rust edition 2024 e práticas de segurança aprimoradas.

## Funcionalidades

- Derive de funções nativas SA-MP com o atributo `#[native]`
- Parsing automático de argumentos do AMX para tipos Rust
- Suporte a encoding de strings (Windows-1251, Windows-1252)
- Logging integrado via `fern` e `log`
- Abstrações seguras sobre a API bruta do AMX

## Estrutura do Projeto

| Crate | Descrição |
|---|---|
| `samp` | Crate principal que une tudo (é o que você precisa) |
| `samp-codegen` | Macros procedurais que geram funções FFI `extern "C"` |
| `samp-sdk` | Tipos e bindings de baixo nível para a máquina virtual AMX |

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
       #[native(name = "TestNative")]
       fn my_native(&mut self, _amx: &Amx, text: AmxString) -> AmxResult<bool> {
           let text = text.to_string();
           println!("rust plugin: {}", text);
           Ok(true)
       }
   }

   initialize_plugin!(
       natives: [Plugin::my_native],
       {
           let plugin = Plugin;
           return plugin;
       }
   );
   ```

## Migração de Versões Anteriores

Veja o [guia de migração](migration.md) para atualizar do `samp_sdk` para o `samp`.

## Exemplos

Um exemplo completo de plugin memcache está disponível no diretório [`plugin-example`](plugin-example/).

## Reconhecimentos

Este projeto é baseado no [samp-rs](https://github.com/Pycckue-Bnepeg/samp-rs) por [ZOTTCE](https://github.com/ZOTTCE) e colaboradores. O trabalho original é licenciado sob MIT.

## Licença

MIT
