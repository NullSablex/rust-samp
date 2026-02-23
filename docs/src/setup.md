# Setup e Instalação

## Pré-requisitos

- [Rust](https://rustup.rs) (stable)
- Target **i686** — servidores SA-MP são 32-bit

## Instalando o toolchain

```sh
# Instalar Rust (se ainda não tiver)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Adicionando o target i686

Servidores SA-MP rodam em 32-bit. Você precisa do target i686 correspondente ao seu sistema:

```sh
# Linux
rustup target add i686-unknown-linux-gnu

# Windows
rustup target add i686-pc-windows-msvc
```

### Dependências do sistema (Linux)

No Linux, você precisa dos compiladores multilib para cross-compilar para 32-bit:

```sh
# Debian/Ubuntu
sudo apt-get install gcc-multilib g++-multilib
```

## Criando o projeto

```sh
cargo new --lib meu-plugin
cd meu-plugin
```

## Configurando o Cargo.toml

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
samp = { git = "https://github.com/NullSablex/rust-samp.git" }
```

O `crate-type = ["cdylib"]` faz o Cargo gerar uma biblioteca dinâmica (`.so` no Linux, `.dll` no Windows) que o servidor SA-MP carrega como plugin.

## Compilando

```sh
# Linux
cargo build --target i686-unknown-linux-gnu

# Windows
cargo build --target i686-pc-windows-msvc
```

O artefato estará em `target/i686-<plataforma>/debug/`:
- Linux: `libmeu_plugin.so`
- Windows: `meu_plugin.dll`

Para produção, use `--release`:

```sh
cargo build --release --target i686-unknown-linux-gnu
```

## Simplificando o build

Para evitar digitar `--target` toda vez, crie um arquivo `.cargo/config.toml` na raiz do projeto:

```toml
# Linux
[build]
target = "i686-unknown-linux-gnu"

# Windows (descomente a linha abaixo no lugar da de cima)
# target = "i686-pc-windows-msvc"
```

Agora `cargo build` já compila para o target correto.

## Instalando no servidor

1. Compile em modo release
2. Copie o `.so` ou `.dll` para a pasta `plugins/` do servidor SA-MP
3. Adicione o nome do plugin (sem extensão no Linux, com `.dll` no Windows) no `server.cfg`:
   ```
   plugins meu_plugin
   ```
4. Inicie o servidor
