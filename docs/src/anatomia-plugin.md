# Anatomia de um Plugin

Todo plugin rust-samp segue a mesma estrutura: um struct que implementa `SampPlugin`, funções nativas com `#[native]`, e o macro `initialize_plugin!` para juntar tudo.

## A trait SampPlugin

`SampPlugin` define o ciclo de vida do plugin. Todos os métodos são opcionais:

```rust
pub trait SampPlugin {
    /// Chamado quando o servidor carrega o plugin.
    fn on_load(&mut self) {}

    /// Chamado quando o servidor descarrega o plugin.
    fn on_unload(&mut self) {}

    /// Chamado quando um script AMX é carregado (gamemode ou filterscript).
    fn on_amx_load(&mut self, amx: &Amx) {}

    /// Chamado quando um script AMX é descarregado.
    fn on_amx_unload(&mut self, amx: &Amx) {}

    /// Chamado a cada tick do servidor (~5ms). Precisa ser habilitado.
    fn process_tick(&mut self) {}
}
```

Como todos os métodos têm implementação padrão vazia, um plugin sem overrides não precisa escrever `impl SampPlugin` manualmente — use o derive:

```rust
#[derive(SampPlugin, Default)]
struct MeuPlugin;
```

> [!NOTE]
> `#[derive(SampPlugin)]` gera exatamente `impl SampPlugin for MeuPlugin {}`. Se você precisar sobrescrever qualquer método (ex: `on_load`), escreva o `impl` manualmente e remova o derive.

### Estado do plugin

O struct do plugin é mutável (`&mut self`), então você pode armazenar estado:

```rust
struct MeuPlugin {
    jogadores_online: u32,
    conexoes: Vec<String>,
}

impl SampPlugin for MeuPlugin {
    fn on_load(&mut self) {
        self.jogadores_online = 0;
        println!("Plugin pronto.");
    }
}
```

### Ordem de execução

1. **`initialize_plugin! { ... }`** — cria a instância do plugin
2. **`on_load`** — uma vez, quando o servidor inicia e carrega o plugin
3. **`on_amx_load`** — cada vez que um script PAWN é carregado
4. **`process_tick`** — a cada tick do servidor (se habilitado)
5. **`on_amx_unload`** — quando um script PAWN é descarregado
6. **`on_unload`** — uma vez, quando o servidor encerra

## O macro initialize_plugin!

`initialize_plugin!` faz duas coisas:
1. Registra suas funções nativas no servidor
2. Cria a instância do plugin

Existem duas formas:

### Forma simples — `type: T`

Para plugins sem lógica de inicialização. Usa `Default::default()` como construtor:

```rust
#[derive(SampPlugin, Default)]
struct MeuPlugin;

initialize_plugin!(
    type: MeuPlugin,
    natives: [
        MeuPlugin::funcao_a,
        MeuPlugin::funcao_b,
    ],
);
```

> [!TIP]
> Esta é a forma recomendada para novos plugins. Elimina código boilerplate quando não há nenhuma configuração necessária no início.

### Forma completa — bloco construtor

Para plugins que precisam configurar logging, encoding, `process_tick`, ou qualquer lógica antes de retornar a instância:

```rust
initialize_plugin!(
    natives: [
        MeuPlugin::funcao_a,
        MeuPlugin::funcao_b,
    ],
    {
        samp::plugin::enable_process_tick();
        samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1251);

        return MeuPlugin {
            jogadores_online: 0,
            conexoes: Vec::new(),
        };
    }
);
```

> [!IMPORTANT]
> O bloco construtor **deve** terminar com `return <instância>;`. Qualquer código antes do `return` é executado uma única vez durante o carregamento do plugin.

### Sem natives

Se o plugin não registra natives (apenas reage a eventos):

```rust
// Forma simples
initialize_plugin!(type: MeuPlugin, natives: []);

// Forma completa
initialize_plugin!({
    return MeuPlugin;
});
```

## Habilitando process_tick

Por padrão, `process_tick` não é chamado. Para habilitá-lo, chame dentro do bloco construtor:

```rust
initialize_plugin!(
    natives: [],
    {
        samp::plugin::enable_process_tick();
        return MeuPlugin;
    }
);
```

O `process_tick` é útil para tarefas periódicas como verificar filas, processar timers, ou sincronizar estado.

## Diagrama de vida

```
Servidor inicia
  └─ Plugin carrega
       ├─ initialize_plugin! { ... }  ← cria instância
       ├─ on_load()
       ├─ Gamemode carrega → on_amx_load(amx)
       ├─ [loop] process_tick()  (se habilitado)
       ├─ Gamemode descarrega → on_amx_unload(amx)
       └─ on_unload()
Servidor encerra
```
