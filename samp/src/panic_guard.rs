//! The one way the SDK stops a panic at the server boundary.
//!
//! Unwinding into the server's C or C++ frames aborts the process, so every
//! entry point the server calls runs its body through [`catch`]. A bare
//! `catch_unwind` is not enough: it hands back the panic payload, and dropping
//! that payload runs arbitrary `Drop` code — a payload whose `Drop` panics
//! (`std::panic::panic_any` with such a type) unwinds out of the very frame
//! that was meant to stop it.

use std::any::Any;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Runs `f`; on panic, returns the panic message instead of unwinding.
///
/// # Errors
/// The panic message when `f` panics (`"(non-string payload)"` when it is
/// not a string).
pub fn catch<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|payload| {
        let message = payload
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(non-string payload)".to_owned());
        dispose(payload);
        message
    })
}

/// Drops a panic payload without letting a panic in its `Drop` escape. That
/// panic has a payload of its own, usually a plain message, dropped the same
/// way; one that keeps panicking is leaked after a few rounds.
fn dispose(mut payload: Box<dyn Any + Send>) {
    for _ in 0..4 {
        match catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            Ok(()) => return,
            Err(again) => payload = again,
        }
    }
    std::mem::forget(payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("the payload's drop");
        }
    }

    #[test]
    fn returns_the_value_when_nothing_panics() {
        assert_eq!(catch(|| 7), Ok(7));
    }

    #[test]
    fn returns_the_message_of_a_panic() {
        assert_eq!(catch(|| panic!("static")), Err::<(), _>("static".into()));
        let n = 3;
        assert_eq!(
            catch(|| panic!("formatted {n}")),
            Err::<(), _>("formatted 3".into())
        );
        assert_eq!(
            catch(|| std::panic::panic_any(5_u8)),
            Err::<(), _>("(non-string payload)".into())
        );
    }

    #[test]
    fn a_payload_whose_drop_panics_does_not_escape() {
        let outer = catch_unwind(|| catch(|| std::panic::panic_any(Bomb)));
        assert_eq!(outer.ok(), Some(Err("(non-string payload)".into())));
    }
}
