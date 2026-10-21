//! The rules of a life, read from a `.life.toml` file.
//!
//! Nothing here knows about geeking, family or surfing. Those are *pursuits* that one
//! particular file defines. The engine only knows:
//!
//! * **priorities**: the `select! { biased; ... }` order, per kind of day
//! * **windows** (`when`): times a pursuit is available, like family dinner
//! * **signals** (`on`): things that start a pursuit, like an idea after a good night's sleep
//! * **preempts**: who may interrupt whom, mid-`.await`
//! * **protected**: who may never be interrupted
//! * **woken_by**: what may ring your alarm at night
//! * **caffeine**: when coffee turns sleep into a spin loop
//!
//! See `presets/` for complete files, starting with `scott.life.toml`.

use crate::toml::{self, Table, Value};
use std::fmt;

/// Names the engine reserves for its own activities.
pub const BUILTIN: [&str; 6] = ["sleep", "wired", "eat", "work", "travel", "living"];

const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/// Built-in presets: `(name, about, file)`.
pub const PRESETS: [(&str, &str, &str); 3] = [
    ("scott", "the motto: geek, then people, then surf", include_str!("../presets/scott.life.toml")),
    (
        "parent",
        "kids first, guitar when inspired, running on weekends",
        include_str!("../presets/parent.life.toml"),
    ),
    (
        "agent",
        "an AI agent: users preempt everything, nightly memory compaction",
        include_str!("../presets/agent.life.toml"),
    ),
];

/// Something wrong with a life file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleError(pub String);

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for RuleError {}

impl From<toml::Error> for RuleError {
    fn from(e: toml::Error) -> Self {
        RuleError(e.to_string())
    }
}

type Result<T> = std::result::Result<T, RuleError>;

fn fail<T>(msg: impl Into<String>) -> Result<T> {
    Err(RuleError(msg.into()))
}

/// Which days of the week (Monday = 0).
pub type Days = [bool; 7];

/// When a pursuit is available, every week.
#[derive(Clone, Debug, PartialEq)]
pub struct Window {
    pub days: Days,
    pub start: f64,
    pub end: f64,
}

/// Something you do by choice. Its name is yours.
#[derive(Clone, Debug, PartialEq)]
pub struct Pursuit {
    pub name: String,
    /// One character on the timeline.
    pub glyph: char,
    /// What the diary says when it starts.
    pub note: String,
    /// The signal that starts it, like `"idea"`.
    pub on: Option<String>,
    /// How long one session runs, for pursuits that are not windows.
    pub session: f64,
    /// How long an opportunity stays open (a swell lasts 3 hours). `None`: until bedtime.
    pub lasts: Option<f64>,
    /// Nothing may interrupt it.
    pub protected: bool,
    /// Indexes of pursuits this one interrupts, with an urgent event or when one of its `when` windows opens.
    pub preempts: Vec<usize>,
    pub windows: Vec<Window>,
}

/// Sleep, dreams, and what can wake you.
#[derive(Clone, Debug, PartialEq)]
pub struct Sleep {
    pub wake: f64,
    pub bed: f64,
    pub weekend_wake: f64,
    pub weekend_bed: f64,
    /// Length of one REM cycle.
    pub rem_cycle: f64,
    /// Cycles in one night that produce `signal`. `None`: dreams are just dreams.
    pub signal_after: Option<u32>,
    /// What a good night produces, matched against pursuits' `on`.
    pub signal: String,
    /// What the diary says when you wake up with it.
    pub signal_note: String,
    /// Pursuits whose urgent events ring the alarm at night.
    pub woken_by: Vec<usize>,
}

