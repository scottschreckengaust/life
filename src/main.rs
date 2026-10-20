use life::{rt::Mode, Life, Scenario};
use std::{process::exit, time::Duration};

const USAGE: &str = "\
life: if not geeking out, try being with people, catch surfing, finally living, eating, sleeping

USAGE: life [SCENARIO] [--scale SECS] [--instant] [--quiet] [--list]

  SCENARIO          which life to live (default: week)
  -s, --scale SECS  wall-clock seconds per simulated hour (default: 1, hours -> seconds)
  -i, --instant     virtual time: skip straight to the next thing that happens
  -q, --quiet       no diary, just the report
  -l, --list        list scenarios";

fn main() {
    let (mut name, mut scale, mut instant, mut quiet) = ("week".to_string(), 1.0, false, false);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-i" | "--instant" => instant = true,
            "-q" | "--quiet" => quiet = true,
            "-s" | "--scale" => match args.next().and_then(|v| v.parse::<f64>().ok()) {
                Some(s) if s > 0.0 => scale = s,
                _ => fail("--scale needs a positive number of seconds per hour"),
            },
            "-l" | "--list" => return list(),
            "-h" | "--help" => return println!("{USAGE}"),
            other => name = other.to_string(),
        }
    }
    let Some(scenario) = Scenario::by_name(&name) else {
        eprintln!("no such life: {name}\n");
        list();
        exit(2);
    };
    let mode = if instant { Mode::Virtual } else { Mode::RealTime(Duration::from_secs_f64(scale)) };
    let pace = if instant { "instant".to_string() } else { format!("1h = {scale}s") };
    println!(
        "🌍 {}: {} ({} day{}, {pace})\n",
        scenario.name,
        scenario.about,
        scenario.days.len(),
        if scenario.days.len() == 1 { "" } else { "s" }
    );
    let journal = Life::scenario(&scenario).echo(!quiet).run(mode);
    print!("{}", journal.report());
}

fn list() {
    for s in Scenario::all() {
        println!("  {:<14} {}", s.name, s.about);
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("{msg}\n\n{USAGE}");
    exit(2)
}
