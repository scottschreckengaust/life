//! A tiny single-threaded executor with a clock you can bend.
//!
//! * [`Mode::Virtual`]: when every task is waiting, time jumps straight to the next
//!   timer. A week of life runs in microseconds, and it runs the same way every time.
//! * [`Mode::RealTime`]: one simulated hour takes a fixed amount of wall-clock time
//!   (1 second by default in the CLI, which runs 3600 times faster than real life).
//!
//! Time is measured in simulated hours (`f64`) at the API and in whole simulated
//! milliseconds internally, so timers never disagree about rounding.

use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    future::Future,
    pin::{pin, Pin},
    rc::Rc,
    sync::{Arc, Mutex},
    task::{Context, Poll, Wake, Waker},
    thread::{self, Thread},
    time::{Duration, Instant},
};

const MS_PER_HOUR: f64 = 3_600_000.0;
const MAIN: usize = usize::MAX;

/// How the clock moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Time travel: idle means "skip to the next timer".
    Virtual,
    /// Wall clock: each simulated hour takes this long for real.
    RealTime(Duration),
}

type Task = Pin<Box<dyn Future<Output = ()>>>;

struct Ready {
    queue: Mutex<VecDeque<usize>>,
    thread: Thread,
}

struct TaskWaker {
    id: usize,
    ready: Arc<Ready>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        let mut q = self.ready.queue.lock().unwrap();
        if !q.contains(&self.id) {
            q.push_back(self.id);
        }
        drop(q);
        self.ready.thread.unpark();
    }
}

struct Exec {
    mode: Mode,
    /// Real-time runs follow the wall clock from `origin`, which it reached at `started`.
    origin: Cell<u64>,
    started: Cell<Instant>,
    /// Before this, a real-time run travels in time like a virtual one.
    live_from: u64,
    live: Cell<bool>,
    virt: Cell<u64>,
    timers: RefCell<Vec<(u64, Waker)>>,
    tasks: RefCell<Vec<Option<Task>>>,
    ready: Arc<Ready>,
}

impl Exec {
    /// Simulated time. It only moves when the executor is idle, and then lands exactly on a
    /// timer's deadline, in both modes, so a real-time run lives the same day as a virtual one.
    fn now(&self) -> u64 {
        self.virt.get()
    }

    /// Where the wall clock says simulated time should be, for real-time runs.
    fn wall(&self, per_hour: Duration) -> u64 {
        let hours = self.started.get().elapsed().as_secs_f64() / per_hour.as_secs_f64();
        // An hour of zero wall-clock time makes `hours` infinite: saturate, so the clock is
        // always past the next deadline (time travels, as in Mode::Virtual), instead of
        // wrapping round and spinning forever.
        self.origin.get().saturating_add((hours * MS_PER_HOUR) as u64)
    }

    fn waker(&self, id: usize) -> Waker {
        Waker::from(Arc::new(TaskWaker { id, ready: self.ready.clone() }))
    }

    fn pop(&self) -> Option<usize> {
        self.ready.queue.lock().unwrap().pop_front()
    }

    /// Nothing is runnable: fire the next timer, travelling in time if allowed.
    fn idle(&self) {
        let next = self.timers.borrow().iter().map(|t| t.0).min();
        if matches!(self.mode, Mode::RealTime(_)) && !self.live.get() {
            match next {
                // Still before the live part: skip ahead like Mode::Virtual.
                Some(n) if n < self.live_from => {
                    self.virt.set(self.virt.get().max(n));
                    return self.fire_due();
                }
                // Reached it: from here the wall clock runs.
                _ => {
                    self.virt.set(self.virt.get().max(self.live_from));
                    self.origin.set(self.virt.get());
                    self.started.set(Instant::now());
                    self.live.set(true);
                }
            }
        }
        let Some(next) = next else {
            match self.mode {
                Mode::Virtual => panic!("deadlock: every task is asleep and nobody set an alarm"),
                Mode::RealTime(per_hour) => {
                    thread::park();
                    self.virt.set(self.virt.get().max(self.wall(per_hour)));
                    return;
                }
            }
        };
        match self.mode {
            Mode::Virtual => self.virt.set(self.virt.get().max(next)),
            Mode::RealTime(per_hour) => {
                let wall = self.wall(per_hour);
                if next > wall {
                    let hours = (next - wall) as f64 / MS_PER_HOUR;
                    thread::park_timeout(per_hour.mul_f64(hours));
                }
                // Fire on the deadline, not a few microseconds of wall clock after it. A
                // wake-up from another thread before the deadline just catches up to the wall.
                let wall = self.wall(per_hour);
                self.virt.set(self.virt.get().max(if wall >= next { next } else { wall }));
            }
        }
        self.fire_due();
    }

