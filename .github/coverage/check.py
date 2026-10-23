#!/usr/bin/env python3
"""Fail unless every line and function in src/ is covered, and say exactly where it isn't.

    check.py lcov.info

Prints one ::error:: annotation per file with its uncovered line ranges and functions (one
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
    files = defaultdict(lambda: {"lines": {}, "fns": {}, "fn_line": {}})
    current = None
    for raw in open(path):
        line = raw.strip()
        if line.startswith("SF:"):
            current = files[line[3:].replace(root, "")]
        elif line.startswith("DA:"):
            n, hits = line[3:].split(",")[:2]
            current["lines"][int(n)] = current["lines"].get(int(n), 0) + int(hits)
        elif line.startswith("FN:"):
            n, name = line[3:].split(",", 1)
            current["fn_line"][name] = int(n)
        elif line.startswith("FNDA:"):
            hits, name = line[5:].split(",", 1)
            current["fns"][name] = current["fns"].get(name, 0) + int(hits)
    summary = ["| file | lines | functions | uncovered |", "|---|---|---|---|"]
    bad = 0
    for name in sorted(files):
        f = files[name]
        if not name.startswith("src/"):
            continue
        missed = [n for n, h in f["lines"].items() if h == 0]
        fns = [k for k, h in f["fns"].items() if h == 0]
        total, total_fn = len(f["lines"]), len(f["fns"])
        pct = 100.0 * (total - len(missed)) / total if total else 100.0
        summary.append(
            f"| `{name}` | {total - len(missed)}/{total} ({pct:.1f}%) | {total_fn - len(fns)}/{total_fn} | {ranges(missed) or '-'} |"
        )
        if missed or fns:
            bad += 1
            first = min(missed) if missed else min(f["fn_line"].get(k, 1) for k in fns)
            fn_list = "; functions never called: " + ", ".join(sorted(fns)) if fns else ""
            print(f"::error file={name},line={first},title=uncovered::lines {ranges(missed) or 'none'}{fn_list}")
    with open(os.environ.get("GITHUB_STEP_SUMMARY", os.devnull), "a") as out:
        out.write("### coverage\n\n" + "\n".join(summary) + "\n")
    print("\n".join(summary))
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
