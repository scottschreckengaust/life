# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.3](https://github.com/scottschreckengaust/life/compare/v0.2.2...v0.2.3) - 2026-10-30

### Fixed

- an absurdly long session or opportunity no longer hangs a run: a run never records past its own end, so durations of any finite length work, and one too large to be a number is refused ([#59](https://github.com/scottschreckengaust/life/pull/59), [#62](https://github.com/scottschreckengaust/life/pull/62))
- `--speed-up` so large that an hour takes under a nanosecond no longer hangs; it says to use `--instant` ([#62](https://github.com/scottschreckengaust/life/pull/62))
- a life file nesting arrays or inline tables more than 64 deep is an error, not a stack-overflow crash ([#62](https://github.com/scottschreckengaust/life/pull/62))
- `sleep.signal_after` beyond 4294967295 or below 0 is an error, instead of wrapping round to 0 ([#62](https://github.com/scottschreckengaust/life/pull/62))
- `--days` outside 1 to 366 is an error, like a scenario's `days`, instead of being clamped; a daily recurring window may be at most `PT24H` ([#62](https://github.com/scottschreckengaust/life/pull/62))

### Other

- cargo-deny, so a dependency can't land without changing deny.toml ([#58](https://github.com/scottschreckengaust/life/pull/58))
- the README's fork notes match the workflows, and the ruleset list has signed commits ([#53](https://github.com/scottschreckengaust/life/pull/53))
- OpenSSF Scorecard, and zizmor findings in code scanning ([#49](https://github.com/scottschreckengaust/life/pull/49))
- SECURITY.md, with private vulnerability reporting ([#48](https://github.com/scottschreckengaust/life/pull/48))
- the README describes the main rulesets and the tag rulesets as configured ([#46](https://github.com/scottschreckengaust/life/pull/46))
- the release-bot App opens and merges the release PR ([#45](https://github.com/scottschreckengaust/life/pull/45))
- show the motto in the README's rules example ([#43](https://github.com/scottschreckengaust/life/pull/43))

## [0.2.2](https://github.com/scottschreckengaust/life/compare/v0.2.1...v0.2.2) - 2026-10-29

### Fixed

- a rule that keeps you up past your next alarm (`up_late = "06:30"` or later) no longer hangs the run; the alarm rings and the night is skipped ([#30](https://github.com/scottschreckengaust/life/pull/30))
- life-file errors name the line the problem is on, not the next one (unclosed `[header`, unterminated string, missing `=`, an inline table running off its line) ([#30](https://github.com/scottschreckengaust/life/pull/30))

### Other

- mutation testing with cargo-mutants, no surviving mutants ([#33](https://github.com/scottschreckengaust/life/pull/33))
- 100% line coverage, enforced in CI ([#30](https://github.com/scottschreckengaust/life/pull/30))
- harden against fork PRs (zizmor, CODEOWNERS)
- refuse to cut a version that already exists anywhere

## [0.2.1](https://github.com/scottschreckengaust/life/compare/v0.2.0...v0.2.1) - 2026-10-23

### Fixed

- stop quietly when stdout closes, and say "1 spin of the earth" ([#20](https://github.com/scottschreckengaust/life/pull/20))

### Other

- remind PRs that touch the release machinery to rehearse ([#21](https://github.com/scottschreckengaust/life/pull/21))

## [0.2.0](https://github.com/scottschreckengaust/life/compare/v0.1.2...v0.2.0) - 2026-10-23

### Added

- [**breaking**] ISO 8601 times, a millisecond clock and --from/--until/--for run windows ([#16](https://github.com/scottschreckengaust/life/pull/16))
- [**breaking**] --speed-up replaces --scale; add --format json

### Fixed

- the JSON golden ignores the version, so release bumps don't break it ([#15](https://github.com/scottschreckengaust/life/pull/15))

### Other

- --speed-up runs on the wall clock and lives the same day as --instant
- only the release-tagger App creates v* tags, from the release environment ([#10](https://github.com/scottschreckengaust/life/pull/10))

## [0.1.2](https://github.com/scottschreckengaust/life/compare/v0.1.1...v0.1.2) - 2026-10-21

### Added

- say how long a real-time run will take ([#6](https://github.com/scottschreckengaust/life/pull/6))

### Other

- run the release train as github-actions[bot], no App or token ([#7](https://github.com/scottschreckengaust/life/pull/7))
- Dependabot for pinned actions; release candidates ship as prereleases ([#5](https://github.com/scottschreckengaust/life/pull/5))
- release PRs from conventional commits, cut after 6 quiet hours

## [0.1.1](https://github.com/scottschreckengaust/life/compare/v0.1.0...v0.1.1) - 2026-10-21

### Fixed

- Reports, legends and `spin-life check` list pursuits in `[priorities]` order instead of alphabetically ([#3](https://github.com/scottschreckengaust/life/pull/3))

## [0.1.0](https://github.com/scottschreckengaust/life/releases/tag/v0.1.0) - 2026-10-21

### Added

- Rules engine: describe a life in a `.life.toml` file with pursuits, per-day-kind priorities, `when` windows, `on` signals, `preempts`, `protected`, `sleep.woken_by` and caffeine, and simulate it ([#1](https://github.com/scottschreckengaust/life/pull/1))
- Presets: `scott` (the motto, eleven scenarios), `parent` and `agent`
- CLI: `run`, `list`, `presets`, `init`, `check`; `--instant` virtual time or `--scale` real time
- Zero-dependency async runtime with virtual time, `select!` and `join!` as `macro_rules!`, `rt::select_biased`, and an `AtomicBool` alarm with `Acquire`/`Release`
- Zero-dependency TOML-subset parser with line-numbered errors
- Release pipeline: draft, build, sha256 and build provenance, test the downloaded assets, publish with crates.io trusted publishing, verify, then release ([#2](https://github.com/scottschreckengaust/life/pull/2))
