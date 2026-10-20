//! The reference life (presets/scott.life.toml), tested. Every test runs on virtual
//! time, so a week takes microseconds and the same life comes out every time.
//! These are the same assertions the hard-coded v0.1 engine passed: the rules engine
//! reproduces it exactly.

use spin_life::{rt::Mode, Journal, Life, Rules, PRESETS};

const SCOTT: &str = PRESETS[0].2;

fn scenario(name: &str) -> Journal {
    let rules = Rules::preset("scott").unwrap();
    Life::new(&rules, rules.scenario(name).expect(name)).run(Mode::Virtual)
}

/// The reference life with one extra scenario, written inline.
fn custom(days: usize, events: &str) -> Journal {
    let src = format!("{SCOTT}\n[scenario.test]\ndays = {days}\nevents = [{events}]\n");
    let rules = Rules::parse(&src).unwrap();
    Life::new(&rules, rules.scenario("test").unwrap()).run(Mode::Virtual)
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
fn doing(j: &Journal, day: usize, hour: f64, expected: &str) {
    assert_eq!(j.doing(day, hour), Some(expected), "day {day} at {hour}h\n{}", j.timeline());
}

// ------------------------------------------------------------------ happy path

#[test]
fn happy_path_wake_eat_work_lunch_family_eat_sleep() {
    let j = custom(1, "");
    doing(&j, 1, 6.75, "eat");
    doing(&j, 1, 10.0, "work");
    doing(&j, 1, 12.5, "eat");
    doing(&j, 1, 15.0, "work");
    doing(&j, 1, 19.0, "people");
    doing(&j, 1, 20.5, "eat");
    doing(&j, 1, 23.0, "sleep");
    doing(&j, 2, 3.0, "sleep");
    hours(&j, 1, "work", 7.0);
    hours(&j, 1, "people", 2.0);
    hours(&j, 1, "sleep", 8.0);
    assert_eq!(j.day(1).tally.dreams, 5);
    assert!(j.day(1).tally.signal, "8h of sleep is 5 REM cycles, which is an idea");
}

#[test]
fn happy_path_repeats() {
    let j = scenario("happy-path");
    assert_eq!(j.days(), 3);
    for d in 1..=3 {
        hours(&j, d, "work", 7.0);
        hours(&j, d, "sleep", 8.0);
    }
    let (two, three) = (j.day(2), j.day(3));
    assert_eq!((two.hours("geek"), two.tally.clone()), (three.hours("geek"), three.tally.clone()));
}

#[test]
fn the_earth_spins_once_per_alarm() {
    let j = scenario("week");
    let alarms: Vec<f64> =
        j.moments.iter().filter(|m| m.note.starts_with("⏰ alarm")).map(|m| m.at).collect();
    assert_eq!(alarms.len(), 7);
    assert!(alarms.windows(2).all(|w| w[1] > w[0]));
    assert_eq!(alarms[0], 24.0 + 6.5, "Tuesday 06:30");
}

#[test]
fn weekends_have_no_work_and_more_sleep() {
    let j = scenario("week");
    hours(&j, 6, "work", 0.0);
    hours(&j, 7, "work", 0.0);
    hours(&j, 5, "sleep", 9.5); // Friday night until Saturday 08:00
    hours(&j, 6, "surf", 2.0);
}

// ---------------------------------------------------------------- priorities

#[test]
fn an_idea_from_last_night_is_geeked_out_first() {
    let j = custom(2, "");
    doing(&j, 2, 17.5, "geek");
    doing(&j, 2, 19.5, "people"); // whatever family time is left
    hours(&j, 2, "people", 1.0);
}

#[test]
fn on_a_tie_biased_goes_to_geek() {
    let j = custom(1, r#"{ day = 1, at = "18:00", signal = "idea" }"#);
    doing(&j, 1, 18.5, "geek");
    hours(&j, 1, "people", 0.0); // family was 18:00 to 20:00. Priorities have consequences.
}

#[test]
fn people_beat_surf_on_a_tie() {
    let src = SCOTT.replace("[scenario.happy-path]", "[scenario.t]\nstart = \"sat\"\nevents = [{ day = 1, at = \"12:00\", pursuit = \"surf\" }]\n\n[scenario.happy-path]");
    let rules = Rules::parse(&src).unwrap();
    let j = Life::new(&rules, rules.scenario("t").unwrap()).run(Mode::Virtual);
    doing(&j, 1, 12.5, "people");
    hours(&j, 1, "surf", 0.0); // the swell was gone by 15:00
}

#[test]
fn surf_when_nobody_needs_you() {
    let j = custom(1, r#"{ day = 1, skip = "people" }, { day = 1, at = "17:30", pursuit = "surf" }"#);
    doing(&j, 1, 18.0, "surf");
    hours(&j, 1, "surf", 2.0);
}

#[test]
fn quiet_evening_is_just_living() {
    let j = custom(1, r#"{ day = 1, skip = "people" }"#);
    doing(&j, 1, 18.0, "living");
    hours(&j, 1, "people", 0.0);
}

// ----------------------------------------------------------------- big waves

#[test]
fn a_big_wave_breaks_into_geeking() {
    let j = scenario("big-wave");
    doing(&j, 1, 17.5, "geek");
    doing(&j, 1, 18.5, "surf");
    hours(&j, 1, "geek", 1.0); // cancelled mid-.await, but the Drop guard kept the hour
    hours(&j, 1, "surf", 2.0);
    assert!(j.said("surf preempts geek"));
}

#[test]
fn a_big_wave_does_not_break_family_time() {
    let j = custom(1, r#"{ day = 1, at = "18:30", pursuit = "surf", urgent = true }"#);
    hours(&j, 1, "people", 2.0);
    hours(&j, 1, "surf", 0.0);
}

#[test]
fn dawn_patrol_rings_the_alarm_early() {
    let j = scenario("dawn-patrol");
    assert_eq!(j.day(1).tally.wakeups, 1);
    doing(&j, 2, 6.0, "surf"); // the real alarm rang at 06:30 while you were out
    doing(&j, 2, 7.75, "eat"); // breakfast after the session
    doing(&j, 2, 9.5, "work"); // still on time for work
    hours(&j, 1, "sleep", 7.0);
    assert!(!j.day(1).tally.signal, "4 REM cycles, no idea");
}

#[test]
fn a_small_swell_at_night_lets_you_sleep() {
    let j = custom(1, r#"{ day = 2, at = "04:00", pursuit = "surf" }"#);
    assert_eq!(j.day(1).tally.wakeups, 0);
    hours(&j, 1, "sleep", 8.0);
}

// -------------------------------------------------------------------- coffee

#[test]
fn morning_coffee_is_free() {
    let j = scenario("coffee");
    assert_eq!(j.day(1).tally.coffee, 1);
    hours(&j, 1, "wired", 0.0);
    hours(&j, 1, "sleep", 8.0);
}

#[test]
fn a_3pm_coffee_wears_off_by_bedtime() {
    let j = scenario("coffee");
    hours(&j, 2, "wired", 0.0);
    assert_eq!(j.day(2).tally.dreams, 5);
}

#[test]
fn a_triple_espresso_at_8pm_spins_all_night() {
    let j = scenario("coffee");
    assert_eq!(j.day(3).tally.coffee, 3);
    hours(&j, 3, "wired", 6.5); // 22:30 until 05:00: spin_loop() without dream()
    hours(&j, 3, "sleep", 1.5);
    assert_eq!(j.day(3).tally.dreams, 1);
    assert!(!j.day(3).tally.signal);
    hours(&j, 4, "geek", 0.0); // no idea the next day
}

// ------------------------------------------------------- parties and events

#[test]
fn a_party_next_door_wakes_you_up() {
    let j = scenario("party");
    assert_eq!(j.day(1).tally.wakeups, 1);
    doing(&j, 2, 1.0, "people");
    doing(&j, 2, 3.0, "sleep"); // back to bed
    hours(&j, 1, "sleep", 6.0);
    hours(&j, 1, "people", 4.0);
    assert!(!j.day(1).tally.signal);
}

#[test]
fn a_special_event_runs_past_the_evening_window() {
    let j = scenario("special-event");
    doing(&j, 1, 21.5, "people");
    hours(&j, 1, "people", 3.5);
    hours(&j, 1, "sleep", 7.5); // late dinner, late bed
}

// ------------------------------------------------- deadlines and late nights

#[test]
fn late_night_coding_costs_tomorrows_idea() {
    let j = scenario("late-night");
    doing(&j, 2, 1.0, "geek");
    hours(&j, 1, "geek", 3.5);
    hours(&j, 1, "sleep", 4.5);
    assert!(!j.day(1).tally.signal);
    hours(&j, 2, "geek", 0.0);
}

#[test]
fn a_deadline_eats_the_evening() {
    let j = scenario("deadline");
    hours(&j, 1, "work", 11.0);
    hours(&j, 1, "people", 0.0);
    doing(&j, 1, 21.5, "eat");
    assert!(j.said("shipped"));
}

#[test]
fn an_early_flight_means_an_early_alarm() {
    let j = scenario("early-flight");
    hours(&j, 1, "sleep", 6.0); // alarm at 04:30
    doing(&j, 2, 6.0, "travel");
    hours(&j, 2, "work", 7.0); // lands at 09:00, works a full day
    hours(&j, 2, "people", 0.0);
    assert!(!j.day(1).tally.signal);
}

// --------------------------------------------------------------- all of them

#[test]
fn every_scenario_of_every_preset_lives_the_same_life_twice() {
    for (preset, _, _) in PRESETS {
        let rules = Rules::preset(preset).unwrap();
        assert!(!rules.scenarios.is_empty(), "{preset} has no scenarios");
        for s in &rules.scenarios {
            let a = Life::new(&rules, s).run(Mode::Virtual);
            let b = Life::new(&rules, s).run(Mode::Virtual);
            assert_eq!(a, b, "{preset}/{} is not deterministic", s.name);
            assert_eq!(a.days(), s.days);
            for d in 1..=a.days() {
                let total: f64 = a.spans.iter().filter(|x| x.day == d).map(|x| x.end - x.start).sum();
                assert!(total > 20.0, "{preset}/{} day {d} only accounts for {total}h", s.name);
            }
        }
    }
}

#[test]
fn the_timeline_draws_two_characters_per_hour() {
    let j = scenario("happy-path");
    let timeline = j.timeline();
    let rows: Vec<&str> = timeline.lines().filter(|l| l.starts_with("day")).collect();
    assert_eq!(rows.len(), 4); // three days, plus the morning after
    assert!(rows.iter().all(|r| r.chars().count() == 8 + 48 + 1));
}

#[test]
fn real_time_mode_matches_virtual_time_roughly() {
    // 20ms per simulated hour: one day takes about half a second.
    let rules = Rules::preset("scott").unwrap();
    let j = Life::new(&rules, &spin_life::Scenario::plain(1))
        .run(Mode::RealTime(std::time::Duration::from_millis(20)));
    let close = |a: f64, b: f64| (a - b).abs() < 0.5;
    assert!(close(j.day(1).hours("work"), 7.0), "{}", j.timeline());
    assert!(close(j.day(1).hours("sleep"), 8.0), "{}", j.timeline());
}
