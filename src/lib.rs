//! # spin-life
//!
//! ```text
//! loop { select! { biased; _ = geek() => {}, _ = people() => {}, _ = surf() => {} } join!(live(), eat()); ALARM.store(false, Relaxed); while !ALARM.load(Acquire) { dream(); spin_loop(); } }
//! ```
//!
//! That line is one person's rules. This crate runs anyone's: write the pursuits, the
//! priorities, who may interrupt whom and what may wake you in a `.life.toml` file
//! ([`Rules`]), pick a [`Scenario`], and read the [`Journal`]. It all runs on a
//! zero-dependency async runtime ([`rt`]) whose clock can skip ahead (virtual time) or
//! run hours as seconds.
//!
//! ```
//! use spin_life::{rt::Mode, Life, Rules};
//!
//! let rules = Rules::preset("scott").unwrap();
//! let journal = Life::new(&rules, rules.scenario("happy-path").unwrap()).run(Mode::Virtual);
//! assert_eq!(journal.day(1).hours("work"), 7.0);
//! assert!(journal.day(1).tally.signal); // 8 hours of sleep, 5 REM cycles, 1 idea
//! assert_eq!(journal.day(2).hours("geek"), 2.0); // which comes first the next evening
//! ```

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

mod alarm;
pub mod iso;
mod journal;
pub mod json;
mod macros;
pub mod rt;
pub mod rules;
pub mod toml;

pub use alarm::{Alarm, Why};
pub use journal::{Activity, DaySummary, Journal, Moment, Span, Tally};
pub use rules::{Event, Pursuit, RuleError, Rules, Scenario, PRESETS};

use std::{
    cell::{Cell, RefCell},
    future::pending,
    hint::spin_loop,
    sync::Arc,
};

/// One spin of the earth, in hours.
pub const EARTH_SPIN: f64 = 24.0;
/// The granularity of one `dream()`: the alarm can still cut it short.
const TICK: f64 = 0.25;

/// `"d3 07:30"` for an absolute simulated hour, with seconds only when there are some:
/// `"d1 06:30:15"`, `"d1 06:30:02.5"` (to the millisecond).
pub fn hhmm(abs: f64) -> String {
    let ms = (abs * 3_600_000.0).round().max(0.0) as u64;
    let (day, ms) = (ms / 86_400_000 + 1, ms % 86_400_000);
    let (h, m, sec, frac) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    let mut out = format!("d{day} {h:02}:{m:02}");
    if sec > 0 || frac > 0 {
        out += &format!(":{sec:02}");
    }
    if frac > 0 {
        out += format!(".{frac:03}").trim_end_matches('0');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::hhmm;

    #[test]
    fn hhmm_shows_seconds_only_when_there_are_some() {
        assert_eq!(hhmm(6.5), "d1 06:30");
        assert_eq!(hhmm(6.5 + 15.0 / 3600.0), "d1 06:30:15");
        assert_eq!(hhmm(6.5 + 2.5 / 3600.0), "d1 06:30:02.5");
        assert_eq!(hhmm(6.5 + 0.125 / 3600.0), "d1 06:30:00.125");
        assert_eq!(hhmm(24.0), "d2 00:00");
        assert_eq!(hhmm(30.5), "d2 06:30");
    }
}

/// A chance to do a pursuit, in absolute hours.
#[derive(Clone, Debug)]
struct Opportunity {
    pursuit: usize,
    at: f64,
    /// A window ends at a fixed time; otherwise it is one session long.
    ends: Option<f64>,
    /// Gone after this (a swell flattens out, family dinner ends).
    expires: f64,
    urgent: bool,
    note: Option<String>,
    /// Produced by a good night's sleep.
    rested: bool,
    /// A recurring `when` window. Like urgent events, these can preempt.
    scheduled: bool,
    used: bool,
}

/// One day, after the scenario's events have been applied to the rules. Hours are local.
#[derive(Clone, Debug)]
struct Plan {
    works: bool,
    wake: f64,
    bed: f64,
    dinner: f64,
    work_until: Option<(f64, String)>,
    flight: Option<(f64, f64)>,
    up_late: Option<(f64, usize, Option<String>)>,
}

/// Somebody living a scenario by a set of rules.
#[derive(Debug)]
pub struct Life<'r> {
    rules: &'r Rules,
    days: usize,
    plans: Vec<Plan>,
    echo: bool,
    alarm: Arc<Alarm>,
    journal: RefCell<Journal>,
    today: Cell<usize>,
    opps: RefCell<Vec<Opportunity>>,
    coffee: RefCell<Vec<(usize, f64, u32, bool)>>,
    dreamt: Cell<f64>,
    /// Only `[from, until)` is reported (absolute hours); see [`Life::window`].
    from: Option<f64>,
    until: Option<f64>,
}

