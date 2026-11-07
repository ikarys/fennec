"""Audit licenses of dependencies shipped in fennec's binary.

The release archive must carry the dependencies' notices; the PR gate refuses
a dependency whose licence the AGPL cannot absorb.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

# Licences that can be absorbed by an AGPL-3.0-only product.
ALLOWED: set[str] = {
    "MIT",
    "MIT-0",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "0BSD",
    "Zlib",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "Unlicense",
    "CC0-1.0",
    "BSL-1.0",
    "CDLA-Permissive-2.0",
    "OFL-1.1",
    "MPL-2.0",
    "BlueOak-1.0.0",
    "Python-2.0",
    "GPL-3.0-or-later",
    "LGPL-3.0-or-later",
    "AGPL-3.0-only",
    "AGPL-3.0-or-later",
}

# Package name -> licence used when the declared licence is missing/empty.
OVERRIDES: dict[str, str] = {
    # khroma's package.json declares none, its licence file is MIT.
    "khroma": "MIT",
}

LICENSE_FILE_PREFIXES: tuple[str, ...] = (
    "license",
    "licence",
    "copying",
    "notice",
    "copyright",
)

_SPDX_IDENTIFIER_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9.\-+]*")
_SPDX_OPERATORS: set[str] = {"AND", "OR", "WITH", "/", "(", ")"}


@dataclass(frozen=True)
class Package:
    """One audited dependency and the licence files shipped with it."""

    ecosystem: str
    name: str
    version: str
    license: str | None
    files: list[Path] = field(default_factory=list)


def _tokenize_spdx(expression: str) -> list[str]:
    """Split an SPDX expression into identifiers, operators, and punctuation."""
    tokens: list[str] = []
    pos = 0
    while pos < len(expression):
        char = expression[pos]
        if char.isspace():
            pos += 1
            continue
        if char in "()/":
            tokens.append(char)
            pos += 1
            continue
        match = _SPDX_IDENTIFIER_RE.match(expression, pos)
        if match is None:
            raise ValueError(f"unexpected character {char!r} in SPDX expression")
        tokens.append(match.group(0))
        pos = match.end()
    return tokens


def is_allowed_spdx(expression: str) -> bool:
    """Return True iff the SPDX expression is compatible with AGPL-3.0-only.

    Grammar implemented here:
      OR has the lowest precedence, AND binds tighter, WITH binds to a single
      licence term. The legacy "/" separator is treated as OR. A term
      "X WITH Y" is allowed iff X is allowed.
    """
    try:
        tokens = _tokenize_spdx(expression)
    except ValueError:
        return False
    if not tokens:
        return False

    pos = 0

    def peek() -> str | None:
        return tokens[pos] if pos < len(tokens) else None

    def take() -> str | None:
        nonlocal pos
        if pos >= len(tokens):
            return None
        token = tokens[pos]
        pos += 1
        return token

    def parse_or() -> bool | None:
        left = parse_and()
        if left is None:
            return None
        while peek() in ("OR", "/"):
            take()
            right = parse_and()
            if right is None:
                return None
            left = left or right
        return left

    def parse_and() -> bool | None:
        left = parse_with()
        if left is None:
            return None
        while peek() == "AND":
            take()
            right = parse_with()
            if right is None:
                return None
            left = left and right
        return left

    def parse_with() -> bool | None:
        left = parse_atom()
        if left is None:
            return None
        if peek() == "WITH":
            take()
            exception = take()
            # The exception identifier is irrelevant for compatibility; only
            # the base licence matters. But the expression must be well-formed.
            if exception is None or exception in _SPDX_OPERATORS:
                return None
        return left

    def parse_atom() -> bool | None:
        token = take()
        if token is None:
            return None
        if token == "(":
            value = parse_or()
            if value is None:
                return None
            if take() != ")":
                return None
            return value
        if token in _SPDX_OPERATORS:
            return None
        return token in ALLOWED

    result = parse_or()
    return result is True


def effective_license(name: str, declared: str | None) -> str | None:
    """Return the licence expression to evaluate for a package."""
    if declared is None or not declared.strip():
        return OVERRIDES.get(name)
    return declared.strip()


def license_is_allowed(license_expr: str | None) -> bool:
    """Return True iff a package's effective licence is allowed."""
    return license_expr is not None and is_allowed_spdx(license_expr)


def license_files(directory: Path) -> list[Path]:
    """Return licence/notice files in a directory, deterministically sorted."""
    if not directory.is_dir():
        return []
    try:
        entries = sorted(directory.iterdir(), key=lambda item: item.name.lower())
    except OSError:
        return []
    found: list[Path] = []
    for path in entries:
        if not path.is_file():
            continue
        name = path.name.lower()
        if any(name.startswith(prefix) for prefix in LICENSE_FILE_PREFIXES):
            found.append(path)
    return found


