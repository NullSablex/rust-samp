# Tratamento de Erros

O rust-samp usa o padrão Rust de `Result` para tratamento de erros, com tipos específicos para erros da VM AMX.

## AmxResult\<T\>

`AmxResult<T>` é um alias para `Result<T, AmxError>`:

```rust
pub type AmxResult<T> = Result<T, AmxError>;
```

Toda função nativa retorna `AmxResult<T>`:

```rust
#[native(name = "Dividir")]
fn dividir(&mut self, _amx: &Amx, a: i32, b: i32) -> AmxResult<i32> {
    if b == 0 {
        return Err(AmxError::Divide); // divisão por zero
    }
    Ok(a / b)
}
```

## AmxError

`AmxError` representa os 28 tipos de erro da VM AMX:

| Variante | Descrição |
|----------|-----------|
| `Exit` | Saída forçada |
| `Assert` | Asserção falhou |
| `StackError` | Colisão stack/heap |
| `Bounds` | Índice fora dos limites |
| `MemoryAccess` | Acesso a memória inválida |
| `InvalidInstruction` | Instrução inválida |
| `StackLow` | Stack underflow |
| `HeapLow` | Heap underflow |
| `Callback` | Callback inválido |
| `Native` | Função nativa falhou |
| `Divide` | Divisão por zero |
| `Sleep` | Modo sleep |
| `InvalidState` | Estado inválido |
| `Memory` | Sem memória |
| `Format` | Formato de arquivo inválido |
| `Version` | Versão incompatível |
| `NotFound` | Função não encontrada |
| `Index` | Índice de entrada inválido |
| `Debug` | Debugger não pode rodar |
| `Init` | AMX não inicializado |
| `UserData` | Erro ao definir dados do usuário |
| `InitJit` | Erro ao inicializar JIT |
| `Params` | Erro de parâmetros |
| `Domain` | Resultado fora do domínio |
| `General` | Erro genérico |
| `Overlay` | Overlays não suportados |
| `Unknown` | Erro desconhecido |

## Propagação com ?

Use o operador `?` para propagar erros nas funções nativas:

```rust
#[native(name = "CallCallback")]
fn call_callback(&mut self, amx: &Amx) -> AmxResult<bool> {
    // find_public retorna AmxResult — propaga erro se não encontrar
    let index = amx.find_public("OnMyCallback")?;

    // exec também retorna AmxResult
    let result = amx.exec(index)?;

    Ok(result > 0)
}
```

## Conversão de códigos de erro

`AmxError` converte de/para `i32` automaticamente:

```rust
let err = AmxError::from(19); // AmxError::NotFound
let code = AmxError::NotFound as i32; // 19
```

Códigos desconhecidos viram `AmxError::Unknown`.

## Display

Todos os erros têm mensagens legíveis:

```rust
let err = AmxError::NotFound;
println!("{}", err); // "Function not found"
```

## Padrões comuns

### Retornar erro ao PAWN via valor

Em vez de retornar `Err`, é comum retornar um código de status:

```rust
#[native(name = "TryConnect")]
fn try_connect(&mut self, _amx: &Amx, address: AmxString) -> AmxResult<i32> {
    match self.connect(&address.to_string()) {
        Ok(_) => Ok(1),   // sucesso
        Err(_) => Ok(-1), // falha, mas não é um erro AMX
    }
}
```

### Tipo de retorno customizado

Crie um enum com `AmxCell` para retornos ricos:

```rust
#[derive(Clone, Copy)]
enum Status {
    Ok,
    NotFound,
    Error,
}

impl AmxCell<'_> for Status {
    fn as_cell(&self) -> i32 {
        match self {
            Status::Ok => 1,
            Status::NotFound => 0,
            Status::Error => -1,
        }
    }
}

#[native(name = "DoSomething")]
fn do_something(&mut self, _amx: &Amx) -> AmxResult<Status> {
    Ok(Status::Ok)
}
```
