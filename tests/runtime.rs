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
        select! { biased; _ = rt::sleep(1.0) => "geek", _ = rt::sleep(1.0) => "people", _ = rt::sleep(1.0) => "surf" }
    });
    assert_eq!(won, "geek");
}

#[test]
fn select_takes_whatever_finishes_first() {
    let won = virt(async {
        select! { biased; _ = rt::sleep(2.0) => "geek", _ = rt::sleep(1.0) => "people", _ = pending::<()>() => "surf" }
    });
    assert_eq!(won, "people");
}

#[test]
fn select_binds_patterns() {
    let sum = virt(async {
        select! { (a, b) = async { (40, 2) } => a + b, _ = pending::<()>() => 0 }
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
            pending::<()>().await
        }
    };
    let seen = virt(async {
        select! { _ = loser => false, _ = rt::sleep(1.0) => dropped.get() }
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
