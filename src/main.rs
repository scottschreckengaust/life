use spin_life::{rt::Mode, Life, Rules, Scenario, PRESETS};
use std::{process::exit, time::Duration};

const USAGE: &str = "\
spin-life: simulate a life from rules you write

USAGE:
  spin-life [run] [SCENARIO] [OPTIONS]   live a scenario (default: week, or the file's first)
  spin-life list    [--life FILE]        list a life's scenarios
  spin-life presets                      list built-in lives
  spin-life init    [--preset NAME]      print a life file to start from (default: scott)
  spin-life check   FILE                 validate a life file

OPTIONS:
  -L, --life FILE    your rules (a .life.toml file)
  -p, --preset NAME  a built-in life instead (default: scott)
  -d, --days N       ignore scenarios; live N ordinary days
  -s, --scale SECS   wall-clock seconds per simulated hour (default: 1, hours become seconds)
  -i, --instant      virtual time: skip straight to the next thing that happens
  -q, --quiet        no diary, just the report";

#[derive(Default)]
struct Args {
    cmd: Option<String>,
    target: Option<String>,
    life: Option<String>,
    preset: Option<String>,
    days: Option<usize>,
    scale: Option<f64>,
    instant: bool,
    quiet: bool,
}

fn main() {
    let a = parse_args();
    match a.cmd.as_deref().unwrap_or("run") {
        "presets" => {
            for (name, about, _) in PRESETS {
                println!("  {name:<8} {about}");
            }
        }
        "init" => {
            let name = a.preset.as_deref().or(a.target.as_deref()).unwrap_or("scott");
            match Rules::preset_source(name) {
                Some(src) => print!("{src}"),
                None => fail(&format!("no preset named `{name}`; try `spin-life presets`")),
            }
        }
        "check" => {
            let Some(path) = a.target.as_deref().or(a.life.as_deref()) else { fail("check needs a FILE") };
            let rules = load_file(path);
            let names: Vec<&str> = rules.pursuits.iter().map(|p| p.name.as_str()).collect();
            println!(
                "✅ {path}: {} pursuits ({}), {} scenarios",
                names.len(),
                names.join(", "),
                rules.scenarios.len()
            );
        }
        "list" => {
            let rules = load(&a);
            if rules.scenarios.is_empty() {
                println!("  (no scenarios; `spin-life run --days 7` lives an ordinary week)");
            }
            for s in &rules.scenarios {
                println!("  {:<16} {}", s.name, s.about);
            }
        }
        "run" => run(&a),
        other => fail(&format!("unknown command `{other}`")),
    }
}

fn run(a: &Args) {
    let rules = load(a);
    let scenario = match (a.days, a.target.as_deref()) {
        (Some(n), _) => Scenario::plain(n.clamp(1, 366)),
        (None, Some(name)) => rules.scenario(name).cloned().unwrap_or_else(|| {
            eprintln!("no scenario `{name}` in {}'s life. Scenarios:", rules.name);
            rules.scenarios.iter().for_each(|s| eprintln!("  {}", s.name));
            exit(2)
        }),
        (None, None) => {
            rules.scenario("week").or(rules.scenarios.first()).cloned().unwrap_or_else(|| Scenario::plain(7))
        }
    };
    let mode = match (a.instant, a.scale) {
        (true, _) => Mode::Virtual,
        (false, s) => Mode::RealTime(Duration::from_secs_f64(s.unwrap_or(1.0))),
    };
    let pace = if a.instant { "instant".to_string() } else { format!("1h = {}s", a.scale.unwrap_or(1.0)) };
    let plural = if scenario.days == 1 { "" } else { "s" };
    if let Some(motto) = &rules.motto {
        println!("“{motto}”");
    }
    println!(
        "🌍 {} / {}: {} ({} day{plural}, {pace})",
        rules.name, scenario.name, scenario.about, scenario.days
    );
    if !a.instant {
        // Each day runs from one morning's alarm to the next: one spin of the earth.
        let secs = scenario.days as f64 * spin_life::EARTH_SPIN * a.scale.unwrap_or(1.0);
        println!("⏱  about {} of real time; add --instant to skip ahead, or --scale to speed up", wall(secs));
    }
    println!();
    let journal = Life::new(&rules, &scenario).echo(!a.quiet).run(mode);
    print!("{}", journal.report());
}

fn load(a: &Args) -> Rules {
    match (&a.life, &a.preset) {
        (Some(path), _) => load_file(path),
        (None, name) => {
            let name = name.as_deref().unwrap_or("scott");
            Rules::preset(name)
                .unwrap_or_else(|| fail(&format!("no preset named `{name}`; try `spin-life presets`")))
        }
    }
}

fn load_file(path: &str) -> Rules {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| fail(&format!("{path}: {e}")));
    Rules::parse(&src).unwrap_or_else(|e| {
        eprintln!("❌ {path}: {e}");
        exit(1)
    })
}

fn parse_args() -> Args {
    let mut a = Args::default();
    let mut it = std::env::args().skip(1);
    let value = |flag: &str, it: &mut dyn Iterator<Item = String>| {
        it.next().unwrap_or_else(|| fail(&format!("{flag} needs a value")))
    };
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                exit(0)
            }
            "-V" | "--version" => {
                println!("spin-life {}", env!("CARGO_PKG_VERSION"));
                exit(0)
            }
            "-i" | "--instant" => a.instant = true,
            "-q" | "--quiet" => a.quiet = true,
            "-L" | "--life" => a.life = Some(value(&arg, &mut it)),
            "-p" | "--preset" => a.preset = Some(value(&arg, &mut it)),
            "-d" | "--days" => {
                a.days = Some(value(&arg, &mut it).parse().unwrap_or_else(|_| fail("--days needs a number")))
            }
            "-s" | "--scale" => match value(&arg, &mut it).parse::<f64>() {
                Ok(s) if s > 0.0 => a.scale = Some(s),
                _ => fail("--scale needs a positive number of seconds per hour"),
            },
            s if s.starts_with('-') => fail(&format!("unknown option `{s}`")),
            "run" | "list" | "presets" | "init" | "check" if a.cmd.is_none() && a.target.is_none() => {
                a.cmd = Some(arg)
            }
            _ if a.target.is_none() => a.target = Some(arg),
            _ => fail(&format!("unexpected argument `{arg}`")),
        }
    }
    a
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}\n\n{USAGE}");
    exit(2)
}

/// `"2m 48s"`, `"1h 12m"`, `"0.4s"`.
fn wall(secs: f64) -> String {
    let s = secs.round() as u64;
    match s {
        _ if secs < 10.0 => format!("{secs:.1}s"),
        0..=59 => format!("{s}s"),
        60..=3599 => format!("{}m {:02}s", s / 60, s % 60),
        _ => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
    }
}
