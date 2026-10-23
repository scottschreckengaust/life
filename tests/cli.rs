//! The CLI as a shell sees it: closed pipes and the report's wording.

use std::process::{Command, Stdio};

/// Run spin-life with its stdout already closed by the reader, like `spin-life | head -0`.
fn with_stdout_closed(args: &[&str]) -> (Option<i32>, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_spin-life"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take()); // the reader goes away before anything is written
    let out = child.wait_with_output().unwrap();
    (out.status.code(), String::from_utf8_lossy(&out.stderr).into_owned())
}

#[test]
fn a_closed_pipe_is_not_a_panic() {
    for args in [
        &["chaos", "--instant"][..],
        &["chaos", "--instant", "--quiet"],
        &["chaos", "--instant", "--format", "json"],
        &["init"],
        &["list"],
        &["presets"],
        &["--days", "1", "--speed-up", "3600000"],
    ] {
        let (code, stderr) = with_stdout_closed(args);
        assert!(!stderr.contains("panicked"), "{args:?} panicked: {stderr}");
        // Unix: killed by SIGPIPE (no exit code), like `yes | head`; elsewhere: exit 0.
        assert!(code.is_none() || code == Some(0), "{args:?} exited {code:?}: {stderr}");
    }
}

#[test]
fn one_day_is_one_spin() {
    let out =
        Command::new(env!("CARGO_BIN_EXE_spin-life")).args(["--days", "1", "-i", "-q"]).output().unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("\n1 spin of the earth: "), "{stdout}");
    let out =
        Command::new(env!("CARGO_BIN_EXE_spin-life")).args(["--days", "2", "-i", "-q"]).output().unwrap();
    assert!(String::from_utf8(out.stdout).unwrap().contains("\n2 spins of the earth: "));
}
