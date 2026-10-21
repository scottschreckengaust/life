#!/usr/bin/env bash
# Smoke-test a spin-life binary as a user would get it.
#   smoke.sh <binary> <expected-version> <golden-dir>
set -euo pipefail
bin=$1 version=$2 golden=$3
work=$(mktemp -d)

echo "::group::version"
got=$("$bin" --version | tr -d '\r')
[[ "$got" == "spin-life $version" ]] || { echo "::error::expected 'spin-life $version', got '$got'"; exit 1; }
echo "$got"
echo "::endgroup::"

echo "::group::every preset: init, check, and every scenario"
for preset in $("$bin" presets | awk '{print $1}'); do
  "$bin" init --preset "$preset" > "$work/$preset.life.toml"
  "$bin" check "$work/$preset.life.toml"
  for scenario in $("$bin" list --preset "$preset" | awk '{print $1}'); do
    "$bin" run "$scenario" --preset "$preset" --instant --quiet > /dev/null
    echo "  ok  $preset/$scenario"
  done
done
echo "::endgroup::"

echo "::group::golden output"
diff --strip-trailing-cr "$golden/scott-chaos.txt" <("$bin" chaos --instant --quiet)
diff --strip-trailing-cr "$golden/agent-incident.txt" <("$bin" incident -p agent -i -q)
diff --strip-trailing-cr "$golden/parent-teething.txt" <("$bin" teething -p parent -i -q)
echo "matches tests/golden"
echo "::endgroup::"

echo "::group::bad input is rejected"
printf '[pursuit.a]\nsesion = "2h"\n' > "$work/bad.toml"
if "$bin" check "$work/bad.toml"; then echo "::error::a typo was accepted"; exit 1; fi
echo "::endgroup::"

echo "::group::real time (hours as seconds, scaled down)"
"$bin" run --days 1 --scale 0.01 --quiet | tail -1
echo "::endgroup::"
