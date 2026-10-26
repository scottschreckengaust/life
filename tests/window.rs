//! Run windows: `--from`/`--until`/`--for`, `Life::window` and scenario `from`/`until`/`for`.

use spin_life::{iso, rt::Mode, Journal, Life, Rules, PRESETS};
use std::process::Command;

fn spin_life(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_spin-life")).args(args).output().unwrap()
}

fn chaos(from: Option<f64>, until: Option<f64>) -> Journal {
    let rules = Rules::preset("scott").unwrap();
    let life = Life::new(&rules, rules.scenario("chaos").unwrap());
    if from.is_none() && until.is_none() {
        life.run(Mode::Virtual)
    } else {
        life.window(from, until).run(Mode::Virtual)
    }
}

#[test]
fn a_window_is_the_full_run_clipped() {
    let (from, until) = (iso::when("P2DT18H").unwrap(), iso::when("P2DT22H").unwrap());
    let windowed = chaos(Some(from), Some(until));
    let mut full = chaos(None, None);
    full.clip(from, until);
    assert_eq!(windowed.spans, full.spans);
    assert_eq!(windowed.moments, full.moments);
    assert_eq!(windowed.day_range(), 3..=3);
    assert!(windowed.spans.iter().all(|s| from <= s.start && s.end <= until));
    assert!(windowed.moments.iter().all(|m| from <= m.at && m.at < until));
    for name in ["geek", "people", "eat", "living", "work", "sleep"] {
        assert_eq!(windowed.day(3).hours(name), full.day(3).hours(name), "{name}");
    }
}

#[test]
fn a_window_with_only_a_start_runs_to_the_end_and_is_clipped() {
    let from = iso::when("P2DT18H").unwrap();
    let j = chaos(Some(from), None);
    assert!(j.spans.iter().all(|s| s.start >= from), "nothing before --from");
    assert_eq!(j.day_range(), 3..=7);
}

#[test]
fn the_end_of_a_window_is_not_in_it() {
    // Day 1's coffee is at 07:00: a window ending then leaves it out of the tally, the diary
    // and the journal; one ending later has it.
    let j = chaos(Some(6.5), Some(7.0));
    assert_eq!(j.day(1).tally.coffee, 0);
    assert!(!j.said("☕"), "{:?}", j.moments);
    let j = chaos(Some(6.5), Some(7.5));
    assert_eq!(j.day(1).tally.coffee, 1);
    assert!(j.said("☕"));
    // Journal::clip by itself, too.
    let mut j = chaos(None, None);
    j.clip(6.5, 7.0);
    assert!(j.moments.iter().all(|m| m.at < 7.0), "{:?}", j.moments);
}

#[test]
fn a_window_can_end_exactly_when_the_run_does() {
    let out = spin_life(&["--days", "1", "-i", "-q", "--from", "06:30", "--until", "P1DT6H30M"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("from d1 06:30 until d2 06:30)"), "{stdout}");
}

#[test]
fn no_window_changes_nothing() {
    let rules = Rules::preset("scott").unwrap();
    let life = Life::new(&rules, rules.scenario("chaos").unwrap());
    let (start, end) = (life.starts_at(), life.ends_at());
    assert_eq!(life.window(Some(start), Some(end)).run(Mode::Virtual).spans, chaos(None, None).spans);
}

#[test]
#[should_panic(expected = "must end after it starts")]
fn an_empty_window_panics() {
    chaos(Some(10.0), Some(10.0));
}

#[test]
fn the_cli_reports_only_the_window() {
    let out = spin_life(&["chaos", "-i", "--from", "P2DT18H", "--until", "P2DT22H"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("(7 days, instant, from d3 18:00 until d3 22:00)"), "{stdout}");
    // Report rows look like "  3     0.0h ...".
    let rows: Vec<&str> =
        stdout.lines().filter(|l| l.split_whitespace().nth(1).is_some_and(|c| c.ends_with('h'))).collect();
    assert_eq!(rows.len(), 1, "{stdout}");
    assert!(rows[0].trim_start().starts_with("3 "), "{stdout}");
    for line in
        stdout.lines().filter(|l| l.starts_with("d3 ") || l.starts_with("d1 ") || l.starts_with("d4 "))
    {
        assert!(
            line.starts_with("d3 18:") || line.starts_with("d3 19:") || line.starts_with("d3 2"),
            "{line}"
        );
    }
}

#[test]
fn bad_windows_exit_2() {
    for (args, says) in [
        (&["--until", "07:00", "--for", "PT1S"][..], "don't mix"),
        (&["--from", "05:00"], "before the first alarm (d1 06:30)"),
        (&["--until", "P30D"], "after the run ends (d8 06:30)"),
        (&["--from", "07:00", "--until", "06:45"], "must be after --from"),
        (&["--for", "2s"], "--for: `2s` is not an ISO 8601 duration"),
        (&["--from", "6:30"], "--from: `6:30` is not an ISO 8601 time of day"),
        (&["list", "--for", "PT1S"], "only apply to run"),
        (&["list", "--from", "06:30"], "only apply to run"),
        (&["list", "--until", "07:00"], "only apply to run"),
        (&["--from", "07:00", "--until", "07:00"], "must be after --from"),
    ] {
        let out = spin_life(&[&["-i"][..], args].concat());
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8(out.stderr).unwrap();
        assert!(stderr.contains(says), "{args:?}: {stderr}");
    }
}

#[test]
fn a_scenario_can_set_its_own_window() {
    let src = format!(
        "{}\n[scenario.blink]\nabout = \"one second\"\nfrom = \"06:30\"\nfor = \"PT1S\"\n",
        PRESETS[0].2
    );
    let rules = Rules::parse(&src).unwrap();
    let s = rules.scenario("blink").unwrap();
    assert_eq!((s.from, s.until), (Some(6.5), Some(6.5 + 1.0 / 3600.0)));
    let j = Life::new(&rules, s).run(Mode::Virtual);
    assert!(j.spans.iter().all(|sp| sp.start >= 6.5 && sp.end <= 6.5 + 1.0 / 3600.0));
    assert_eq!(j.moments.len(), 2, "{:?}", j.moments); // the alarm and breakfast

    for (keys, says) in [
        ("until = \"07:00\"\nfor = \"PT1S\"", "don't mix"),
        ("for = \"PT1S\"", "needs a `from`"),
        ("from = \"07:00\"\nuntil = \"06:00\"", "must be after `from`"),
        ("from = \"6:30\"", "scenario.bad.from: `6:30`"),
    ] {
        let src = format!("{}\n[scenario.bad]\n{keys}\n", PRESETS[0].2);
        let e = Rules::parse(&src).unwrap_err().to_string();
        assert!(e.contains(says), "{keys}: {e}");
    }
}
