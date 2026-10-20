//! # life
//!
//! ```text
//! loop { select! { biased; _ = geek() => {}, _ = people() => {}, _ = surf() => {} } join!(live(), eat()); ALARM.store(false, Relaxed); while !ALARM.load(Acquire) { dream(); spin_loop(); } }
//! ```
//!
//! A day-by-day simulation of that line, on a zero-dependency runtime with a clock
//! you can bend. Plan some [`Day`]s, hand them to [`Life`], and read the [`Journal`].
//!
//! ```
//! use life::{rt::Mode, Activity, Day, Life};
//!
//! let journal = Life::new(vec![Day::workday(), Day::workday()]).run(Mode::Virtual);
//! assert_eq!(journal.day(1).hours(Activity::Work), 7.0);
//! assert!(journal.day(1).tally.idea); // 8 hours of sleep, 5 REM cycles, 1 idea
//! ```

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

mod alarm;
mod journal;
mod macros;
mod plan;
pub mod rt;

pub use alarm::{Alarm, Why};
pub use journal::{Activity, DaySummary, Journal, Moment, Span, Tally};
pub use plan::{Day, DayKind, Event, Scenario};

use std::{
    cell::{Cell, RefCell},
    collections::HashSet,
    future::pending,
    hint::spin_loop,
    sync::Arc,
};

/// One spin of the earth, in hours.
pub const EARTH_SPIN: f64 = 24.0;
/// One full dream.
pub const REM_CYCLE: f64 = 1.5;
/// REM cycles in one night that send you into the day with an idea.
pub const IDEA_AFTER: u32 = 5;
/// How long a geek or surf session runs.
pub const SESSION: f64 = 2.0;
/// How long a swell stays rideable.
pub const SWELL_LASTS: f64 = 3.0;
/// An afternoon coffee keeps you wired this long, plus 1.5h per extra shot.
pub const CAFFEINE: f64 = 6.0;
/// The granularity of one `dream()`: the alarm can still cut it short.
const TICK: f64 = 0.25;

/// `"d3 07:30"` for an absolute simulated hour.
pub fn hhmm(abs: f64) -> String {
    let minutes = (abs * 60.0).round().max(0.0) as u64;
    let (day, m) = (minutes / 1440 + 1, minutes % 1440);
    format!("d{day} {:02}:{:02}", m / 60, m % 60)
}

/// Somebody living the days in a plan.
#[derive(Debug)]
pub struct Life {
    days: Vec<Day>,
    echo: bool,
    alarm: Arc<Alarm>,
    journal: RefCell<Journal>,
    today: Cell<usize>,
    idea: Cell<bool>,
    used: RefCell<HashSet<usize>>,
    dreamt: Cell<f64>,
}

enum Pick {
    Geek,
    People(usize),
    Surf(usize),
    Rest,
}

/// Records how long an activity ran when it ends, *or when it is cancelled*.
/// That makes [`Life::spend`] cancel-safe: a big swell can drop a geek session
/// mid-`.await` and the hours you already put in still land in the journal.
struct Doing<'a> {
    life: &'a Life,
    what: Activity,
    since: f64,
}

impl Drop for Doing<'_> {
    fn drop(&mut self) {
        self.life.mark(self.what, self.since, self.life.now());
    }
}

impl Life {
    pub fn new(days: Vec<Day>) -> Self {
        Life {
            journal: RefCell::new(Journal::new(days.len())),
            days,
            echo: false,
            alarm: Arc::default(),
            today: Cell::new(1),
            idea: Cell::new(false),
            used: RefCell::default(),
            dreamt: Cell::new(0.0),
        }
    }

    pub fn scenario(s: &Scenario) -> Self {
        Self::new(s.days.clone())
    }

    /// Print the diary as it happens.
    pub fn echo(mut self, on: bool) -> Self {
        self.echo = on;
        self
    }

    /// Live every planned day. The clock starts at the first morning's alarm.
    pub fn run(self, mode: rt::Mode) -> Journal {
        let start = self.days.first().map_or(0.0, |d| d.wake);
        rt::block_on(mode, start, self.live())
    }