/// The job, if there is one.
#[derive(Clone, Debug, PartialEq)]
pub struct Work {
    pub days: Days,
    pub start: f64,
    pub end: f64,
    pub lunch: Option<(f64, f64)>,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Meals {
    pub breakfast: f64,
    /// Free time ends and dinner starts (workdays).
    pub dinner: f64,
    pub weekend_dinner: f64,
    pub dinner_lasts: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Caffeine {
    /// Coffee at or after this hour keeps you wired at bedtime.
    pub cutoff: f64,
    pub wired: f64,
    pub per_extra_shot: f64,
}

/// Something that happens once, in a scenario. Days are 1-based.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// An opportunity for a pursuit: a window (`until`) or a session.
    Pursue {
        day: usize,
        pursuit: usize,
        at: f64,
        until: Option<f64>,
        urgent: bool,
        note: Option<String>,
    },
    /// A signal that starts whichever pursuit listens `on` it.
    Signal {
        day: usize,
        at: f64,
        signal: String,
    },
    Coffee {
        day: usize,
        at: f64,
        shots: u32,
    },
    /// Work runs late.
    WorkUntil {
        day: usize,
        until: f64,
        note: String,
    },
    /// An early flight: the alarm rings an hour before departure.
    Flight {
        day: usize,
        departs: f64,
        lands: f64,
    },
    /// Stay up doing a pursuit after dinner.
    UpLate {
        day: usize,
        until: f64,
        pursuit: usize,
        note: Option<String>,
    },
    /// No recurring windows for this pursuit today.
    Skip {
        day: usize,
        pursuit: usize,
    },
}

impl Event {
    pub fn day(&self) -> usize {
        match *self {
            Event::Pursue { day, .. }
            | Event::Signal { day, .. }
            | Event::Coffee { day, .. }
            | Event::WorkUntil { day, .. }
            | Event::Flight { day, .. }
            | Event::UpLate { day, .. }
            | Event::Skip { day, .. } => day,
        }
    }
}

/// A named sequence of days to live.
#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub name: String,
    pub about: String,
    /// Day of the week day 1 falls on (Monday = 0).
    pub start: usize,
    pub days: usize,
    pub events: Vec<Event>,
}

impl Scenario {
    /// `days` ordinary days starting on a Monday.
    pub fn plain(days: usize) -> Self {
        Scenario {
            name: "plain".into(),
            about: format!("{days} ordinary day{}", if days == 1 { "" } else { "s" }),
            start: 0,
            days,
            events: vec![],
        }
    }
}

/// Everything a life file says.
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub name: String,
    pub motto: Option<String>,
    pub sleep: Sleep,
    pub work: Option<Work>,
    pub meals: Meals,
    pub caffeine: Caffeine,
    pub pursuits: Vec<Pursuit>,
    /// The `select!` order on workdays and on days off.
    pub workday: Vec<usize>,
    pub weekend: Vec<usize>,
    pub scenarios: Vec<Scenario>,
}

