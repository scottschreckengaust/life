#!/usr/bin/env python3
"""Report a cargo-mutants run, and fail if any mutant survived.

    report.py MUTANTS_OUT EXIT_CODE

Prints one ::error:: annotation per file listing every missed mutant in it (one per file
keeps under GitHub's annotation limits, so none get dropped), a ::notice:: with the counts,
and a table for the job summary. Exits 1 if any mutant was missed, or if cargo-mutants
itself failed (exit codes other than 0 = all caught, 2 = some missed, 3 = some timed out).
Timeouts and unviable mutants are fine: a mutant that hangs or doesn't build can't pass.
"""
import os
import sys
from collections import defaultdict


def lines(out, name):
    path = os.path.join(out, name)
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as f:
        return [line.rstrip("\n") for line in f if line.strip()]


def escape(s):
    """Workflow command data: %, CR and LF must be encoded."""
    return s.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def main(out, code):
    counts = {k: lines(out, f"{k}.txt") for k in ("caught", "missed", "timeout", "unviable")}
    n = {k: len(v) for k, v in counts.items()}
    total = sum(n.values())

    # `src/lib.rs:310:9: replace >= with < in Life::window`
    by_file = defaultdict(list)
    for m in counts["missed"]:
        file, line, rest = m.split(":", 2)
        by_file[file].append((int(line), m[len(file) + 1 :]))
    for file in sorted(by_file):
        missed = sorted(by_file[file])
        msg = "\n".join(text for _, text in missed)
        title = f"{len(missed)} surviving mutant{'s' if len(missed) > 1 else ''}"
        print(f"::error file={file},line={missed[0][0]},title={title}::{escape(msg)}")
    if counts["timeout"]:
        msg = "\n".join(counts["timeout"])
        print(f"::notice title={n['timeout']} timed out (fine; a hang can't pass)::{escape(msg)}")

    print(
        f"::notice title=mutants::{total} mutants: {n['caught']} caught, {n['missed']} missed, "
        f"{n['timeout']} timeout, {n['unviable']} unviable (cargo-mutants exit {code})"
    )
    summary = [
        "### mutants",
        "",
        "| caught | missed | timeout | unviable | total |",
        "|---|---|---|---|---|",
        f"| {n['caught']} | {n['missed']} | {n['timeout']} | {n['unviable']} | {total} |",
    ]
    if counts["missed"]:
        summary += ["", "Missed:", "", *(f"- `{m}`" for m in counts["missed"])]
    if counts["timeout"]:
        summary += ["", "Timed out:", "", *(f"- `{m}`" for m in counts["timeout"])]
    with open(os.environ.get("GITHUB_STEP_SUMMARY", os.devnull), "a", encoding="utf-8") as f:
        f.write("\n".join(summary) + "\n")

    if code not in (0, 2, 3):
        print(f"::error title=mutants::cargo-mutants failed (exit {code}); see the log")
        return 1
    return 1 if counts["missed"] else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1], int(sys.argv[2])))