    async fn live(self) -> Journal {
        let n = self.days.len();
        // Sunrise: one ring per spin of the earth, at each next morning's wake time.
        let wakes: Vec<f64> = (1..=n)
            .map(|d| d as f64 * EARTH_SPIN + self.days.get(d).map_or(Day::workday().wake, |next| next.wake))
            .collect();
        let alarm = self.alarm.clone();
        let sunrise = rt::spawn(async move {
            for at in wakes {
                rt::sleep_until(at).await;
                alarm.ring(Why::MORNING);
            }
        });

        for d in 1..=n {
            self.today.set(d);
            self.used.borrow_mut().clear();
            self.morning().await;
            self.work().await;
            self.evening().await;
            self.night().await;
        }
        sunrise.abort();
        self.log("⏰ alarm. To be continued.");
        self.journal.into_inner()
    }

    // ---------------------------------------------------------------- the day

    async fn morning(&self) {
        let plan = self.plan();
        self.log(if self.today.get() == 1 { "⏰ rise and shine" } else { "⏰ alarm: the earth spun once" });
        if self.idea.get() {
            self.log("💡 woke up with an idea");
        }
        self.log("🍳 breakfast");
        self.spend(Activity::Eat, self.now() + 0.5).await;
        if let Some(departs) = plan.flight {
            self.spend(Activity::Live, self.abs(departs)).await;
            self.log("✈️  wheels up");
            self.spend(Activity::Flight, self.abs(9.0)).await;
        }
    }

    async fn work(&self) {
        let plan = self.plan();
        if plan.kind != DayKind::Workday {
            return;
        }
        self.spend(Activity::Live, self.abs(9.0)).await;
        self.log("💼 work");
        self.spend(Activity::Work, self.abs(12.0)).await;
        self.log("🥪 lunch");
        self.spend(Activity::Eat, self.abs(13.0)).await;
        self.spend(Activity::Work, self.abs(plan.deadline().unwrap_or(17.0))).await;
        if plan.deadline().is_some() {
            self.log("🚀 shipped it");
        }
    }

    /// If not geeking out: try being with people, catch surfing: finally living, eating.
    async fn evening(&self) {
        let plan = self.plan();
        let close = self.abs(if plan.kind == DayKind::Workday { 20.0 } else { 19.0 });
        while self.now() < close {
            let idle = self.now();
            let pick = select! {
                biased;
                _ = self.geek_ready(close) => Pick::Geek,
                (i, _) = self.arrives(self.company()) => Pick::People(i),
                (i, _) = self.arrives(self.waves()) => Pick::Surf(i),
                _ = rt::sleep_until(close) => Pick::Rest,
            };
            self.mark(Activity::Live, idle, self.now());
            match pick {
                Pick::Geek => self.geek().await,
                Pick::People(i) => self.people(i).await,
                Pick::Surf(i) => self.ride(i).await,
                Pick::Rest => break,
            }
        }
        let (now, bed) = (self.now(), self.abs(plan.bedtime));
        self.log("🍝 dinner");
        join!(self.spend(Activity::Live, bed), self.spend(Activity::Eat, now + 1.0));
        if let Some(until) = plan.late_night() {
            self.log("🌙 one more commit...");
            self.spend(Activity::Geek, self.abs(until)).await;
        }
    }

    /// `ALARM.store(false, Relaxed); while !ALARM.load(Acquire) { dream(); spin_loop(); }`
    async fn night(&self) {
        let plan = self.plan();
        // Things that go bump in the night ring the same alarm as the sunrise.
        let bumps: Vec<rt::JoinHandle> = plan
            .events
            .iter()
            .filter_map(|e| match *e {
                Event::Party { at, .. } if at >= plan.bedtime => Some((self.abs(at), Why::PARTY)),
                Event::Swell { at, big: true } if at >= plan.bedtime => Some((self.abs(at), Why::SWELL)),
                _ => None,
            })
            .map(|(at, why)| {
                let alarm = self.alarm.clone();
                rt::spawn(async move {
                    rt::sleep_until(at).await;
                    alarm.ring(why);
                })
            })
            .collect();
        let wired = self.caffeine();
        self.dreamt.set(0.0);
        self.log("😴 lights out");

        self.alarm.reset();
        loop {
            while !self.alarm.rung() {
                self.dream(wired).await;
                spin_loop();
            }
            let why = self.alarm.take();
            if why.has(Why::PARTY) {
                self.night_owl().await;
            }
            if why.has(Why::SWELL) {
                self.dawn_patrol().await;
            }
            if why.has(Why::MORNING) {
                break;
            }
        }

        bumps.iter().for_each(rt::JoinHandle::abort);
        if Self::cycles(self.dreamt.get()) >= IDEA_AFTER {
            self.idea.set(true);
            self.tally(|t| t.idea = true);
        }
    }

