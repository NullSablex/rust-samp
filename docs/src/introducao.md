# Introdução

**rust-samp** é um toolkit em Rust para desenvolvimento de plugins de servidor [SA-MP](http://sa-mp.com) (San Andreas Multiplayer). Com ele, você escreve plugins seguros, rápidos e confiáveis usando Rust no lugar de C/C++.

> Este projeto é um fork de [samp-rs](https://github.com/Pycckue-Bnepeg/samp-rs), originalmente criado por [ZOTTCE](https://github.com/ZOTTCE). Foi atualizado com dependências modernas, Rust edition 2024 e práticas de segurança aprimoradas.

## Por que Rust?

Plugins SA-MP tradicionais são escritos em C/C++, onde erros de memória (buffer overflow, use-after-free, dangling pointers) são a principal causa de crashes no servidor. Rust elimina essas classes de bugs em tempo de compilação, enquanto mantém performance equivalente a C.

Com rust-samp, você ganha:

- **Segurança de memória** sem garbage collector
- **Derive de funções nativas** com o atributo `#[native]` — sem boilerplate FFI manual
- **Parsing automático** de argumentos do AMX para tipos Rust
- **Suporte a encoding** de strings (Windows-1251 para cirílico, Windows-1252 para latim)
- **Logging integrado** via `fern` e `log`
- **Abstrações seguras** sobre a API bruta do AMX

## Estrutura do Projeto

rust-samp é organizado como um workspace Cargo com 3 crates publicáveis:

| Crate | Descrição |
|---|---|
| `samp` | Crate principal — é o que você adiciona como dependência |
| `samp-codegen` | Macros procedurais que geram as funções FFI `extern "C"` |
| `samp-sdk` | Tipos e bindings de baixo nível para a máquina virtual AMX |

Na prática, você só precisa depender de `samp`. Ele reexporta tudo que é necessário de `samp-sdk` e `samp-codegen`.

## Exemplo rápido

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

Nas próximas páginas, vamos configurar o ambiente e construir um plugin do zero.
