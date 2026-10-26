#!/usr/bin/env bash
# Tests for version-is-new.sh, with curl and gh replaced by stubs. Run: bash .github/release/test-version-is-new.sh
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
stubs=$(mktemp -d)
trap 'rm -rf "$stubs"' EXIT

# curl stub: STUB_CURL = "<http code> <index file lines...>" or "down"
cat >"$stubs/curl" <<'STUB'
#!/usr/bin/env bash
out=""; while [[ $# -gt 0 ]]; do case "$1" in -o) out=$2; shift ;; https://*) echo "$1" >"$(dirname "$0")/url" ;; esac; shift; done
[[ "$STUB_CURL" == down ]] && { echo "curl: (6) Could not resolve host" >&2; exit 6; }
code=${STUB_CURL%% *}; body=${STUB_CURL#* }; [[ "$body" == "$STUB_CURL" ]] && body=""
printf '%b' "$body" >"$out"; printf '%s' "$code"
STUB
# gh stub: STUB_TAGS / STUB_RELEASES are the lines to print, or "error"
cat >"$stubs/gh" <<'STUB'
#!/usr/bin/env bash
case "$*" in
  *matching-refs*) v=$STUB_TAGS ;;
  *releases*) v=$STUB_RELEASES ;;
  *) echo "unexpected gh $*" >&2; exit 99 ;;
esac
[[ "$v" == error ]] && { echo "HTTP 502" >&2; exit 1; }
[[ -n "$v" ]] && printf '%b\n' "$v"; exit 0
STUB
chmod +x "$stubs/curl" "$stubs/gh"

export PATH="$stubs:$PATH" GITHUB_REPOSITORY=o/r GH_TOKEN=x
pass=0 fail=0
# check <name> <expected exit> <expected output substring> -- <args...>
check() {
  local what=$1 want=$2 says=$3; shift 4
  set +e; out=$("$here/version-is-new.sh" "$@" 2>&1); got=$?; set -e
  if [[ $got == "$want" && "$out" == *"$says"* ]]; then pass=$((pass + 1)); else
    fail=$((fail + 1)); echo "FAIL $what: exit $got (want $want), output: $out"; fi
}
old='{"name":"spin-life","vers":"0.1.0","yanked":false}\n{"name":"spin-life","vers":"0.2.0","yanked":true}\n'

STUB_CURL="200 $old" STUB_TAGS="" STUB_RELEASES="" check "new version" 0 "spin-life 0.3.0 is new" -- spin-life 0.3.0
STUB_CURL="200 $old" STUB_TAGS="" STUB_RELEASES="" check "on crates.io" 1 "0.1.0 is already on crates.io" -- spin-life 0.1.0
STUB_CURL="200 $old" STUB_TAGS="" STUB_RELEASES="" check "yanked counts" 1 "(yanked, which still can't be reused)" -- spin-life 0.2.0
STUB_CURL="404 " STUB_TAGS="" STUB_RELEASES="" check "never published" 0 "is new" -- spin-life 0.1.0
STUB_CURL="500 oops" STUB_TAGS="" STUB_RELEASES="" check "index 500 fails closed" 2 "answered HTTP 500" -- spin-life 0.3.0
STUB_CURL="down" STUB_TAGS="" STUB_RELEASES="" check "index unreachable fails closed" 2 "couldn't reach" -- spin-life 0.3.0
STUB_CURL="200 not json" STUB_TAGS="" STUB_RELEASES="" check "garbled index fails closed" 2 "couldn't read" -- spin-life 0.3.0
STUB_CURL="200 $old" STUB_TAGS="refs/tags/v0.3.0" STUB_RELEASES="" check "tag exists" 1 "the tag v0.3.0 already exists" -- spin-life 0.3.0
STUB_CURL="200 $old" STUB_TAGS="refs/tags/v0.3.0-rc.1" STUB_RELEASES="" check "a prefix-matching tag isn't the tag" 0 "is new" -- spin-life 0.3.0
STUB_CURL="200 $old" STUB_TAGS="error" STUB_RELEASES="" check "tag lookup fails closed" 2 "couldn't list tags" -- spin-life 0.3.0
STUB_CURL="200 $old" STUB_TAGS="" STUB_RELEASES="v0.2.0\nv0.3.0" check "release (or draft) exists" 1 "a GitHub release for v0.3.0 already exists" -- spin-life 0.3.0
STUB_CURL="200 $old" STUB_TAGS="" STUB_RELEASES="error" check "release lookup fails closed" 2 "couldn't list the GitHub releases" -- spin-life 0.3.0
STUB_CURL="down" STUB_TAGS="refs/tags/v0.1.0" STUB_RELEASES="" check "--only tag skips crates.io" 1 "the tag v0.1.0 already exists" -- --only tag spin-life 0.1.0
STUB_CURL="200 $old" STUB_TAGS="error" STUB_RELEASES="error" check "--only crates skips GitHub" 1 "already on crates.io" -- --only crates spin-life 0.1.0
STUB_CURL="200 $old" STUB_TAGS="refs/tags/rehearsal/0.1.0.7.1" STUB_RELEASES="" check "a custom tag" 1 "rehearsal/0.1.0.7.1 already exists" -- --only tag spin-life 0.1.0 rehearsal/0.1.0.7.1
# The index path depends on the name's length.
for case in "a 1/a" "ab 2/ab" "abc 3/a/abc" "Spin-Life sp/in/spin-life"; do
  set -- $case
  STUB_CURL="404 " check "index path for $1" 0 "is new" -- --only crates "$1" 1.0.0
  [[ "$(cat "$stubs/url")" == "https://index.crates.io/$2" ]] && pass=$((pass + 1)) ||
    { fail=$((fail + 1)); echo "FAIL index path for $1: $(cat "$stubs/url")"; }
done

echo "$pass passed, $fail failed"
[[ $fail == 0 ]]
