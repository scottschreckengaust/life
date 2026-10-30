//! ISO 8601 times, durations and intervals: the only formats spin-life reads.
//!
//! Everything comes back as `f64` hours, the engine's unit, rounded to the millisecond.
//! Only the extended format (with `:` separators) is accepted.
//!
//! | kind | examples |
//! |---|---|
//! | time of day | `06:30`, `06:30:15`, `06:30:02.5`, `24:00` |
//! | duration | `PT30M`, `PT1H30M`, `PT2.5S`, `P1DT6H`, `P1W` |
//! | interval | `18:00/20:00`, `22:30/06:30` (crosses midnight), `09:00/PT8H` |

const MS_PER_HOUR: f64 = 3_600_000.0;

/// Round hours to the nearest millisecond.
pub fn round_ms(hours: f64) -> f64 {
    (hours * MS_PER_HOUR).round() / MS_PER_HOUR
}

/// A time of day, `hh:mm[:ss[.fff]]`, as hours after midnight. `24:00` is the end of the day.
pub fn time(s: &str) -> Result<f64, String> {
    let s = s.trim();
    let bad =
        |why: &str| format!("`{s}` is not an ISO 8601 time of day ({why}); use one like 06:30 or 06:30:02.5");
    if s.contains(['-', '/']) {
        return Err(bad("one time, not a window"));
    }
    let mut parts = s.split(':');
    let (Some(h), Some(m)) = (parts.next(), parts.next()) else {
        return Err(bad("needs hh:mm"));
    };
    let sec = parts.next();
    if parts.next().is_some() {
        return Err(bad("too many parts"));
    }
    let two = |p: &str| p.len() == 2 && p.bytes().all(|b| b.is_ascii_digit());
    if !two(h) || !two(m) {
        return Err(bad("hours and minutes need 2 digits each"));
    }
    let (h, m): (u32, u32) = (h.parse().unwrap(), m.parse().unwrap());
    let secs = match sec {
        None => 0.0,
        Some(p) => {
            let (whole, frac) = match p.split_once(['.', ',']) {
                Some((w, f)) => (w, Some(f)),
                None => (p, None),
            };
            if !two(whole) {
                return Err(bad("seconds need 2 digits"));
            }
            if let Some(f) = frac {
                if f.is_empty() || f.len() > 9 || !f.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(bad("the fraction needs 1 to 9 digits"));
                }
            }
            let v: f64 = format!("{whole}.{}", frac.unwrap_or("0")).parse().unwrap();
            if v >= 60.0 {
                return Err(bad("seconds go up to 59"));
            }
            v
        }
    };
    if m >= 60 {
        return Err(bad("minutes go up to 59"));
    }
    if h > 24 || (h == 24 && (m > 0 || secs > 0.0)) {
        return Err(bad("hours go up to 23, or 24:00 for midnight at the end of the day"));
    }
    Ok(round_ms(f64::from(h) + f64::from(m) / 60.0 + secs / 3600.0))
}

