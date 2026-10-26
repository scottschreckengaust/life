//! `--speed-up` really runs on the wall clock: a day at N× takes 86400/N real seconds,
//! and comes out exactly like the `--instant` run. A run window makes actual real time
//! (`--speed-up 1`) testable too. Kept to a few seconds; the tests run in parallel.

use std::process::Command;
use std::time::{Duration, Instant};

/// The report after the header (motto, title line, then the ⏱ hint for real-time runs).
fn report(args: &[&str]) -> (String, Duration) {
    let started = Instant::now();
    let out = Command::new(env!("CARGO_BIN_EXE_spin-life")).args(args).output().unwrap();
    let took = started.elapsed();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let body = stdout.split_once("\n\n").map_or(stdout.as_str(), |(_, body)| body).to_string();
    (body, took)
}

#[track_caller]
fn one_day_at(speed_up: u32, expected_secs: f64) {
    let (live, took) = report(&["--days", "1", "--speed-up", &speed_up.to_string(), "--quiet"]);
    let (instant, _) = report(&["--days", "1", "--instant", "--quiet"]);
    let took = took.as_secs_f64();
    // Timers never fire early, so a run can't beat the clock; allow slack for slow runners.
    assert!(took >= expected_secs * 0.95, "{speed_up}×: finished in {took:.2}s, expected ~{expected_secs}s");
    assert!(took <= expected_secs + 2.0, "{speed_up}×: took {took:.2}s, expected ~{expected_secs}s");
    assert_eq!(live, instant, "{speed_up}× lived a different day than --instant");
}

#[test]
fn a_day_at_86400x_takes_a_second() {
    one_day_at(86_400, 1.0);
}

#[test]
fn a_day_at_43200x_takes_two_seconds() {
    one_day_at(43_200, 2.0);
}

/// A window at actual real time: `secs` of simulated time take `secs` of wall time.
#[track_caller]
fn real_time_window(window: &[&str], secs: f64, slack: f64) -> String {
    let live_args = [&["--days", "1", "--speed-up", "1", "--quiet"][..], window].concat();
    let instant_args = [&["--days", "1", "--instant", "--quiet"][..], window].concat();
    let (live, took) = report(&live_args);
    let (instant, _) = report(&instant_args);
    let took = took.as_secs_f64();
    assert!(took >= secs * 0.9, "{window:?}: finished in {took:.2}s, expected ~{secs}s");
    assert!(took <= secs + slack, "{window:?}: took {took:.2}s, expected ~{secs}s");
    assert_eq!(live, instant, "{window:?} at 1× lived a different window than --instant");
    live
}

#[test]
fn two_seconds_at_actual_real_time() {
    let report = real_time_window(&["--from", "06:30", "--for", "PT2S"], 2.0, 2.0);
    assert!(report.contains("Last entry d1 06:30."), "{report}");
}

#[test]
fn half_a_second_at_actual_real_time() {
    real_time_window(&["--from", "06:30", "--for", "PT0.5S"], 0.5, 2.0);
}

#[test]
fn the_hint_says_how_long_the_window_takes() {
    // 1 s of simulated time at 2×: half a second of real time, and the hint says so.
    let out = Command::new(env!("CARGO_BIN_EXE_spin-life"))
        .args(["--days", "1", "--speed-up", "2", "--from", "06:30", "--for", "PT1S", "--quiet"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("\n⏱  about 0.5s of real time;"), "{stdout}");
    assert!(stdout.contains("(1 day, 2× real time, from d1 06:30 until d1 06:30:01)"), "{stdout}");
}

#[test]
fn a_late_window_skips_ahead_to_it() {
    // Day 6 at 10:00, five days in: virtual until the window, then 1 s of real time.
    let started = Instant::now();
    let (live, _) = report(&["chaos", "--speed-up", "1", "--from", "P5DT10H", "--for", "PT1S", "--quiet"]);
    let took = started.elapsed().as_secs_f64();
    assert!((0.9..=3.0).contains(&took), "took {took:.2}s");
    let (instant, _) = report(&["chaos", "--instant", "--from", "P5DT10H", "--for", "PT1S", "--quiet"]);
    assert_eq!(live, instant);
}
