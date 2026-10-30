//! The runtime underneath: select!, join!, virtual time, and the alarm's memory ordering.

use spin_life::{
    join,
    rt::{self, Mode},
    select, Alarm, Why,
};
use std::{
    cell::Cell,
    future::pending,
    hint::spin_loop,
    rc::Rc,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

fn virt<F: std::future::Future>(f: F) -> F::Output {
    rt::block_on(Mode::Virtual, 0.0, f)
}

#[test]
fn biased_select_breaks_ties_top_down() {
    let won = virt(async {
        select! { biased; () = rt::sleep(1.0) => "geek", () = rt::sleep(1.0) => "people", () = rt::sleep(1.0) => "surf" }
    });
    assert_eq!(won, "geek");
}

#[test]
fn select_takes_whatever_finishes_first() {
    let won = virt(async {
        select! { biased; () = rt::sleep(2.0) => "geek", () = rt::sleep(1.0) => "people", () = pending::<()>() => "surf" }
    });
    assert_eq!(won, "people");
}

#[test]
fn select_binds_patterns() {
    let sum = virt(async {
        select! { (a, b) = async { (40, 2) } => a + b, () = pending::<()>() => 0 }
    });
    assert_eq!(sum, 42);
}

#[test]
fn select_drops_the_losers_before_running_the_arm() {
    struct Flag(Rc<Cell<bool>>);
    impl Drop for Flag {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = Rc::new(Cell::new(false));
    let loser = {
        let flag = Flag(dropped.clone());
        async move {
            let _flag = flag;
            pending::<()>().await;
        }
    };
    let seen = virt(async {
        select! { () = loser => false, () = rt::sleep(1.0) => dropped.get() }
    });
    assert!(seen, "the loser was still alive when the winning arm ran");
}

#[test]
fn join_waits_for_everyone() {
    let (out, at) = virt(async {
        let out = join!(
            async {
                rt::sleep(1.0).await;
                "eat"
            },
            async {
                rt::sleep(3.0).await;
                "live"
            }
        );
        (out, rt::now())
    });
    assert_eq!(out, ("eat", "live"));
    assert_eq!(at, 3.0);
}

#[test]
fn virtual_time_skips_the_boring_parts() {
    let wall = Instant::now();
    let now = virt(async {
        rt::sleep(24.0 * 365.0 * 100.0).await; // a century
        rt::now()
    });
    assert_eq!(now, 876_000.0);
    assert!(wall.elapsed() < Duration::from_secs(1));
}

#[test]
fn spawned_tasks_run_and_aborted_ones_do_not() {
    let hits = Rc::new(Cell::new(0));
    virt({
        let hits = hits.clone();
        async move {
            let (a, b) = (hits.clone(), hits.clone());
            let _keep = rt::spawn(async move {
                rt::sleep(1.0).await;
                a.set(a.get() + 1);
            });
            let gone = rt::spawn(async move {
                rt::sleep(1.0).await;
                b.set(b.get() + 100);
            });
            gone.abort();
            rt::sleep(2.0).await;
        }
    });
    assert_eq!(hits.get(), 1);
}

#[test]
#[should_panic(expected = "deadlock")]
fn sleeping_forever_without_an_alarm_is_a_deadlock() {
    virt(pending::<()>());
}

#[test]
fn acquire_sees_why_the_alarm_rang() {
    // The reason is written Relaxed before the Release store of `rung`. After an
    // Acquire load sees `rung`, the reason must be visible. On a real thread.
    for _ in 0..1_000 {
        let alarm = Arc::new(Alarm::default());
        let ringer = {
            let alarm = alarm.clone();
            thread::spawn(move || alarm.ring(Why::URGENT))
        };
        while !alarm.rung() {
            spin_loop();
        }
        assert!(alarm.take().has(Why::URGENT));
        ringer.join().unwrap();
    }
}

#[test]
fn the_nightly_reset_forgets_old_rings() {
    let alarm = Alarm::default();
    alarm.ring(Why::URGENT);
    alarm.reset();
    assert!(!alarm.rung());
    alarm.ring(Why::MORNING);
    let why = alarm.take();
    assert!(why.has(Why::MORNING) && !why.has(Why::URGENT));
    assert!(!alarm.rung());
}

#[test]
fn a_real_time_run_with_no_timers_waits_for_another_thread() {
    // Nothing is scheduled, so the executor parks until a bell rung from another thread wakes it;
    // the clock then catches up with the wall clock instead of jumping.
    let bell = Arc::new(rt::Bell::default());
    let ringer = bell.clone();
    let t = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        ringer.ring();
    });
    let woke_at = rt::block_on(Mode::RealTime(Duration::from_secs(3600)), 1.0, async move {
        bell.heard().await;
        rt::now()
    });
    t.join().unwrap();
    // 50 ms of wall clock at real time is 50 ms of simulated time (give it up to a few seconds).
    assert!(woke_at >= 1.0 + 0.04 / 3600.0, "{woke_at}");
    assert!(woke_at < 1.0 + 5.0 / 3600.0, "{woke_at}");
}

