//! A tiny single-threaded executor with a clock you can bend.
//!
//! * [`Mode::Virtual`]: when every task is waiting, time jumps straight to the next
//!   timer. A week of life runs in microseconds, and it runs the same way every time.
//! * [`Mode::RealTime`]: one simulated hour takes a fixed amount of wall-clock time
//!   (1 second by default in the CLI: hours become seconds).
//!
//! Time is measured in simulated hours (`f64`) at the API and in whole simulated
//! seconds internally, so timers never disagree about rounding.

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

const SECS_PER_HOUR: f64 = 3600.0;
const MAIN: usize = usize::MAX;

/// How the clock moves.
#[derive(Clone, Copy, Debug, PartialEq)]
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
        self.wake_by_ref()
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
    origin: u64,
    started: Instant,
    virt: Cell<u64>,
    timers: RefCell<Vec<(u64, Waker)>>,
    tasks: RefCell<Vec<Option<Task>>>,
    ready: Arc<Ready>,
}

impl Exec {
    fn now(&self) -> u64 {
        match self.mode {
            Mode::Virtual => self.virt.get(),
            Mode::RealTime(per_hour) => {
                let hours = self.started.elapsed().as_secs_f64() / per_hour.as_secs_f64();
                self.origin + (hours * SECS_PER_HOUR) as u64
            }
        }
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
        let Some(next) = next else {
            match self.mode {
                Mode::Virtual => panic!("deadlock: every task is asleep and nobody set an alarm"),
                Mode::RealTime(_) => return thread::park(),
            }
        };
        match self.mode {
            Mode::Virtual => self.virt.set(self.virt.get().max(next)),
            Mode::RealTime(per_hour) => {
                let now = self.now();
                if next > now {
                    let hours = (next - now) as f64 / SECS_PER_HOUR;
                    thread::park_timeout(per_hour.mul_f64(hours));
                }
            }
        }
        let now = self.now();
        let due: Vec<(u64, Waker)> = {
            let mut timers = self.timers.borrow_mut();
            let (due, keep) = timers.drain(..).partition(|(at, _)| *at <= now);
            *timers = keep;
            due
        };
        due.into_iter().for_each(|(_, w)| w.wake());
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

fn secs(hours: f64) -> u64 {
    (hours * SECS_PER_HOUR).round().max(0.0) as u64
}

/// Run `fut` to completion, starting the clock at `start` simulated hours.
pub fn block_on<F: Future>(mode: Mode, start: f64, fut: F) -> F::Output {
    let exec = Rc::new(Exec {
        mode,
        origin: secs(start),
        started: Instant::now(),
        virt: Cell::new(secs(start)),
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
    with(|e| e.now()) as f64 / SECS_PER_HOUR
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
    Sleep { at: secs(at), armed: false }
}

/// Sleep for `hours` simulated hours.
pub fn sleep(hours: f64) -> Sleep {
    Sleep { at: with(|e| e.now()) + secs(hours), armed: false }
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
        })
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
