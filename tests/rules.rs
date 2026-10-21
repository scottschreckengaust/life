//! Lives that are not the reference one, and life files that are wrong.

use spin_life::{rt::Mode, toml, Journal, Life, Rules, Scenario};

/// A minimal life: one pursuit, defaults for everything else.
const MINIMAL: &str = r#"
[pursuit.reading]
when = "daily 18:00-21:00"
"#;

fn live(src: &str, scenario: &str) -> Journal {
    let rules = Rules::parse(src).unwrap_or_else(|e| panic!("{e}"));
    let s = rules.scenario(scenario).cloned().unwrap_or_else(|| Scenario::plain(2));
    Life::new(&rules, &s).run(Mode::Virtual)
}

#[track_caller]
fn hours(j: &Journal, day: usize, what: &str, expected: f64) {
    let got = j.day(day).hours(what);
    assert!(
        (got - expected).abs() < 1e-6,
        "day {day} {what}: {got}h, expected {expected}h\n{}",
        j.timeline()
    );
}

#[track_caller]
fn rejects(src: &str, needle: &str) {
    match Rules::parse(src) {
        Ok(_) => panic!("expected an error mentioning `{needle}`"),
        Err(e) => assert!(e.to_string().contains(needle), "error `{e}` does not mention `{needle}`"),
    }
}

// --------------------------------------------------------------- custom lives

#[test]
fn a_minimal_life_runs_on_defaults() {
    let j = live(MINIMAL, "none");
    hours(&j, 1, "reading", 3.0); // a window runs to its end, even past dinner time
    hours(&j, 1, "work", 0.0); // no [work] table, no job
}

#[test]
fn a_life_with_a_job() {
    let src = format!("{MINIMAL}\n[work]\ndays = \"mon-fri\"\n");
    let j = live(&src, "none");
    hours(&j, 1, "work", 7.0); // 09:00-17:00 minus lunch, by default
    hours(&j, 1, "reading", 3.0);
}

#[test]
fn workdays_need_a_job() {
    rejects("[pursuit.a]\nwhen = \"workdays 18:00-20:00\"", "needs a [work] table");
}

#[test]
fn not_everyone_puts_people_first() {
    // Same three pursuits as the reference life, opposite values: the hobby wins.
    let src = r#"
        [priorities]
        all = ["surf", "people", "geek"]
        [pursuit.geek]
        on = "idea"
        [pursuit.people]
        when = "18:00-20:00"
        [pursuit.surf]
        session = "2h"
        [work]
        days = "mon-fri"
        [sleep]
        signal_after = 5
        [scenario.swell]
        events = [{ day = 1, at = "18:00", pursuit = "surf" }]
    "#;
    let j = live(src, "swell");
    hours(&j, 1, "surf", 2.0);
    hours(&j, 1, "people", 0.0);
    assert_eq!(j.doing(1, 18.5), Some("surf"));
}

#[test]
fn weekend_priorities_can_differ() {
    let parent = Rules::preset("parent").unwrap();
    let j = Life::new(&parent, &Scenario { start: 5, ..Scenario::plain(1) }).run(Mode::Virtual);
    assert_eq!(j.doing(1, 7.5), Some("running"), "{}", j.timeline());
}

#[test]
fn protected_pursuits_are_never_interrupted() {
    let src = r#"
        [pursuit.focus]
        when = "17:00-19:00"
        protected = true
        [pursuit.pager]
        session = "1h"
        preempts = ["focus"]
        [scenario.paged]
        events = [{ day = 1, at = "17:30", pursuit = "pager", urgent = true }]
    "#;
    let j = live(src, "paged");
    hours(&j, 1, "focus", 2.0);
    hours(&j, 1, "pager", 1.0); // it waits, then gets its hour
    assert_eq!(j.doing(1, 19.5), Some("pager"));

    let unprotected = src.replace("protected = true", "protected = false");
    let j = live(&unprotected, "paged");
    hours(&j, 1, "focus", 0.5); // dropped at 17:30
    assert_eq!(j.doing(1, 18.0), Some("pager"));
}

#[test]
fn scheduled_windows_can_preempt() {
    let parent = Rules::preset("parent").unwrap();
    let j = Life::new(&parent, &Scenario { start: 5, ..Scenario::plain(2) }).run(Mode::Virtual);
    assert_eq!(j.doing(2, 8.75), Some("guitar"), "{}", j.timeline());
    assert_eq!(j.doing(2, 9.25), Some("kids"), "kids' time interrupts guitar");
}

