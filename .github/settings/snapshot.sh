#!/usr/bin/env bash
# Print the repository's settings that a workflow token can read, as stable, sorted JSON.
# settings-check.yml compares this against .github/settings/expected.json; to change a
# setting on purpose, change it in GitHub and update expected.json in the same PR
# (bash .github/settings/snapshot.sh > .github/settings/expected.json).
#
#   GH_TOKEN=... REPO=owner/name snapshot.sh
set -euo pipefail
: "${REPO:?}"

# gh api, but a refused read names itself (as an annotation in CI) instead of failing
# silently somewhere in the middle.
api() {
  local out
  if ! out=$(gh api "$@" 2>&1); then
    echo "::error title=settings snapshot::can't read $1: $out" >&2
    exit 1
  fi
  printf '%s\n' "$out"
}

repo=$(api "repos/$REPO" --jq '{
  visibility, allow_forking, has_wiki, has_projects, has_discussions,
  allow_squash_merge, allow_merge_commit, allow_rebase_merge, allow_auto_merge,
  allow_update_branch, delete_branch_on_merge, web_commit_signoff_required,
  squash_merge_commit_title, squash_merge_commit_message
}')

rulesets='[]'
for id in $(api "repos/$REPO/rulesets" --jq '.[].id'); do
  one=$(api "repos/$REPO/rulesets/$id" --jq '{
    name, target, enforcement,
    conditions,
    rules: ([.rules[] | {type, parameters}] | sort_by(.type)),
    bypass_actors: ((.bypass_actors // []) | map({actor_type, actor_id, bypass_mode}) | sort_by(.actor_type, .actor_id))
  }')
  rulesets=$(jq --argjson one "$one" '. + [$one]' <<<"$rulesets")
done
rulesets=$(jq 'sort_by(.name)' <<<"$rulesets")

# Immutable releases (Settings > General > Releases) applies from when it was turned on, so
# the latest release shows whether it is on now.
releases=$(api "repos/$REPO/releases/latest" --jq '{latest_immutable: .immutable}')
pvr=$(api "repos/$REPO/private-vulnerability-reporting" --jq '{private_vulnerability_reporting: .enabled}')
codeql=$(api "repos/$REPO/code-scanning/default-setup" --jq '{codeql_default_setup: .state}')

jq -n --argjson repo "$repo" --argjson rulesets "$rulesets" --argjson releases "$releases" \
  --argjson pvr "$pvr" --argjson codeql "$codeql" \
  '{repository: $repo, rulesets: $rulesets, releases: $releases, security: ($pvr + $codeql)}' --sort-keys
