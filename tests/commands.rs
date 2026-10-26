//! Every CLI command and error, run the way a shell would (and exiting normally, so coverage
//! sees them: a run ended by SIGPIPE never writes its profile).

use std::process::{Command, Output, Stdio};

fn spin_life(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_spin-life")).args(args).output().unwrap()
}

#[track_caller]
fn succeeds(args: &[&str]) -> String {
    let out = spin_life(args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[track_caller]
fn fails(args: &[&str], code: i32, says: &str) {
    let out = spin_life(args);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(code), "{args:?}: {stderr}");
    assert!(stderr.contains(says), "{args:?}: `{stderr}` doesn't mention `{says}`");
}

/// A life file in a fresh temp dir; returns its path.
fn life_file(name: &str, src: &str) -> String {
    let dir = std::env::temp_dir().join(format!("spin-life-commands-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.life.toml"));
    std::fs::write(&path, src).unwrap();
    path.to_string_lossy().into_owned()
}

#[test]
fn presets_lists_the_built_in_lives() {
    let out = succeeds(&["presets"]);
    for name in ["scott", "parent", "agent"] {
        assert!(out.contains(&format!("  {name:<8} ")), "{out}");
    }
}

#[test]
fn init_prints_a_life_file() {
    assert!(succeeds(&["init"]).contains("name  = \"scott\""));
    assert!(succeeds(&["init", "--preset", "agent"]).contains("[pursuit.users]"));
    assert!(succeeds(&["init", "parent"]).contains("[pursuit.kids]"));
    fails(&["init", "--preset", "nobody"], 2, "no preset named `nobody`; try `spin-life presets`");
}

#[test]
fn check_validates_a_file() {
    let good = life_file("good", "name = \"me\"\n[pursuit.a]\n[scenario.s]\n");
    let out = succeeds(&["check", &good]);
    assert!(out.starts_with("✅ ") && out.contains(": 1 pursuits (a), 1 scenarios"), "{out}");
    assert!(succeeds(&["check", "--life", &good]).starts_with("✅ "));
    fails(&["check"], 2, "check needs a FILE");
    fails(&["check", "/nonexistent/x.life.toml"], 2, "/nonexistent/x.life.toml: ");
    let bad = life_file("bad", "[pursuit.a]\nsession = \"2h\"\n");
    fails(&["check", &bad], 1, "is not an ISO 8601 duration");
}

#[test]
fn list_shows_a_lifes_scenarios() {
    let out = succeeds(&["list", "-p", "parent"]);
    assert!(out.contains("teething"), "{out}");
    let bare = life_file("bare", "[pursuit.a]\n");
    assert!(succeeds(&["list", "-L", &bare])
        .contains("(no scenarios; `spin-life run --days 7` lives an ordinary week)"));
}

#[test]
fn run_finds_its_scenario_or_says_which_exist() {
    let out = spin_life(&["nope", "-i"]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no scenario `nope` in scott's life. Scenarios:") && stderr.contains("  chaos"),
        "{stderr}"
    );
    // After a command, a command's name is the scenario's.
    fails(&["run", "list", "-i"], 2, "no scenario `list` in scott's life");
    // No `week` scenario: the file's first one runs. No scenarios at all: an ordinary week.
    let firsts = life_file("firsts", "[pursuit.a]\n[scenario.zeta]\ndays = 1\n");
    assert!(succeeds(&["-L", &firsts, "-i", "-q"]).contains("/ zeta:"));
    let bare = life_file("bare-run", "name = \"bare\"\n[pursuit.a]\n");
    let out = succeeds(&["-L", &bare, "-i", "-q"]);
    assert!(
        out.starts_with("🌍 bare / plain: 7 ordinary days"),
        "a life without a motto starts with the title: {out}"
    );
    // --days caps at a year.
    assert!(succeeds(&["--days", "999", "-i", "-q"]).contains("(366 days, instant)"));
}

#[test]
fn text_is_the_default_format_and_can_be_asked_for() {
    assert_eq!(succeeds(&["--days", "1", "-i", "-f", "text"]), succeeds(&["--days", "1", "-i"]));
}

#[test]
fn json_without_a_motto_says_null() {
    let bare = life_file("bare-json", "name = \"bare\"\n[pursuit.a]\n");
    assert!(succeeds(&["-L", &bare, "-i", "-f", "json", "--days", "1"]).contains(r#""motto":null"#));
}

#[test]
fn a_window_must_start_inside_the_run() {
    fails(&["--days", "1", "-i", "--from", "P1DT6H30M"], 2, "--from is after the run ends (d2 06:30)");
}

#[test]
fn help_and_version() {
    assert!(succeeds(&["--help"]).starts_with("spin-life: simulate a life from rules you write"));
    assert!(succeeds(&["-h"]).contains("OPTIONS:"));
    assert_eq!(succeeds(&["--version"]), format!("spin-life {}\n", env!("CARGO_PKG_VERSION")));
    assert_eq!(succeeds(&["-V"]), format!("spin-life {}\n", env!("CARGO_PKG_VERSION")));
}

#[test]
fn bad_arguments_exit_2_with_usage() {
    for (args, says) in [
        (&["--days", "seven"][..], "--days needs a number"),
        (&["--days"], "--days needs a value"),
        (&["--speed-up", "fast"], "--speed-up needs a positive number"),
        (&["--wat"], "unknown option `--wat`"),
        (&["run", "chaos", "extra"], "unexpected argument `extra`"),
        // A command name only counts first: after a scenario it's an extra argument.
        (&["chaos", "list"], "unexpected argument `list`"),
        (&["--preset", "nobody", "-i"], "no preset named `nobody`"),
        (&["-L", "/nonexistent/x.toml", "-i"], "/nonexistent/x.toml: "),
    ] {
        fails(args, 2, says);
        let out = spin_life(args);
        assert!(String::from_utf8_lossy(&out.stderr).contains("USAGE:"), "{args:?} shows usage");
    }
}

#[test]
#[cfg(target_os = "linux")]
fn a_full_disk_is_an_error_not_a_quiet_stop() {
    // /dev/full fails every write with ENOSPC: not a closed pipe, so exit 1 with a message.
    let full = std::fs::OpenOptions::new().write(true).open("/dev/full").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_spin-life"))
        .args(["presets"])
        .stdout(Stdio::from(full))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("spin-life: can't write output:"));
}
