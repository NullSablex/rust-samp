# Logging e Debug

O rust-samp integra as crates `log` e `fern` para logging. Por padrão, mensagens são enviadas ao console do servidor via `logprintf`.

## Logging básico

Sem nenhuma configuração, o plugin já tem logging básico disponível:

```rust
use log::{info, warn, error, debug, trace};

impl SampPlugin for MeuPlugin {
    fn on_load(&mut self) {
        info!("Plugin carregado");
        warn!("Isso é um aviso");
        error!("Isso é um erro");
    }
}
```

## Logging personalizado

Use `samp::plugin::logger()` para obter controle total sobre o logging:

```rust
initialize_plugin!(
    natives: [],
    {
        // Logger padrão SA-MP (envia para logprintf/console)
        let samp_logger = samp::plugin::logger()
            .level(log::LevelFilter::Info);

        // Logger de arquivo
        let log_file = fern::log_file("meu_plugin.log")
            .expect("Falha ao criar arquivo de log");

        let file_logger = fern::Dispatch::new()
            .level(log::LevelFilter::Trace)
            .chain(log_file);

        // Combinar ambos com formato customizado
        let _ = fern::Dispatch::new()
            .format(|callback, message, record| {
                callback.finish(format_args!(
                    "[MeuPlugin][{}]: {}",
                    record.level(),
                    message
                ))
            })
            .chain(samp_logger)
            .chain(file_logger)
            .apply();

        return MeuPlugin;
    }
);
```

### O que isso produz

No console do servidor:
```
[MeuPlugin][INFO]: Plugin carregado
[MeuPlugin][ERROR]: Algo deu errado
```

No arquivo `meu_plugin.log`:
```
[MeuPlugin][TRACE]: Detalhes internos
[MeuPlugin][DEBUG]: Informação de debug
[MeuPlugin][INFO]: Plugin carregado
```

## Níveis de log

| Nível | Uso |
|-------|-----|
| `error!` | Erros que afetam funcionalidade |
| `warn!` | Situações inesperadas mas não críticas |
| `info!` | Eventos importantes (carregamento, conexões) |
| `debug!` | Informação útil para desenvolvimento |
| `trace!` | Detalhes internos granulares |

## Filtrando por nível

```rust
// Só mostra warn e error no console
let samp_logger = samp::plugin::logger()
    .level(log::LevelFilter::Warn);

// Mostra tudo no arquivo
let file_logger = fern::Dispatch::new()
    .level(log::LevelFilter::Trace)
    .chain(log_file);
```

## Dependências necessárias

Para usar logging no seu plugin, adicione ao `Cargo.toml`:

```toml
[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git" }
log = "0.4"
fern = "0.7"  # apenas se quiser personalizar o logging
```

A crate `log` fornece os macros (`info!`, `error!`, etc.). A crate `fern` é opcional — só necessária se quiser configurar destinos e formatos customizados.
