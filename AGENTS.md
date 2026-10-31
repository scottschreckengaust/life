# AGENTS.md

Rules for anyone changing this repository, people and coding agents alike. They're written
as [EARS](https://alistairmavin.com/ears/) requirements: each says when it applies and what
shall happen. CI enforces the ones marked **(CI)**; the rest are on you.

## Pull requests and the changelog

PRs are squash-merged, so a PR's title becomes its commit, and release-plz writes one
changelog line per commit from its title.

- The PR title **shall** be a [Conventional Commit](https://www.conventionalcommits.org/):
  `feat:`, `fix:`, `docs:`, `ci:`, `refactor:`, `test:`, `chore:` and so on, with `!` for a
  breaking change. **(CI: `conventional`)**
- When a PR changes behaviour a user can see, its title **shall** describe that change the
  way the changelog should say it: what changed for the user, not how the code changed.
- When a PR changes `src/`, its type **shall not** be `test`, `ci`, `chore` or `build`.
  Those never release, so the change would ship nowhere. **(CI: `conventional`)**
- When a PR breaks the public API, its title **shall** carry `!` (`feat!:`, `fix!:`).
  **(CI: `semver`)**
- When a PR reverses or reworks a change that hasn't been released yet, its title **shall**
  describe the final behaviour, and it **shall** correct the earlier PR's changelog line in
  `release-plz.toml`'s `[changelog]`: a `{ sha = "<its commit>", skip = true }` parser to
  drop it, or a `commit_preprocessors` rewrite to reword it. The file says how. Its
  description **shall** say which earlier PR it supersedes. **(CI: `changelog-config`
  proves past releases are unchanged, and previews the next one in its summary)**
- When a merged PR's changelog line reads wrong, the fix **shall** be the same kind of
  correction in `release-plz.toml`, in its own PR.
- The release PR **shall not** be edited by hand. Every merge to `main` regenerates it and
  discards hand edits.

## Code

- Every line in `src/` **shall** run under the tests. **(CI: `coverage`, 100% of lines)**
- When a PR changes code, every mutant of the changed lines **shall** make a test fail.
  **(CI: `mutants`)**
- The published crate **shall** have no dependencies. **(CI: `deny`)**
- When a bug is found (by fuzzing, by a report or by probing), a regression test **shall**
  reproduce it before the fix lands, and a fuzzing input that found it **shall** be kept in
  `fuzz/seeds/`.
- The code **shall** pass `cargo clippy --all-targets -- -D warnings` with the lints in
  `Cargo.toml`'s `[lints.clippy]`. A lint, once on, **shall not** be turned off again
  (#57). **(CI: `test`)**
- When input is parsed, every value **shall** be bounded or proven safe: no input may hang a
  run, overflow the stack, or be silently clamped or wrapped.

## Workflows and the release

- Every workflow **shall** start from `permissions: {}`, each job asking for only what it
  uses; the one exception is `default-token-check.yml`. **(CI: `check-workflows.sh`)**
- Every run step **shall** run under `bash -eo pipefail`. **(CI: `check-workflows.sh`)**
- Every action **shall** be pinned to a full commit SHA, with the release in a comment, and a
  new pin **shall** be at least 7 days old. **(repository setting, Dependabot cooldown)**
- When a PR adds an action, it **shall** also list every action that action uses itself, so
  the allowed-actions setting can be updated before it merges.
- When a PR changes the release machinery (`release.yml`, `release-cut.yml`,
  `release-plz.yml`, `.github/release/`, `release-plz.toml`, `Cargo.toml`) or an action pin,
  it **shall** be rehearsed on a `release/**` branch before it merges. **(reminder:
  `rehearse-reminder`)**
- Code scanning findings **shall** be fixed, or dismissed with a written reason by the
  maintainer. **(ruleset: code scanning, all severities)**

## Agents

- An agent **shall not** approve deployments, pull requests or code-scanning dismissals on
  the maintainer's behalf; those are the maintainer's to give.
- An agent **shall not** use, print or store secrets or private keys, even when given one.
- When something needs a setting changed, an agent **shall** say exactly which setting, where
  it is and what to set it to, and then prove it by behaviour where it can
  (`settings-check`, `default-token-check`, `bot-check`).
