#!/usr/bin/env python3
"""Check repository-local Markdown links and documentation entry points."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from urllib.parse import unquote


INLINE_LINK = re.compile(r"!?\[[^\]]*\]\(([^)]+)\)")
REFERENCE_LINK = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)", re.MULTILINE)
EXTERNAL_SCHEMES = ("http://", "https://", "mailto:")
IGNORED_PARTS = {".git", ".venv", "node_modules", "target"}


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
