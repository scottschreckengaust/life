#!/usr/bin/env bash
# Scan files with ClamAV and report what it finds. Report-only: always exits 0, so a false
# positive (brand-new unsigned Windows .exe files get them) can't block a release (#60).
# Findings become ::warning annotations, and the run summary records the ClamAV and
# signature versions, so a clean result says exactly what it was checked against.
#
#   clamscan.sh <title> <path>...
#
# Needs Ubuntu's clamav package (no third-party action). Run on a GitHub runner.
set -uo pipefail
title=$1; shift
summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}

warn() { echo "::warning title=ClamAV ($title)::$*"; }

if ! command -v clamscan >/dev/null; then
  sudo apt-get update -qq && sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq clamav >/dev/null
fi
# The package starts its own updater, which holds the database lock: stop it and update here.
sudo systemctl stop clamav-freshclam 2>/dev/null || true
if ! sudo freshclam --quiet; then
  warn "couldn't update the signatures (freshclam failed); scanning with what is installed"
fi

version=$(clamscan --version)
log=$(mktemp)
clamscan --recursive --infected --no-summary --scan-archive=yes "$@" >"$log" 2>&1
status=$?

{
  echo "### ClamAV: $title"
  echo
  echo "- **Engine / signatures:** \`$version\`"
  echo "- **Scanned:** $(find "$@" -type f | wc -l) files in \`$*\`"
} >>"$summary"

case $status in
  0)
    echo "- **Result:** clean" >>"$summary"
    echo "ClamAV: clean ($version)"
    ;;
  1)
    echo "- **Result:** ⚠️ found something (report-only: this does not block the release)" >>"$summary"
    { echo; echo '```'; grep ' FOUND$' "$log"; echo '```'; } >>"$summary"
    grep ' FOUND$' "$log" | while IFS= read -r line; do warn "$line"; done
    ;;
  *)
    echo "- **Result:** scan error (exit $status)" >>"$summary"
    { echo; echo '```'; tail -n 20 "$log"; echo '```'; } >>"$summary"
    warn "clamscan exited $status: $(tail -n 1 "$log")"
    ;;
esac
exit 0
