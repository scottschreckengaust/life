# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
