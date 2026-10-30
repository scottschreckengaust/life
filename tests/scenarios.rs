//! The reference life (presets/scott.life.toml), tested. Every test runs on virtual
//! time, so a week takes microseconds and the same life comes out every time.
//! These are the same assertions the original hard-coded engine passed: the rules engine
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
fn a_span_starts_at_its_first_moment_and_ends_before_its_last() {
    // Breakfast is [06:30, 07:00): at 07:00 you are already living, not still eating.
    let j = custom(1, "");
    doing(&j, 1, 6.5, "eat");
    doing(&j, 1, 7.0, "living");
    assert_eq!(j.doing(1, 6.0), None, "nothing happened before the first alarm");
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
fn a_swell_that_comes_as_geeking_ends_does_not_preempt_it() {
    // Geeking runs 17:00-19:00; a big swell at 19:00 doesn't cut it short, and at 19:00
    // the hour of family time left comes first.
    let j = custom(
        1,
        r#"{ day = 1, at = "17:00", signal = "idea" }, { day = 1, at = "19:00", pursuit = "surf", urgent = true }"#,
    );
    hours(&j, 1, "geek", 2.0);
    hours(&j, 1, "people", 1.0);
    hours(&j, 1, "surf", 0.0);
    assert!(!j.said("preempts"), "{:?}", j.moments);
}

#[test]
fn a_swell_that_is_gone_can_not_preempt() {
    // A big swell at 14:00 lasts 3 hours: gone at 17:00, just as the idea's geeking starts.
    let j = custom(2, r#"{ day = 2, at = "14:00", pursuit = "surf", urgent = true }"#);
    hours(&j, 2, "geek", 2.0);
    hours(&j, 2, "surf", 0.0);
    assert!(!j.said("preempts"), "{:?}", j.moments);
}

#[test]
fn a_big_wave_does_not_break_family_time() {
    let j = custom(1, r#"{ day = 1, at = "18:30", pursuit = "surf", urgent = true }"#);
    hours(&j, 1, "people", 2.0);
    hours(&j, 1, "surf", 0.0);
    assert!(!j.said("preempts"), "nothing was dropped");
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
fn a_swell_at_the_alarm_is_the_morning_not_a_wake_up() {
    let j = custom(2, r#"{ day = 2, at = "06:30", pursuit = "surf", urgent = true }"#);
    assert_eq!(j.day(1).tally.wakeups, 0);
    hours(&j, 1, "sleep", 8.0);
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
fn coffee_due_at_the_table_waits_until_after_the_meal() {
    // Work stops for lunch at 12:00; a cup due then is served when work resumes at 13:00.
    let j = custom(1, r#"{ day = 1, at = "12:00", coffee = 1 }"#);
    let served: Vec<f64> = j.moments.iter().filter(|m| m.note.starts_with("☕")).map(|m| m.at).collect();
    assert_eq!(served, [13.0]);
    assert_eq!(j.day(1).tally.coffee, 1);
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
fn an_event_that_ends_before_it_starts_runs_past_midnight() {
    // 23:00 until 01:00 is two hours, into the next day.
    let j = custom(1, r#"{ day = 1, at = "23:00", until = "01:00", pursuit = "people", urgent = true }"#);
    assert_eq!(j.day(1).tally.wakeups, 1);
    hours(&j, 1, "people", 2.0 + 2.0); // family dinner, then the party
    doing(&j, 2, 0.5, "people");
    doing(&j, 2, 1.5, "sleep");
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
fn up_late_until_noon_is_not_a_late_night() {
    // Before 12:00 means after midnight; 12:00 itself is today's noon, long gone by dinner.
    let j = custom(1, r#"{ day = 1, up_late = "12:00", doing = "geek" }"#);
    hours(&j, 1, "geek", 0.0);
    hours(&j, 1, "sleep", 8.0);
}

#[test]
fn up_late_into_the_morning_skips_the_night_not_the_next_day() {
    // Up until the alarm, or past it: no sleep, and the next day picks up from there. (This
    // used to wait forever for an alarm that had already rung.)
    let j = custom(2, r#"{ day = 1, up_late = "06:30", doing = "geek" }"#);
    hours(&j, 1, "geek", 8.0);
    hours(&j, 1, "sleep", 0.0);
    doing(&j, 2, 6.75, "eat");
    let j = custom(2, r#"{ day = 1, up_late = "10:00", doing = "geek" }"#);
    hours(&j, 1, "geek", 11.5);
    hours(&j, 1, "sleep", 0.0);
    doing(&j, 2, 10.25, "eat"); // a late breakfast
    hours(&j, 2, "work", 5.5); // 10:30 to 17:00, less lunch
    hours(&j, 2, "sleep", 8.0);
}

#[test]
fn an_idea_waits_for_a_free_evening_and_a_second_one_does_not_pile_up() {
    // Day 2's deadline leaves no evening for night 1's idea. Night 2 has another, but an idea
    // you haven't acted on yet is still the one idea: day 3 geeks one session, not two.
    let j = custom(3, r#"{ day = 2, work_until = "21:00" }"#);
    assert!(j.day(1).tally.signal && j.day(2).tally.signal);
    hours(&j, 2, "geek", 0.0);
    hours(&j, 3, "geek", 2.0);
    hours(&j, 3, "people", 1.0);
}

#[test]
fn work_that_ends_at_lunchtime_skips_lunch() {
    let j = custom(1, r#"{ day = 1, work_until = "12:00" }"#);
    hours(&j, 1, "work", 3.0);
    assert!(!j.said("lunch"), "{:?}", j.moments);
    doing(&j, 1, 12.5, "living");
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

#[test]
fn the_diary_echoes_to_any_writer_and_a_failing_one_just_stops_it() {
    use std::{
        io,
        sync::{Arc, Mutex},
    };
    /// Accepts `left` lines, then fails every write, like a reader that went away.
    struct GoesAway {
        got: Arc<Mutex<String>>,
        left: usize,
        tries: Arc<Mutex<usize>>,
    }
    impl io::Write for GoesAway {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            *self.tries.lock().unwrap() += 1;
            if self.left == 0 {
                return Err(io::ErrorKind::BrokenPipe.into());
            }
            if buf.ends_with(b"\n") {
                self.left -= 1;
            }
            self.got.lock().unwrap().push_str(std::str::from_utf8(buf).unwrap());
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let rules = Rules::preset("scott").unwrap();
    let (got, tries) = (Arc::new(Mutex::new(String::new())), Arc::new(Mutex::new(0)));
    let out = GoesAway { got: got.clone(), left: 2, tries: tries.clone() };
    let j = Life::new(&rules, rules.scenario("happy-path").unwrap()).echo_to(out).run(Mode::Virtual);
    assert_eq!(
        got.lock().unwrap().lines().collect::<Vec<_>>(),
        ["d1 06:30  ⏰ rise and shine", "d1 06:30  🍳 breakfast"]
    );
    let tries_after = *tries.lock().unwrap();
    assert!(j.moments.len() > 10, "the journal keeps everything");
    // After the failure the echo is gone: no more write attempts for the rest of the run.
    assert!(tries_after < 10, "{tries_after} write attempts");
    // echo(false) turns it off; echo(true) is stdout.
    let quiet = Life::new(&rules, rules.scenario("happy-path").unwrap()).echo(false);
    assert!(format!("{quiet:?}").contains("Echo(off)"));
    let loud = Life::new(&rules, rules.scenario("happy-path").unwrap()).echo(true);
    assert!(format!("{loud:?}").contains("Echo(on)"));
}

#[test]
fn life_stays_send_and_unwind_safe() {
    // cargo-semver-checks treats losing either as a breaking change (0.2.1 had both).
    fn ok<T: Send + std::panic::UnwindSafe>() {}
    ok::<Life>();
}

#[test]
fn nothing_is_recorded_past_the_end_of_a_run() {
    // Every scenario of every preset, and a life whose sessions and opportunities outlast
    // any run: no span or diary line may reach past the run's last morning, and the
    // timeline has one row per day.
    let mut lives: Vec<(Rules, String)> =
        PRESETS.iter().map(|(name, ..)| (Rules::preset(name).unwrap(), name.to_string())).collect();
    let src = Rules::preset_source("scott")
        .unwrap()
        .replace("session = \"PT2H\"", "session = \"P99999999999D\"")
        .replace("lasts    = \"PT3H\"", "lasts    = \"P30D\"");
    assert!(src.contains("P99999999999D") && src.contains("P30D"), "the preset changed; update this test");
    lives.push((Rules::parse(&src).unwrap(), "endless sessions".into()));
    for (rules, who) in &lives {
        for scenario in &rules.scenarios {
            let life = Life::new(rules, scenario);
            let end = life.ends_at();
            let j = life.run(Mode::Virtual);
            let what = format!("{who} / {}", scenario.name);
            for span in &j.spans {
                assert!(span.end <= end + 1e-9, "{what}: {span:?} ends after the run ({end})");
                // Spans that began after the end are dropped, not left empty or inside out.
                assert!(span.end - span.start > 1e-9, "{what}: {span:?} is empty or reversed");
            }
            assert!(j.moments.iter().all(|m| m.at <= end + 1e-9), "{what}: a diary line after the run");
            let rows = j.timeline().lines().filter(|l| l.starts_with("day ")).count();
            assert!(rows <= j.days() + 1, "{what}: {rows} timeline rows for {} days", j.days());
        }
    }
}