impl Rules {
    /// Parse and validate a life file.
    pub fn parse(src: &str) -> Result<Self> {
        let mut root = Fields::new("", toml::parse(src)?);
        let name = root.str("name")?.unwrap_or_else(|| "me".into());
        let motto = root.str("motto")?;

        // Pursuits first: everything else refers to them by name.
        let mut specs = Vec::new();
        if let Some(table) = root.table("pursuit")? {
            for (pname, value) in table {
                if BUILTIN.contains(&pname.as_str()) {
                    return fail(format!("[pursuit.{pname}]: `{pname}` is built in; pick another name"));
                }
                let Value::Table(t) = value else {
                    return fail(format!("`pursuit.{pname}` must be a table"));
                };
                specs.push((pname.clone(), Fields::new(&format!("pursuit.{pname}"), t)));
            }
        }
        if specs.is_empty() {
            return fail("a life needs at least one [pursuit.<name>]");
        }
        // Report pursuits in the order you rank them (workday first), not alphabetically.
        // Anything unranked follows, alphabetically. Peek only: [priorities] is validated below.
        let ranked: Vec<String> = match root.t.get("priorities") {
            Some(Value::Table(p)) => ["workday", "all", "weekend"]
                .iter()
                .filter_map(|k| match p.get(*k) {
                    Some(Value::Array(items)) => Some(items.clone()),
                    Some(v @ Value::Str(_)) => Some(vec![v.clone()]),
                    _ => None,
                })
                .flatten()
                .filter_map(|v| match v {
                    Value::Str(s) => Some(s),
                    _ => None,
                })
                .collect(),
            _ => vec![],
        };
        specs.sort_by_key(|(name, _)| ranked.iter().position(|r| r == name).unwrap_or(usize::MAX));
        let names: Vec<String> = specs.iter().map(|(n, _)| n.clone()).collect();
        let index = |name: &str, ctx: &str| -> Result<usize> {
            names.iter().position(|n| n == name).ok_or_else(|| {
                RuleError(format!("{ctx}: no pursuit named `{name}` (have: {})", names.join(", ")))
            })
        };

        let mut work = None;
        if let Some(t) = root.table("work")? {
            let mut w = Fields::new("work", t);
            let days = match w.raw("days") {
                Some(v) => days(&v, None, "work.days")?,
                None => [true, true, true, true, true, false, false],
            };
            let (start, end) =
                window(&w.str("hours")?.unwrap_or_else(|| "09:00-17:00".into()), "work.hours")?;
            let lunch = match w.str("lunch")? {
                Some(s) if s == "none" => None,
                Some(s) => Some(window(&s, "work.lunch")?),
                None => Some((12.0, 13.0)),
            };
            let note = w.str("note")?.unwrap_or_else(|| "💼 work".into());
            w.done()?;
            work = Some(Work { days, start, end, lunch, note });
        }
        let workdays = work.as_ref().map(|w| w.days);

        let mut pursuits = Vec::new();
        let mut preempt_names = Vec::new();
        for (i, (pname, mut f)) in specs.into_iter().enumerate() {
            let ctx = format!("pursuit.{pname}");
            let glyph = match f.str("glyph")? {
                Some(g) if g.chars().count() == 1 => g.chars().next().unwrap(),
                Some(g) => return fail(format!("{ctx}.glyph: `{g}` must be one character")),
                None => pname.chars().next().unwrap().to_ascii_uppercase(),
            };
            let note = f.str("note")?.unwrap_or_else(|| format!("▶ {pname}"));
            let on = f.str("on")?;
            let session = dur_or(&mut f, "session", 2.0)?;
            let lasts = f.str("lasts")?.map(|s| duration(&s, &format!("{ctx}.lasts"))).transpose()?;
            let protected = f.bool("protected")?.unwrap_or(false);
            preempt_names.push((i, f.strs("preempts")?.unwrap_or_default()));
            let windows = match f.raw("when") {
                None => vec![],
                Some(Value::Str(s)) => vec![when(&s, workdays, &format!("{ctx}.when"))?],
                Some(Value::Array(items)) => items
                    .iter()
                    .map(|v| match v {
                        Value::Str(s) => when(s, workdays, &format!("{ctx}.when")),
                        _ => fail(format!("{ctx}.when: expected strings like \"workdays 18:00-20:00\"")),
                    })
                    .collect::<Result<_>>()?,
                Some(v) => {
                    return fail(format!("{ctx}.when: expected a string or array, found {}", v.kind()))
                }
            };
            f.done()?;
            pursuits.push(Pursuit {
                name: pname,
                glyph,
                note,
                on,
                session,
                lasts,
                protected,
                preempts: vec![],
                windows,
            });
        }
        for (i, list) in preempt_names {
            let ctx = format!("pursuit.{}.preempts", pursuits[i].name);
            pursuits[i].preempts = list.iter().map(|n| index(n, &ctx)).collect::<Result<_>>()?;
        }
        if let Some((a, b)) = pursuits.iter().enumerate().find_map(|(i, p)| {
            pursuits[i + 1..].iter().find(|q| q.glyph == p.glyph).map(|q| (&p.name, &q.name))
        }) {
            return fail(format!("pursuits `{a}` and `{b}` share a glyph; set `glyph` on one of them"));
        }

        let order = |list: Vec<String>, ctx: &str| -> Result<Vec<usize>> {
            let order: Vec<usize> = list.iter().map(|n| index(n, ctx)).collect::<Result<_>>()?;
            if let Some(dup) = order.iter().enumerate().find(|(i, p)| order[..*i].contains(p)) {
                return fail(format!("{ctx}: `{}` is listed twice", pursuits[*dup.1].name));
            }
            Ok(order)
        };
        let (workday, weekend) = match root.table("priorities")? {
            Some(t) => {
                let mut p = Fields::new("priorities", t);
                let all = p.strs("all")?;
                let wd = p.strs("workday")?.or_else(|| all.clone());
                let we = p.strs("weekend")?.or_else(|| wd.clone());
                p.done()?;
                let Some(wd) = wd else { return fail("[priorities] needs `workday` (or `all`)") };
                (order(wd, "priorities.workday")?, order(we.unwrap_or_default(), "priorities.weekend")?)
            }
            None => {
                let all: Vec<usize> = (0..pursuits.len()).collect();
                (all.clone(), all)
            }
        };

        let mut s = Fields::new("sleep", root.table("sleep")?.unwrap_or_default());
        let wake = time_or(&mut s, "wake", 6.5)?;
        let mut bed = time_or(&mut s, "bed", 22.5)?;
        let weekend_wake = time_or(&mut s, "weekend_wake", wake)?;
        let mut weekend_bed = time_or(&mut s, "weekend_bed", bed)?;
        for b in [&mut bed, &mut weekend_bed] {
            if *b <= wake.max(weekend_wake) {
                *b += 24.0; // "02:00" means after midnight
            }
        }
        let rem_cycle = dur_or(&mut s, "rem_cycle", 1.5)?;
        if rem_cycle <= 0.0 {
            return fail("sleep.rem_cycle must be longer than 0");
        }
        let signal_after = s.int("signal_after")?.map(|n| n.max(0) as u32);
        let signal = s.str("signal")?.unwrap_or_else(|| "idea".into());
        let signal_note = s.str("signal_note")?.unwrap_or_else(|| format!("💡 woke up with: {signal}"));
        let woken_by = s
            .strs("woken_by")?
            .unwrap_or_default()
            .iter()
            .map(|n| index(n, "sleep.woken_by"))
            .collect::<Result<_>>()?;
        s.done()?;
        let sleep = Sleep {
            wake,
            bed,
            weekend_wake,
            weekend_bed,
            rem_cycle,
            signal_after,
            signal,
            signal_note,
            woken_by,
        };

        let mut m = Fields::new("meals", root.table("meals")?.unwrap_or_default());
        let breakfast = dur_or(&mut m, "breakfast", 0.5)?;
        let dinner = time_or(&mut m, "dinner", 20.0)?;
        let weekend_dinner = time_or(&mut m, "weekend_dinner", dinner)?;
        let dinner_lasts = dur_or(&mut m, "dinner_lasts", 1.0)?;
        m.done()?;
        let meals = Meals { breakfast, dinner, weekend_dinner, dinner_lasts };

        let mut c = Fields::new("caffeine", root.table("caffeine")?.unwrap_or_default());
        let caffeine = Caffeine {
            cutoff: time_or(&mut c, "cutoff", 12.0)?,
            wired: dur_or(&mut c, "wired", 6.0)?,
            per_extra_shot: dur_or(&mut c, "per_extra_shot", 1.5)?,
        };
        c.done()?;

        let mut scenarios = Vec::new();
        if let Some(table) = root.table("scenario")? {
            for (sname, value) in table {
                let ctx = format!("scenario.{sname}");
                let Value::Table(t) = value else { return fail(format!("`{ctx}` must be a table")) };
                let mut f = Fields::new(&ctx, t);
                let about = f.str("about")?.unwrap_or_default();
                let start = match f.str("start")? {
                    Some(d) => {
                        weekday(&d).ok_or_else(|| RuleError(format!("{ctx}.start: `{d}` is not mon..sun")))?
                    }
                    None => 0,
                };
                let days = f.int("days")?.unwrap_or(1);
                if !(1..=366).contains(&days) {
                    return fail(format!("{ctx}.days must be 1 to 366"));
                }
                let mut events = Vec::new();
                if let Some(v) = f.raw("events") {
                    let Value::Array(items) = v else {
                        return fail(format!("{ctx}.events must be an array"));
                    };
                    for (k, item) in items.into_iter().enumerate() {
                        let Value::Table(t) = item else {
                            return fail(format!(
                                "{ctx}.events[{k}] must be a table like {{ day = 1, ... }}"
                            ));
                        };
                        events.push(event(
                            Fields::new(&format!("{ctx}.events[{k}]"), t),
                            days as usize,
                            &index,
                        )?);
                    }
                }
                f.done()?;
                scenarios.push(Scenario { name: sname, about, start, days: days as usize, events });
            }
        }
        root.done()?;
        Ok(Rules { name, motto, sleep, work, meals, caffeine, pursuits, workday, weekend, scenarios })
    }

