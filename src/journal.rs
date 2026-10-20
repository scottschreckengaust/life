//! What actually happened.

use crate::{hhmm, EARTH_SPIN};
use std::fmt::Write;

/// Everything you can be doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Activity {
    Sleep,
    /// Awake in bed, `spin_loop()`ing on caffeine.
    Wired,
    Eat,
    Work,
    Flight,
    Geek,
    People,
    Surf,
    Live,
}

impl Activity {
    pub const ALL: [Activity; 9] = [
        Activity::Sleep,
        Activity::Wired,
        Activity::Eat,
        Activity::Work,
        Activity::Flight,
        Activity::Geek,
        Activity::People,
        Activity::Surf,
        Activity::Live,
    ];

    /// One character on the timeline.
    pub fn glyph(self) -> char {
        match self {
            Activity::Sleep => 'z',
            Activity::Wired => '~',
            Activity::Eat => 'e',
            Activity::Work => 'W',
            Activity::Flight => '>',
            Activity::Geek => 'G',
            Activity::People => 'P',
            Activity::Surf => 'S',
            Activity::Live => '.',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Activity::Sleep => "sleep",
            Activity::Wired => "wired",
            Activity::Eat => "eat",
            Activity::Work => "work",
            Activity::Flight => "flight",
            Activity::Geek => "geek",
            Activity::People => "people",
            Activity::Surf => "surf",
            Activity::Live => "living",
        }
    }

    /// When two things overlap on the timeline, the higher rank is drawn.
    fn rank(self) -> u8 {
        match self {
            Activity::Live => 0,
            Activity::Eat => 1,
            Activity::Work => 2,
            Activity::Flight => 3,
            Activity::People => 4,
            Activity::Geek => 5,
            Activity::Surf => 6,
            Activity::Sleep => 7,
            Activity::Wired => 8,
        }
    }
}

/// A stretch of time doing one thing. `day` is the plan it belongs to, so a night's
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
    /// Woke up with an idea after this day's night.
    pub idea: bool,
    pub coffee: u32,
    /// Times something pulled you out of bed before the alarm.
    pub wakeups: u32,
}

/// One day, totalled.
#[derive(Clone, Debug, PartialEq)]
pub struct DaySummary {
    pub day: usize,
    hours: [f64; 9],
    pub tally: Tally,
}

impl DaySummary {
    /// Hours spent on `what`, including the night that followed this day.
    pub fn hours(&self, what: Activity) -> f64 {
        self.hours[Activity::ALL.iter().position(|a| *a == what).unwrap()]
    }
}

/// Everything that happened, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Journal {
    pub spans: Vec<Span>,
    pub moments: Vec<Moment>,
    pub(crate) tallies: Vec<Tally>,
}

impl Journal {
    pub(crate) fn new(days: usize) -> Self {
        Journal { tallies: vec![Tally::default(); days], ..Default::default() }
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

    /// How many days were lived.
    pub fn days(&self) -> usize {
        self.tallies.len()
    }

    /// Totals for day `day` (1-based).
    pub fn day(&self, day: usize) -> DaySummary {
        let mut hours = [0.0; 9];
        for s in self.spans.iter().filter(|s| s.day == day) {
            let i = Activity::ALL.iter().position(|a| *a == s.what).unwrap();
            hours[i] += s.end - s.start;
        }
        DaySummary { day, hours, tally: self.tallies[day - 1].clone() }
    }

    /// What you were doing on calendar day `day` (1-based) at `hour` o'clock.
    pub fn doing(&self, day: usize, hour: f64) -> Option<Activity> {
        self.doing_at((day - 1) as f64 * EARTH_SPIN + hour)
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
                    self.doing_at(t).map_or(' ', Activity::glyph)
                })
                .collect();
            let _ = writeln!(out, "day {d:<3} {row}|");
        }
        let legend: Vec<String> =
            Activity::ALL.iter().map(|a| format!("{} {}", a.glyph(), a.label())).collect();
        let _ = writeln!(out, "        {}", legend.join("  "));
        out
    }

    /// The table, the timeline, and a one-line verdict.
    pub fn report(&self) -> String {
        let mut out = String::from("\nday");
        for col in ["sleep", "wired", "dreams", "idea", "coffee", "work", "geek", "people", "surf", "wakeups"]
        {
            let _ = write!(out, "{col:>8}");
        }
        out.push('\n');
        for d in 1..=self.days() {
            let s = self.day(d);
            let h = |a| format!("{:.1}h", s.hours(a));
            let cells = [
                h(Activity::Sleep),
                h(Activity::Wired),
                s.tally.dreams.to_string(),
                (if s.tally.idea { "yes" } else { "-" }).to_string(),
                s.tally.coffee.to_string(),
                h(Activity::Work),
                h(Activity::Geek),
                h(Activity::People),
                h(Activity::Surf),
                s.tally.wakeups.to_string(),
            ];
            let _ = write!(out, "{d:>3}");
            for c in cells {
                let _ = write!(out, "{c:>8}");
            }
            out.push('\n');
        }
        let _ = writeln!(out, "(each row is a day plus the night after it)");
        let _ = writeln!(out, "\n{}", self.timeline());
        let total = |a| (1..=self.days()).map(|d| self.day(d).hours(a)).sum::<f64>();
        let ideas = self.tallies.iter().filter(|t| t.idea).count();
        let _ = writeln!(
            out,
            "{} spins of the earth: {:.0}h geeking, {:.0}h with people, {:.0}h surfing, {:.0}h asleep, {ideas} idea(s). Last entry {}.",
            self.days(),
            total(Activity::Geek),
            total(Activity::People),
            total(Activity::Surf),
            total(Activity::Sleep),
            self.moments.last().map_or_else(String::new, |m| hhmm(m.at)),
        );
        out
    }
}
