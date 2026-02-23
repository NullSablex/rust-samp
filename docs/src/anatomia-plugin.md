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

### Ordem de execução

1. **`on_load`** — Uma vez, quando o servidor inicia e carrega o plugin
2. **`on_amx_load`** — Cada vez que um script PAWN é carregado (gamemode, filterscripts)
3. **`process_tick`** — A cada tick do servidor (se habilitado)
4. **`on_amx_unload`** — Quando um script PAWN é descarregado
5. **`on_unload`** — Uma vez, quando o servidor encerra

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

## O macro initialize_plugin!

`initialize_plugin!` faz duas coisas:
1. Registra suas funções nativas no servidor
2. Executa código de inicialização e cria a instância do plugin

### Sintaxe completa

```rust
initialize_plugin!(
    natives: [
        MeuPlugin::funcao_a,
        MeuPlugin::funcao_b,
    ],
    {
        // Código de inicialização (opcional):
        // - Configurar encoding
        // - Configurar logging
        // - Habilitar process_tick

        return MeuPlugin {
            jogadores_online: 0,
            conexoes: Vec::new(),
        };
    }
);
```

### Sem natives

Se o plugin não registra natives (apenas reage a eventos):

```rust
initialize_plugin!({
    return MeuPlugin;
});
```

## Habilitando process_tick

Por padrão, `process_tick` não é chamado. Para habilitá-lo:

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
