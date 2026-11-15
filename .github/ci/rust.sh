#!/usr/bin/env bash
# Install Rust with rustup, which every GitHub runner has (#97).
#
#   rust.sh [--target <triple>]...                  the toolchain in rust-toolchain.toml,
#                                                   with its components
#   TOOLCHAIN=<version> rust.sh [--target ...]      another one (msrv, fuzz), used with
#                                                   `cargo +<version>`
set -euo pipefail
file=$(git rev-parse --show-toplevel)/rust-toolchain.toml
components=()
if [[ -n ${TOOLCHAIN:-} ]]; then
  toolchain=$TOOLCHAIN
else
  toolchain=$(sed -n 's/^channel *= *"\(.*\)"/\1/p' "$file")
  mapfile -t components < <(sed -n 's/^components *= *\[\(.*\)\]/\1/p' "$file" | grep -o '"[^"]*"' | tr -d '"')
fi
[[ -n $toolchain ]] || { echo "::error::no toolchain: set TOOLCHAIN, or channel in $file"; exit 1; }
args=(--profile minimal)
for c in "${components[@]}"; do args+=(--component "$c"); done
rustup toolchain install "$toolchain" "${args[@]}" --no-self-update "$@"
rustc "+$toolchain" --version
