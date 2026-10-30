//! The life file's TOML subset: any input is parsed or refused, never a panic, a hang or a
//! stack overflow.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|src: &str| {
    if let Err(e) = spin_life::toml::parse(src) {
        // An error always names a line of the input.
        let lines = src.matches('\n').count() + 1;
        assert!((1..=lines).contains(&e.line), "{e}, but the input has {lines} lines");
    }
});
