#!/usr/bin/env bash
# Tests for plan.sh, with cargo, gh, git and version-is-new.sh replaced by stubs.
# Run: bash .github/release/test-plan.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)

pass=0 fail=0
# run_plan: env VERSION RELEASED(true|draft|none|error) TAG_AT(sha|""|error) CRATES(0|1|2) REF; prints outputs and messages
run_plan() {
  local dir
  dir=$(mktemp -d)
  mkdir -p "$dir/bin" "$dir/.github/release"
  cp "$here/plan.sh" "$dir/.github/release/plan.sh"
  cat >"$dir/bin/cargo" <<STUB
#!/usr/bin/env bash
echo '{"packages":[{"name":"spin-life","version":"$VERSION"}]}'
STUB
  cat >"$dir/bin/gh" <<STUB
#!/usr/bin/env bash
case "$RELEASED" in
  true) echo false ;;
  draft) echo true ;;
  none) echo "release not found" >&2; exit 1 ;;
  error) echo "HTTP 502" >&2; exit 1 ;;
esac
STUB
  cat >"$dir/bin/git" <<STUB
#!/usr/bin/env bash
case "$TAG_AT" in
  error) echo "fatal: unable to access" >&2; exit 128 ;;
  "") exit 0 ;; # ls-remote prints nothing for a missing ref (without --exit-code)
  *) printf '%s\trefs/tags/whatever\n' "$TAG_AT" ;;
esac
STUB
  cat >"$dir/.github/release/version-is-new.sh" <<STUB
#!/usr/bin/env bash
case "$CRATES" in
  0) echo "new" ;;
  1) echo "::error::already on crates.io"; exit 1 ;;
  *) echo "::error::couldn't reach the index"; exit 2 ;;
esac
STUB
  chmod +x "$dir"/bin/* "$dir/.github/release/"*.sh
  (
    cd "$dir"
    export PATH="$dir/bin:$PATH" RUN=7 ATTEMPT=1 GITHUB_SHA=here GITHUB_REPOSITORY=o/r GH_TOKEN=x \
      GITHUB_OUTPUT="$dir/out" GITHUB_STEP_SUMMARY=/dev/null
    set +e
    msgs=$(bash .github/release/plan.sh 2>&1)
    code=$?
    set -e
    echo "exit=$code $( { tr '\n' ' ' <"$dir/out"; } 2>/dev/null) $msgs"
  )
  rm -rf "$dir"
}

# check <what> <expected substrings...> -- env assignments
check() {
  local what=$1
  shift
  local wants=()
  while [[ "$1" != -- ]]; do wants+=("$1"); shift; done
  shift
  local got
  got=$(env "$@" bash -c "$(declare -f run_plan); here='$here'; run_plan")
  for w in "${wants[@]}"; do
    if [[ "$got" != *"$w"* ]]; then
      fail=$((fail + 1))
      echo "FAIL $what: wanted '$w' in: $got"
      return
    fi
  done
  pass=$((pass + 1))
}

main=(REF=refs/heads/main VERSION=0.3.0)
# The everyday case: a push to main whose version is already released. Nothing to do, and green.
check "already released: nothing to do, not an error" "exit=0" "go=false" "is already released" -- \
  "${main[@]}" RELEASED=true TAG_AT=old CRATES=1
# A merged release PR (pushed to main, not cut by release-cut): no tag yet. It must release.
check "a new version with no tag yet releases" "exit=0" "go=true" "published=false" "tag=v0.3.0" -- \
  "${main[@]}" RELEASED=none TAG_AT= CRATES=0
check "cut by release-cut: tagged here, releases" "exit=0" "go=true" "published=false" -- \
  "${main[@]}" RELEASED=none TAG_AT=here CRATES=0
check "a draft left by a failed run is resumed" "exit=0" "go=true" -- \
  "${main[@]}" RELEASED=draft TAG_AT=here CRATES=0
check "resuming after crates.io got it: publish is skipped" "exit=0" "go=true" "published=true" "resuming" -- \
  "${main[@]}" RELEASED=draft TAG_AT=here CRATES=1
check "the tag points elsewhere: bump the version" "exit=0" "go=false" "already points at old" -- \
  "${main[@]}" RELEASED=none TAG_AT=old CRATES=0
check "on crates.io from somewhere else: an error" "exit=1" "not from a release of this commit" -- \
  "${main[@]}" RELEASED=none TAG_AT= CRATES=1
check "a failed crates.io lookup fails the plan" "exit=1" "couldn't reach the index" -- \
  "${main[@]}" RELEASED=none TAG_AT= CRATES=2
check "a failed tag lookup fails the plan" "exit=128" -- \
  "${main[@]}" RELEASED=none TAG_AT=error CRATES=0
check "a v* tag ref is a real release" "go=true" "rehearsal=false" -- \
  REF=refs/tags/v0.3.0 VERSION=0.3.0 RELEASED=none TAG_AT=here CRATES=0
check "release/** is a rehearsal, never checked against crates.io" "exit=0" "go=true" "rehearsal=true" "tag=rehearsal/0.2.1.7.1" -- \
  REF=refs/heads/release/try VERSION=0.2.1 RELEASED=error TAG_AT=error CRATES=2
check "a pre-release version ships as a prerelease" "prerelease=true" "tag=v0.3.0-rc.1" -- \
  REF=refs/heads/main VERSION=0.3.0-rc.1 RELEASED=none TAG_AT= CRATES=0

echo "$pass passed, $fail failed"
[[ $fail == 0 ]]