#[test]
fn you_choose_what_wakes_you() {
    let src = r#"
        [pursuit.kids]
        session = "1h"
        [sleep]
        woken_by = ["kids"]
        [scenario.night]
        days = 1
        events = [{ day = 2, at = "02:00", pursuit = "kids", urgent = true }]
    "#;
    let j = live(src, "night");
    assert_eq!(j.day(1).tally.wakeups, 1);
    assert_eq!(j.doing(2, 2.5), Some("kids"));

    let deep_sleeper = src.replace("woken_by = [\"kids\"]", "woken_by = []");
    let j = live(&deep_sleeper, "night");
    assert_eq!(j.day(1).tally.wakeups, 0);
    assert_eq!(j.doing(2, 2.5), Some("sleep"));
}

#[test]
fn signals_need_enough_sleep() {
    let src = r#"
        [pursuit.write]
        on = "muse"
        session = "1h"
        [sleep]
        bed = "23:30"
        signal = "muse"
        signal_after = 5
    "#;
    let j = live(src, "none");
    assert!(!j.day(1).tally.signal, "7h is 4 cycles");
    hours(&j, 2, "write", 0.0);

    let j = live(&src.replace("signal_after = 5", "signal_after = 4"), "none");
    assert!(j.day(1).tally.signal);
    hours(&j, 2, "write", 1.0);
}

#[test]
fn caffeine_rules_are_yours() {
    let src = r#"
        [pursuit.x]
        when = "18:00-19:00"
        [caffeine]
        cutoff = "18:00"
        wired = "1h"
        [scenario.cup]
        events = [{ day = 1, at = "17:00", coffee = 1 }, { day = 1, at = "18:30", coffee = 1 }]
    "#;
    let j = live(src, "cup");
    assert_eq!(j.day(1).tally.coffee, 2);
    hours(&j, 1, "wired", 0.0); // 18:30 + 1h is long before bed
}

#[test]
fn the_agent_preset_preempts_research() {
    let agent = Rules::preset("agent").unwrap();
    let j = Life::new(&agent, agent.scenario("incident").unwrap()).run(Mode::Virtual);
    assert!(j.said("users preempts research"));
    assert_eq!(j.day(1).tally.wakeups, 1, "the 02:00 page");
}

// ----------------------------------------------------------- bad life files

#[test]
fn typos_are_errors_not_silence() {
    rejects("[pursuit.a]\nsesion = \"2h\"", "unknown key `pursuit.a.sesion`");
    rejects("[pursuit.a]\n[sleeep]\nwake = \"06:00\"", "unknown key `sleeep`");
}

#[test]
fn unknown_pursuits_are_named() {
    rejects("[pursuit.a]\n[priorities]\nall = [\"a\", \"b\"]", "no pursuit named `b` (have: a)");
    rejects("[pursuit.a]\n[sleep]\nwoken_by = [\"kids\"]", "no pursuit named `kids`");
}

#[test]
fn bad_values_say_what_they_should_be() {
    rejects("[pursuit.a]\nsession = \"two hours\"", "not a duration like \"1h30m\"");
    rejects("[pursuit.a]\nwhen = \"18:00 to 20:00\"", "window like \"18:00-20:00\"");
    rejects("[sleep]\nwake = 6\n[pursuit.a]", "should be a string");
    rejects("[pursuit.a]\n[scenario.s]\nevents = [{ day = 1 }]", "needs one of");
    rejects("[pursuit.sleep]", "built in");
    rejects("", "at least one [pursuit");
    rejects("[pursuit.ab]\n[pursuit.ac]", "share a glyph");
}

// ------------------------------------------------------------------- parser

#[test]
fn toml_subset() {
    let t = toml::parse(
        r#"
        # comment
        title = "life" # trailing
        n = 5
        x = -1.5
        ok = true
        list = [
            "a", # comments inside arrays
            "b",
        ]
        inline = { day = 1, at = "06:30" }
        "quoted key" = 'literal \n'
        [a.b]
        c = "☕ \"coffee\""
        [[e]]
        k = 1
        [[e]]
        k = 2
        "#,
    )
    .unwrap();
    use toml::Value::*;
    assert_eq!(t["title"], Str("life".into()));
    assert_eq!(t["n"], Int(5));
    assert_eq!(t["x"], Float(-1.5));
    assert_eq!(t["ok"], Bool(true));
    assert_eq!(t["list"], Array(vec![Str("a".into()), Str("b".into())]));
    assert_eq!(t["quoted key"], Str("literal \\n".into()));
    let Table(a) = &t["a"] else { panic!() };
    let Table(b) = &a["b"] else { panic!() };
    assert_eq!(b["c"], Str("☕ \"coffee\"".into()));
    let Array(e) = &t["e"] else { panic!() };
    assert_eq!(e.len(), 2);
}

#[test]
fn toml_errors_have_line_numbers() {
    let e = toml::parse("a = 1\nb = \nc = 3").unwrap_err();
    assert_eq!(e.line, 2);
    let e = toml::parse("a = 1\na = 2").unwrap_err();
    assert!(e.msg.contains("defined twice"));
    let e = toml::parse("wake = 06:30").unwrap_err();
    assert!(e.msg.contains("times need quotes"));
}
