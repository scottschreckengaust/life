"""Print release-plz.toml's [changelog] without its corrections, appended to another file.

    changelog-defaults.py release-plz.toml without.toml > with.toml

A correction is a commit parser that names a `sha`, or any commit preprocessor except
release-plz's PR-link rewrite. What's left has to be exactly release-plz's defaults, which
changelog-config.yml checks by generating changelogs with both files.
"""
import json
import sys
import tomllib

PR_LINK = r"\(#([0-9]+)\)"

with open(sys.argv[1], "rb") as f:
    changelog = tomllib.load(f)["changelog"]
with open(sys.argv[2], encoding="utf-8") as f:
    out = f.read().rstrip("\n") + "\n\n[changelog]\n"


def inline(table):
    # TOML inline table; JSON string escapes are valid TOML basic strings.
    return "{ " + ", ".join(f"{k} = {json.dumps(v)}" for k, v in table.items()) + " }"


for key, value in changelog.items():
    if key == "commit_parsers":
        value = [p for p in value if "sha" not in p]
    elif key == "commit_preprocessors":
        value = [p for p in value if p.get("pattern") == PR_LINK]
    if isinstance(value, list):
        out += f"{key} = [\n" + "".join(f"  {inline(v)},\n" for v in value) + "]\n"
    else:
        out += f"{key} = {json.dumps(value)}\n"
sys.stdout.write(out)
