//! `spin-life run --format json`: one JSON document, with the same numbers the library computes.

use spin_life::{json, rt::Mode, Life, Rules};
use std::process::{Command, Output};

fn spin_life(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_spin-life")).args(args).output().unwrap()
}

fn stdout(args: &[&str]) -> String {
    let out = spin_life(args);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn json_is_the_only_thing_on_stdout() {
    let got = stdout(&["chaos", "-p", "scott", "-i", "-f", "json"]);
    assert!(got.starts_with(r#"{"spin_life":""#), "{got}");
    assert!(got.ends_with("}\n"), "{got}");
    assert_eq!(got.lines().count(), 1);
    assert!(!got.contains('🌍') && !got.contains('⏱'));
    assert!(got.contains(r#""speed_up":null"#));
}

#[test]
fn every_days_hours_match_the_library() {
    let got = stdout(&["chaos", "-p", "scott", "-i", "-f", "json"]);
    let rules = Rules::preset("scott").unwrap();
    let journal = Life::new(&rules, rules.scenario("chaos").unwrap()).run(Mode::Virtual);
    let names = ["sleep", "wired", "eat", "work", "travel", "living", "geek", "people", "surf"];
    let days: Vec<&str> = got.split(r#"{"day":"#).skip(1).collect();
    assert!(days.len() >= journal.days());
    for d in 1..=journal.days() {
        let summary = journal.day(d);
        let (day, _) = days[d - 1].split_once(r#","dreams""#).unwrap();
        assert!(day.starts_with(&format!(r#"{d},"hours":{{"#)), "{day}");
        for n in names {
            let mut want = format!(r#""{n}":"#);
            json::number(&mut want, summary.hours(n));
            assert!(day.contains(&want), "day {d}: wanted {want} in {day}");
        }
    }
}

#[test]
fn real_time_hints_go_to_stderr() {
    let out = spin_life(&["--days", "1", "--speed-up", "3600000", "-f", "json"]);
    assert!(out.status.success());
    let (stdout, stderr) = (String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap());
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
    assert!(stdout.contains(r#""speed_up":3600000"#), "{stdout}");
    assert!(stderr.contains("⏱  about 0.0s of real time; add --instant"), "{stderr}");
}

#[test]
fn bad_options_exit_2() {
    for args in [
        &["-f", "yaml"][..],
        &["list", "-f", "json"],
        &["init", "--format", "json"],
        &["--speed-up", "0"],
        &["--speed-up", "-5"],
        &["--speed-up", "abc"],
        &["--instant", "--speed-up", "10"],
        &["--scale", "1"],
        &["-s", "1"],
    ] {
        let out = spin_life(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8(out.stderr).unwrap();
        if args[0] == "--scale" || args[0] == "-s" {
            assert!(stderr.starts_with("--scale is gone: use --speed-up"), "{stderr}");
        }
    }
}
