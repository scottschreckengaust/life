//! What actually happened.

use crate::{hhmm, EARTH_SPIN};
use std::fmt::Write;

/// Everything you can be doing: the engine's own activities, plus the pursuits a life file defines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Activity {
    Sleep,
    /// Awake in bed, `spin_loop()`ing on caffeine.
    Wired,
    Eat,
    Work,
    Travel,
    Live,
    /// Index into the life file's pursuits.
    Pursuit(usize),
}

impl Activity {
    /// The built-in activities, in the order the names in [`crate::rules::BUILTIN`] use.
    pub const BUILTIN: [Activity; 6] =
        [Activity::Sleep, Activity::Wired, Activity::Eat, Activity::Work, Activity::Travel, Activity::Live];

    /// When two things overlap on the timeline, the higher rank is drawn.
    fn rank(self) -> u8 {
        match self {
            Activity::Live => 0,
            Activity::Eat => 1,
            Activity::Work => 2,
            Activity::Travel => 3,
            Activity::Pursuit(_) => 4,
            Activity::Sleep => 5,
            Activity::Wired => 6,
        }
    }
}

/// A stretch of time doing one thing. `day` is the day it belongs to, so a night's
/// sleep counts toward the day you went to bed.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub day: usize,
    pub what: Activity,
    pub start: f64,
    pub end: f64,
}

/// A line in the diary.
#[derive(Clone, Debug, PartialEq)]
pub struct Moment {
    pub at: f64,
    pub note: String,
}

/// Counters that are not durations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tally {
    /// Full REM cycles.
    pub dreams: u32,
    /// The night after this day produced the sleep signal (an idea, for the reference life).
    pub signal: bool,
    pub coffee: u32,
    /// Times something pulled you out of bed before the alarm.
    pub wakeups: u32,
}

/// One day, totalled.
#[derive(Clone, Debug, PartialEq)]
pub struct DaySummary {
    pub day: usize,
    names: Vec<String>,
    hours: Vec<f64>,
    pub tally: Tally,
}

impl DaySummary {
    /// Hours spent on `name` (a pursuit, or sleep, wired, eat, work, travel, living),
    /// including the night that followed this day.
    pub fn hours(&self, name: &str) -> f64 {
        self.names.iter().position(|n| n == name).map_or(0.0, |i| self.hours[i])
    }
}

/// Everything that happened, in order.
#[derive(Clone, Debug, PartialEq)]
pub struct Journal {
    pub spans: Vec<Span>,
    pub moments: Vec<Moment>,
    pub(crate) tallies: Vec<Tally>,
    /// Pursuit names and glyphs, from the life file.
    pub(crate) pursuits: Vec<(String, char)>,
    pub(crate) signal: String,
}

impl Journal {
    pub(crate) fn new(days: usize, pursuits: Vec<(String, char)>, signal: String) -> Self {
        Journal { spans: vec![], moments: vec![], tallies: vec![Tally::default(); days], pursuits, signal }
    }

    pub(crate) fn mark(&mut self, day: usize, what: Activity, start: f64, end: f64) {
        if end - start < 1e-9 {
            return;
        }
        match self.spans.last_mut() {
            Some(last) if last.day == day && last.what == what && (last.end - start).abs() < 1e-9 => {
                last.end = end;
            }
            _ => self.spans.push(Span { day, what, start, end }),
        }
    }

    /// The name of an activity: a pursuit's name, or a built-in one.
    pub fn name(&self, what: Activity) -> &str {
        match what {
            Activity::Pursuit(i) => &self.pursuits[i].0,
            other => {
                let i = Activity::BUILTIN.iter().position(|a| *a == other).unwrap();
                crate::rules::BUILTIN[i]
            }
        }
    }

    /// One character on the timeline.
    pub fn glyph(&self, what: Activity) -> char {
        match what {
            Activity::Sleep => 'z',
            Activity::Wired => '~',
            Activity::Eat => 'e',
            Activity::Work => 'W',
            Activity::Travel => '>',
            Activity::Live => '.',
            Activity::Pursuit(i) => self.pursuits[i].1,
        }
    }

    fn activities(&self) -> Vec<Activity> {
        let mut all = Activity::BUILTIN.to_vec();
        all.extend((0..self.pursuits.len()).map(Activity::Pursuit));
        all
    }

    /// How many days were lived.
    pub fn days(&self) -> usize {
        self.tallies.len()
    }

    /// Totals for day `day` (1-based).
    pub fn day(&self, day: usize) -> DaySummary {
        let all = self.activities();
        let mut hours = vec![0.0; all.len()];
        for s in self.spans.iter().filter(|s| s.day == day) {
            hours[all.iter().position(|a| *a == s.what).unwrap()] += s.end - s.start;
        }
        let names = all.iter().map(|&a| self.name(a).to_string()).collect();
        DaySummary { day, names, hours, tally: self.tallies[day - 1].clone() }
    }

    /// What you were doing on calendar day `day` (1-based) at `hour` o'clock.
    pub fn doing(&self, day: usize, hour: f64) -> Option<&str> {
        self.doing_at((day - 1) as f64 * EARTH_SPIN + hour).map(|a| self.name(a))
    }

    fn doing_at(&self, t: f64) -> Option<Activity> {
        self.spans.iter().filter(|s| s.start <= t && t < s.end).map(|s| s.what).max_by_key(|a| a.rank())
    }