    /// Wake every timer whose deadline has come.
    fn fire_due(&self) {
        let now = self.now();
        let due: Vec<(u64, Waker)> = {
            let mut timers = self.timers.borrow_mut();
            let (due, keep) = timers.drain(..).partition(|(at, _)| *at <= now);
            *timers = keep;
            due
        };
        for (_, w) in due {
            w.wake();
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<Exec>>> = const { RefCell::new(None) };
}

fn with<T>(f: impl FnOnce(&Exec) -> T) -> T {
    let exec = CURRENT.with(|c| c.borrow().clone());
    f(&exec.expect("no life here: call this inside life::rt::block_on"))
}

struct Enter;
impl Drop for Enter {
    fn drop(&mut self) {
        CURRENT.with(|c| c.borrow_mut().take());
    }
}

fn ms(hours: f64) -> u64 {
    (hours * MS_PER_HOUR).round().max(0.0) as u64
}

/// Run `fut` to completion, starting the clock at `start` simulated hours.
pub fn block_on<F: Future>(mode: Mode, start: f64, fut: F) -> F::Output {
    block_on_live_from(mode, start, start, fut)
}

/// Like [`block_on`], but a [`Mode::RealTime`] clock travels in time like [`Mode::Virtual`]
/// until `live_from`, and only follows the wall clock from there: a window late in a long run
/// doesn't wait through everything before it. In [`Mode::Virtual`], `live_from` changes nothing.
pub fn block_on_live_from<F: Future>(mode: Mode, start: f64, live_from: f64, fut: F) -> F::Output {
    let live_from = ms(live_from).max(ms(start));
    let exec = Rc::new(Exec {
        mode,
        origin: Cell::new(ms(start)),
        started: Cell::new(Instant::now()),
        live_from,
        live: Cell::new(live_from <= ms(start)),
        virt: Cell::new(ms(start)),
        timers: RefCell::default(),
        tasks: RefCell::default(),
        ready: Arc::new(Ready { queue: Mutex::new(VecDeque::from([MAIN])), thread: thread::current() }),
    });
    CURRENT.with(|c| *c.borrow_mut() = Some(exec.clone()));
    let _enter = Enter;
    let mut fut = pin!(fut);
    let main = exec.waker(MAIN);
    loop {
        while let Some(id) = exec.pop() {
            if id == MAIN {
                if let Poll::Ready(out) = fut.as_mut().poll(&mut Context::from_waker(&main)) {
                    return out;
                }
                continue;
            }
            let task = exec.tasks.borrow_mut().get_mut(id).and_then(Option::take);
            if let Some(mut task) = task {
                let waker = exec.waker(id);
                if task.as_mut().poll(&mut Context::from_waker(&waker)).is_pending() {
                    exec.tasks.borrow_mut()[id] = Some(task);
                }
            }
        }
        exec.idle();
    }
}

/// The current simulated time, in hours.
pub fn now() -> f64 {
    with(Exec::now) as f64 / MS_PER_HOUR
}

/// A timer future. Cancel it by dropping it.
#[derive(Debug)]
pub struct Sleep {
    at: u64,
    armed: bool,
}

impl Future for Sleep {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let at = self.at;
        let ready = with(|e| {
            let ready = e.now() >= at;
            if !ready && !self.armed {
                e.timers.borrow_mut().push((at, cx.waker().clone()));
            }
            ready
        });
        self.armed = true;
        if ready {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}

/// Sleep until simulated hour `at`.
pub fn sleep_until(at: f64) -> Sleep {
    Sleep { at: ms(at), armed: false }
}

/// Sleep for `hours` simulated hours.
pub fn sleep(hours: f64) -> Sleep {
    Sleep { at: with(Exec::now) + ms(hours), armed: false }
}

/// A handle to a spawned task.
#[derive(Debug)]
pub struct JoinHandle(usize);

impl JoinHandle {
    /// Drop the task wherever it is suspended.
    pub fn abort(&self) {
        with(|e| {
            if let Some(slot) = e.tasks.borrow_mut().get_mut(self.0) {
                *slot = None;
            }
        });
    }
}

/// Run a background task on the current runtime.
pub fn spawn(fut: impl Future<Output = ()> + 'static) -> JoinHandle {
    with(|e| {
        let mut tasks = e.tasks.borrow_mut();
        tasks.push(Some(Box::pin(fut)));
        let id = tasks.len() - 1;
        e.ready.queue.lock().unwrap().push_back(id);
        JoinHandle(id)
    })
}

/// A one-permit doorbell: `ring` before anyone listens and the next `heard` returns at once.
#[derive(Debug, Default)]
pub struct Bell(Mutex<(bool, Option<Waker>)>);

impl Bell {
    /// Ring it. Safe from any thread.
    pub fn ring(&self) {
        let waker = {
            let mut s = self.0.lock().unwrap();
            s.0 = true;
            s.1.take()
        };
        if let Some(w) = waker {
            w.wake();
        }
    }

    /// Resolves once the bell has rung, consuming the permit.
    pub fn heard(&self) -> impl Future<Output = ()> + '_ {
        std::future::poll_fn(move |cx| {
            let mut s = self.0.lock().unwrap();
            if std::mem::take(&mut s.0) {
                Poll::Ready(())
            } else {
                s.1 = Some(cx.waker().clone());
                Poll::Pending
            }
        })
    }
}

/// A boxed future, for building a `select` out of a list only known at runtime.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// `select! { biased; ... }` over a `Vec`: poll in order, return the first finisher's
/// index and output. Losers are dropped when the returned future is.
pub fn select_biased<'a, T: 'a>(mut futs: Vec<BoxFuture<'a, T>>) -> impl Future<Output = (usize, T)> + 'a {
    std::future::poll_fn(move |cx| {
        for (i, f) in futs.iter_mut().enumerate() {
            if let Poll::Ready(v) = f.as_mut().poll(cx) {
                return Poll::Ready((i, v));
            }
        }
        Poll::Pending
    })
}
