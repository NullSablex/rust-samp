# Seu Primeiro Plugin

Vamos criar um plugin completo que registra uma função nativa chamável do PAWN.

## Estrutura mínima

Após seguir o [setup](./setup.md), abra `src/lib.rs` e substitua o conteúdo por:

```rust
use samp::prelude::*;
use samp::{native, initialize_plugin};

struct MeuPlugin;

impl SampPlugin for MeuPlugin {
    fn on_load(&mut self) {
        println!("MeuPlugin carregado com sucesso!");
    }
}

initialize_plugin!(
    natives: [],
    {
        return MeuPlugin;
    }
);
```

Isso já é um plugin válido. Ao carregar no servidor, a mensagem aparece no console.

## Adicionando uma função nativa

Funções nativas são métodos do seu struct anotados com `#[native]`:

```rust
use samp::prelude::*;
use samp::{native, initialize_plugin};

struct MeuPlugin;

impl SampPlugin for MeuPlugin {
    fn on_load(&mut self) {
        println!("MeuPlugin carregado!");
    }
}

impl MeuPlugin {
    #[native(name = "RustSayHello")]
    fn say_hello(&mut self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
        let name = name.to_string();
        println!("Olá, {}!", name);
        Ok(true)
    }
}

initialize_plugin!(
    natives: [MeuPlugin::say_hello],
    {
        return MeuPlugin;
    }
);
```

## Chamando do PAWN

No seu script PAWN, declare a native e use normalmente:

```pawn
native RustSayHello(const name[]);

public OnGameModeInit()
{
    RustSayHello("Mundo");
    return 1;
}
```

O servidor imprimirá: `Olá, Mundo!`

## O que aconteceu?

1. `SampPlugin` define o ciclo de vida do plugin — `on_load` é chamado quando o servidor carrega o plugin
2. `#[native(name = "RustSayHello")]` gera automaticamente uma função `extern "C"` que o SA-MP espera, convertendo argumentos do AMX para tipos Rust
3. `initialize_plugin!` registra as natives e inicializa o plugin — o bloco `{ ... }` é executado uma vez durante o carregamento
4. `AmxString` é a representação de uma string vinda do AMX — use `.to_string()` para convertê-la em `String` Rust
5. O retorno `AmxResult<bool>` é convertido automaticamente para o valor de retorno da native (1 para `true`, 0 para `false`)

## Próximos passos

- [Anatomia de um Plugin](./anatomia-plugin.md) — entenda o ciclo de vida completo
- [Funções Nativas](./natives.md) — aprenda todas as opções do `#[native]`
- [Exemplos Avançados](./exemplos-avancados.md) — veja um plugin real com memcache
