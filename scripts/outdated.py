"""The direct dependencies behind their latest release, Rust and npm, one row each.

Prints `name current latest` rows, the evidence file VIBECODING.md declares
(`evidence.outdated`). cargo outdated prints one JSON document per workspace
crate, so a dependency shared by several crates is listed once.
"""

import json
import subprocess

rows = {}
cargo = subprocess.run(
    ["cargo", "outdated", "--root-deps-only", "--workspace", "--format", "json"],
    capture_output=True, text=True, check=True,
).stdout
for line in cargo.splitlines():
    for dep in json.loads(line)["dependencies"]:
        if dep["project"] != dep["latest"]:
            rows[dep["name"]] = (dep["project"], dep["latest"])

# npm outdated exits 1 when something is outdated: its output is the answer.
npm = subprocess.run(["npm", "outdated", "--json"], cwd="web", capture_output=True, text=True).stdout
for name, dep in json.loads(npm or "{}").items():
    rows[name] = (dep["current"], dep["latest"])

for name, (current, latest) in sorted(rows.items()):
    print(f"{name} {current} {latest}")
