# Exemplos Avançados

Este capítulo analisa o `plugin-example` incluso no repositório — um plugin de memcache completo que demonstra funcionalidades avançadas.

## Plugin Memcache

O exemplo implementa um cliente memcache com operações CRUD, demonstrando:
- Tipo de retorno customizado com `AmxCell`
- Estado persistente no plugin
- Múltiplos tipos de argumentos nas natives
- Escrita de strings em buffers do PAWN

### Tipo de retorno customizado

```rust
#[derive(Debug, Clone, Copy)]
enum MemcacheResult {
    Success(i32),
    NoData,
    NoClient,
    NoKey,
}

impl AmxCell<'_> for MemcacheResult {
    fn as_cell(&self) -> i32 {
        match self {
            MemcacheResult::Success(result) => *result,
            MemcacheResult::NoData => -1,
            MemcacheResult::NoClient => -2,
            MemcacheResult::NoKey => -3,
        }
    }
}
```

No PAWN, a native retorna um inteiro que codifica o resultado:
```pawn
new result = Memcached_Connect("memcache://127.0.0.1:11211");
if (result >= 0) {
    // result é o ID da conexão
} else if (result == -2) {
    // falha na conexão
}
```

### Estado do plugin

O struct armazena conexões ativas:

```rust
struct Memcached {
    clients: Vec<Client>,
}
```

Cada `Memcached_Connect` adiciona um cliente ao vetor, retornando o índice como ID.

### Natives com diferentes assinaturas

**Conectar** — recebe string, retorna resultado:
```rust
#[native(name = "Memcached_Connect")]
pub fn connect(&mut self, _: &Amx, address: AmxString) -> AmxResult<MemcacheResult> {
    match Client::connect(address.to_string()) {
        Ok(client) => {
            self.clients.push(client);
            Ok(MemcacheResult::Success(self.clients.len() as i32 - 1))
        }
        Err(_) => Ok(MemcacheResult::NoClient),
    }
}
```

**Get com referência** — modifica variável PAWN por referência:
```rust
#[native(name = "Memcached_Get")]
pub fn get(
    &mut self,
    _: &Amx,
    con: usize,           // ID da conexão
    key: AmxString,        // chave
    mut value: Ref<i32>,   // valor de saída (referência)
) -> AmxResult<MemcacheResult> {
    if con < self.clients.len() {
        match self.clients[con].get(&key.to_string()) {
            Ok(Some(data)) => {
                *value = data;  // escreve na variável PAWN
                Ok(MemcacheResult::Success(1))
            }
            Ok(None) => Ok(MemcacheResult::NoData),
            Err(_) => Ok(MemcacheResult::NoKey),
        }
    } else {
        Ok(MemcacheResult::NoClient)
    }
}
```

**Get string com buffer** — escreve string em buffer PAWN:
```rust
#[native(name = "Memcached_GetString")]
pub fn get_string(
    &mut self,
    _: &Amx,
    con: usize,
    key: AmxString,
    buffer: UnsizedBuffer,   // buffer de saída
    size: usize,             // tamanho do buffer
) -> AmxResult<MemcacheResult> {
    if con < self.clients.len() {
        match self.clients[con].get::<String>(&key.to_string()) {
            Ok(Some(data)) => {
                let mut buffer = buffer.into_sized_buffer(size);
                let _ = samp::cell::string::put_in_buffer(&mut buffer, &data);
                Ok(MemcacheResult::Success(1))
            }
            Ok(None) => Ok(MemcacheResult::NoData),
            Err(_) => Ok(MemcacheResult::NoKey),
        }
    } else {
        Ok(MemcacheResult::NoClient)
    }
}
```

### Inicialização completa

O bloco de inicialização demonstra encoding e logging juntos:

```rust
initialize_plugin!(
    natives: [
        Memcached::connect,
        Memcached::get,
        Memcached::set,
        Memcached::get_string,
        Memcached::set_string,
        Memcached::increment,
        Memcached::delete,
    ],
    {
        samp::plugin::enable_process_tick();

        // Encoding cirílico para servidores russos
        samp::encoding::set_default_encoding(samp::encoding::WINDOWS_1251);

        // Logging: console SA-MP + arquivo
        let samp_logger = samp::plugin::logger()
            .level(log::LevelFilter::Info);

        let log_file = fern::log_file("myplugin.log")
            .expect("Falha ao criar arquivo de log");

        let trace_level = fern::Dispatch::new()
            .level(log::LevelFilter::Trace)
            .chain(log_file);

        let _ = fern::Dispatch::new()
            .format(|callback, message, record| {
                callback.finish(format_args!(
                    "memcached {}: {}",
                    record.level().to_string().to_lowercase(),
                    message
                ))
            })
            .chain(samp_logger)
            .chain(trace_level)
            .apply();

        return Memcached {
            clients: Vec::new(),
        };
    }
);
```

## Padrões úteis

### Gerenciando múltiplos AMX

Quando o servidor tem gamemode e filterscripts, cada um recebe seu próprio AMX:

```rust
use std::collections::HashMap;
use samp::amx::AmxExt;

struct MeuPlugin {
    dados_por_amx: HashMap<samp::amx::AmxIdent, Vec<String>>,
}

impl SampPlugin for MeuPlugin {
    fn on_amx_load(&mut self, amx: &Amx) {
        self.dados_por_amx.insert(amx.ident(), Vec::new());
    }

    fn on_amx_unload(&mut self, amx: &Amx) {
        self.dados_por_amx.remove(&amx.ident());
    }
}
```

### Process tick para tarefas periódicas

```rust
struct MeuPlugin {
    tick_count: u64,
}

impl SampPlugin for MeuPlugin {
    fn process_tick(&mut self) {
        self.tick_count += 1;

        // Executar a cada ~5 segundos (1000 ticks)
        if self.tick_count % 1000 == 0 {
            self.tarefa_periodica();
        }
    }
}
```
