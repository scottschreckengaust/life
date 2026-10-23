//! The release pipeline runs the *downloaded* binaries against these files, so they
//! must match what the code produces. If a change to the engine is intended,
//! regenerate them with:
//!
//!     cargo run -q -- chaos -i -q > tests/golden/scott-chaos.txt
//!     cargo run -q -- incident -p agent -i -q > tests/golden/agent-incident.txt
//!     cargo run -q -- teething -p parent -i -q > tests/golden/parent-teething.txt
//!     cargo run -q -- chaos -i -f json | sed 's/"spin_life":"[^"]*"/"spin_life":"VERSION"/' > tests/golden/scott-chaos.json

use std::process::Command;

#[track_caller]
fn golden(args: &[&str], file: &str) {
    let out = Command::new(env!("CARGO_BIN_EXE_spin-life")).args(args).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    // The JSON names the version that wrote it; goldens say VERSION so a release bump doesn't break them.
    let version = format!(r#""spin_life":"{}""#, env!("CARGO_PKG_VERSION"));
    let got = String::from_utf8(out.stdout).unwrap().replace(&version, r#""spin_life":"VERSION""#);
    let want =
        std::fs::read_to_string(format!("{}/tests/golden/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    assert_eq!(got, want.replace("\r\n", "\n"), "output drifted from tests/golden/{file}");
}

#[test]
fn scott_chaos() {
    golden(&["chaos", "--instant", "--quiet"], "scott-chaos.txt");
}

#[test]
fn agent_incident() {
    golden(&["incident", "-p", "agent", "-i", "-q"], "agent-incident.txt");
}

#[test]
fn parent_teething() {
    golden(&["teething", "-p", "parent", "-i", "-q"], "parent-teething.txt");
}

#[test]
fn scott_chaos_json() {
    golden(&["chaos", "--instant", "--format", "json"], "scott-chaos.json");
}

#[test]
fn real_time_runs_say_how_long_they_take() {
    // 1 day, 3.6 million times faster than real life: ~0.02s of wall time.
    let out = Command::new(env!("CARGO_BIN_EXE_spin-life"))
        .args(["--days", "1", "--speed-up", "3600000", "--quiet"])
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout
            .contains("⏱  about 0.0s of real time; add --instant to skip ahead, or --speed-up to go faster"),
        "{stdout}"
    );
    assert!(stdout.contains("1 ordinary day (1 day, 3600000× real time)"), "{stdout}");
}
