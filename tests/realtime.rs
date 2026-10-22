//! `--speed-up` really runs on the wall clock: a day at N× takes 86400/N real seconds,
//! and comes out exactly like the `--instant` run. Kept to a few seconds; the two
//! tests run in parallel.

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
