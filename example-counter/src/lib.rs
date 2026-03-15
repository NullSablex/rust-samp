//! Exemplo de plugin com estado e ciclo de vida completo.
//!
//! Demonstra:
//! - Estado no struct do plugin (`count`, `max`, `ticks`)
//! - `impl SampPlugin` manual com `on_load` e `process_tick`
//! - `initialize_plugin!` com bloco construtor
//! - `Ref<i32>` para saída por referência (`&value` no PAWN)
//! - Múltiplas natives com lógica real
//!
//! Natives expostas ao PAWN:
//! ```pawn
//! native Counter_Increment();
//! native Counter_Decrement();
//! native Counter_Reset();
//! native Counter_Get(&out);
//! native Counter_SetMax(max);
//! native bool:Counter_IsAtMax();
//! ```

use samp::prelude::*;
use samp::{initialize_plugin, native};
use log::info;

struct Counter {
    count: i32,
    max: i32,
    ticks: u32,
}

impl SampPlugin for Counter {
    fn on_load(&mut self) {
        info!("Counter plugin carregado. Max={}", self.max);
    }

    fn on_unload(&mut self) {
        info!("Counter plugin descarregado. Valor final={}", self.count);
    }

    fn process_tick(&mut self) {
        self.ticks += 1;
        // Loga o estado a cada ~5 segundos (1000 ticks × ~5ms)
        if self.ticks.is_multiple_of(1000) {
            info!("Counter tick={} count={}/{}", self.ticks, self.count, self.max);
        }
    }
}

impl Counter {
    /// Incrementa o contador. Retorna o novo valor, ou -1 se já estiver no máximo.
    #[native(name = "Counter_Increment")]
    fn increment(&mut self, _amx: &Amx) -> AmxResult<i32> {
        if self.count >= self.max {
            return Ok(-1);
        }
        self.count += 1;
        Ok(self.count)
    }

    /// Decrementa o contador. Retorna o novo valor, ou -1 se já for zero.
    #[native(name = "Counter_Decrement")]
    fn decrement(&mut self, _amx: &Amx) -> AmxResult<i32> {
        if self.count <= 0 {
            return Ok(-1);
        }
        self.count -= 1;
        Ok(self.count)
    }

    /// Zera o contador. Retorna o valor que foi descartado.
    #[native(name = "Counter_Reset")]
    fn reset(&mut self, _amx: &Amx) -> AmxResult<i32> {
        let old = self.count;
        self.count = 0;
        Ok(old)
    }

    /// Escreve o valor atual em `out` (saída por referência).
    ///
    /// ```pawn
    /// new val;
    /// Counter_Get(val);
    /// printf("Valor: %d", val);
    /// ```
    #[native(name = "Counter_Get")]
    fn get(&mut self, _amx: &Amx, mut out: Ref<i32>) -> AmxResult<bool> {
        *out = self.count;
        Ok(true)
    }

    /// Define o valor máximo do contador.
    #[native(name = "Counter_SetMax")]
    fn set_max(&mut self, _amx: &Amx, max: i32) -> AmxResult<bool> {
        if max <= 0 {
            return Ok(false);
        }
        self.max = max;
        if self.count > self.max {
            self.count = self.max;
        }
        Ok(true)
    }

    /// Retorna true se o contador estiver no valor máximo.
    #[native(name = "Counter_IsAtMax")]
    fn is_at_max(&mut self, _amx: &Amx) -> AmxResult<bool> {
        Ok(self.count >= self.max)
    }
}

initialize_plugin!(
    natives: [
        Counter::increment,
        Counter::decrement,
        Counter::reset,
        Counter::get,
        Counter::set_max,
        Counter::is_at_max,
    ],
    {
        samp::plugin::enable_process_tick();

        let _ = fern::Dispatch::new()
            .level(log::LevelFilter::Info)
            .chain(samp::plugin::logger())
            .apply();

        return Counter {
            count: 0,
            max: 100,
            ticks: 0,
        };
    }
);
