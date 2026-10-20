//! What a day is supposed to look like, before it actually happens.
//!
//! All hours are relative to that day's midnight. Hours past 24 spill into the night
//! (25.5 is 1:30 the next morning) and still belong to this day's plan.

/// Something that happens (or might) during a day.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// Coffee. After noon, it keeps you spinning instead of dreaming.
    Coffee { at: f64, shots: u8 },
    /// Family time (people).
    Family { at: f64, until: f64 },
    /// A party or special event (people). After bedtime, it rings the alarm.
    Party { at: f64, until: f64 },
    /// Waves (surf). A big swell preempts geeking, and after bedtime it rings the alarm.
    Swell { at: f64, big: bool },
    /// An idea strikes (geek).
    Idea { at: f64 },
    /// "One more commit": geek after dinner until this hour.
    LateNightCoding { until: f64 },
    /// Work runs until this hour instead of 17:00.
    Deadline { at: f64 },
}

/// Workdays have work. Weekends have more of everything else.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayKind {
    Workday,
    Weekend,
}

/// One spin of the earth, planned.
#[derive(Clone, Debug, PartialEq)]
pub struct Day {
    pub kind: DayKind,
    /// When this morning's alarm rings.
    pub wake: f64,
    /// Lights out.
    pub bedtime: f64,
    /// Departure time of an early work flight, which lands at 09:00.
    pub flight: Option<f64>,
    pub events: Vec<Event>,
}

impl Day {
    /// Wake 06:30, work 9 to 5, family 18:00 to 20:00, bed 22:30.
    pub fn workday() -> Self {
        Day {
            kind: DayKind::Workday,
            wake: 6.5,
            bedtime: 22.5,
            flight: None,
            events: vec![Event::Family { at: 18.0, until: 20.0 }],
        }
    }

    /// Wake 08:00, family 12:00 to 15:00, bed 23:00.
    pub fn weekend() -> Self {
        Day {
            kind: DayKind::Weekend,
            wake: 8.0,
            bedtime: 23.0,
            flight: None,
            events: vec![Event::Family { at: 12.0, until: 15.0 }],
        }
    }

    /// Add an event.
    pub fn with(mut self, event: Event) -> Self {
        self.events.push(event);
        self
    }

    /// Drop every family event (a work trip, say).
    pub fn without_family(mut self) -> Self {
        self.events.retain(|e| !matches!(e, Event::Family { .. }));
        self
    }

    /// An early work flight: the alarm rings an hour before departure and you land at 09:00.
    pub fn early_flight(mut self, departs: f64) -> Self {
        self.wake = departs - 1.0;
        self.flight = Some(departs);
        self
    }

    /// When work ends if there is a deadline.
    pub fn deadline(&self) -> Option<f64> {
        self.events.iter().find_map(|e| match *e {
            Event::Deadline { at } => Some(at),
            _ => None,
        })
    }

    /// When the late-night coding session ends, if there is one.
    pub fn late_night(&self) -> Option<f64> {
        self.events.iter().find_map(|e| match *e {
            Event::LateNightCoding { until } => Some(until),
            _ => None,
        })
    }
}

/// A named sequence of days.
#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub name: &'static str,
    pub about: &'static str,
    pub days: Vec<Day>,
}

impl Scenario {
    /// Every built-in scenario.
    pub fn all() -> Vec<Scenario> {
        use Event::*;
        let wd = Day::workday;
        let s = |name, about, days| Scenario { name, about, days };
        vec![
            s("happy-path", "wake, eat, work, lunch, family, eat, sleep. Repeat.", vec![wd(), wd(), wd()]),
            s(
                "week",
                "five workdays, a surf Saturday, an idea on Sunday",
                vec![
                    wd(),
                    wd(),
                    wd(),
                    wd(),
                    wd(),
                    Day::weekend().with(Swell { at: 9.0, big: false }),
                    Day::weekend().with(Idea { at: 16.0 }),
                ],
            ),
            s(
                "coffee",
                "morning cup, 3pm cup, then a triple espresso at 20:00",
                vec![
                    wd().with(Coffee { at: 7.0, shots: 1 }),
                    wd().with(Coffee { at: 15.0, shots: 1 }),
                    wd().with(Coffee { at: 20.0, shots: 3 }),
                    wd(),
                ],
            ),
            s(
                "party",
                "a party next door at 00:30 on a work night",
                vec![wd().with(Party { at: 24.5, until: 26.5 }), wd()],
            ),
            s(
                "special-event",
                "a friend's launch party instead of family dinner",
                vec![wd().without_family().with(Party { at: 18.5, until: 22.0 }), wd()],
            ),
            s(
                "dawn-patrol",
                "a big swell at 05:30 rings the alarm early",
                vec![wd().with(Swell { at: 29.5, big: true }), wd()],
            ),
            s(
                "big-wave",
                "an idea at 17:00, then a big swell at 18:00 breaks into it",
                vec![wd().with(Idea { at: 17.0 }).with(Swell { at: 18.0, big: true })],
            ),
            s(
                "late-night",
                "one more commit... until 02:00",
                vec![wd().with(LateNightCoding { until: 26.0 }), wd()],
            ),
            s(
                "early-flight",
                "a 05:30 work flight the next morning",
                vec![wd(), wd().early_flight(5.5).without_family(), wd()],
            ),
            s(
                "deadline",
                "shipping by 21:00, family dinner skipped",
                vec![wd().with(Deadline { at: 21.0 }), wd()],
            ),
            s(
                "chaos",
                "a week where everything happens",
                vec![
                    wd().with(Coffee { at: 7.0, shots: 1 }),
                    wd().with(Deadline { at: 21.0 }).with(Coffee { at: 16.0, shots: 2 }),
                    wd().with(LateNightCoding { until: 25.5 }),
                    wd().early_flight(5.5).without_family().with(Party { at: 25.0, until: 26.0 }),
                    wd().with(Idea { at: 17.0 }).with(Swell { at: 18.5, big: true }),
                    Day::weekend().with(Swell { at: 31.0, big: true }),
                    Day::weekend().with(Idea { at: 9.0 }).with(Swell { at: 16.0, big: false }),
                ],
            ),
        ]
    }

    /// Look one up by name.
    pub fn by_name(name: &str) -> Option<Scenario> {
        Self::all().into_iter().find(|s| s.name == name)
    }
}
