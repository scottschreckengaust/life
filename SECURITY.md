# Security policy

## Reporting a vulnerability

Please report it privately through
[**Report a vulnerability**](https://github.com/scottschreckengaust/life/security/advisories/new) on this
repository's Security tab, not in a public issue or pull request. Include what you found, how to reproduce it, and
which version (`spin-life --version`) or commit it affects.

You'll get an acknowledgement within a week. Fixes go out as a normal release, with a GitHub security advisory
(and a CVE where it warrants one) once a fixed version is on crates.io. You'll be credited unless you'd rather not
be.

## Supported versions

Only the latest release gets fixes. spin-life is pre-1.0, so a fix ships as the next `0.x` release rather than a
backport.

## What's in scope

- The `spin-life` crate and binary: the TOML parser, the rules engine, the runtime and the CLI.
- The release pipeline: anything that could get code into a release, onto crates.io or into a `v*` tag without
  going through the reviewed `main` branch, or make a release's artifacts differ from what its provenance attests.

The crate has no dependencies. The library forbids `unsafe` (`#![forbid(unsafe_code)]`). The binary has one
`unsafe` block, which restores the default SIGPIPE handling on Unix so `spin-life | head` exits cleanly.

## Verifying a release

Every release's binaries and `.crate` come with a `.sha256` and signed build provenance:

```sh
gh attestation verify spin-life-x86_64-unknown-linux-musl.tar.gz -R scottschreckengaust/life
```

Releases are immutable once published, and only the release workflow can create `v*` tags.
