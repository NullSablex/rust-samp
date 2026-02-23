# O Tipo Amx

`Amx` é a interface principal para interagir com a máquina virtual AMX do SA-MP. Cada script PAWN carregado tem sua própria instância AMX.

## Recebendo o Amx

Você recebe uma referência `&Amx` em dois contextos:

1. **Nas funções nativas** — como segundo parâmetro:
   ```rust
   #[native(name = "MyNative")]
   fn my_native(&mut self, amx: &Amx, /* ... */) -> AmxResult<bool> {
       // amx é o script que chamou esta native
       Ok(true)
   }
   ```

2. **Nos hooks do SampPlugin** — `on_amx_load` e `on_amx_unload`:
   ```rust
   fn on_amx_load(&mut self, amx: &Amx) {
       // amx é o script que acabou de carregar
   }
   ```

## AmxIdent

Cada `Amx` tem um identificador único (`AmxIdent`). Use para armazenar e recuperar instâncias depois:

```rust
use samp::amx::AmxExt; // trait que fornece .ident()

struct MeuPlugin {
    scripts: Vec<samp::amx::AmxIdent>,
}

impl SampPlugin for MeuPlugin {
    fn on_amx_load(&mut self, amx: &Amx) {
        self.scripts.push(amx.ident());
    }

    fn on_amx_unload(&mut self, amx: &Amx) {
        self.scripts.retain(|id| *id != amx.ident());
    }
}
```

Para recuperar o `Amx` a partir do `AmxIdent`:

```rust
if let Some(amx) = samp::amx::get(ident) {
    // usar amx
}
```

## Executando funções públicas

Funções `public` do PAWN podem ser chamadas do Rust:

```rust
// Encontrar e executar uma public sem argumentos
let result = amx.find_public("OnMyCallback")
    .and_then(|idx| amx.exec(idx));
```

### Com argumentos — exec_public!

O macro `exec_public!` simplifica a chamada com argumentos:

```rust
use samp::exec_public;

// Sem argumentos
exec_public!(amx, "OnMyCallback");

// Com argumentos primitivos (empilhados na ordem inversa automaticamente)
exec_public!(amx, "OnPlayerScore", player_id, score);

// Com strings Rust (aloca memória AMX automaticamente)
let msg = "Olá!";
exec_public!(amx, "OnMessage", msg => string);

// Com arrays Rust
let dados = vec![1, 2, 3];
exec_public!(amx, "OnData", &dados => array);
```

## Registrando natives manualmente

Normalmente `initialize_plugin!` cuida disso, mas é possível registrar natives manualmente:

```rust
use samp::raw::types::AMX_NATIVE_INFO;

amx.register(&natives)?;
```

## Métodos principais

| Método | Descrição |
|--------|-----------|
| `find_public(name)` | Encontra uma função public por nome |
| `find_native(name)` | Encontra uma função native por nome |
| `exec(index)` | Executa uma função no índice dado |
| `push(value)` | Empilha um valor para a próxima chamada |
| `get_ref(address)` | Obtém uma referência a uma célula AMX |
| `register(natives)` | Registra funções nativas |
| `allocator()` | Acessa o alocador de memória do AMX |
| `strlen(address)` | Retorna o tamanho de uma string no AMX |
| `flags()` | Retorna as flags do AMX |
