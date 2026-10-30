//! A whole life file: parse it, then live its first scenarios on virtual time. Nothing may
//! panic, and nothing may be recorded past the end of the run.
#![no_main]
use libfuzzer_sys::fuzz_target;
use spin_life::{rt::Mode, Life, Rules};

fuzz_target!(|src: &str| {
    let Ok(rules) = Rules::parse(src) else { return };
    for scenario in rules.scenarios.iter().take(2) {
        let life = Life::new(&rules, scenario);
        let end = life.ends_at();
        let journal = life.run(Mode::Virtual);
        for s in &journal.spans {
            assert!(s.end <= end + 1e-9, "{s:?} ends after the run ({end})");
            assert!(s.end - s.start > 1e-9, "{s:?} is empty or reversed");
        }
        assert!(journal.moments.iter().all(|m| m.at <= end + 1e-9), "a diary line after the run");
    }
});