    /// One of the built-in presets.
    pub fn preset(name: &str) -> Option<Self> {
        let (_, _, src) = PRESETS.iter().find(|p| p.0 == name)?;
        Some(Self::parse(src).expect("built-in presets are valid"))
    }

    /// The source text of a built-in preset.
    pub fn preset_source(name: &str) -> Option<&'static str> {
        PRESETS.iter().find(|p| p.0 == name).map(|p| p.2)
    }

    pub fn scenario(&self, name: &str) -> Option<&Scenario> {
        self.scenarios.iter().find(|s| s.name == name)
    }

    pub fn pursuit(&self, name: &str) -> Option<usize> {
        self.pursuits.iter().position(|p| p.name == name)
    }

    /// Is this day of the week (Monday = 0) a workday?
    pub fn works(&self, weekday: usize) -> bool {
        self.work.as_ref().is_some_and(|w| w.days[weekday % 7])
    }
}

fn event(mut f: Fields, days: usize, index: &dyn Fn(&str, &str) -> Result<usize>) -> Result<Event> {
    let ctx = f.ctx.clone();
    let day = f.int("day")?.ok_or_else(|| RuleError(format!("{ctx}: every event needs a `day`")))?;
    if day < 1 || day as usize > days + 1 {
        return fail(format!("{ctx}.day: {day} is outside this {days}-day scenario"));
    }
    let day = day as usize;
    let at = f.str("at")?.map(|s| time(&s, &format!("{ctx}.at"))).transpose()?;
    let need_at = || at.ok_or_else(|| RuleError(format!("{ctx}: this event needs `at = \"HH:MM\"`")));
    let note = f.str("note")?;
    let ev = if let Some(p) = f.str("pursuit")? {
        let at = need_at()?;
        let until = f.str("until")?.map(|s| time(&s, &format!("{ctx}.until"))).transpose()?;
        let until = until.map(|u| if u <= at { u + 24.0 } else { u });
        let urgent = f.bool("urgent")?.unwrap_or(false);
        Event::Pursue { day, pursuit: index(&p, &ctx)?, at, until, urgent, note }
    } else if let Some(signal) = f.str("signal")? {
        Event::Signal { day, at: need_at()?, signal }
    } else if let Some(shots) = f.int("coffee")? {
        Event::Coffee { day, at: need_at()?, shots: shots.clamp(1, 20) as u32 }
    } else if let Some(u) = f.str("work_until")? {
        Event::WorkUntil {
            day,
            until: time(&u, &format!("{ctx}.work_until"))?,
            note: note.unwrap_or_else(|| "🚀 shipped it".into()),
        }
    } else if let Some(d) = f.str("flight")? {
        let lands = f.str("lands")?.unwrap_or_else(|| "09:00".into());
        Event::Flight {
            day,
            departs: time(&d, &format!("{ctx}.flight"))?,
            lands: time(&lands, &format!("{ctx}.lands"))?,
        }
    } else if let Some(u) = f.str("up_late")? {
        let doing = f
            .str("doing")?
            .ok_or_else(|| RuleError(format!("{ctx}: `up_late` needs `doing = \"<pursuit>\"`")))?;
        let mut until = time(&u, &format!("{ctx}.up_late"))?;
        if until < 12.0 {
            until += 24.0;
        }
        Event::UpLate { day, until, pursuit: index(&doing, &ctx)?, note }
    } else if let Some(p) = f.str("skip")? {
        Event::Skip { day, pursuit: index(&p, &ctx)? }
    } else {
        return fail(format!(
            "{ctx}: needs one of pursuit, signal, coffee, work_until, flight, up_late, skip"
        ));
    };
    f.done()?;
    Ok(ev)
}

