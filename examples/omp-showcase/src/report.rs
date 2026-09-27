//! What the showcase found, counted and logged.
//!
//! A round trip lands in one of three places. The value came back: both slots
//! and both types are right. The old value stayed: the setter ignored it, which
//! is sometimes the server's own rule and sometimes a wrong slot — listed, so a
//! person decides. Anything else: a wrong slot or a wrong type.

use std::fmt::Debug;

use log::{debug, info, warn};

/// A value different from `self`, to set and read back.
///
/// Small and valid for most setters: interior 3, world 3, team 3, weather 3 —
/// a round trip is about the slot, not about exploring the value's range.
pub trait Other: Copy {
    fn other(self) -> Self;
}

macro_rules! other_integer {
    ($($ty:ty),*) => {$(
        impl Other for $ty {
            fn other(self) -> Self {
                if self == 3 { 4 } else { 3 }
            }
        }
    )*};
}

other_integer!(i8, u8, i16, u16, i32, u32, i64, u64);

impl Other for f32 {
    fn other(self) -> Self {
        if (self - 1.5).abs() < f32::EPSILON {
            2.5
        } else {
            1.5
        }
    }
}

impl Other for bool {
    fn other(self) -> Self {
        !self
    }
}

/// Equality for a round trip. Exact for integers and booleans; for a float,
/// within a hair — the server may store it in another form (a rotation goes
/// through a quaternion) and hand back `1.5000001` for `1.5`, which is the same
/// value, not a wrong slot.
pub trait Close: PartialEq {
    fn close_to(self, other: Self) -> bool;
}

macro_rules! close_exactly {
    ($($ty:ty),*) => {$(
        impl Close for $ty {
            fn close_to(self, other: Self) -> bool {
                self == other
            }
        }
    )*};
}

close_exactly!(bool, i8, u8, i16, u16, i32, u32, i64, u64);

impl Close for f32 {
    fn close_to(self, other: Self) -> bool {
        (self - other).abs() <= 1e-4 * self.abs().max(other.abs()).max(1.0)
    }
}

#[derive(Default)]
pub struct Report {
    passed: usize,
    ignored: Vec<String>,
    wrong: Vec<String>,
}

impl Report {
    /// Announces the call about to be made.
    ///
    /// A wrong slot on Windows ends the process instead of returning a wrong
    /// value; the last of these lines in the log names the call that did it.
    pub fn begin(&mut self, name: &str) {
        debug!("[showcase] > {name}");
    }

    /// Records one set/get round trip.
    pub fn round_trip<T: Close + Debug + Copy>(&mut self, name: &str, before: T, want: T, got: T) {
        if got.close_to(want) {
            self.passed += 1;
        } else if got == before {
            self.ignored
                .push(format!("{name}: set {want:?}, stayed {before:?}"));
        } else {
            self.wrong.push(format!(
                "{name}: was {before:?}, set {want:?}, read {got:?}"
            ));
        }
    }

    /// Records a check that is not a round trip — a lookup, a creation.
    pub fn check(&mut self, name: &str, ok: bool, detail: impl Debug) {
        debug!("[showcase] = {name}");
        if ok {
            self.passed += 1;
        } else {
            self.wrong.push(format!("{name}: {detail:?}"));
        }
    }

    /// The totals, then every case that was not a clean pass.
    pub fn log(&self) {
        info!(
            "[showcase] {} passed, {} ignored by the setter, {} wrong",
            self.passed,
            self.ignored.len(),
            self.wrong.len()
        );
        for line in &self.ignored {
            info!("[showcase]   ignored  {line}");
        }
        for line in &self.wrong {
            warn!("[showcase]   WRONG    {line}");
        }
    }
}