def npm_license(data: dict[str, Any]) -> str | None:
    """Extract an SPDX expression from an npm package.json document."""
    license_field = data.get("license")
    if isinstance(license_field, str):
        value = license_field.strip()
        return value or None
    if isinstance(license_field, dict):
        value = license_field.get("type")
        if isinstance(value, str):
            value = value.strip()
            return value or None
        return None

    # Very old npm metadata used a "licenses" array.
    licenses = data.get("licenses")
    if isinstance(licenses, list):
        types: list[str] = []
        for item in licenses:
            if isinstance(item, str):
                value = item.strip()
            elif isinstance(item, dict):
                raw = item.get("type")
                value = raw.strip() if isinstance(raw, str) else ""
            else:
                value = ""
            if value:
                types.append(value)
        if types:
            return " OR ".join(types)
    return None


def _run_command(command: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    """Run a subprocess and fail clearly if the executable is missing."""
    try:
        return subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    except OSError as exc:
        sys.stderr.write(f"failed to run {command[0]}: {exc}\n")
        sys.exit(1)


def collect_rust_packages(root: Path) -> list[Package]:
    """Collect Rust dependencies that are normal (non-dev/build) dependencies."""
    command = ["cargo", "metadata", "--format-version", "1", "--locked"]
    proc = _run_command(command, root)
    if proc.returncode != 0:
        sys.stderr.write(f"cargo metadata failed:\n{proc.stderr.strip()}\n")
        sys.exit(1)

    try:
        metadata = json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        sys.stderr.write(f"cargo metadata produced invalid JSON: {exc}\n")
        sys.exit(1)

    nodes = {
        node.get("id"): node
        for node in metadata.get("resolve", {}).get("nodes", [])
    }
    # The resolve graph holds only ids and edges; names, sources and licences live in `packages`.
    by_id = {package.get("id"): package for package in metadata.get("packages", [])}
    packages: list[Package] = []
    visited: set[str] = set()

    def visit(package_id: str | None) -> None:
        if package_id is None or package_id in visited:
            return
        visited.add(package_id)
        node = nodes.get(package_id)
        if node is None:
            return

        meta = by_id.get(package_id, {})
        # Only registry crates are external dependencies; workspace/path crates
        # are part of fennec itself.
        if meta.get("source") is not None:
            raw_manifest = meta.get("manifest_path")
            manifest_path = Path(raw_manifest) if raw_manifest else root / "Cargo.toml"
            if not manifest_path.is_absolute():
                manifest_path = root / manifest_path

            raw_license = meta.get("license")
            declared = (
                raw_license.strip()
                if isinstance(raw_license, str) and raw_license.strip()
                else None
            )
            packages.append(
                Package(
                    ecosystem="rust",
                    name=str(meta.get("name", "")),
                    version=str(meta.get("version", "")),
                    license=declared,
                    files=license_files(manifest_path.parent),
                )
            )

        # Follow only normal dependency edges. In cargo metadata, kind == null
        # means a normal dependency; "dev" and "build" are excluded.
        for dep in node.get("deps", []):
            dep_kinds = dep.get("dep_kinds") or []
            if any(kind.get("kind") is None for kind in dep_kinds):
                visit(dep.get("pkg"))

    for member_id in metadata.get("workspace_members", []):
        visit(member_id)

    packages.sort(key=lambda item: (item.name, item.version))
    return packages


def collect_npm_packages(root: Path) -> list[Package]:
    """Collect production npm dependencies from web/node_modules."""
    web = root / "web"
    node_modules = web / "node_modules"
    if not node_modules.is_dir():
        sys.stderr.write("web/node_modules is missing; run `npm ci` in web\n")
        sys.exit(1)

    command = ["npm", "ls", "--omit=dev", "--all", "--parseable"]
    proc = _run_command(command, web)
    # npm may exit non-zero for peer dependency warnings; that is acceptable.
    if not proc.stdout.strip():
        sys.stderr.write("npm ls produced no output; run `npm ci` in web\n")
        sys.exit(1)

    lines = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
    if not lines:
        sys.stderr.write("npm ls produced no output; run `npm ci` in web\n")
        sys.exit(1)

    packages: list[Package] = []
    seen: set[str] = set()

    # The first line is the web root itself, not a dependency.
    for line in lines[1:]:
        package_dir = Path(line)
        if not package_dir.is_absolute():
            package_dir = web / package_dir
        key = str(package_dir)
        if key in seen:
            continue
        seen.add(key)

        package_json = package_dir / "package.json"
        if not package_json.is_file():
            sys.stderr.write(f"missing package.json in {package_dir}\n")
            sys.exit(1)
        try:
            data = json.loads(package_json.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            sys.stderr.write(f"cannot read {package_json}: {exc}\n")
            sys.exit(1)
        if not isinstance(data, dict):
            sys.stderr.write(f"invalid package.json in {package_dir}\n")
            sys.exit(1)

        packages.append(
            Package(
                ecosystem="npm",
                name=str(data.get("name", package_dir.name)),
                version=str(data.get("version", "")),
                license=npm_license(data),
                files=license_files(package_dir),
            )
        )

    packages.sort(key=lambda item: (item.name, item.version))
    return packages


def audit(root: Path) -> tuple[list[Package], list[Package], list[Package]]:
    """Audit Rust and npm dependencies and return (rust, npm, failures)."""
    rust = collect_rust_packages(root)
    npm = collect_npm_packages(root)
    failures = [
        package
        for package in rust + npm
        if not license_is_allowed(effective_license(package.name, package.license))
    ]
    return rust, npm, failures


def describe_license(package: Package) -> str:
    """Return the licence expression to show for a package in output."""
    expr = effective_license(package.name, package.license)
    return expr if expr is not None else "none declared"


def print_failures(failures: list[Package]) -> None:
    """Print offending packages and a remediation hint to stderr."""
    for package in sorted(
        failures, key=lambda item: (item.ecosystem, item.name, item.version)
    ):
        sys.stderr.write(
            f"  {package.ecosystem} {package.name} {package.version}: "
            f"{describe_license(package)}\n"
        )
    sys.stderr.write(
        "  hint: pick another dependency or, if the licence is compatible, "
        "add it to ALLOWED in scripts/licenses.py with the reason in the commit body\n"
    )


def _code_fence(text: str) -> str:
    """Choose a Markdown fence that does not appear in the text."""
    fence = "~~~~" if "```" in text else "```"
    while fence in text:
        fence += "~" if fence.startswith("~") else "`"
    return fence


def package_markdown(package: Package) -> list[str]:
    """Render one package's licence notice section as Markdown lines."""
    terms = describe_license(package)
    lines = [
        f"### {package.name} {package.version}",
        f"License: {terms}",
        "",
    ]

    if not package.files:
        lines.append(f"No license file shipped with the package; terms: {terms}.")
        lines.append("")
        return lines

    for path in package.files:
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            sys.stderr.write(f"cannot read {path}: {exc}\n")
            sys.exit(1)
        fence = _code_fence(text)
        lines.append(fence)
        lines.append(text.rstrip("\n"))
        lines.append(fence)
        lines.append("")

    return lines


def run_check(root: Path) -> int:
    """Run the audit and print a one-line success summary or failures."""
    rust, npm, failures = audit(root)
    if failures:
        print_failures(failures)
        return 1
    print(
        f"licenses: {len(rust)} crates, {len(npm)} npm packages, "
        "all compatible with AGPL-3.0-only"
    )
    return 0


def run_notices(root: Path, output: Path) -> int:
    """Audit dependencies, then write a Markdown notices file on success."""
    rust, npm, failures = audit(root)
    if failures:
        print_failures(failures)
        return 1

    lines = [
        "# Third-party licenses",
        "",
        "fennec (AGPL-3.0-only) ships these dependencies in its binary; "
        "generated by scripts/licenses.py, do not edit.",
        "",
        "## Rust crates",
        "",
    ]
    for package in rust:
        lines.extend(package_markdown(package))

    lines.extend(
        [
            "## npm packages (the web UI embedded in the binary)",
            "",
        ]
    )
    for package in npm:
        lines.extend(package_markdown(package))

    text = "\n".join(lines).rstrip() + "\n"
    try:
        output.write_text(text, encoding="utf-8")
    except OSError as exc:
        sys.stderr.write(f"cannot write {output}: {exc}\n")
        return 1
    return 0


def self_test() -> None:
    """Plain assertions for the SPDX evaluator."""
    assert is_allowed_spdx("MIT")
    assert is_allowed_spdx("MIT OR GPL-2.0-only")
    assert not is_allowed_spdx("GPL-2.0-only")
    assert is_allowed_spdx("(MIT OR Apache-2.0) AND Unicode-3.0")
    assert not is_allowed_spdx("MIT AND GPL-2.0-only")
    assert is_allowed_spdx("MIT/Apache-2.0")
    assert is_allowed_spdx("Apache-2.0 WITH LLVM-exception")
    assert is_allowed_spdx("EPL-2.0 OR GPL-3.0-or-later")
    assert not is_allowed_spdx("")
    assert not is_allowed_spdx("(MIT")
    print("self-test: ok")


def main(argv: list[str] | None = None) -> int:
    """Command-line entry point."""
    parser = argparse.ArgumentParser(
        description="Audit licenses of dependencies shipped in fennec's binary."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser(
        "check",
        help="audit dependencies and exit non-zero if any licence is not allowed",
    )

    notices_parser = subparsers.add_parser(
        "notices",
        help="audit dependencies and write a Markdown notices file",
    )
    notices_parser.add_argument("file", help="output Markdown file")

    subparsers.add_parser(
        "self-test",
        help="run SPDX evaluator assertions",
    )

    args = parser.parse_args(argv)
    root = Path.cwd()

    if args.command == "check":
        return run_check(root)
    if args.command == "notices":
        return run_notices(root, Path(args.file))
    if args.command == "self-test":
        self_test()
        return 0

    return 2


if __name__ == "__main__":
    sys.exit(main())
