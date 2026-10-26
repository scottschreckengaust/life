//! The CLI as a shell sees it: closed pipes and the report's wording.

use std::process::{Command, Stdio};

/// How a run ended: an exit code, or the signal that ended it (Unix).
#[derive(Debug, PartialEq)]
enum End {
    Code(i32),
    Signal(i32),
}

fn end(status: std::process::ExitStatus) -> End {
    #[cfg(unix)]
    if let Some(sig) = std::os::unix::process::ExitStatusExt::signal(&status) {
        return End::Signal(sig);
    }
    End::Code(status.code().expect("an exit code"))
}

/// Run spin-life with its stdout already closed by the reader, like `spin-life | head -0`.
fn with_stdout_closed(args: &[&str]) -> (End, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_spin-life"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take()); // the reader goes away before anything is written
    let out = child.wait_with_output().unwrap();
    (end(out.status), String::from_utf8_lossy(&out.stderr).into_owned())
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
        let (end, stderr) = with_stdout_closed(args);
        assert!(!stderr.contains("panicked"), "{args:?} panicked: {stderr}");
        // Unix: ended by SIGPIPE (13), like `yes | head`; elsewhere, or when the output fit in
        // the pipe before it closed: exit 0. Anything else (a crash, exit 101) fails.
        assert!(end == End::Signal(13) || end == End::Code(0), "{args:?} ended {end:?}: {stderr}");
    }
}

#[test]
#[cfg(unix)]
fn a_closed_pipe_ends_the_run_by_sigpipe() {
    // The run writes long after the reader has gone, so on Unix the restored SIGPIPE ends it
    // (signal 13) rather than a write error (exit 0), and it ends at once, not after the day.
    let (end, stderr) = with_stdout_closed(&["--days", "1", "--speed-up", "360000"]);
    assert_eq!(end, End::Signal(13), "{stderr}");
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