/// Records how long an activity ran when it ends, *or when it is cancelled*.
/// That makes [`Life::spend`] cancel-safe: an urgent pursuit can drop another one
/// mid-`.await` and the hours already put in still land in the journal.
struct Doing<'a, 'r> {
    life: &'a Life<'r>,
    what: Activity,
    since: f64,
}

impl Drop for Doing<'_, '_> {
    fn drop(&mut self) {
        self.life.mark(self.what, self.since, self.life.now());
    }
}

fn abs(day: usize, hour: f64) -> f64 {
    (day - 1) as f64 * EARTH_SPIN + hour
}

impl<'r> Life<'r> {
    pub fn new(rules: &'r Rules, scenario: &Scenario) -> Self {
        let n = scenario.days;
        let mut plans: Vec<Plan> = (0..=n)
            .map(|d| {
                let works = rules.works(scenario.start + d);
                let s = &rules.sleep;
                Plan {
                    works,
                    wake: if works { s.wake } else { s.weekend_wake },
                    bed: if works { s.bed } else { s.weekend_bed },
                    dinner: if works { rules.meals.dinner } else { rules.meals.weekend_dinner },
                    work_until: None,
                    flight: None,
                    up_late: None,
                }
            })
            .collect();

        let mut opps = Vec::new();
        let mut coffee = Vec::new();
        let mut skips = Vec::new();
        for ev in &scenario.events {
            let day = ev.day();
            let plan = &mut plans[day - 1];
            match ev {
                Event::Pursue { pursuit, at, until, urgent, note, .. } => {
                    let p = &rules.pursuits[*pursuit];
                    let at = abs(day, *at);
                    let ends = until.map(|u| abs(day, u));
                    let expires =
                        ends.or(p.lasts.map(|l| at + l)).unwrap_or(abs(day, plan.bed).max(at + p.session));
                    opps.push(Opportunity {
                        pursuit: *pursuit,
                        at,
                        ends,
                        expires,
                        urgent: *urgent,
                        note: note.clone(),
                        rested: false,
                        scheduled: false,
                        used: false,
                    });
                }
                Event::Signal { at, signal, .. } => {
                    for (i, p) in
                        rules.pursuits.iter().enumerate().filter(|(_, p)| p.on.as_ref() == Some(signal))
                    {
                        let at = abs(day, *at);
                        let expires = p.lasts.map_or(abs(day, plan.bed), |l| at + l);
                        opps.push(Opportunity {
                            pursuit: i,
                            at,
                            ends: None,
                            expires,
                            urgent: false,
                            note: None,
                            rested: false,
                            scheduled: false,
                            used: false,
                        });
                    }
                }
                Event::Coffee { at, shots, .. } => coffee.push((day, abs(day, *at), *shots, false)),
                Event::WorkUntil { until, note, .. } => plan.work_until = Some((*until, note.clone())),
                Event::Flight { departs, lands, .. } => {
                    plan.wake = departs - 1.0;
                    plan.flight = Some((*departs, *lands));
                }
                Event::UpLate { until, pursuit, note, .. } => {
                    plan.up_late = Some((*until, *pursuit, note.clone()))
                }
                Event::Skip { pursuit, .. } => skips.push((day, *pursuit)),
            }
        }
        for day in 1..=n {
            let weekday = (scenario.start + day - 1) % 7;
            for (i, p) in rules.pursuits.iter().enumerate() {
                if skips.contains(&(day, i)) {
                    continue;
                }
                for w in p.windows.iter().filter(|w| w.days[weekday]) {
                    let (at, end) = (abs(day, w.start), abs(day, w.end));
                    opps.push(Opportunity {
                        pursuit: i,
                        at,
                        ends: Some(end),
                        expires: end,
                        urgent: false,
                        note: None,
                        rested: false,
                        scheduled: true,
                        used: false,
                    });
                }
            }
        }

        let pursuits = rules.pursuits.iter().map(|p| (p.name.clone(), p.glyph)).collect();
        Life {
            rules,
            days: n,
            plans,
            echo: false,
            alarm: Arc::default(),
            journal: RefCell::new(Journal::new(n, pursuits, rules.sleep.signal.clone())),
            today: Cell::new(1),
            opps: RefCell::new(opps),
            coffee: RefCell::new(coffee),
            dreamt: Cell::new(0.0),
            from: scenario.from,
            until: scenario.until,
        }
    }