/// An ISO 8601 duration, `P[nD][T[nH][nM][nS]]` or `PnW`, as hours. Only the last component
/// may have a fraction.
pub fn duration(s: &str) -> Result<f64, String> {
    let s = s.trim();
    let hint = "use one like PT1H30M, PT2.5S or P1DT6H";
    let bad = |why: &str| format!("`{s}` is not an ISO 8601 duration ({why}); {hint}");
    let Some(rest) = s.strip_prefix('P') else {
        return Err(format!("`{s}` is not an ISO 8601 duration; {hint}"));
    };
    if rest.is_empty() || rest == "T" {
        return Err(bad("it has no components"));
    }
    let (mut total, mut num, mut in_time, mut any) = (0.0, String::new(), false, false);
    let mut fraction_seen = false;
    let mut last_order = 0; // W=1, D=2, H=3, M=4, S=5: components must come in this order
    for c in rest.chars() {
        match c {
            '0'..='9' => num.push(c),
            '.' | ',' => num.push('.'),
            'T' if !in_time && num.is_empty() => in_time = true,
            'T' => return Err(bad("T comes once, before the hours")),
            _ => {
                let (order, hours) = match (c, in_time) {
                    ('W', false) => (1, 168.0),
                    ('D', false) => (2, 24.0),
                    ('H', true) => (3, 1.0),
                    ('M', true) => (4, 1.0 / 60.0),
                    ('S', true) => (5, 1.0 / 3600.0),
                    ('Y' | 'M', false) => {
                        return Err(format!("`{s}`: years and months aren't fixed lengths; use days (P30D)"))
                    }
                    ('H' | 'S', false) => return Err(bad("hours, minutes and seconds go after T")),
                    _ => return Err(bad(&format!("unexpected `{c}`"))),
                };
                if order <= last_order {
                    return Err(bad("components go in the order W, D, T, H, M, S, once each"));
                }
                if fraction_seen {
                    return Err(bad("only the last component may have a fraction"));
                }
                if num.is_empty()
                    || num.starts_with('.')
                    || num.ends_with('.')
                    || num.matches('.').count() > 1
                {
                    return Err(bad(&format!("`{c}` needs a number before it")));
                }
                fraction_seen = num.contains('.');
                total += num.parse::<f64>().unwrap() * hours;
                last_order = order;
                num.clear();
                any = true;
            }
        }
    }
    if !num.is_empty() {
        return Err(bad("a number at the end needs a unit"));
    }
    // `any` is true here: a non-empty `rest` that isn't "T" either hit a unit or returned above.
    debug_assert!(any);
    if in_time && last_order < 3 {
        return Err(bad("T needs hours, minutes or seconds after it"));
    }
    if rest.contains('W') && rest.contains(['D', 'T']) {
        return Err(bad("weeks don't combine with other components"));
    }
    if !total.is_finite() {
        return Err(bad("too large"));
    }
    Ok(round_ms(total))
}

/// A point in a run, as absolute hours since midnight at the start of day 1: a time of day
/// (on day 1, `06:30:00`) or a duration from that midnight (`P1DT18H` is day 2 at 18:00).
pub fn when(s: &str) -> Result<f64, String> {
    if s.trim_start().starts_with('P') {
        duration(s)
    } else {
        time(s).map_err(|e| format!("{e}, or a duration from midnight of day 1 like P1DT18H"))
    }
}

