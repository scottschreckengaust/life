use crate::rt::Bell;
use std::sync::atomic::{
    AtomicBool, AtomicU8,
    Ordering::{AcqRel, Acquire, Relaxed, Release},
};

/// Why you woke up. Several reasons can pile up in one night.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Why(u8);

impl Why {
    /// The earth spun once.
    pub const MORNING: Self = Self(1);
    /// Something urgent that your rules let wake you: a party, a swell, a baby, a page.
    pub const URGENT: Self = Self(2);

    /// Does this include `other`?
    pub fn has(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

/// `ALARM` from the motto, plus a note saying *why* it rang.
///
/// The note is written with `Relaxed` *before* the `Release` store of `rung`, and
/// read *after* an `Acquire` load of `rung`. That pairing is the entire reason the
/// motto says `Acquire`: when you wake up, you see everything that happened before
/// the alarm went off, including why it went off.
#[derive(Debug, Default)]
pub struct Alarm {
    rung: AtomicBool,
    why: AtomicU8,
    pub(crate) bell: Bell,
}

impl Alarm {
    /// Ring the alarm, from any thread or task.
    pub fn ring(&self, why: Why) {
        self.why.fetch_or(why.0, Relaxed);
        self.rung.store(true, Release); // ALARM.store(true, Release)
        self.bell.ring();
    }

    /// Has it rung? `while !ALARM.load(Acquire)`
    pub fn rung(&self) -> bool {
        self.rung.load(Acquire)
    }

    /// The nightly reset: `ALARM.store(false, Relaxed)`. Relaxed is enough because it
    /// publishes nothing; ordering on the same atomic within one thread is free.
    pub fn reset(&self) {
        self.why.store(0, Relaxed);
        self.rung.store(false, Relaxed);
    }

    /// Hit the alarm: clear it and learn why it rang. A ring that lands in between
    /// leaves `rung` set, so it shows up as one more (possibly empty) wake-up.
    /// Spurious wake-ups are allowed. Ask anyone who has dreamt.
    pub fn take(&self) -> Why {
        self.rung.store(false, Relaxed);
        Why(self.why.swap(0, AcqRel))
    }
}
