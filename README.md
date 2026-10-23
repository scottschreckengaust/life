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
spin-life --life me.life.toml --days 7 --speed-up 36000
```

Runs play out on the wall clock 3600 times faster than real life, an hour per second, wherever you run them.
`--speed-up N` sets how much faster (`1` is real time), and `--instant` skips straight to the report.

### Run windows

`--from`, `--until` and `--for` cut a run to any slice, down to the millisecond. Everything before `--from` is
lived instantly (so a window on day 6 doesn't wait five days), the window runs at your pace, and only the window
is reported:

```sh
spin-life --days 1 --speed-up 1 --from 06:30 --for PT2S    # 2 real seconds of an actual morning
spin-life chaos -i --from P2DT18H --until P2DT22H           # day 3, 18:00 to 22:00
```

`WHEN` is a time on day 1 (`06:30:00.5`) or an ISO 8601 duration from midnight of day 1 (`P2DT18H` is day 3 at
18:00). A scenario can set its own window with `from`, and `until` or `for`; the flags override it.

### Machine-readable output

`--format json` prints one JSON document and nothing else, for agents, dashboards and scripts:

```sh
spin-life chaos -i -f json | jq '.totals.hours'
```

| field | what |
|---|---|
| `spin_life` | the version that wrote it |
| `life`, `motto` | whose life (`motto` is `null` if there isn't one) |
| `scenario` | `name`, `about`, `days`, `start` (the weekday of day 1, Monday = 0) |
| `speed_up` | how many times faster than real life it ran, or `null` for `--instant` |
| `window` | `from` and `until` in seconds, or `null` for the whole run |
| `signal` | what a good night's sleep produces (`idea` for the reference life) |
| `activities` | `name`, `glyph` and `pursuit` (built-in or from the life file) for every activity, in report order |
| `days` | per day: `hours` for every activity (the night after counts toward the day), `dreams`, `signal`, `coffee`, `wakeups` |
| `totals` | the same, summed; `signals` counts the days that produced one |
| `spans` | every stretch of time: `day`, `what`, `start`, `end` (seconds) |
| `diary` | every diary line: `at` (seconds), `when` (`d1 06:30`, or `d1 06:30:02.5`), `note` |

`start`, `end`, `at` and `window` are seconds since midnight at the start of day 1, to the millisecond; `hours`
are hours. The output is compact, and the key order is fixed, so the same run always gives the same bytes.

## Write your rules

Every rule maps to a piece of the motto. This is the heart of
[`presets/scott.life.toml`](presets/scott.life.toml):

```toml
[priorities]                       # select! { biased; ... }: first ready wins, ties go left
all = ["geek", "people", "surf"]   # or: workday = [...] and weekend = [...]

[pursuit.geek]                     # _ = geek()
on      = "idea"                   # started by a signal...
session = "PT2H"                   # ...and runs one session

[pursuit.people]                   # _ = people()
when = ["workdays 18:00/20:00", "weekends 12:00/15:00"]   # a window: runs to its end

[pursuit.surf]                     # _ = surf()
lasts    = "PT3H"                  # a swell stays rideable for 3 hours
preempts = ["geek"]                # an urgent swell drops the geek future mid-.await

[sleep]                            # while !ALARM.load(Acquire) { dream(); spin_loop(); }
signal_after = 5                   # 5 REM cycles in one night: wake up with an idea
signal       = "idea"
woken_by     = ["people", "surf"]  # urgent parties and big swells ring the alarm

