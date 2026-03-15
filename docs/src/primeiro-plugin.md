# Seu Primeiro Plugin

Vamos criar um plugin completo que registra uma função nativa chamável do PAWN.

## Estrutura mínima

Após seguir o [setup](./setup.md), abra `src/lib.rs` e substitua o conteúdo por:

```rust
use samp::prelude::*;
use samp::{initialize_plugin, SampPlugin};

#[derive(SampPlugin, Default)]
struct MeuPlugin;

initialize_plugin!(
    type: MeuPlugin,
    natives: [],
);
```

Isso já é um plugin válido. `#[derive(SampPlugin)]` gera o ciclo de vida automaticamente e `initialize_plugin!(type: ...)` usa `Default::default()` como construtor.

> [!TIP]
> Se o seu plugin precisar de lógica em `on_load` (logging, encoding, estado inicial), use a forma completa com bloco construtor — descrita em [Anatomia de um Plugin](./anatomia-plugin.md).

## Adicionando uma função nativa

Funções nativas são métodos do seu struct anotados com `#[native]`:

```rust
use samp::prelude::*;
use samp::{native, initialize_plugin, SampPlugin};

#[derive(SampPlugin, Default)]
struct MeuPlugin;

impl MeuPlugin {
    #[native(name = "RustSayHello")]
    fn say_hello(&mut self, _amx: &Amx, name: AmxString) -> AmxResult<bool> {
        // AmxString implementa Deref<Target=str>
        // use &*name para acessar como &str, sem alocação
        println!("Olá, {}!", &*name);
        Ok(true)
    }
}

initialize_plugin!(
    type: MeuPlugin,
    natives: [MeuPlugin::say_hello],
);
```

> [!NOTE]
> `AmxString` implementa `Deref<Target=str>`. Isso significa que todos os métodos de `&str` estão disponíveis diretamente — `name.starts_with("x")`, `name.contains("y")`, `format!("{}", &*name)`. Use `name.to_string()` apenas quando precisar de uma `String` com lifetime próprio.

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

1. `#[derive(SampPlugin)]` gera `impl SampPlugin for MeuPlugin {}` automaticamente — sem precisar escrever os métodos manualmente quando não há overrides
2. `initialize_plugin!(type: MeuPlugin, ...)` usa `MeuPlugin::default()` como construtor — elimina o bloco `{ return MeuPlugin; }`
3. `#[native(name = "RustSayHello")]` gera automaticamente a função `extern "C"` que o SA-MP espera, convertendo argumentos do AMX para tipos Rust
4. `AmxString` recebe a string do AMX — `Deref<Target=str>` permite usá-la como `&str` sem copiar
5. O retorno `AmxResult<bool>` é convertido automaticamente para o valor de retorno da native (1 para `true`, 0 para `false`)

## Próximos passos

- [Anatomia de um Plugin](./anatomia-plugin.md) — entenda o ciclo de vida completo e a forma com construtor
- [Funções Nativas](./natives.md) — aprenda todas as opções do `#[native]`
- [Exemplos Avançados](./exemplos-avancados.md) — veja um plugin real com memcache
