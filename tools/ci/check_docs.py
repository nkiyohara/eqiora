#!/usr/bin/env python3
"""Check repository-local Markdown links and documentation entry points."""

from __future__ import annotations

import argparse
import re
import sys
import tomllib
from pathlib import Path
from urllib.parse import unquote


INLINE_LINK = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
REFERENCE_LINK = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)", re.MULTILINE)
EXTERNAL_SCHEMES = ("http://", "https://", "mailto:")
IGNORED_PARTS = {".git", ".venv", "node_modules", "target"}

BENCHMARKS = "docs/benchmarks.md"
CITATION = re.compile(r"`(case|symbol|key):([A-Za-z0-9_.\-]+)`")
UNCITED = "none declared"
CITED_SECTIONS = (
    "## Reproduced today",
    "## Reachable without new numerical capability",
    "## Needs new numerical capability",
)


def benchmark_failures(root: Path) -> list[str]:
    """Every benchmark row states a resolvable capability or says it declares none.

    A row whose citation stops resolving is a row describing something the
    repository no longer has. Rows are checked as well as citations, so deleting
    a citation cannot quietly turn a claim into prose.
    """
    document = root / BENCHMARKS
    if not document.exists():
        return []
    source = document.read_text(encoding="utf-8")
    failures: list[str] = []

    section = ""
    for line in source.splitlines():
        if line.startswith("## "):
            section = line
        elif (
            section in CITED_SECTIONS
            and line.startswith("|")
            and set(line) - set("|-: ")
            and not CITATION.search(line)
            and UNCITED not in line
            and not line.startswith("| Problem")
        ):
            failures.append(f"{BENCHMARKS}: row cites no capability: {line[:60]}")

    statuses = {
        str(tomllib.loads(path.read_text(encoding="utf-8"))["id"]): str(
            tomllib.loads(path.read_text(encoding="utf-8")).get("status", "")
        )
        for path in sorted((root / "verify").glob("*/*/case.toml"))
    }
    manifests = "\n".join(
        path.read_text(encoding="utf-8") for path in sorted((root / "verify").glob("*/*/case.toml"))
    )
    sources = [
        path.read_text(encoding="utf-8", errors="replace")
        for path in sorted((root / "crates").rglob("*.rs"))
        if not IGNORED_PARTS.intersection(path.parts)
    ]

    for kind, name in CITATION.findall(source):
        if kind == "case":
            if name not in statuses:
                failures.append(f"{BENCHMARKS}: cites unknown case {name}")
            elif statuses[name] != "verified":
                failures.append(f"{BENCHMARKS}: cites case {name}, whose status is {statuses[name]!r}")
        elif kind == "key":
            if name not in manifests:
                failures.append(f"{BENCHMARKS}: cites manifest key {name}, declared by no case")
        elif not any(name in text for text in sources):
            failures.append(f"{BENCHMARKS}: cites symbol {name}, present in no Rust source")
    return failures



def link_targets(source: str) -> list[str]:
    targets = [match.group(1).strip() for match in INLINE_LINK.finditer(source)]
    targets.extend(match.group(1).strip() for match in REFERENCE_LINK.finditer(source))
    return targets


def local_target(raw: str) -> str | None:
    target = raw.strip("<>")
    if not target or target.startswith(("#", "/", *EXTERNAL_SCHEMES)):
        return None
    if " " in target:
        target = target.split(" ", 1)[0]
    return unquote(target.split("#", 1)[0]) or None


def check(root: Path) -> list[str]:
    root = root.resolve()
    failures: list[str] = []
    for document in sorted(root.rglob("*.md")):
        if IGNORED_PARTS.intersection(document.parts):
            continue
        source = document.read_text(encoding="utf-8")
        for raw in link_targets(source):
            relative = local_target(raw)
            if relative is None:
                continue
            resolved = (document.parent / relative).resolve()
            try:
                resolved.relative_to(root)
            except ValueError:
                failures.append(f"{document.relative_to(root)}: link escapes repository: {raw}")
                continue
            if not resolved.exists():
                failures.append(f"{document.relative_to(root)}: missing local link target: {raw}")

    required = {
        "README.md": "docs/capability-matrix.md",
        "AGENTS.md": "docs/capability-matrix.md",
        "CONTRIBUTING.md": "docs/capability-matrix.md",
    }
    for source_name, target in required.items():
        source = (root / source_name).read_text(encoding="utf-8")
        if target not in source:
            failures.append(f"{source_name}: must reference {target}")
    failures.extend(benchmark_failures(root))
    return failures


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", nargs="?", default=".")
    root = Path(parser.parse_args().root)
    try:
        failures = check(root)
    except (OSError, UnicodeError) as error:
        print(f"documentation check failed: {error}", file=sys.stderr)
        return 2
    if failures:
        for failure in failures:
            print(failure, file=sys.stderr)
        return 1
    print("documentation links and entry points are valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