    // ------------------------------------------------------------ activities

    async fn geek(&self) {
        if !self.idea.replace(false) {
            if let Some((i, _)) = self.idea_strikes(f64::INFINITY) {
                self.used.borrow_mut().insert(i);
            }
        }
        self.log("🧠 geeking out");
        let end = self.now() + SESSION;
        select! {
            biased;
            _ = self.spend(Activity::Geek, end) => {},
            (i, _) = self.arrives(self.big_wave(end)) => {
                self.log("🌊 BIG swell: dropping the geek future mid-.await (Drop guard saved the hours)");
                self.ride(i).await;
            },
        }
    }

    async fn people(&self, i: usize) {
        self.used.borrow_mut().insert(i);
        let (until, note) = match self.plan().events[i] {
            Event::Family { until, .. } => (until, "👨‍👩‍👧 family time"),
            Event::Party { until, .. } => (until, "🎉 party"),
            _ => return,
        };
        self.log(note);
        self.spend(Activity::People, self.abs(until)).await;
    }

    async fn ride(&self, i: usize) {
        self.used.borrow_mut().insert(i);
        self.log("🏄 catching waves");
        self.spend(Activity::Surf, self.now() + SESSION).await;
    }

    async fn night_owl(&self) {
        let bed = self.plan().bedtime;
        let next = self.next(|e| match *e {
            Event::Party { at, until } if at >= bed && until > at => Some(at),
            _ => None,
        });
        let Some((i, _)) = next else { return };
        self.tally(|t| t.wakeups += 1);
        self.log("🎉 party next door, you're up");
        self.people(i).await;
        self.log("😴 back to bed");
    }

    async fn dawn_patrol(&self) {
        let bed = self.plan().bedtime;
        let next = self.next(|e| match *e {
            Event::Swell { at, big: true } if at >= bed => Some(at),
            _ => None,
        });
        let Some((i, _)) = next else { return };
        self.tally(|t| t.wakeups += 1);
        self.log("🌊 big swell before dawn: dawn patrol");
        self.ride(i).await;
    }

    /// One slice of the night: dream, or lie there spinning if you had coffee.
    /// The alarm's bell cuts the slice short, so you wake exactly on time.
    async fn dream(&self, wired_until: f64) {
        let t0 = self.now();
        let wired = t0 < wired_until;
        let stop = if wired { (t0 + TICK).min(wired_until) } else { t0 + TICK };
        select! { biased; _ = self.alarm.bell.heard() => {}, _ = rt::sleep_until(stop) => {} }
        let t1 = self.now();
        if wired {
            return self.mark(Activity::Wired, t0, t1);
        }
        self.mark(Activity::Sleep, t0, t1);
        let before = self.dreamt.get();
        self.dreamt.set(before + (t1 - t0));
        let new = Self::cycles(self.dreamt.get()) - Self::cycles(before);
        if new > 0 {
            self.tally(|t| t.dreams += new);
            self.log("💭 dreaming");
        }
    }

    /// Run an activity until `until`. Cancel-safe (see [`Doing`]). The barista
    /// serves any coffee due along the way, except at the dinner table.
    async fn spend(&self, what: Activity, until: f64) {
        let _doing = Doing { life: self, what, since: self.now() };
        if what != Activity::Eat {
            self.barista(until).await;
        }
        rt::sleep_until(until).await;
    }

