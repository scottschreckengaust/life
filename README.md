# life

[![ci](https://github.com/scottschreckengaust/life/actions/workflows/ci.yml/badge.svg)](https://github.com/scottschreckengaust/life/actions/workflows/ci.yml)
![dependencies: 0](https://img.shields.io/badge/dependencies-0-brightgreen)
![unsafe: forbidden](https://img.shields.io/badge/unsafe-forbidden-success)

```rust
thread::spawn(|| loop { thread::sleep(EARTH_SPIN); ALARM.store(true, Release) });
loop { select! { biased; _ = geek() => {}, _ = people() => {}, _ = surf() => {} } join!(live(), eat()); ALARM.store(false, Relaxed); while !ALARM.load(Acquire) { dream(); spin_loop(); } }
```

*If not geeking out: try being with people, catch surfing: finally living, eating, sleeping.*

This crate runs that line. It simulates days of life on a **zero-dependency async runtime** with a clock you can
bend: hours become seconds, or virtual time skips straight to whatever happens next. `select!` and `join!` are
`macro_rules!` written from scratch. The alarm is a real `AtomicBool` with real memory orderings. A night's sleep
is a real `spin_loop()`.

## Run it

```sh
cargo run -- week                 # a week of life, 1 simulated hour = 1 real second
cargo run -- chaos --scale 0.1    # faster: 1 hour = 0.1s
cargo run -- coffee --instant     # virtual time: done in microseconds
cargo run -- --list               # every scenario
cargo run --example motto         # the motto, literally (one CPU core spins all night)
cargo test                        # 35 tests, all on virtual time, deterministic
```

| scenario        | what happens                                                     |
|-----------------|------------------------------------------------------------------|
| `happy-path`    | wake, eat, work, lunch, family, eat, sleep. Repeat.              |
| `week`          | five workdays, a surf Saturday, an idea on Sunday                |
| `coffee`        | a morning cup, a 3pm cup, then a triple espresso at 20:00         |
| `party`         | a party next door at 00:30 on a work night                       |
| `special-event` | a friend's launch party instead of family dinner                  |
| `dawn-patrol`   | a big swell at 05:30 rings the alarm early                        |
| `big-wave`      | an idea at 17:00, then a big swell at 18:00 breaks into it        |
| `late-night`    | one more commit... until 02:00                                   |
| `early-flight`  | a 05:30 work flight the next morning                             |
| `deadline`      | shipping by 21:00, family dinner skipped                          |
| `chaos`         | a week where everything happens                                   |

```text
$ cargo run -- chaos --instant --quiet

day   sleep   wired  dreams    idea  coffee    work    geek  people    surf wakeups
  1    8.0h    0.0h       5     yes       1    7.0h    0.0h    2.0h    0.0h       0
  2    7.0h    1.0h       4       -       2   11.0h    0.0h    0.0h    0.0h       0
  3    3.0h    0.0h       2       -       0    7.0h    5.0h    1.0h    0.0h       0
  4    7.0h    0.0h       4       -       0    7.0h    0.0h    1.0h    0.0h       1
  5    9.5h    0.0h       6     yes       0    7.0h    1.5h    0.0h    2.0h       0
  6    8.0h    0.0h       5     yes       0    0.0h    2.0h    3.0h    2.0h       1
  7    7.5h    0.0h       5     yes       0    0.0h    4.0h    1.5h    2.0h       0
(each row is a day plus the night after it)

        0     3     6     9     12    15    18    21    24
day 1                e....WWWWWWeeWWWWWWWW..PPPPee...zzz|
day 2   zzzzzzzzzzzzze....WWWWWWeeWWWWWWWWWWWWWWWWee.~~z|
day 3   zzzzzzzzzzzzze....WWWWWWeeWWWWWWWWGGGGPPee...GGG|
day 4   GGGzzzzzze.>>>>>>>WWWWWWeeWWWWWWWW......ee...zzz|
day 5   zzPPzzzzzzzzze....WWWWWWeeWWWWWWWWGGGSSSSee..zzz|
day 6   zzzzzzzzzzzzzzzzeGGGG...PPPPPP........ee......zz|
day 7   zzzzzzzzzzzzzzSSSSeGGGGGGGGPPP..SSSS..ee......zz|
day 8   zzzzzzzzzzzzz                                   |
        z sleep  ~ wired  e eat  W work  > flight  G geek  P people  S surf  . living
```

Day 2: a deadline and a double shot at 16:00, so the first hour in bed is `~` (spinning, no dreams). Day 3: a
late-night commit until 01:30. Day 4: a 05:30 flight after three hours of sleep, then a party next door at 01:00.
Day 5: an idea at 17:00, interrupted by a big swell at 18:30. Day 7: dawn patrol at 07:00.

## The rules of life

* **Priorities.** Evenings (and weekends) run `select! { biased; geek, people, surf, bedtime }`. Whatever is
  ready first wins; on a tie, geek beats people beats surf. Sleep 8 hours and you wake up with an idea, which is
  ready the moment work ends, so the next evening starts with geeking and family gets what's left.
* **Big waves preempt geeking.** A geek session is itself a `select!` against a big swell. The swell wins, the
  geek future is dropped mid-`.await`, and a `Drop` guard still writes the hours you put in to the journal.
  Big waves do *not* preempt family time.
* **Sleep** is `while !alarm.rung() { dream().await; spin_loop(); }`. Each `dream()` is a 15-minute slice that the
  alarm can cut short. Every 1.5 hours is one REM cycle; 5 cycles in a night means you wake with an idea.
* **Coffee** after noon keeps you wired for 6 hours (+1.5 per extra shot): `spin_loop()` without `dream()`.
* **Things that go bump in the night ring the same alarm.** A party after bedtime or a big swell before dawn
  calls `alarm.ring(Why::PARTY)` from its own task. You get up, deal with it, and go back to bed. If the real alarm
  goes off while you're out, you go straight into the morning.
* **The earth spins once per alarm.** A `sunrise` task rings `Why::MORNING` at each next day's wake time.

## Questions you're going to ask

**Why `Acquire` on the load and not `Relaxed`?** The alarm also carries *why* it rang (`Why::PARTY`,
`Why::SWELL`, `Why::MORNING`). The ringer writes the reason, then does a `Release` store of the flag. Your `Acquire`
load synchronizes with it, so you're guaranteed to see the reason. With `Relaxed` you could wake up and not know
why. `tests/runtime.rs` checks this a thousand times across real threads.

**Then why is the nightly reset `Relaxed`?** It publishes nothing. Operations on a single atomic within one thread
are already ordered (that's coherence), so the reset can't drift past your own `Acquire` load.

**`spin_loop()` inside async code?** In the [`motto` example](examples/motto.rs) it's the real thing: a hot spin
that blocks the executor and burns a core until a real `std::thread` sets the flag (about 820 million iterations
per simulated night on a laptop). In the library, the spin is a hint between `.await`ed dream slices, so time can
still pass, and in virtual mode the clock can jump.

**How do you write `select!` as `macro_rules!` with no proc-macro?** Each recursive expansion writes the
identifiers `__slot` and `__fut`. Macro hygiene makes every one of them a *different* variable, and the
accumulator carries them to the final rule, which can name all N futures. Losers are dropped before the winning
arm runs. See [`src/macros.rs`](src/macros.rs).

**What's virtual time?** When every task is waiting, the executor jumps the clock to the next timer instead of
sleeping. A century of `rt::sleep` returns instantly (there's a test), and a week of life runs the same way every
time. `--scale` switches to wall-clock time with `park_timeout`, so the real-time mode idles at ~0% CPU.

**Is `select!` random like tokio's?** No. The runtime is deterministic, so every `select!` is biased whether you
write it or not. Writing `biased;` is a statement of values.

**What if everyone's asleep and nobody set an alarm?** `panic!("deadlock: every task is asleep and nobody set an
alarm")`. There's a test for that too.

## Layout

```text
src/rt.rs        the executor: virtual/real-time clock, timers, spawn, a one-permit Bell
src/macros.rs    select! and join!
src/alarm.rs     ALARM: AtomicBool + why it rang, with the orderings explained
src/plan.rs      Day, Event, and the built-in scenarios
src/lib.rs       Life: morning, work, evening (select! biased), night (spin until the alarm)
src/journal.rs   what actually happened: spans, diary, totals, timeline
src/main.rs      the CLI
examples/motto.rs   the motto, verbatim
```

## For a GitHub profile

An animated version of the motto for a profile README, using [readme-typing-svg](https://github.com/DenverCoder1/readme-typing-svg):

```markdown
[![life](https://readme-typing-svg.demolab.com?font=Fira+Code&size=15&pause=2500&multiline=true&width=900&height=60&lines=loop%20%7B%20select%21%20%7B%20biased%3B%20_%20%3D%20geek%28%29%20%3D%3E%20%7B%7D%2C%20_%20%3D%20people%28%29%20%3D%3E%20%7B%7D%2C%20_%20%3D%20surf%28%29%20%3D%3E%20%7B%7D%20%7D%20join%21%28live%28%29%2C%20eat%28%29%29%3B;ALARM.store%28false%2C%20Relaxed%29%3B%20while%20%21ALARM.load%28Acquire%29%20%7B%20dream%28%29%3B%20spin_loop%28%29%3B%20%7D%20%7D)](https://github.com/scottschreckengaust/life)
```

The semicolons inside the code are encoded as `%3B` so they don't split the animation's lines.

## License

MIT