#[test]
#[should_panic(expected = "no life here")]
fn the_clock_is_gone_once_the_run_is_over() {
    virt(async {});
    rt::now();
}

#[test]
fn a_timer_wakes_its_task_once_when_it_is_due() {
    // An already-due sleep finishes without a timer, and a pending one isn't re-armed when
    // its task is polled again; either would mean an extra, spurious wake-up.
    let polls = Cell::new(0);
    let mut life = Box::pin(async {
        rt::sleep(0.0).await;
        rt::sleep(1.0).await;
    });
    virt(std::future::poll_fn(|cx| {
        polls.set(polls.get() + 1);
        std::future::Future::poll(life.as_mut(), cx)
    }));
    assert_eq!(polls.get(), 2, "the first poll, then the one the timer's due for");
}

#[test]
fn real_time_keeps_running_while_a_task_is_busy() {
    // A window from 1.0 on the wall clock, one simulated hour per second. The task works
    // (spins) for 0.3 s at the start, then sleeps 0.5 h. The busy time counts, so the clock
    // is already 0.3 h along and the run ends after ~0.5 s, not 0.3 + 0.5. The gap between the
    // two (0.5 vs 0.8 s) leaves a slow CI runner plenty of room.
    let started = Instant::now();
    rt::block_on_live_from(Mode::RealTime(Duration::from_secs(1)), 0.0, 1.0, async {
        rt::sleep(1.0).await;
        let busy = Instant::now();
        while busy.elapsed() < Duration::from_millis(300) {
            spin_loop();
        }
        rt::sleep(0.5).await;
    });
    let took = started.elapsed();
    assert!(took >= Duration::from_millis(450) && took < Duration::from_millis(700), "{took:?}");
}

/// This thread's CPU time so far in clock ticks, and how many times it has gone to sleep (Linux).
#[cfg(target_os = "linux")]
fn cpu_ticks_and_sleeps() -> (u64, u64) {
    let stat = std::fs::read_to_string("/proc/thread-self/stat").unwrap();
    // After the `(comm)` field: state is field 3, utime and stime are fields 14 and 15.
    let fields: Vec<&str> = stat.rsplit_once(')').unwrap().1.split_whitespace().collect();
    let ticks = fields[11].parse::<u64>().unwrap() + fields[12].parse::<u64>().unwrap();
    let status = std::fs::read_to_string("/proc/thread-self/status").unwrap();
    let sleeps = status
        .lines()
        .find_map(|l| l.strip_prefix("voluntary_ctxt_switches:"))
        .map(|n| n.trim().parse::<u64>().unwrap())
        .unwrap();
    (ticks, sleeps)
}

#[test]
#[cfg(target_os = "linux")]
fn real_time_waits_without_burning_a_core() {
    // Half a second of waiting for a timer is spent parked, in one long nap: not spinning, and
    // not waking up every few microseconds to check the time.
    let (ticks, sleeps) = cpu_ticks_and_sleeps();
    let started = Instant::now();
    rt::block_on(Mode::RealTime(Duration::from_millis(500)), 0.0, async { rt::sleep(1.0).await });
    let took = started.elapsed();
    let (ticks, sleeps) = (cpu_ticks_and_sleeps().0 - ticks, cpu_ticks_and_sleeps().1 - sleeps);
    assert!(took >= Duration::from_millis(500), "{took:?}");
    assert!(ticks < 15, "{ticks} ticks of CPU in {took:?}"); // ticks are 10 ms; spinning is ~50
    assert!(sleeps < 20, "went to sleep {sleeps} times in {took:?}");
}

#[test]
fn an_hour_of_zero_wall_clock_time_is_virtual_time() {
    // RealTime(ZERO) used to divide by zero, overflow the clock and spin forever.
    let t = std::time::Instant::now();
    let at = rt::block_on(rt::Mode::RealTime(std::time::Duration::ZERO), 0.0, async {
        rt::sleep_until(1e6).await;
        rt::now()
    });
    assert!((at - 1e6).abs() < 1e-6, "{at}");
    assert!(t.elapsed().as_secs() < 5);
}
