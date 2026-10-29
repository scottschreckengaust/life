#!/usr/bin/env bash
# The release plan: what release.yml should do for this commit. Prints the outputs
# (go, published, prerelease, name, version, tag, rehearsal) to $GITHUB_OUTPUT.
#
#   REF RUN ATTEMPT GITHUB_SHA GITHUB_REPOSITORY GH_TOKEN  plan.sh
#
# - Real releases come from main or a v* tag; any other ref (release/**) is a rehearsal,
#   tagged rehearsal/<version>.<run>.<attempt> so the v* tag rules never stop its cleanup.
# - go=false when there's nothing to do: the version is already released, or its tag points
#   at another commit (bump the version). Every push to main runs this, so "already released"
#   is the usual answer and isn't an error.
# - A version already on crates.io may only be this commit's own half-finished release (its tag
#   is here; a re-run resumes it). From anywhere else it's an error, before anything is tagged.
# - A lookup that fails fails the plan: it never reads as "not there".
set -euo pipefail

meta=$(cargo metadata --no-deps --format-version 1)
name=$(jq -r '.packages[0].name' <<<"$meta")
version=$(jq -r '.packages[0].version' <<<"$meta")
rehearsal=true
[[ "$REF" == refs/heads/main || "$REF" == refs/tags/v* ]] && rehearsal=false
# A SemVer pre-release (0.2.0-rc.1, -beta.2, ...) ships as a GitHub prerelease, never "latest".
prerelease=false
[[ "$version" == *-* ]] && prerelease=true
if $rehearsal; then tag="rehearsal/$version.$RUN.$ATTEMPT"; else tag="v$version"; fi

go=true
published=false
if ! $rehearsal; then
  # Done means a *published* release. A tag, draft or crates.io upload left by a failed run is
  # resumed instead (drafts aren't visible to a read-only token, so they read as "none").
  draft=$(gh release view "$tag" -R "$GITHUB_REPOSITORY" --json isDraft -q .isDraft 2>/dev/null || true)
  # Empty when the tag doesn't exist; a failed lookup fails the plan (pipefail).
  tagged=$(git ls-remote --tags origin "refs/tags/$tag" | cut -f1)
  if [[ "$draft" == false ]]; then
    echo "::notice::$tag is already released"
    go=false
  elif [[ -n "$tagged" && "$tagged" != "$GITHUB_SHA" ]]; then
    echo "::warning::$tag already points at $tagged, not $GITHUB_SHA; bump the version"
    go=false
  else
    set +e
    out=$(.github/release/version-is-new.sh --only crates "$name" "$version" 2>&1)
    found=$?
    set -e
    case $found in
      0) ;;
      1)
        if [[ "$tagged" == "$GITHUB_SHA" ]]; then
          published=true
          echo "::notice::$name $version is already on crates.io from this release; publish will be skipped (resuming)"
        else
          echo "::error::$name $version is already on crates.io, but not from a release of this commit; bump the version"
          exit 1
        fi
        ;;
      *)
        echo "$out"
        exit 1
        ;;
    esac
  fi
fi

$go || echo "::notice::nothing to release: the next release PR will bump the version"
{
  echo "go=$go"
  echo "published=$published"
  echo "prerelease=$prerelease"
  echo "name=$name"
  echo "version=$version"
  echo "tag=$tag"
  echo "rehearsal=$rehearsal"
} >>"$GITHUB_OUTPUT"
echo "### $tag (rehearsal: $rehearsal, prerelease: $prerelease, go: $go)" >>"${GITHUB_STEP_SUMMARY:-/dev/null}"
