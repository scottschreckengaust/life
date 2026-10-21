# spin-life

[![ci](https://github.com/scottschreckengaust/life/actions/workflows/ci.yml/badge.svg)](https://github.com/scottschreckengaust/life/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/spin-life.svg)](https://crates.io/crates/spin-life)
[![docs.rs](https://img.shields.io/docsrs/spin-life)](https://docs.rs/spin-life)
![dependencies: 0](https://img.shields.io/badge/dependencies-0-brightgreen)
![unsafe: forbidden](https://img.shields.io/badge/unsafe-forbidden-success)

```rust
thread::spawn(|| loop { thread::sleep(EARTH_SPIN); ALARM.store(true, Release) });
loop { select! { biased; _ = geek() => {}, _ = people() => {}, _ = surf() => {} } join!(live(), eat()); ALARM.store(false, Relaxed); while !ALARM.load(Acquire) { dream(); spin_loop(); } }
```

*If not geeking out: try being with people, catch surfing: finally living, eating, sleeping.*

That line is one person's rules. **spin-life runs anyone's.** Write what you pursue, what comes first, who may
interrupt whom and what may wake you in a `.life.toml` file, then simulate a week of it in microseconds, or at
one hour per second. Not everyone codes, and not everyone puts people before the hobby. The engine doesn't care:
it only knows priorities, windows, signals, preemption and sleep.

Under the hood: a **zero-dependency** async runtime with virtual time, `select!` and `join!` written as
`macro_rules!`, a real `AtomicBool` alarm with `Acquire`/`Release`, and a TOML parser, all in about 2,500 lines.

## Install

```sh
cargo install spin-life            # from crates.io
cargo binstall spin-life           # prebuilt binary, no compiling
```

Or as a library: `cargo add spin-life`.

## Run

```sh
spin-life                          # the reference life, a week, 1 simulated hour = 1 real second
spin-life chaos --instant          # a scenario, on virtual time
spin-life presets                  # scott (the motto), parent, agent
spin-life list --preset parent     # a life's scenarios
spin-life teething -p parent -i -q # someone else's week, report only

spin-life init > me.life.toml      # start your own from the reference life
spin-life check me.life.toml       # catch typos and bad times
spin-life --life me.life.toml --days 7 --scale 0.1
```

## Write your rules

Every rule maps to a piece of the motto. This is the heart of
[`presets/scott.life.toml`](presets/scott.life.toml):

```toml
[priorities]                       # select! { biased; ... }: first ready wins, ties go left
all = ["geek", "people", "surf"]   # or: workday = [...] and weekend = [...]

[pursuit.geek]                     # _ = geek()
on      = "idea"                   # started by a signal...
session = "2h"                     # ...and runs one session

[pursuit.people]                   # _ = people()
when = ["workdays 18:00-20:00", "weekends 12:00-15:00"]   # a window: runs to its end

[pursuit.surf]                     # _ = surf()
lasts    = "3h"                    # a swell stays rideable for 3 hours
preempts = ["geek"]                # an urgent swell drops the geek future mid-.await

[sleep]                            # while !ALARM.load(Acquire) { dream(); spin_loop(); }
signal_after = 5                   # 5 REM cycles in one night: wake up with an idea
signal       = "idea"
woken_by     = ["people", "surf"]  # urgent parties and big swells ring the alarm

[caffeine]                         # spin_loop() without dream()
cutoff = "12:00"
wired  = "6h"
```

| rule | what it means |
|---|---|
| `priorities` | The `select!` order. Per kind of day: `workday`, `weekend`, or `all`. |
| `when` | Recurring windows: `"18:00-20:00"`, `"workdays 18:00-20:00"`, `"sat,sun 07:00-08:30"`, `"mon-fri ..."`. |
| `on` | A signal that starts the pursuit. Sleep produces one (`sleep.signal`); scenarios can send them. |
| `session`, `lasts` | How long one session runs; how long an opportunity stays open. |
| `preempts` | Pursuits this one interrupts, with an urgent event or when one of its windows opens. |
| `protected` | Nothing interrupts it, whatever `preempts` says. Distinct from priority: it can rank low and still be off-limits. |
| `sleep.woken_by` | Whose urgent events may wake you. A deep sleeper sets `[]`. |
| `[work]`, `[meals]`, `[caffeine]` | The fixed parts of a day. Optional: a life without `[work]` has no job. |

Unknown keys are errors, not silence: `sesion = "2h"` fails with `unknown key pursuit.a.sesion`.

### Scenarios: what happens this week

```toml
[scenario.dawn-patrol]
about  = "a big swell at 05:30 rings the alarm early"
days   = 2
start  = "mon"
events = [
  { day = 2, at = "05:30", pursuit = "surf", urgent = true, note = "🌊 dawn patrol" },
]
```

Events: `pursuit` (with optional `until`, `urgent`, `note`), `signal`, `coffee = <shots>`, `work_until`, `flight`
(and `lands`), `up_late` with `doing`, and `skip` (no windows for that pursuit today).

### Presets

| preset | the life |
|---|---|
| `scott` | The motto. Geek, then people, then surf. Eleven scenarios: happy path, a week, coffee, party, special event, dawn patrol, big wave, late night, early flight, deadline, chaos. |
| `parent` | Kids first and protected. Their time interrupts guitar. Running moves to the top on weekends. A teething week. |
| `agent` | An AI agent: users preempt research and jobs, pages wake it, nightly compaction surfaces insights. |

The agent preset is the serious one. An agent's operating policy has the same shape as a life (priorities,
interrupts, a maintenance window, rate limits), so you can simulate a week of load on virtual time and see what
starves before you deploy it.

## What it looks like

```text
$ spin-life chaos --instant --quiet

day    sleep    wired   dreams     idea   coffee     work     geek   people     surf  wakeups
  1     8.0h     0.0h        5      yes        1     7.0h     0.0h     2.0h     0.0h        0
  2     7.0h     1.0h        4        -        2    11.0h     0.0h     0.0h     0.0h        0
  3     3.0h     0.0h        2        -        0     7.0h     5.0h     1.0h     0.0h        0
  4     7.0h     0.0h        4        -        0     7.0h     0.0h     1.0h     0.0h        1
  5     9.5h     0.0h        6      yes        0     7.0h     1.5h     0.0h     2.0h        0
  6     8.0h     0.0h        5      yes        0     0.0h     2.0h     3.0h     2.0h        1
  7     7.5h     0.0h        5      yes        0     0.0h     4.0h     1.5h     2.0h        0
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
        z sleep  ~ wired  e eat  W work  > travel  . living  G geek  P people  S surf
```

Day 2: a deadline and a double shot at 16:00, so the first hour in bed is `~` (spinning, no dreams). Day 3: a
late-night commit. Day 4: a 05:30 flight after three hours of sleep, then a party next door at 01:00. Day 5: an
idea at 17:00, cut off by a big swell at 18:30:

```text
d5 17:00  🧠 geeking out
d5 18:30  ⚡ surf preempts geek: dropped its future mid-.await (the Drop guard kept the hours)
d5 18:30  🌊 BIG swell
```

## Questions you're going to ask

**Why `Acquire` on the load and not `Relaxed`?** The alarm also carries *why* it rang (`Why::MORNING`,
`Why::URGENT`). The ringer writes the reason, then does a `Release` store of the flag. Your `Acquire` load
synchronizes with it, so you're guaranteed to see the reason. With `Relaxed` you could wake up and not know why.
`tests/runtime.rs` checks this a thousand times across real threads.

**Then why is the nightly reset `Relaxed`?** It publishes nothing. Operations on a single atomic within one thread
are already ordered (that's coherence), so the reset can't drift past your own `Acquire` load.

**How does a dynamic priority list become `select! { biased; }`?** `select!` needs its arms at compile time, and
your priorities live in a file. So the evening uses `rt::select_biased(Vec<BoxFuture>)`: the same "poll in order,
first ready wins," over a list built at runtime. The static `select!` still handles preemption and the alarm.

**How do you write `select!` as `macro_rules!` with no proc-macro?** Each recursive expansion writes the
identifiers `__slot` and `__fut`. Macro hygiene makes every one of them a *different* variable, and the
accumulator carries them to the final rule, which can name all N futures. Losers are dropped before the winning
arm runs. See [`src/macros.rs`](src/macros.rs).

**How can preemption be cancel-safe?** Dropping a future mid-`.await` would lose the hours it already spent. Every
activity holds a `Drop` guard that writes its span to the journal when it ends *or is dropped*.

**What's virtual time?** When every task is waiting, the executor jumps the clock to the next timer instead of
sleeping. A century of `rt::sleep` returns instantly, and every scenario produces the same journal every time
(there are tests for both). `--scale` switches to wall-clock time with `park_timeout`, at roughly 0% CPU.

**`spin_loop()` inside async code?** In the [`motto` example](examples/motto.rs) it's the real thing: a hot spin
that blocks the executor and burns a core until a real `std::thread` sets the flag. In the library, the spin is a
hint between `.await`ed dream slices, so time can still pass.

**Did the rules engine change the reference life?** No. All eleven scenarios produce the same numbers and
timelines to the hard-coded v0.1 engine, and the same tests pass against the preset.

## Layout

```text
src/rules.rs     the life file: pursuits, priorities, windows, signals, preemption, sleep, scenarios
src/toml.rs      the TOML subset the life file needs, with line-numbered errors
src/lib.rs       Life: morning, work, evening (biased select over your priorities), night (spin until the alarm)
src/rt.rs        the executor: virtual/real-time clock, timers, spawn, select_biased, a one-permit Bell
src/macros.rs    select! and join!
src/alarm.rs     ALARM: AtomicBool + why it rang, with the orderings explained
src/journal.rs   what happened: spans, diary, totals, timeline
src/main.rs      the CLI
presets/         scott (the motto), parent, agent
examples/motto.rs   the motto, verbatim
```

## Releasing

`release.yml` publishes to crates.io with [trusted publishing](https://crates.io/docs/trusted-publishing): no API
token is stored in the repo. GitHub mints a short-lived OIDC token, crates.io trades it for a publish token, and
that token is revoked when the job ends.

1. **First release, by hand** (crates.io requires the crate to exist): `cargo login`, then `cargo publish`.
2. **On crates.io**: open the crate's Settings, find Trusted Publishing, and add GitHub with owner
   `scottschreckengaust`, repository `life`, workflow `release.yml`, environment `release`.
3. **On GitHub**: Settings → Environments → create `release` (optionally limited to `v*` tags, or requiring a
   reviewer).
4. **Every release after that**: bump `version` in `Cargo.toml`, then `git tag v0.2.1 && git push --tags`. The
   workflow tests, publishes, creates the GitHub Release and attaches binaries for Linux (x86_64, arm64), macOS
   (arm64) and Windows.

## For a GitHub profile

```markdown
[![life](https://readme-typing-svg.demolab.com?font=Fira+Code&size=15&pause=2500&multiline=true&width=900&height=60&lines=loop%20%7B%20select%21%20%7B%20biased%3B%20_%20%3D%20geek%28%29%20%3D%3E%20%7B%7D%2C%20_%20%3D%20people%28%29%20%3D%3E%20%7B%7D%2C%20_%20%3D%20surf%28%29%20%3D%3E%20%7B%7D%20%7D%20join%21%28live%28%29%2C%20eat%28%29%29%3B;ALARM.store%28false%2C%20Relaxed%29%3B%20while%20%21ALARM.load%28Acquire%29%20%7B%20dream%28%29%3B%20spin_loop%28%29%3B%20%7D%20%7D)](https://github.com/scottschreckengaust/life)
```

## License

MIT
