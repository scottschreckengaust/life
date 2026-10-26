#!/usr/bin/env python3
"""Fail unless every line in src/ is covered, and say exactly where it isn't.

Lines, not lcov's function records: those list every monomorphized copy of a generic or
closure per test binary, so a function the tests do run can still show up "never called".

    check.py lcov.info

Prints one ::error:: annotation per file with its uncovered line ranges (one
per file keeps under GitHub's annotation limits), a table for the job summary, and exits 1
if anything is uncovered.
"""
import os
import sys
from collections import defaultdict


def ranges(lines):
    out, start, prev = [], None, None
    for n in sorted(lines):
        if start is None:
            start = prev = n
        elif n == prev + 1:
            prev = n
        else:
            out.append((start, prev))
            start = prev = n
    if start is not None:
        out.append((start, prev))
    return ", ".join(f"{a}" if a == b else f"{a}-{b}" for a, b in out)


def main(path):
    root = os.getcwd() + os.sep
    files = defaultdict(lambda: {"lines": {}})
    current = None
    for raw in open(path):
        line = raw.strip()
        if line.startswith("SF:"):
            current = files[line[3:].replace(root, "")]
        elif line.startswith("DA:"):
            n, hits = line[3:].split(",")[:2]
            current["lines"][int(n)] = current["lines"].get(int(n), 0) + int(hits)
    summary = ["| file | lines | uncovered |", "|---|---|---|"]
    bad = 0
    for name in sorted(files):
        f = files[name]
        if not name.startswith("src/"):
            continue
        missed = [n for n, h in f["lines"].items() if h == 0]
        total = len(f["lines"])
        pct = 100.0 * (total - len(missed)) / total if total else 100.0
        summary.append(
            f"| `{name}` | {total - len(missed)}/{total} ({pct:.1f}%) | {ranges(missed) or '-'} |"
        )
        if missed:
            bad += 1
            print(f"::error file={name},line={min(missed)},title=uncovered::lines {ranges(missed)}")
    probe = os.environ.get("COVERAGE_PROBE")  # e.g. "src/toml.rs:236-262": print raw line counts
    if probe:
        pf, pr = probe.split(":")
        lo, hi = map(int, pr.split("-"))
        counts = {n: h for n, h in files[pf]["lines"].items() if lo <= n <= hi}
        print(f"::notice file={pf},line={lo},title=probe::{sorted(counts.items())}")
    with open(os.environ.get("GITHUB_STEP_SUMMARY", os.devnull), "a") as out:
        out.write("### coverage\n\n" + "\n".join(summary) + "\n")
    print("\n".join(summary))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