/// An ISO 8601 interval of times of day, `start/end` or `start/duration`, as `(start, end)`
/// hours. An end at or before the start crosses midnight.
pub fn window(s: &str) -> Result<(f64, f64), String> {
    let s = s.trim();
    let Some((a, b)) = s.split_once('/') else {
        let hint = match s.split_once('-') {
            Some((a, b)) => format!("use an ISO 8601 interval like {}/{}", a.trim(), b.trim()),
            None => "use an ISO 8601 interval like 18:00/20:00 or 09:00/PT8H".into(),
        };
        return Err(format!("`{s}` is not an ISO 8601 interval; {hint}"));
    };
    let start = time(a)?;
    if start >= 24.0 {
        return Err(format!("`{s}`: 24:00 is the end of a day, so a window can't start there; use 00:00"));
    }
    let end = if b.trim_start().starts_with('P') {
        let d = duration(b)?;
        if d <= 0.0 {
            return Err(format!("`{s}`: the interval needs a positive duration"));
        }
        start + d
    } else {
        let mut end = time(b)?;
        if end <= start {
            end += 24.0;
        }
        end
    };
    Ok((start, round_ms(end)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: f64 = 1.0 / 3600.0;

    #[track_caller]
    fn close(got: f64, want: f64) {
        assert!((got - want).abs() < 1e-9, "got {got}, want {want}");
    }

    #[test]
    fn times() {
        close(time("06:30").unwrap(), 6.5);
        close(time("06:30:15").unwrap(), 6.5 + 15.0 * S);
        close(time("06:30:02.5").unwrap(), 6.5 + 2.5 * S);
        close(time("06:30:02,5").unwrap(), 6.5 + 2.5 * S);
        close(time("23:59:59.999").unwrap(), 24.0 - 0.001 * S);
        close(time("00:00:00.123456789").unwrap(), 0.123 * S); // up to 9 digits, rounded to the ms
        close(time("00:00").unwrap(), 0.0);
        close(time("24:00").unwrap(), 24.0);
        close(time("24:00:00").unwrap(), 24.0);
        // rounded to the millisecond
        close(time("00:00:00.0004").unwrap(), 0.0);
    }

    #[test]
    fn bad_times() {
        for s in [
            "6:30",
            "0630",
            "25:00",
            "24:01",
            "06:60",
            "06:30:60",
            "06:30:2",
            "06:30:02.",
            "aa:bb",
            "06:30:00:00",
            "06:30:02.1234567890",
            "06:30:02.5x",
            "24:00:01",
            "24:00:00.5",
        ] {
            let e = time(s).unwrap_err();
            assert!(e.contains("ISO 8601 time of day"), "{s}: {e}");
        }
        assert!(time("18:00-20:00").unwrap_err().contains("not a window"));
    }

    #[test]
    fn durations() {
        close(duration("PT30M").unwrap(), 0.5);
        close(duration("PT1H30M").unwrap(), 1.5);
        close(duration("PT2.5S").unwrap(), 2.5 * S);
        close(duration("PT0.25S").unwrap(), 0.25 * S);
        close(duration("PT0,5S").unwrap(), 0.5 * S);
        close(duration("P1DT6H").unwrap(), 30.0);
        close(duration("P1D").unwrap(), 24.0);
        close(duration("P1W").unwrap(), 168.0);
        close(duration("PT1.5H").unwrap(), 1.5);
        close(duration("PT0S").unwrap(), 0.0);
    }

    #[test]
    fn bad_durations() {
        assert!(duration("1h30m").unwrap_err().contains("PT1H30M"));
        assert!(duration("30m").unwrap_err().contains("PT1H30M"));
        assert!(duration("P1Y").unwrap_err().contains("use days (P30D)"));
        assert!(duration("P1M").unwrap_err().contains("use days (P30D)"));
        assert!(duration("P1H").unwrap_err().contains("hours, minutes and seconds go after T"));
        assert!(duration("PTT1H").unwrap_err().contains("T comes once"));
        assert!(duration(&format!("PT{}H", "9".repeat(400))).unwrap_err().contains("too large"));
        for s in [
            "P",
            "PT",
            "PT1.5H30M",
            "P1H",
            "PT1D",
            "PT30M1H",
            "P1W2D",
            "PT1",
            "PT.5S",
            "PT1.S",
            "P1DT",
            "PTT1H",
        ] {
            let e = duration(s).unwrap_err();
            assert!(e.contains("ISO 8601 duration"), "{s}: {e}");
        }
    }

    #[test]
    fn whens() {
        close(when("06:30").unwrap(), 6.5);
        close(when("P1DT18H").unwrap(), 42.0);
        close(when("PT6H30M2.5S").unwrap(), 6.5 + 2.5 * S);
        assert!(when("d2 18:00").unwrap_err().contains("P1DT18H"));
    }

    #[test]
    fn windows() {
        let w = |s| {
            let (a, b) = window(s).unwrap();
            (round_ms(a), round_ms(b))
        };
        assert_eq!(w("18:00/20:00"), (18.0, 20.0));
        assert_eq!(w("22:30/06:30"), (22.5, 30.5));
        assert_eq!(w("09:00/PT8H"), (9.0, 17.0));
        assert_eq!(w("12:00/12:00"), (12.0, 36.0));
    }

    #[test]
    fn bad_windows() {
        assert!(window("18:00-20:00").unwrap_err().contains("use an ISO 8601 interval like 18:00/20:00"));
        assert!(window("09:00/PT0S").unwrap_err().contains("positive duration"));
        assert!(window("9:00/17:00").unwrap_err().contains("time of day"));
        assert!(window("09:00/8h").unwrap_err().contains("time of day"));
        assert!(window("18:00").unwrap_err().contains("like 18:00/20:00 or 09:00/PT8H"));
        // Found by fuzzing (#55): it parsed as an empty window, 24.0..24.0.
        assert!(window("24:00/00:00").unwrap_err().contains("can't start there; use 00:00"));
        assert!(window("24:00/PT1H").unwrap_err().contains("can't start there"));
        assert_eq!(window("00:00/24:00"), Ok((0.0, 24.0)));
    }
}