    /// Print the diary as it happens.
    pub fn echo(mut self, on: bool) -> Self {
        self.echo = on;
        self
    }

    /// When the run starts: the first morning's alarm, in absolute hours.
    pub fn starts_at(&self) -> f64 {
        self.plans[0].wake
    }

    /// When the run ends: the alarm the morning after the last day, in absolute hours.
    pub fn ends_at(&self) -> f64 {
        abs(self.days + 1, self.plans[self.days].wake)
    }

    /// Report only `[from, until)`, in absolute hours since midnight of day 1 (`None`: the
    /// start or end of the run). Before `from` the run skips ahead in virtual time, even in
    /// [`rt::Mode::RealTime`]; it stops at `until`. Overrides the scenario's own window.
    ///
    /// # Panics
    /// If the window is empty or reaches outside the run.
    pub fn window(mut self, from: Option<f64>, until: Option<f64>) -> Self {
        let (start, end) = (self.starts_at(), self.ends_at());
        let (f, u) = (from.unwrap_or(start), until.unwrap_or(end));
        assert!(f >= start - 1e-9, "the window starts before the run does ({})", hhmm(start));
        assert!(u <= end + 1e-9, "the window ends after the run does ({})", hhmm(end));
        assert!(u > f, "the window must end after it starts");
        self.from = from;
        self.until = until;
        self
    }

    /// Live every day of the scenario, or the window of it. The clock starts at the first
    /// morning's alarm.
    pub fn run(self, mode: rt::Mode) -> Journal {
        let start = self.starts_at();
        let from = self.from.unwrap_or(start);
        let until = self.until;
        rt::block_on_live_from(mode, start, from, async {
            match until {
                None => self.live().await,
                Some(until) => {
                    let stop = async move { rt::sleep_until(until).await };
                    let _ = rt::select_biased(vec![Box::pin(self.live()), Box::pin(stop)]).await;
                }
            }
        });
        let windowed = self.from.is_some() || self.until.is_some();
        let mut journal = self.journal.into_inner();
        if windowed {
            journal.clip(from, until.unwrap_or(f64::INFINITY));
        }
        journal
    }

    /// Is `at` inside the window?
    fn in_window(&self, at: f64) -> bool {
        self.from.map_or(true, |f| at >= f) && self.until.map_or(true, |u| at < u)
    }

    async fn live(&self) {
        // Sunrise: one ring per spin of the earth, at each next morning's wake time.
        let wakes: Vec<f64> = (1..=self.days).map(|d| abs(d + 1, self.plans[d].wake)).collect();
        let alarm = self.alarm.clone();
        let sunrise = rt::spawn(async move {
            for at in wakes {
                rt::sleep_until(at).await;
                alarm.ring(Why::MORNING);
            }
        });
        for d in 1..=self.days {
            self.today.set(d);
            self.morning().await;
            self.work().await;
            self.evening().await;
            self.night().await;
        }
        sunrise.abort();
        self.log("⏰ alarm. To be continued.");
    }

    // ---------------------------------------------------------------- the day

    async fn morning(&self) {
        self.log(if self.today.get() == 1 { "⏰ rise and shine" } else { "⏰ alarm: the earth spun once" });
        if self.opps.borrow().iter().any(|o| o.rested && !o.used) {
            self.log(self.rules.sleep.signal_note.clone());
        }
        self.log("🍳 breakfast");
        self.spend(Activity::Eat, self.now() + self.rules.meals.breakfast).await;
        if let Some((departs, lands)) = self.plan().flight {
            self.spend(Activity::Live, self.at(departs)).await;
            self.log("✈️  wheels up");
            self.spend(Activity::Travel, self.at(lands)).await;
        }
    }

    async fn work(&self) {
        let (plan, Some(w)) = (self.plan(), self.rules.work.as_ref()) else { return };
        if !plan.works {
            return;
        }
        let end = plan.work_until.as_ref().map_or(w.end, |x| x.0);
        self.spend(Activity::Live, self.at(w.start)).await;
        self.log(w.note.clone());
        if let Some((lunch, back)) = w.lunch.filter(|l| l.0 < end) {
            self.spend(Activity::Work, self.at(lunch)).await;
            self.log("🥪 lunch");
            self.spend(Activity::Eat, self.at(back)).await;
        }
        self.spend(Activity::Work, self.at(end)).await;
        if let Some((_, note)) = &plan.work_until {
            self.log(note.clone());
        }
    }

