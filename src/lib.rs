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
    fmt::Write as _,
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
        let _ = write!(out, ":{sec:02}");
    }
    if frac > 0 {
        out += format!(".{frac:03}").trim_end_matches('0');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{hhmm, rt::Mode, Life, Opportunity, Rules};

    const AGENT: &str = include_str!("../presets/agent.life.toml");

    fn windows(life: &Life) -> Vec<f64> {
        life.opps.borrow().iter().filter(|o| o.scheduled).map(|o| o.at).collect()
    }

    #[test]
    fn windows_open_a_day_ahead_and_go_once_spent_or_over() {
        // agent's only window is jobs, daily 17:00/18:00; `steady` is 7 days.
        let rules = Rules::parse(AGENT).unwrap();
        let life = Life::new(&rules, rules.scenarios.iter().find(|s| s.name == "steady").unwrap());
        assert!(windows(&life).is_empty(), "the day loop opens them, not new()");
        life.open_windows(2);
        assert_eq!(windows(&life), [17.0, 41.0]);
        life.open_windows(2);
        assert_eq!(windows(&life), [17.0, 41.0], "a day's windows open once");
        life.open_windows(99);
        assert_eq!(windows(&life), [17.0, 41.0, 65.0, 89.0, 113.0, 137.0, 161.0], "never past the last day");

        // Day 4's is spent; days 1 and 2 are over at 42:00 (day 2's ends exactly then).
        life.opps.borrow_mut().iter_mut().find(|o| o.at == 89.0).unwrap().used = true;
        // An unused urgent event that has expired stays: it can still wake you tonight.
        let page = Opportunity {
            pursuit: 0,
            at: 30.0,
            ends: None,
            expires: 31.0,
            urgent: true,
            note: None,
            rested: false,
            scheduled: false,
            used: false,
        };
        life.opps.borrow_mut().push(page);
        life.prune(42.0);
        assert_eq!(windows(&life), [65.0, 113.0, 137.0, 161.0]);
        assert_eq!(life.opps.borrow().len(), 5, "the expired page is kept");
    }

    #[test]
    fn tomorrows_window_can_preempt_tonights_page() {
        // The 02:00 page on day 2 happens during day 1's night; a 02:30 window on day 2 that
        // preempts it has to be open by then.
        let src = AGENT
            .replace(r#"all = ["users", "jobs", "research"]"#, r#"all = ["oncall", "users", "jobs", "research"]"#)
            .replace(
                "[pursuit.jobs]",
                "[pursuit.oncall]\nnote = \"📟 on call\"\nwhen = \"daily 02:30/03:00\"\npreempts = [\"users\"]\n\n[pursuit.jobs]",
            );
        let rules = Rules::parse(&src).unwrap();
        let incident = rules.scenarios.iter().find(|s| s.name == "incident").unwrap();
        let journal = Life::new(&rules, incident).run(Mode::Virtual);
        assert!(journal.said("oncall preempts users"), "{:?}", journal.moments);
    }

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
    /// Where the diary goes as it happens, if anywhere; dropped if writing to it fails.
    echo: Echo,
    alarm: Arc<Alarm>,
    journal: RefCell<Journal>,
    today: Cell<usize>,
    /// Open chances to do something. Each day's `when` windows are added a day ahead and
    /// dropped once they're used or over, so a long run doesn't search all of them (#81).
    opps: RefCell<Vec<Opportunity>>,
    /// Days whose windows are in `opps`.
    windows_through: Cell<usize>,
    /// `[scenario.*] start`, the weekday of day 1.
    start: usize,
    /// `(day, pursuit)` pairs whose windows a scenario skips.
    skips: Vec<(usize, usize)>,
    coffee: RefCell<Vec<(usize, f64, u32, bool)>>,
    dreamt: Cell<f64>,
    /// Only `[from, until)` is reported (absolute hours); see [`Life::window`].
    from: Option<f64>,
    until: Option<f64>,
}

/// The diary's live output.
// Send, so Life stays Send. AssertUnwindSafe: a writer left mid-line by a panic only loses
// echo output, and keeps Life UnwindSafe as it was before echo_to.
struct Echo(RefCell<Option<std::panic::AssertUnwindSafe<Box<dyn std::io::Write + Send>>>>);

impl std::fmt::Debug for Echo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.borrow().is_some() { "Echo(on)" } else { "Echo(off)" })
    }
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

