#!/usr/bin/env bash
# Install Rust with rustup, which every GitHub runner has (#97).
#
#   rust.sh [--target <triple>]...                  the toolchain in rust-toolchain.toml,
#                                                   with its components
#   TOOLCHAIN=<version> rust.sh [--target ...]      another one (msrv, fuzz), used with
#                                                   `cargo +<version>`
set -euo pipefail
# Runs on macOS too, whose bash is 3.2: no `mapfile`, and no empty arrays under `set -u`.
file=$(git rev-parse --show-toplevel)/rust-toolchain.toml
args=(--profile minimal --no-self-update)
if [[ -n ${TOOLCHAIN:-} ]]; then
  toolchain=$TOOLCHAIN
else
  toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$file")
  while IFS= read -r c; do
    [[ -n $c ]] && args+=(--component "$c")
  done < <(sed -n 's/^components *= *\[\(.*\)\]/\1/p' "$file" | grep -o '"[^"]*"' | tr -d '"')
fi
[[ -n $toolchain ]] || { echo "::error::no toolchain: set TOOLCHAIN, or channel in $file"; exit 1; }
rustup toolchain install "$toolchain" "${args[@]}" "$@"
rustc "+$toolchain" --version