// ------------------------------------------------------------------ parsing helpers

/// A table that remembers which keys were read, so typos are errors, not silence.
struct Fields {
    ctx: String,
    t: Table,
}

impl Fields {
    fn new(ctx: &str, t: Table) -> Self {
        Fields { ctx: ctx.to_string(), t }
    }

    fn key(&self, k: &str) -> String {
        if self.ctx.is_empty() {
            k.to_string()
        } else {
            format!("{}.{k}", self.ctx)
        }
    }

    fn raw(&mut self, k: &str) -> Option<Value> {
        self.t.remove(k)
    }

    fn str(&mut self, k: &str) -> Result<Option<String>> {
        match self.raw(k) {
            None => Ok(None),
            Some(Value::Str(s)) => Ok(Some(s)),
            Some(v) => fail(format!("{} should be a string, found {}", self.key(k), v.kind())),
        }
    }

    fn int(&mut self, k: &str) -> Result<Option<i64>> {
        match self.raw(k) {
            None => Ok(None),
            Some(Value::Int(n)) => Ok(Some(n)),
            Some(v) => fail(format!("{} should be a whole number, found {}", self.key(k), v.kind())),
        }
    }

    fn bool(&mut self, k: &str) -> Result<Option<bool>> {
        match self.raw(k) {
            None => Ok(None),
            Some(Value::Bool(b)) => Ok(Some(b)),
            Some(v) => fail(format!("{} should be true or false, found {}", self.key(k), v.kind())),
        }
    }