    async fn barista(&self, until: f64) {
        while let Some((i, at)) = self
            .next(|e| match *e {
                Event::Coffee { at, .. } => Some(at),
                _ => None,
            })
            .filter(|&(_, at)| at < until)
        {
            self.used.borrow_mut().insert(i);
            rt::sleep_until(at).await;
            let Event::Coffee { shots, .. } = self.plan().events[i] else { continue };
            self.tally(|t| t.coffee += u32::from(shots));
            self.log(if shots > 1 { format!("☕ coffee x{shots}") } else { "☕ coffee".into() });
        }
    }

    // -------------------------------------------------------------- triggers

    async fn geek_ready(&self, close: f64) {
        if !self.idea.get() {
            self.arrives(self.idea_strikes(close)).await;
        }
    }

    /// Resolves when the planned event arrives; never, if there isn't one.
    async fn arrives(&self, next: Option<(usize, f64)>) -> (usize, f64) {
        match next {
            Some(next) => {
                rt::sleep_until(next.1).await;
                next
            }
            None => pending().await,
        }
    }

    fn idea_strikes(&self, before: f64) -> Option<(usize, f64)> {
        self.next(|e| match *e {
            Event::Idea { at } => Some(at),
            _ => None,
        })
        .filter(|&(_, at)| at < before)
    }

    fn company(&self) -> Option<(usize, f64)> {
        let (now, bed) = (self.now(), self.plan().bedtime);
        self.next(|e| match *e {
            Event::Family { at, until } | Event::Party { at, until } if at < bed && self.abs(until) > now => {
                Some(at)
            }
            _ => None,
        })
    }

    fn waves(&self) -> Option<(usize, f64)> {
        let (now, bed) = (self.now(), self.plan().bedtime);
        self.next(|e| match *e {
            Event::Swell { at, .. } if at < bed && self.abs(at + SWELL_LASTS) > now => Some(at),
            _ => None,
        })
    }

    fn big_wave(&self, before: f64) -> Option<(usize, f64)> {
        let (now, bed) = (self.now(), self.plan().bedtime);
        self.next(|e| match *e {
            Event::Swell { at, big: true } if at < bed && self.abs(at + SWELL_LASTS) > now => Some(at),
            _ => None,
        })
        .filter(|&(_, at)| at < before)
    }

    // ---------------------------------------------------------------- plumbing

    fn plan(&self) -> &Day {
        &self.days[self.today.get() - 1]
    }

    fn now(&self) -> f64 {
        rt::now()
    }

    /// Today's hour `h` as an absolute simulated hour.
    fn abs(&self, h: f64) -> f64 {
        (self.today.get() - 1) as f64 * EARTH_SPIN + h
    }

    /// The earliest unused event `pick` cares about, as `(index, absolute time)`.
    fn next(&self, pick: impl Fn(&Event) -> Option<f64>) -> Option<(usize, f64)> {
        let used = self.used.borrow();
        self.plan()
            .events
            .iter()
            .enumerate()
            .filter(|(i, _)| !used.contains(i))
            .filter_map(|(i, e)| pick(e).map(|h| (i, self.abs(h))))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    fn caffeine(&self) -> f64 {
        self.plan()
            .events
            .iter()
            .filter_map(|e| match *e {
                Event::Coffee { at, shots } if at >= 12.0 => {
                    Some(self.abs(at + CAFFEINE + 1.5 * f64::from(shots.saturating_sub(1))))
                }
                _ => None,
            })
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn cycles(hours: f64) -> u32 {
        (hours / REM_CYCLE + 1e-9).floor() as u32
    }

    fn mark(&self, what: Activity, start: f64, end: f64) {
        self.journal.borrow_mut().mark(self.today.get(), what, start, end);
    }

    fn tally(&self, f: impl FnOnce(&mut Tally)) {
        f(&mut self.journal.borrow_mut().tallies[self.today.get() - 1]);
    }

    fn log(&self, note: impl Into<String>) {
        let (at, note) = (self.now(), note.into());
        if self.echo {
            println!("{}  {note}", hhmm(at));
        }
        self.journal.borrow_mut().moments.push(Moment { at, note });
    }
}
