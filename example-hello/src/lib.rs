//! Exemplo mínimo de plugin rust-samp.
//!
//! Demonstra:
//! - `#[derive(SampPlugin, Default)]` — sem boilerplate de ciclo de vida
//! - `initialize_plugin!(type: T, ...)` — construtor via Default::default()
//! - `AmxString` com `Deref<Target=str>` — métodos de &str sem alocação
//! - `UnsizedBuffer::write_str` — escrita de string de saída em um passo
//!
//! Natives expostas ao PAWN:
//! ```pawn
//! native Hello_Greet(const name[], greeting[] = "", size = sizeof(greeting));
//! native Hello_IsPalindrome(const text[]);
//! ```

use samp::prelude::*;
use samp::{initialize_plugin, native, SampPlugin};

#[derive(SampPlugin, Default)]
struct Hello;

impl Hello {
    /// Saúda um jogador, escrevendo a mensagem em `greeting`.
    ///
    /// ```pawn
    /// new msg[64];
    /// Hello_Greet("Mundo", msg);
    /// // msg == "Olá, Mundo! (5 letras)"
    /// ```
    #[native(name = "Hello_Greet")]
    fn greet(
        &mut self,
        _amx: &Amx,
        name: AmxString,
        greeting: UnsizedBuffer,
        size: usize,
    ) -> AmxResult<bool> {
        // AmxString implementa Deref<Target=str> — métodos de &str disponíveis diretamente
        let msg = if name.is_empty() {
            "Olá, Anônimo!".to_string()
        } else if name.starts_with("Admin") {
            format!("[ADMIN] Bem-vindo, {}!", &*name)
        } else {
            format!("Olá, {}! ({} letras)", &*name, name.len())
        };

        greeting.write_str(size, &msg)?;
        Ok(true)
    }

    /// Verifica se um texto é um palíndromo.
    ///
    /// ```pawn
    /// Hello_IsPalindrome("arara"); // retorna 1
    /// Hello_IsPalindrome("hello"); // retorna 0
    /// ```
    #[native(name = "Hello_IsPalindrome")]
    fn is_palindrome(&mut self, _amx: &Amx, text: AmxString) -> AmxResult<bool> {
        // to_bytes() lê o buffer AMX diretamente, sem passar por Deref/String
        let bytes = text.to_bytes();
        let is_pal = bytes == bytes.iter().rev().copied().collect::<Vec<_>>();
        Ok(is_pal)
    }
}

initialize_plugin!(
    type: Hello,
    natives: [Hello::greet, Hello::is_palindrome],
);
