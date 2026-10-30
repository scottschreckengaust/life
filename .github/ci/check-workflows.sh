#!/usr/bin/env bash
# Every workflow starts from no token permissions and runs its steps under bash -eo
# pipefail. GitHub's default shell is `bash -e` with no pipefail, so `a | b` there passes when `a` fails, which is how the
# release plan once read a failed tag lookup as "no tag". Fails listing the offenders.
set -euo pipefail
dir=${1:-.github/workflows}
bad=0
for f in "$dir"/*.yml "$dir"/*.yaml; do
  [[ -e "$f" ]] || continue
  # Only run steps have a shell; a workflow of nothing but actions (scorecard) needs none.
  # Any `run` key, block or flow style, quoted or not, counts.
  grep -qE '(^|[^A-Za-z0-9_-])["'"'"']?run["'"'"']? *:' "$f" || continue
  # A top-level `defaults:` whose `run:` sets `shell: bash`.
  if ! awk '
    /^defaults:/ { d=1; next }
    d && /^[^ #]/ { d=0 }
    d && /^  [^ ]/ { r=0 }
    d && /^  run:/ { r=1; next }
    d && r && /^    shell: *bash *(#.*)?$/ { ok=1 }
    END { exit !ok }' "$f"; then
    echo "::error file=$f::set top-level 'defaults: run: shell: bash' so pipelines fail on any failure"
    bad=1
  fi
  # Every workflow starts from no permissions (`permissions: {}`), and each job asks for
  # what it uses. The one exception proves the repository default by running with it.
  if [[ "$(basename "$f")" != default-token-check.yml ]] && ! grep -qxE 'permissions: \{\}( *#.*)?' "$f"; then
    echo "::error file=$f::start the workflow with top-level 'permissions: {}'; give each job only what it needs"
    bad=1
  fi
  # A job or step may only restate it: any other shell (sh, pwsh, a custom bash line)
  # opts out of pipefail.
  if grep -nE '^ +shell:' "$f" | grep -vE '^[0-9]+: +shell: *bash *(#.*)?$'; then
    echo "::error file=$f::a job or step sets a shell other than plain 'bash'"
    bad=1
  fi
done
exit $bad