    /// Did the diary mention `needle`?
    pub fn said(&self, needle: &str) -> bool {
        self.moments.iter().any(|m| m.note.contains(needle))
    }

    /// One row per calendar day, two characters per hour.
    pub fn timeline(&self) -> String {
        let end = self.spans.iter().map(|s| s.end).fold(0.0, f64::max);
        let rows = (end / EARTH_SPIN).ceil() as usize;
        let mut out = String::from("        0     3     6     9     12    15    18    21    24\n");
        for d in 1..=rows {
            let row: String = (0..48)
                .map(|k| {
                    let t = (d - 1) as f64 * EARTH_SPIN + k as f64 * 0.5 + 0.25;
                    self.doing_at(t).map_or(' ', |a| self.glyph(a))
                })
                .collect();
            let _ = writeln!(out, "day {d:<3} {row}|");
        }
        let legend: Vec<String> =
            self.activities().iter().map(|&a| format!("{} {}", self.glyph(a), self.name(a))).collect();
        let _ = writeln!(out, "        {}", legend.join("  "));
        out
    }

    /// The table, the timeline, and a one-line verdict.
    /// The journal as JSON object members (`"signal"`, `"activities"`, `"days"`, `"totals"`,
    /// `"spans"`, `"diary"`), each preceded by a comma, for appending to an object that already
    /// has at least one member. `spin-life run --format json` wraps them with the run's metadata.
    pub fn to_json(&self, out: &mut String) {
        use crate::json::{array, object, Obj};
        let all = self.activities();
        let mut o = Obj::continuing(out, false);
        o.str("signal", &self.signal);
        array(o.key("activities"), &all, |out, &a| {
            object(out, |o| {
                o.str("name", self.name(a));
                o.str("glyph", &self.glyph(a).to_string());
                o.bool("pursuit", matches!(a, Activity::Pursuit(_)));
            })
        });
        let days: Vec<DaySummary> = (1..=self.days()).map(|d| self.day(d)).collect();
        let hours = |out: &mut String, h: &dyn Fn(&str) -> f64| {
            object(out, |o| all.iter().for_each(|&a| o.num(self.name(a), h(self.name(a)))))
        };
        array(o.key("days"), &days, |out, d| {
            object(out, |o| {
                o.int("day", d.day);
                hours(o.key("hours"), &|n| d.hours(n));
                o.int("dreams", d.tally.dreams);
                o.bool("signal", d.tally.signal);
                o.int("coffee", d.tally.coffee);
                o.int("wakeups", d.tally.wakeups);
            })
        });
        object(o.key("totals"), |o| {
            hours(o.key("hours"), &|n| days.iter().map(|d| d.hours(n)).sum());
            o.int("dreams", days.iter().map(|d| d.tally.dreams).sum::<u32>());
            o.int("signals", days.iter().filter(|d| d.tally.signal).count());
            o.int("coffee", days.iter().map(|d| d.tally.coffee).sum::<u32>());
            o.int("wakeups", days.iter().map(|d| d.tally.wakeups).sum::<u32>());
        });
        array(o.key("spans"), &self.spans, |out, s| {
            object(out, |o| {
                o.int("day", s.day);
                o.str("what", self.name(s.what));
                o.num("start", s.start);
                o.num("end", s.end);
            })
        });
        array(o.key("diary"), &self.moments, |out, m| {
            object(out, |o| {
                o.num("at", m.at);
                o.str("when", &hhmm(m.at));
                o.str("note", &m.note);
            })
        });
    }

    pub fn report(&self) -> String {
        let pursuits: Vec<&str> = self.pursuits.iter().map(|p| p.0.as_str()).collect();
        let mut cols: Vec<String> =
            ["sleep", "wired", "dreams", &self.signal, "coffee", "work"].map(String::from).to_vec();
        cols.extend(pursuits.iter().map(|p| p.to_string()));
        cols.push("wakeups".into());
        let width = cols.iter().map(|c| c.chars().count()).max().unwrap_or(6).max(6) + 2;
        let mut out = String::from("\nday");
        for c in &cols {
            let _ = write!(out, "{c:>width$}");
        }
        out.push('\n');
        for d in 1..=self.days() {
            let s = self.day(d);
            let h = |name: &str| format!("{:.1}h", s.hours(name));
            let mut cells = vec![
                h("sleep"),
                h("wired"),
                s.tally.dreams.to_string(),
                (if s.tally.signal { "yes" } else { "-" }).to_string(),
                s.tally.coffee.to_string(),
                h("work"),
            ];
            cells.extend(pursuits.iter().map(|p| h(p)));
            cells.push(s.tally.wakeups.to_string());
            let _ = write!(out, "{d:>3}");
            for c in cells {
                let _ = write!(out, "{c:>width$}");
            }
            out.push('\n');
        }
        let _ = writeln!(out, "(each row is a day plus the night after it)\n");
        out.push_str(&self.timeline());
        let total = |name: &str| (1..=self.days()).map(|d| self.day(d).hours(name)).sum::<f64>();
        let spent: Vec<String> = pursuits.iter().map(|p| format!("{:.0}h {p}", total(p))).collect();
        let signals = self.tallies.iter().filter(|t| t.signal).count();
        let _ = writeln!(
            out,
            "\n{} spins of the earth: {}, {:.0}h asleep, {signals} {}(s). Last entry {}.",
            self.days(),
            spent.join(", "),
            total("sleep"),
            self.signal,
            self.moments.last().map_or_else(String::new, |m| hhmm(m.at)),
        );
        out
    }
}
