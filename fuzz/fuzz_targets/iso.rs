//! ISO 8601 times, durations and windows: whatever parses is a sane number.
#![no_main]
use libfuzzer_sys::fuzz_target;
use spin_life::iso;

fuzz_target!(|s: &str| {
    if let Ok(t) = iso::time(s) {
        assert!((0.0..=24.0).contains(&t), "time {s:?} = {t}");
    }
    if let Ok(d) = iso::duration(s) {
        assert!(d.is_finite() && d >= 0.0, "duration {s:?} = {d}");
    }
    if let Ok(w) = iso::when(s) {
        assert!(w.is_finite() && w >= 0.0, "when {s:?} = {w}");
    }
    if let Ok((a, b)) = iso::window(s) {
        assert!(a.is_finite() && b.is_finite() && (0.0..=24.0).contains(&a) && b > a, "window {s:?} = {a}..{b}");
    }
});
