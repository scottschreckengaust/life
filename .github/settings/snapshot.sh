#!/usr/bin/env bash
# Print the repository's settings as stable, sorted JSON. settings-check.yml compares this
# against .github/settings/expected.json; to change a setting on purpose, change it in GitHub
# and update expected.json in the same PR.
#
#   GH_TOKEN=... REPO=owner/name snapshot.sh           what a workflow token can read
#   GH_TOKEN=... REPO=owner/name FULL=1 snapshot.sh    everything, with the read-only
#                                                       settings App's token (#77)
#
# The `admin` section is only in a FULL snapshot: merge options, rulesets' bypass lists,
# Actions settings and environments need Administration, Actions and Environments read.
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

repo=$(api "repos/$REPO" --jq '{visibility, allow_forking, has_wiki, has_projects, has_discussions, web_commit_signoff_required}')

rulesets='[]'
for id in $(api "repos/$REPO/rulesets" --jq '.[].id'); do
  one=$(api "repos/$REPO/rulesets/$id" --jq '{
    name, target, enforcement,
    conditions,
    rules: ([.rules[] | {type, parameters}] | sort_by(.type))
  }')
  rulesets=$(jq --argjson one "$one" '. + [$one]' <<<"$rulesets")
done
rulesets=$(jq 'sort_by(.name)' <<<"$rulesets")

# Immutable releases (Settings > General > Releases) applies from when it was turned on, so
# the latest release shows whether it is on now.
releases=$(api "repos/$REPO/releases/latest" --jq '{latest_immutable: .immutable}')
pvr=$(api "repos/$REPO/private-vulnerability-reporting" --jq '{private_vulnerability_reporting: .enabled}')
# (The CodeQL setup needs an admin token to read; codeql.yml running on every PR shows the
# advanced setup is in use.)

admin=null
if [[ ${FULL:-} == 1 ]]; then
  # Merge options, and what each ruleset lets bypass it.
  merge=$(api "repos/$REPO" --jq '{allow_squash_merge, allow_merge_commit, allow_rebase_merge,
    squash_merge_commit_title, squash_merge_commit_message, delete_branch_on_merge,
    allow_update_branch, allow_auto_merge}')
  bypass='[]'
  for id in $(api "repos/$REPO/rulesets" --jq '.[].id'); do
    one=$(api "repos/$REPO/rulesets/$id" --jq '{name,
      bypass_actors: ([.bypass_actors[]? | {actor_type, actor_id, bypass_mode}] | sort_by(.actor_type, .actor_id))}')
    bypass=$(jq --argjson one "$one" '. + [$one]' <<<"$bypass")
  done
  bypass=$(jq 'sort_by(.name)' <<<"$bypass")
  # Actions: which actions may run, SHA pinning, the default token, fork PR approval.
  actions=$(jq -n \
    --argjson p "$(api "repos/$REPO/actions/permissions" --jq '{enabled, allowed_actions, sha_pinning_required}')" \
    --argjson sel "$(api "repos/$REPO/actions/permissions/selected-actions" --jq '{github_owned_allowed, verified_allowed, patterns_allowed: (.patterns_allowed | sort)}')" \
    --argjson wf "$(api "repos/$REPO/actions/permissions/workflow" --jq '{default_workflow_permissions, can_approve_pull_request_reviews}')" \
    --argjson fork "$(api "repos/$REPO/actions/permissions/fork-pr-contributor-approval" --jq '{fork_pr_approval: .approval_policy}')" \
    '$p + {selected_actions: $sel} + $wf + $fork')
  # Environments: who must approve, and which branches and tags may deploy.
  environments='[]'
  for env in $(api "repos/$REPO/environments" --jq '.environments[].name'); do
    one=$(api "repos/$REPO/environments/$env" --jq '{name,
      reviewers: ([.protection_rules[]? | select(.type == "required_reviewers") | .reviewers[] | "\(.type):\(.reviewer.login // .reviewer.slug)"] | sort),
      prevent_self_review: ([.protection_rules[]? | select(.type == "required_reviewers") | .prevent_self_review] | any),
      wait_timer: ([.protection_rules[]? | select(.type == "wait_timer") | .wait_timer] | first // 0),
      deployment_branch_policy}')
    if [[ $(jq -r '.deployment_branch_policy.custom_branch_policies // false' <<<"$one") == true ]]; then
      policies=$(api "repos/$REPO/environments/$env/deployment-branch-policies" --jq '[.branch_policies[] | "\(.type // "branch"):\(.name)"] | sort')
      one=$(jq --argjson p "$policies" '. + {branch_policies: $p}' <<<"$one")
    fi
    environments=$(jq --argjson one "$one" '. + [$one]' <<<"$environments")
  done
  environments=$(jq 'sort_by(.name)' <<<"$environments")
  admin=$(jq -n --argjson merge "$merge" --argjson bypass "$bypass" --argjson actions "$actions" \
    --argjson environments "$environments" \
    '{merge: $merge, ruleset_bypass: $bypass, actions: $actions, environments: $environments}')
fi

jq -n --argjson repo "$repo" --argjson rulesets "$rulesets" --argjson releases "$releases" \
  --argjson pvr "$pvr" --argjson admin "$admin" \
  '{repository: $repo, rulesets: $rulesets, releases: $releases, security: $pvr}
   + (if $admin == null then {} else {admin: $admin} end)' --sort-keys
