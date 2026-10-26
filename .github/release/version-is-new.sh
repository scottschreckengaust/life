#!/usr/bin/env bash
# Is this version new everywhere a release would put it?
#
#   version-is-new.sh [--only crates|tag|release] <crate> <version> [<tag>]
#
# Exits 0 when the version is new, 1 when it already exists somewhere (each place gets an
# ::error::), and 2 when a lookup failed: an unreachable index or API never counts as "new".
# Checks the crates.io sparse index (yanked versions count: they can't be reused either),
# the git tag (default v<version>) and a GitHub release on that tag (drafts are only visible
# to a token that can push). Needs GH_TOKEN and GITHUB_REPOSITORY for the GitHub checks.
set -euo pipefail

only=""
if [[ "${1:-}" == --only ]]; then only=$2; shift 2; fi
[[ $# -ge 2 ]] || { echo "usage: $0 [--only crates|tag|release] <crate> <version> [<tag>]" >&2; exit 2; }
name=$1 version=$2 tag=${3:-v$2}
want() { [[ -z "$only" || "$only" == "$1" ]]; }
exists=0
lookup_failed() { echo "::error::$1"; exit 2; }

if want crates; then
  # https://doc.rust-lang.org/cargo/reference/registry-index.html#index-files
  lower=${name,,}
  case ${#lower} in
    1) path="1/$lower" ;;
    2) path="2/$lower" ;;
    3) path="3/${lower:0:1}/$lower" ;;
    *) path="${lower:0:2}/${lower:2:2}/$lower" ;;
  esac
  url="https://index.crates.io/$path"
  body=$(mktemp)
  code=$(curl -sS -o "$body" -w '%{http_code}' "$url") || lookup_failed "couldn't reach $url"
  case "$code" in
    200)
      hit=$(jq -c --arg v "$version" 'select(.vers == $v) | {yanked}' "$body") ||
        lookup_failed "couldn't read the crates.io index file for $name"
      if [[ -n "$hit" ]]; then
        yanked=$(jq -r .yanked <<<"$hit")
        echo "::error::$name $version is already on crates.io$([[ "$yanked" == true ]] && echo " (yanked, which still can't be reused)")"
        exists=1
      fi
      ;;
    404) ;; # the crate has never been published
    *) lookup_failed "$url answered HTTP $code" ;;
  esac
fi

if want tag; then
  refs=$(gh api "repos/$GITHUB_REPOSITORY/git/matching-refs/tags/$tag" --jq '.[].ref') ||
    lookup_failed "couldn't list tags matching $tag"
  if grep -qxF "refs/tags/$tag" <<<"$refs"; then
    echo "::error::the tag $tag already exists"
    exists=1
  fi
fi

if want release; then
  tags=$(gh api "repos/$GITHUB_REPOSITORY/releases" --paginate --jq '.[].tag_name') ||
    lookup_failed "couldn't list the GitHub releases"
  if grep -qxF "$tag" <<<"$tags"; then
    echo "::error::a GitHub release for $tag already exists"
    exists=1
  fi
fi

if [[ $exists == 0 ]]; then
  echo "$name $version is new${only:+ ($only)}"
fi
exit "$exists"
