#!/usr/bin/env bash
# Scan files with ClamAV, and fail on anything it finds (#60).
#
#   clamscan.sh <title> <path>...
#
# 1. Install ClamAV and its harmless test files from Ubuntu's packages (no third-party
#    action), and update the signatures. A failed update fails the scan: a clean result from
#    stale or missing signatures would mean nothing.
# 2. Self-test: ClamAV has to find its own test files (/usr/share/clamav-testfiles). This is
#    the positive control: it proves the engine and signatures can see a detection, so a
#    clean result below is real. No test virus is ever in this repository or its history.
# 3. Scan the paths. Each finding fails, unless its file's SHA-256 and signature are listed
#    in .github/ci/clamav-allow.txt (or $CLAMAV_ALLOW): a confirmed false positive, reviewed
#    in a PR, with its reason.
#
# Findings and errors are ::error annotations; the run summary records the engine and
# signature versions, the self-test, and each finding.
set -uo pipefail
title=$1; shift
summary=${GITHUB_STEP_SUMMARY:-/dev/stdout}
allow=${CLAMAV_ALLOW:-$(dirname "$0")/clamav-allow.txt}
testfiles=/usr/share/clamav-testfiles

die() { echo "::error title=ClamAV ($title)::$*"; echo "- **Failed:** $*" >>"$summary"; exit 1; }

echo "### ClamAV: $title" >>"$summary"
echo >>"$summary"

if ! command -v clamscan >/dev/null || [[ ! -d $testfiles ]]; then
  sudo apt-get update -qq &&
    sudo DEBIAN_FRONTEND=noninteractive apt-get install -y -qq clamav clamav-testfiles >/dev/null ||
    die "couldn't install clamav and clamav-testfiles"
fi
# The package starts its own updater, which holds the database lock: stop it and update here.
sudo systemctl stop clamav-freshclam 2>/dev/null || true
sudo freshclam --quiet || die "couldn't update the signatures (freshclam failed); re-run the job"
version=$(clamscan --version)
echo "- **Engine / signatures:** \`$version\`" >>"$summary"

# The positive control.
seen=$(clamscan --recursive --infected --no-summary "$testfiles" 2>&1 | grep -c ' FOUND$' || true)
[[ $seen -gt 0 ]] || die "the self-test failed: ClamAV found nothing in its own test files ($testfiles)"
echo "- **Self-test:** found $seen of its own test files, so detections work" >>"$summary"

log=$(mktemp)
clamscan --recursive --infected --no-summary --scan-archive=yes "$@" >"$log" 2>&1
status=$?
echo "- **Scanned:** $(find "$@" -type f | wc -l) files in \`$*\`" >>"$summary"
case $status in
  0)
    echo "- **Result:** clean" >>"$summary"
    echo "ClamAV: clean ($version)"
    exit 0
    ;;
  1) ;;
  *)
    { echo; echo '```'; tail -n 20 "$log"; echo '```'; } >>"$summary"
    die "clamscan exited $status: $(tail -n 1 "$log")"
    ;;
esac

# Findings: each is "path: Signature FOUND". Allowed only as an exact (sha256, signature) pair.
bad=0
while IFS= read -r line; do
  file=${line%: * FOUND}
  sig=${line%" FOUND"}; sig=${sig##*: }
  sum=$(sha256sum "$file" | cut -d' ' -f1)
  reason=$(awk -v s="$sum" -v g="$sig" '$1 == s && $2 == g { $1 = $2 = ""; sub(/^ +/, ""); print; exit }' "$allow" 2>/dev/null)
  if [[ -n $reason ]]; then
    echo "::notice title=ClamAV ($title)::allowed false positive: $file ($sig): $reason"
    echo "- allowed: \`$file\` \`$sig\` ($reason)" >>"$summary"
  else
    bad=1
    echo "::error title=ClamAV ($title)::$file: $sig (sha256 $sum)"
    echo "- ⛔ **found:** \`$file\` \`$sig\`, sha256 \`$sum\`" >>"$summary"
  fi
done < <(grep ' FOUND$' "$log")
if [[ $bad -eq 1 ]]; then
  {
    echo
    echo "If it's a confirmed false positive (check the file on VirusTotal, and report it to"
    echo "ClamAV), add \`<sha256> <signature> <reason>\` to \`.github/ci/clamav-allow.txt\` in a PR."
  } >>"$summary"
  exit 1
fi
echo "- **Result:** only allowed false positives" >>"$summary"