    /// `loop { select! { biased; <your priorities> } }` until dinner, then `join!(live(), eat())`.
    async fn evening(&self) {
        let plan = self.plan();
        let (close, horizon) = (self.at(plan.dinner), self.at(plan.bed));
        let order = if plan.works { &self.rules.workday } else { &self.rules.weekend };
        while self.now() < close {
            let idle = self.now();
            let mut arms: Vec<rt::BoxFuture<'_, Option<usize>>> =
                order.iter().map(|&p| Box::pin(self.ready(p, horizon)) as rt::BoxFuture<'_, _>).collect();
            arms.push(Box::pin(async move {
                rt::sleep_until(close).await;
                None
            }));
            let (_, pick) = rt::select_biased(arms).await;
            self.mark(Activity::Live, idle, self.now());
            match pick {
                Some(o) => self.pursue(o, horizon).await,
                None => break,
            }
        }
        let (now, bed) = (self.now(), self.at(plan.bed));
        self.log("🍝 dinner");
        join!(
            self.spend(Activity::Live, bed),
            self.spend(Activity::Eat, now + self.rules.meals.dinner_lasts)
        );
        if let Some((until, p, note)) = &plan.up_late {
            self.log(note.clone().unwrap_or_else(|| format!("🌙 up late: {}", self.rules.pursuits[*p].name)));
            self.spend(Activity::Pursuit(*p), self.at(*until)).await;
        }
    }

    /// `ALARM.store(false, Relaxed); while !ALARM.load(Acquire) { dream(); spin_loop(); }`
    async fn night(&self) {
        let lights_out = self.now();
        let next_wake = abs(self.today.get() + 1, self.plans[self.today.get()].wake);
        let woken_by = &self.rules.sleep.woken_by;
        let disturbs = move |o: &Opportunity| {
            !o.used && o.urgent && woken_by.contains(&o.pursuit) && o.at >= lights_out && o.at < next_wake
        };
        // Whatever your rules say may wake you rings the same alarm as the sunrise.
        let ringers: Vec<rt::JoinHandle> = self
            .opps
            .borrow()
            .iter()
            .filter(|o| disturbs(o))
            .map(|o| {
                let (alarm, at) = (self.alarm.clone(), o.at);
                rt::spawn(async move {
                    rt::sleep_until(at).await;
                    alarm.ring(Why::URGENT);
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
            if why.has(Why::URGENT) {
                let now = self.now();
                while let Some(o) = self.next(|o| disturbs(o) && o.at <= now) {
                    self.tally(|t| t.wakeups += 1);
                    self.pursue(o, next_wake).await;
                }
                if !self.alarm.rung() {
                    self.log("😴 back to bed");
                }
            }
            if why.has(Why::MORNING) {
                break;
            }
        }
        ringers.iter().for_each(rt::JoinHandle::abort);

        let s = &self.rules.sleep;
        if s.signal_after.is_some_and(|n| Self::cycles(self.dreamt.get(), s.rem_cycle) >= n) {
            self.tally(|t| t.signal = true);
            let now = self.now();
            let mut opps = self.opps.borrow_mut();
            for (i, _) in
                self.rules.pursuits.iter().enumerate().filter(|(_, p)| p.on.as_ref() == Some(&s.signal))
            {
                if !opps.iter().any(|o| o.rested && !o.used && o.pursuit == i) {
                    opps.push(Opportunity {
                        pursuit: i,
                        at: now,
                        ends: None,
                        expires: f64::INFINITY,
                        urgent: false,
                        note: None,
                        rested: true,
                        scheduled: false,
                        used: false,
                    });
                }
            }
        }
    }

    // ------------------------------------------------------------ activities

    /// Resolves when pursuit `p` has an opportunity; never, if it has none before `horizon`.
    async fn ready(&self, p: usize, horizon: f64) -> Option<usize> {
        let now = self.now();
        match self.next(|o| !o.used && o.pursuit == p && o.at < horizon && o.expires > now) {
            Some(i) => {
                let at = self.opps.borrow()[i].at;
                rt::sleep_until(at.max(now)).await;
                Some(i)
            }
            None => pending().await,
        }
    }

    /// Take an opportunity. Unless the pursuit is protected, an urgent event or a scheduled
    /// window of a pursuit that `preempts` this one drops it mid-`.await` and takes over.
    async fn pursue(&self, mut o: usize, horizon: f64) {
        loop {
            let now = self.now();
            let (p, end, note) = {
                let mut opps = self.opps.borrow_mut();
                let op = &mut opps[o];
                op.used = true;
                let pursuit = &self.rules.pursuits[op.pursuit];
                let end = op.ends.unwrap_or(now + pursuit.session);
                (op.pursuit, end, op.note.clone().unwrap_or_else(|| pursuit.note.clone()))
            };
            self.log(note);
            let me = &self.rules.pursuits[p];
            let rules = self.rules;
            let interrupter = (!me.protected)
                .then(|| {
                    self.next(|q| {
                        !q.used
                            && (q.urgent || q.scheduled)
                            && rules.pursuits[q.pursuit].preempts.contains(&p)
                            && q.at < end
                            && q.at < horizon
                            && q.expires > now
                    })
                })
                .flatten();
            let Some(q) = interrupter else {
                self.spend(Activity::Pursuit(p), end).await;
                return;
            };
            let at = self.opps.borrow()[q].at;
            let interrupted = select! {
                biased;
                _ = self.spend(Activity::Pursuit(p), end) => false,
                _ = rt::sleep_until(at) => true,
            };
            if !interrupted {
                return;
            }
            let them = &self.rules.pursuits[self.opps.borrow()[q].pursuit].name;
            self.log(format!(
                "⚡ {them} preempts {}: dropped its future mid-.await (the Drop guard kept the hours)",
                me.name
            ));
            o = q;
        }
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
        let cycle = self.rules.sleep.rem_cycle;
        let before = self.dreamt.get();
        self.dreamt.set(before + (t1 - t0));
        let new = Self::cycles(self.dreamt.get(), cycle) - Self::cycles(before, cycle);
        if new > 0 {
            self.tally(|t| t.dreams += new);
            self.log("💭 dreaming");
        }
    }

    /// Run an activity until `until`. Cancel-safe (see [`Doing`]). Any coffee due along
    /// the way gets served, except at the table.
    async fn spend(&self, what: Activity, until: f64) {
        let _doing = Doing { life: self, what, since: self.now() };
        if what != Activity::Eat {
            self.barista(until).await;
        }
        rt::sleep_until(until).await;
    }

    async fn barista(&self, until: f64) {
        loop {
            let next = {
                let cups = self.coffee.borrow();
                cups.iter()
                    .enumerate()
                    .filter(|(_, c)| !c.3 && c.1 < until)
                    .min_by(|a, b| a.1 .1.total_cmp(&b.1 .1))
                    .map(|(i, c)| (i, c.1, c.2))
            };
            let Some((i, at, shots)) = next else { return };
            self.coffee.borrow_mut()[i].3 = true;
            rt::sleep_until(at).await;
            self.tally(|t| t.coffee += shots);
            self.log(if shots > 1 { format!("☕ coffee x{shots}") } else { "☕ coffee".into() });
        }
    }

    // ---------------------------------------------------------------- plumbing

    fn plan(&self) -> &Plan {
        &self.plans[self.today.get() - 1]
    }

    fn now(&self) -> f64 {
        rt::now()
    }

    /// Today's hour `h` as an absolute simulated hour.
    fn at(&self, h: f64) -> f64 {
        abs(self.today.get(), h)
    }

    /// The earliest opportunity matching `pick`.
    fn next(&self, pick: impl Fn(&Opportunity) -> bool) -> Option<usize> {
        let opps = self.opps.borrow();
        opps.iter()
            .enumerate()
            .filter(|(_, o)| pick(o))
            .min_by(|a, b| a.1.at.total_cmp(&b.1.at))
            .map(|(i, _)| i)
    }

    /// When today's afternoon coffee stops keeping you awake.
    fn caffeine(&self) -> f64 {
        let c = &self.rules.caffeine;
        let today = self.today.get();
        self.coffee
            .borrow()
            .iter()
            .filter(|cup| cup.0 == today && cup.1 >= abs(today, c.cutoff))
            .map(|cup| cup.1 + c.wired + c.per_extra_shot * f64::from(cup.2.saturating_sub(1)))
            .fold(f64::NEG_INFINITY, f64::max)
    }

    fn cycles(hours: f64, cycle: f64) -> u32 {
        (hours / cycle + 1e-9).floor() as u32
    }

    fn mark(&self, what: Activity, start: f64, end: f64) {
        self.journal.borrow_mut().mark(self.today.get(), what, start, end);
    }

    fn tally(&self, f: impl FnOnce(&mut Tally)) {
        if !self.in_window(self.now()) {
            return;
        }
        f(&mut self.journal.borrow_mut().tallies[self.today.get() - 1]);
    }

    fn log(&self, note: impl Into<String>) {
        let (at, note) = (self.now(), note.into());
        if self.echo && self.in_window(at) {
            println!("{}  {note}", hhmm(at));
        }
        self.journal.borrow_mut().moments.push(Moment { at, note });
    }
}