/// Day `day`'s hour `hour` in absolute hours, on the clock's millisecond. `now()` is always a
/// whole millisecond, so a time compared with it has to be one too: unrounded, day 2's 02:13
/// is a hair past the millisecond its alarm rings on, and the page that rang it was missed.
fn abs(day: usize, hour: f64) -> f64 {
    iso::round_ms((day - 1) as f64 * EARTH_SPIN + hour)
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
                    let expires = ends
                        .or_else(|| p.lasts.map(|l| at + l))
                        .unwrap_or_else(|| abs(day, plan.bed).max(at + p.session));
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
                        let expires = p.lasts.map_or_else(|| abs(day, plan.bed), |l| at + l);
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
                    plan.up_late = Some((*until, *pursuit, note.clone()));
                }
                Event::Skip { pursuit, .. } => skips.push((day, *pursuit)),
            }
        }
        let pursuits = rules.pursuits.iter().map(|p| (p.name.clone(), p.glyph)).collect();
        Life {
            rules,
            days: n,
            plans,
            echo: Echo(RefCell::new(None)),
            alarm: Arc::default(),
            journal: RefCell::new(Journal::new(n, pursuits, rules.sleep.signal.clone())),
            today: Cell::new(1),
            opps: RefCell::new(opps),
            windows_through: Cell::new(0),
            start: scenario.start,
            skips,
            coffee: RefCell::new(coffee),
            dreamt: Cell::new(0.0),
            from: scenario.from,
            until: scenario.until,
        }
    }

    /// Add the `when` windows of every day up to `through` (at most the last day) that
    /// aren't in yet. Nothing searches further ahead than the next morning's alarm, so the
    /// windows of today and tomorrow are all that's needed.
    fn open_windows(&self, through: usize) {
        let mut opps = self.opps.borrow_mut();
        for day in self.windows_through.get() + 1..=through.min(self.days) {
            let weekday = (self.start + day - 1) % 7;
            for (i, p) in self.rules.pursuits.iter().enumerate() {
                if self.skips.contains(&(day, i)) {
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
            self.windows_through.set(day);
        }
    }

    /// Forget what can't be taken any more: anything used, and windows that are over (only
    /// urgent events wake you, so a window over by `now` can never be picked). Order is kept,
    /// so ties still go to the same one. Call it only while no opportunity index is held.
    fn prune(&self, now: f64) {
        self.opps.borrow_mut().retain(|o| !o.used && !(o.scheduled && o.expires <= now));
    }

    /// Print the diary to stdout as it happens.
    pub fn echo(self, on: bool) -> Self {
        if on {
            self.echo_to(std::io::stdout())
        } else {
            *self.echo.0.borrow_mut() = None;
            self
        }
    }

    /// Write the diary to `out` as it happens, one line per entry. If a write fails (a closed
    /// pipe, a full disk), the echo stops; the journal still has everything.
    pub fn echo_to(self, out: impl std::io::Write + Send + 'static) -> Self {
        *self.echo.0.borrow_mut() = Some(std::panic::AssertUnwindSafe(Box::new(out)));
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
        let end = self.ends_at();
        let mut journal = self.journal.into_inner();
        journal.end_at(end);
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
            // Between days no opportunity index is held, so the list can shrink here.
            self.prune(self.now());
            self.open_windows(d + 1);
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
        // Up until the alarm or past it (`up_late` into the morning): it rang while you were
        // still up, and the reset just forgot it. Without this, you'd sleep through it forever.
        if lights_out >= next_wake {
            self.alarm.ring(Why::MORNING);
        }
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
            // The interrupter is due before this ends (q.at < end), so it wins the race and
            // `spend` is dropped mid-.await; its Drop guard keeps the hours. Polled first, it also
            // wins a tie on the same millisecond.
            select! {
                biased;
                () = rt::sleep_until(at) => {},
                () = self.spend(Activity::Pursuit(p), end) => {},
            };
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
        select! { biased; () = self.alarm.bell.heard() => {}, () = rt::sleep_until(stop) => {} }
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
    /// Rounded to the clock's millisecond: `dream` compares it with `now()`, which is always
    /// a whole millisecond. Unrounded (23:10 + 3h is no exact f64), it sat a hair past the
    /// millisecond the clock stops on, so the night never got there and never ended.
    fn caffeine(&self) -> f64 {
        let c = &self.rules.caffeine;
        let today = self.today.get();
        let wired = self
            .coffee
            .borrow()
            .iter()
            .filter(|cup| cup.0 == today && cup.1 >= abs(today, c.cutoff))
            .map(|cup| cup.1 + c.wired + c.per_extra_shot * f64::from(cup.2.saturating_sub(1)))
            .fold(f64::NEG_INFINITY, f64::max);
        iso::round_ms(wired)
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
        if self.in_window(at) {
            let mut echo = self.echo.0.borrow_mut();
            // A failing writer (`spin-life | head`) stops the echo; it is not worth a panic.
            let failed = echo.as_mut().is_some_and(|out| writeln!(out, "{}  {note}", hhmm(at)).is_err());
            if failed {
                *echo = None;
            }
        }
        self.journal.borrow_mut().moments.push(Moment { at, note });
    }
}
