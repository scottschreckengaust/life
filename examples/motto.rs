//! The motto, literally. (The library runs anyone's rules; this runs exactly this one.) One simulated hour = one real second.
//!
//!     cargo run --example motto        # Ctrl-C to wake up for good
//!
//! The night is a genuinely hot `spin_loop()` on a real atomic, set by a real
//! `std::thread` that sleeps through one spin of the earth. It burns a core all
//! night. That's the joke, and also why the library itself `.await`s its dreams.

use spin_life::{join, rt, select};
use std::{
    hint::spin_loop,
    sync::atomic::{
        AtomicBool, AtomicU64,
        Ordering::{Acquire, Relaxed, Release},
    },
    thread,
    time::Duration,
};

static ALARM: AtomicBool = AtomicBool::new(false);
static DAY: AtomicU64 = AtomicU64::new(1);
static SPINS: AtomicU64 = AtomicU64::new(0);

/// One spin of the earth, with hours converted to seconds.
const EARTH_SPIN: Duration = Duration::from_secs(24);

fn main() {
    rt::block_on(rt::Mode::RealTime(Duration::from_secs(1)), 7.0, motto());
}

#[rustfmt::skip]
async fn motto() {
    thread::spawn(|| loop { thread::sleep(EARTH_SPIN); ALARM.store(true, Release) });
    loop { select! { biased; () = geek() => {}, () = people() => {}, () = surf() => {} } join!(live(), eat()); ALARM.store(false, Relaxed); while !ALARM.load(Acquire) { dream(); spin_loop(); } }
}

/// Polled first every morning (that's what `biased;` means), so it also wakes you up.
async fn geek() {
    let spins = SPINS.swap(0, Relaxed);
    if spins > 0 {
        DAY.fetch_add(1, Relaxed);
        say(&format!("⏰ alarm. Last night: {spins} spin_loop() iterations"));
    }
    if DAY.load(Relaxed) % 3 != 0 {
        std::future::pending::<()>().await; // no idea today
    }
    say("🧠 an idea: geeking out wins (every third day)");
    rt::sleep(2.0).await;
}

async fn people() {
    rt::sleep(2.0).await;
    say("👨‍👩‍👧 with people (beats a tie with surf, because biased)");
}

async fn surf() {
    let session = if DAY.load(Relaxed) % 2 == 0 { 1.0 } else { 2.0 };
    rt::sleep(session).await;
    say("🏄 caught a quick set (only wins when it's over first)");
}

async fn live() {
    say("🛋  living");
    rt::sleep(3.0).await;
}

async fn eat() {
    say("🍝 eating");
    rt::sleep(1.0).await;
}

fn dream() {
    if SPINS.fetch_add(1, Relaxed) == 0 {
        say("💤 dreaming (hot spin loop: one core at 100% until the alarm thread rings)");
    }
}

fn say(what: &str) {
    println!("day {} {:>5.1}h  {what}", DAY.load(Relaxed), rt::now() % 24.0);
}