    fn strs(&mut self, k: &str) -> Result<Option<Vec<String>>> {
        match self.raw(k) {
            None => Ok(None),
            Some(Value::Str(s)) => Ok(Some(vec![s])),
            Some(Value::Array(items)) => items
                .into_iter()
                .map(|v| match v {
                    Value::Str(s) => Ok(s),
                    v => fail(format!("{} should list names, found {}", self.key(k), v.kind())),
                })
                .collect::<Result<_>>()
                .map(Some),
            Some(v) => fail(format!("{} should be a list of names, found {}", self.key(k), v.kind())),
        }
    }

    fn table(&mut self, k: &str) -> Result<Option<Table>> {
        match self.raw(k) {
            None => Ok(None),
            Some(Value::Table(t)) => Ok(Some(t)),
            Some(v) => fail(format!("{} should be a table, found {}", self.key(k), v.kind())),
        }
    }

    /// Anything left over is a typo.
    fn done(self) -> Result<()> {
        match self.t.keys().next() {
            None => Ok(()),
            Some(k) => fail(format!("unknown key `{}`", self.key(k))),
        }
    }
}

fn time_or(f: &mut Fields, k: &str, default: f64) -> Result<f64> {
    let key = f.key(k);
    f.str(k)?.map_or(Ok(default), |s| time(&s, &key))
}

fn dur_or(f: &mut Fields, k: &str, default: f64) -> Result<f64> {
    let key = f.key(k);
    f.str(k)?.map_or(Ok(default), |s| duration(&s, &key))
}

/// `"06:30"` → 6.5. Hours up to 47 are allowed for "after midnight".
pub fn time(s: &str, ctx: &str) -> Result<f64> {
    let parsed = s.trim().split_once(':').and_then(|(h, m)| {
        let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
        (h < 48 && m < 60).then_some(f64::from(h) + f64::from(m) / 60.0)
    });
    parsed.ok_or_else(|| RuleError(format!("{ctx}: `{s}` is not a time like \"06:30\"")))
}

