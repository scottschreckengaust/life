//! The release pipeline runs the *downloaded* binaries against these files, so they
//! must match what the code produces. If a change to the engine is intended,
//! regenerate them with:
//!
//!     cargo run -q -- chaos -i -q > tests/golden/scott-chaos.txt
//!     cargo run -q -- incident -p agent -i -q > tests/golden/agent-incident.txt
//!     cargo run -q -- teething -p parent -i -q > tests/golden/parent-teething.txt

use std::process::Command;

#[track_caller]
fn golden(args: &[&str], file: &str) {
    let out = Command::new(env!("CARGO_BIN_EXE_spin-life")).args(args).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let got = String::from_utf8(out.stdout).unwrap();
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