[caffeine]                         # spin_loop() without dream()
cutoff = "12:00"
wired  = "PT6H"
```

| rule | what it means |
|---|---|
| `priorities` | The `select!` order. Per kind of day: `workday`, `weekend`, or `all`. |
| `when` | Recurring windows: `"18:00/20:00"`, `"workdays 18:00/20:00"`, `"sat,sun 07:00/08:30"`, `"mon-fri ..."`. |
| `on` | A signal that starts the pursuit. Sleep produces one (`sleep.signal`); scenarios can send them. |
| `session`, `lasts` | How long one session runs; how long an opportunity stays open. |
| `preempts` | Pursuits this one interrupts, with an urgent event or when one of its windows opens. |
| `protected` | Nothing interrupts it, whatever `preempts` says. Distinct from priority: it can rank low and still be off-limits. |
| `sleep.woken_by` | Whose urgent events may wake you. A deep sleeper sets `[]`. |
| `[work]`, `[meals]`, `[caffeine]` | The fixed parts of a day. Optional: a life without `[work]` has no job. |

Unknown keys are errors, not silence: `sesion = "PT2H"` fails with `unknown key pursuit.a.sesion`.

Times, durations and windows are [ISO 8601](https://en.wikipedia.org/wiki/ISO_8601), to the millisecond, and
nothing else:

| kind | examples |
|---|---|
| time of day | `"06:30"`, `"06:30:15"`, `"06:30:02.5"`, `"24:00"` |
| duration | `"PT30M"`, `"PT1H30M"`, `"PT2.5S"`, `"P1DT6H"` |
| window | `"18:00/20:00"`, `"22:30/06:30"` (past midnight), `"09:00/PT8H"` |

An older file with `"1h30m"` or `"18:00-20:00"` fails `spin-life check` with the ISO version to use instead.

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

`from`, and `until` or `for`, give a scenario its own [run window](#run-windows): `from = "06:30"`,
`for = "PT1S"` is one second of a morning.

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
(there are tests for both). Without `--instant`, it runs on the wall clock with `park_timeout`, at roughly 0% CPU.

**`spin_loop()` inside async code?** In the [`motto` example](examples/motto.rs) it's the real thing: a hot spin
that blocks the executor and burns a core until a real `std::thread` sets the flag. In the library, the spin is a
hint between `.await`ed dream slices, so time can still pass.

**Did the rules engine change the reference life?** No. All eleven scenarios produce the same numbers and
timelines to the original hard-coded engine, and the same tests pass against the preset.

## Layout

```text
src/rules.rs     the life file: pursuits, priorities, windows, signals, preemption, sleep, scenarios
src/toml.rs      the TOML subset the life file needs, with line-numbered errors
src/lib.rs       Life: morning, work, evening (biased select over your priorities), night (spin until the alarm)
src/rt.rs        the executor: virtual/real-time clock, timers, spawn, select_biased, a one-permit Bell
src/macros.rs    select! and join!
src/alarm.rs     ALARM: AtomicBool + why it rang, with the orderings explained
src/journal.rs   what happened: spans, diary, totals, timeline, JSON
src/json.rs      just enough JSON writing for --format json
src/iso.rs       ISO 8601 times, durations and windows: the only formats a life file takes
src/main.rs      the CLI
CHANGELOG.md     every release, written by release-plz from PR titles
presets/         scott (the motto), parent, agent
examples/motto.rs   the motto, verbatim
```

## Verify what you download

Every release asset comes with a `.sha256` file and **signed build provenance**: a
[GitHub artifact attestation](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
(Sigstore-signed, SLSA provenance) that ties the file to this repository, the release workflow and the exact
commit it was built from.

```sh
gh release download v0.1.0 -R scottschreckengaust/life -p 'spin-life-x86_64-unknown-linux-musl.*'
sha256sum -c spin-life-x86_64-unknown-linux-musl.sha256
gh attestation verify spin-life-x86_64-unknown-linux-musl.tar.gz -R scottschreckengaust/life
```

For the crate itself, Cargo already checks integrity for you: the crates.io index records each version's SHA-256,
Cargo verifies every download against it, and `Cargo.lock` pins it. The release also attaches the `.crate` with
its own provenance, and the release workflow proves it is byte-for-byte the file crates.io serves, so you can run
`gh attestation verify` on a `.crate` downloaded from crates.io too.

## Releasing

Nobody bumps versions by hand. Releases come from the PR titles:

1. **Title PRs as [Conventional Commits](https://www.conventionalcommits.org/)** (`feat: …`, `fix: …`,
   `docs: …`, `feat!: …` for breaking changes). PRs are squash-merged, so the title becomes the commit, and
   the `pr-title` check enforces it.
2. **release-plz keeps one release PR open** ([`release-plz.yml`](.github/workflows/release-plz.yml)). Every
   merge to `main` updates it: the next version (from the commits, plus `cargo-semver-checks` on the public
   API) and the new `CHANGELOG.md` section. Only `feat`, `fix` and breaking changes open one; `docs`, `ci`,
   `chore` and the like ride along in the next release.
3. **Merging the release PR cuts the release.** Merge it yourself whenever you like, or let
   [`release-cut.yml`](.github/workflows/release-cut.yml) merge it once it has sat unchanged for **6 hours** with
   green checks, so a burst of merges becomes one release. To release right now, run release-cut with
   `quiet_hours = 0`.
4. **[`release.yml`](.github/workflows/release.yml) ships it**, each stage gating the next:

| stage | what it proves |
|---|---|
| **plan** | the version has no published GitHub Release yet (a half-finished run is resumed, not skipped) |
| **draft** | tag `vX.Y.Z` at the release commit, then a *draft* release on it, with the `CHANGELOG.md` section as its notes |
| **build** | static (musl) binaries for Linux x86_64 and arm64, plus macOS arm64 and Windows, each with a `.sha256` and provenance, uploaded to the draft |
| **crate** | the `.crate` crates.io will get, with a `.sha256` and provenance, uploaded to the draft |
| **test** | on each OS, the draft's *downloaded* assets pass checksum and provenance checks, then the smoke test and the golden outputs in `tests/golden` |
| **publish** | `cargo publish` via trusted publishing, after checking that the bytes about to be uploaded are the attested ones (skipped if that version is already on crates.io) |
| **verify** | the crates.io index checksum and the served `.crate` match the attested bytes, provenance verifies on that file, and `cargo install` from crates.io runs |
| **release** | the draft flips to published, then release-plz runs again for anything merged in the meantime |

**Merging during a release is fine.** Every stage builds from the release commit, and its tag exists from the
start, so later merges can't get into the release or break it. They simply go into the next release PR.

**Re-running is safe.** "Re-run all jobs" picks up where a failed run stopped: the tag, the draft and a
crates.io upload are reused, never duplicated.

**Release candidates:** set `version = "0.2.0-rc.1"` in a PR to start them. From then on release-plz bumps
only the last number (`rc.2`, `rc.3`, …), and each ships as a GitHub *prerelease*, never "latest". `cargo install
spin-life` keeps giving the latest stable version unless someone asks for `--version 0.2.0-rc.2`. Set
`version = "0.2.0"` in a PR to finish.

**Rehearsals:** push to any `release/**` branch to run the whole pipeline without releasing. It creates a real
`rehearsal/…` tag (proving tag creation works), a throwaway prerelease draft and `cargo publish --dry-run`, then
deletes the tag and draft.

Rehearse before merging anything that changes the release machinery. CI can't catch a broken release; only
running the pipeline does. A PR touching these files gets a reminder comment
([`rehearse-reminder.yml`](.github/workflows/rehearse-reminder.yml)):

- [ ] `release.yml`, `release-cut.yml` or `release-plz.yml`, including Dependabot bumps of the actions they pin
- [ ] `.github/release/smoke.sh` or the golden files it compares against
- [ ] build targets, or `[package.metadata.binstall]` in `Cargo.toml`
- [ ] `release-plz.toml`

Rehearse with `git push origin HEAD:release/<name>`, watch `release.yml` go green, then delete the branch. Two things
a rehearsal can't check: the release-tagger App and its key (run **release-cut** with `check_tagger` ticked), and
crates.io trusted publishing (only a real release proves it; if it fails, the release stops before publishing and
the draft waits for a re-run).

It's assembled from off-the-shelf actions, pinned to commit SHAs:
[`release-plz`](https://github.com/release-plz/action) (release PR, version, changelog),
[`amannn/action-semantic-pull-request`](https://github.com/amannn/action-semantic-pull-request) (PR titles),
[`softprops/action-gh-release`](https://github.com/softprops/action-gh-release) (draft, publish),
[`taiki-e/upload-rust-binary-action`](https://github.com/taiki-e/upload-rust-binary-action) (cross-platform build,
archive, checksums), [`actions/attest`](https://github.com/actions/attest) (provenance),
[`rust-lang/crates-io-auth-action`](https://github.com/rust-lang/crates-io-auth-action) (trusted publishing)
and the `gh` CLI. Everything runs as `github-actions[bot]`.

### One-time setup

1. **crates.io trusted publishing** (done): owner `scottschreckengaust`, repository `life`, workflow
   `release.yml`, environment `release`. Delete any `CARGO_REGISTRY_TOKEN` secret and revoke the token.
2. **Let the bot open PRs:** Settings → Actions → General → Workflow permissions → tick *Allow GitHub Actions
   to create and approve pull requests*. Everything except creating the `v*` tag runs as `github-actions[bot]`
   with the built-in token. Pushes and PRs made with that token don't start workflows by themselves, so
   release-plz starts CI on its PR, and release-cut tags the merge commit and starts `release.yml` on that tag.
3. **The `release` environment** (Settings → Environments → `release`): deployments only from branch `main` and
   tags `v*`. It is the one place both powers live: crates.io only trusts jobs in `release`, and it holds the
   release-tagger App's key. Rehearsals use no environment, so they can't reach either.
4. **The release-tagger App**, the only identity allowed to create `v*` tags. A GitHub App with no webhook and
   one permission, *Contents: read and write*, installed on this repository only. Its Client ID is the
   `release` environment variable `TAGGER_CLIENT_ID`, and a private key is the environment secret
   `TAGGER_PRIVATE_KEY`. The workflows mint a one-hour token from it only to create the tag; everything else is
   `github-actions[bot]`. Check the wiring any time: run *release-cut* from `main` with **check_tagger** ticked.
5. **Lock what's released:**
   - *Immutable releases* (Settings → General → Releases): published assets and tags can't change.
   - Tag ruleset **release tag creation**: `v*`, *Restrict creations*, bypass: the release-tagger App.
   - Tag ruleset **release tags**: `v*`, *Restrict updates* and *Restrict deletions*, bypass: Repository admin
     (so an admin can remove a stray tag). Rehearsal tags (`rehearsal/…`) are outside `v*`.

## For a GitHub profile

```markdown
[![life](https://readme-typing-svg.demolab.com?font=Fira+Code&size=15&pause=2500&multiline=true&width=900&height=60&lines=loop%20%7B%20select%21%20%7B%20biased%3B%20_%20%3D%20geek%28%29%20%3D%3E%20%7B%7D%2C%20_%20%3D%20people%28%29%20%3D%3E%20%7B%7D%2C%20_%20%3D%20surf%28%29%20%3D%3E%20%7B%7D%20%7D%20join%21%28live%28%29%2C%20eat%28%29%29%3B;ALARM.store%28false%2C%20Relaxed%29%3B%20while%20%21ALARM.load%28Acquire%29%20%7B%20dream%28%29%3B%20spin_loop%28%29%3B%20%7D%20%7D)](https://github.com/scottschreckengaust/life)
```

## License

MIT