/// `"1h30m"`, `"2h"`, `"45m"` → hours.
pub fn duration(s: &str, ctx: &str) -> Result<f64> {
    let bad = || RuleError(format!("{ctx}: `{s}` is not a duration like \"1h30m\""));
    let (mut total, mut num, mut any) = (0.0, String::new(), false);
    for c in s.trim().chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'h' | 'm' => {
                let n: f64 = num.parse().map_err(|_| bad())?;
                total += if c == 'h' { n } else { n / 60.0 };
                num.clear();
                any = true;
            }
            _ => return Err(bad()),
        }
    }
    if !any || !num.is_empty() {
        return Err(bad());
    }
    Ok(total)
}

/// `"18:00-20:00"`; an end before the start wraps past midnight.
pub fn window(s: &str, ctx: &str) -> Result<(f64, f64)> {
    let Some((a, b)) = s.split_once('-') else {
        return fail(format!("{ctx}: `{s}` is not a window like \"18:00-20:00\""));
    };
    let (start, mut end) = (time(a, ctx)?, time(b, ctx)?);
    if end <= start {
        end += 24.0;
    }
    Ok((start, end))
}

fn weekday(s: &str) -> Option<usize> {
    let s = s.trim().to_ascii_lowercase();
    WEEKDAYS.iter().position(|d| s.starts_with(d))
}

/// `"workdays"`, `"weekends"`, `"daily"`, `"mon,wed,fri"`, `"mon-fri"`, or an array of those.
fn days(v: &Value, workdays: Option<Days>, ctx: &str) -> Result<Days> {
    let words: Vec<String> = match v {
        Value::Str(s) => s.split(',').map(str::to_string).collect(),
        Value::Array(items) => items
            .iter()
            .map(|i| match i {
                Value::Str(s) => Ok(s.clone()),
                _ => fail(format!("{ctx}: expected day names")),
            })
            .collect::<Result<_>>()?,
        _ => return fail(format!("{ctx}: expected day names like \"mon-fri\"")),
    };
    let mut out = [false; 7];
    for w in words {
        let w = w.trim().to_ascii_lowercase();
        match (w.as_str(), workdays) {
            ("daily" | "every day" | "all", _) => out = [true; 7],
            ("workdays", Some(wd)) => (0..7).for_each(|d| out[d] |= wd[d]),
            ("weekends" | "days off", Some(wd)) => (0..7).for_each(|d| out[d] |= !wd[d]),
            ("workdays" | "weekends" | "days off", None) => {
                return fail(format!("{ctx}: name the days, like \"mon-fri\""))
            }
            (range, _) => match range.split_once('-') {
                Some((a, b)) => match (weekday(a), weekday(b)) {
                    (Some(a), Some(b)) => {
                        (0..7).filter(|d| (d + 7 - a) % 7 <= (b + 7 - a) % 7).for_each(|d| out[d] = true)
                    }
                    _ => return fail(format!("{ctx}: `{range}` is not a day range like \"mon-fri\"")),
                },
                None => match weekday(range) {
                    Some(d) => out[d] = true,
                    None => return fail(format!("{ctx}: `{range}` is not a day")),
                },
            },
        }
    }
    Ok(out)
}

/// `"workdays 18:00-20:00"`, `"sat,sun 07:00-08:30"`, or just `"18:00-20:00"` (daily).
fn when(s: &str, workdays: Option<Days>, ctx: &str) -> Result<Window> {
    let s = s.trim();
    let (d, w) = s.rsplit_once(' ').unwrap_or(("daily", s));
    if !w.contains('-') {
        return fail(format!(
            "{ctx}: `{s}` is not a window like \"18:00-20:00\" or \"workdays 18:00-20:00\""
        ));
    }
    let d = days(&Value::Str(d.to_string()), workdays, ctx).map_err(|e| match workdays {
        None if d.contains("work") || d.contains("weekend") => RuleError(format!(
            "{ctx}: `{d}` needs a [work] table to know your workdays; or name the days, like \"mon-fri\""
        )),
        _ => e,
    })?;
    let (start, end) = window(w, ctx)?;
    Ok(Window { days: d, start, end })
}
