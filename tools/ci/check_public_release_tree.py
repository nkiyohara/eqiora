#!/usr/bin/env python3
"""Reject repository-local details that must not enter a public source export."""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


IGNORED_DIRECTORIES = frozenset(
    {
        ".git",
        ".mypy_cache",
        ".pytest_cache",
        ".ruff_cache",
        ".venv",
        "__pycache__",
        "node_modules",
        "target",
    }
)

# These expressions are assembled from semantic fragments so this checker does
# not reject its own source as an example of the forbidden value.
PRIVATE_ISSUE_URL = re.compile(
    re.escape(
        "https://"
        + "github.com/"
        + "nkiyohara/"
        + "eqiora/"
        + "issues/"
    )
    + r"[0-9]+\b"
)
BARE_TRACKING_REFERENCE = re.compile(
    r"\b(?:issue|pull\s+request|PR)\s+#?[0-9]+\b",
    re.IGNORECASE,
)
PERSONAL_PATH = re.compile(
    r"(?:"
    + re.escape("/" + "home" + "/")
    + r"[A-Za-z0-9._-]+/"
    + "|"
    + re.escape("/" + "Users" + "/")
    + r"[A-Za-z0-9._-]+/"
    + "|[A-Za-z]:"
    + re.escape("\\" + "Users" + "\\")
    + r"[^\\\s\"']+\\"
    + ")"
)
STALE_RELEASE_PHRASES = (
    re.compile(r"pending\s+public\s+repository", re.IGNORECASE),
    re.compile(r"private[-\s]+bootstrap", re.IGNORECASE),
    re.compile(r"pre-public[-\s]+bootstrap", re.IGNORECASE),
    re.compile(r"repository\s+remains\s+private", re.IGNORECASE),
    re.compile(
        r"(?:once|when)\s+the\s+public\s+(?:GitHub\s+)?repository\s+"
        r"(?:is\s+available|exists)",
        re.IGNORECASE,
    ),
)
GPU_UUID = re.compile(
    r"\bGPU-[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}\b"
)
PCI_ADDRESS = re.compile(
    r"\b(?:[0-9A-Fa-f]{8}|[0-9A-Fa-f]{4}):"
    r"[0-9A-Fa-f]{2}:[0-9A-Fa-f]{2}\.[0-7]\b"
)
RAW_UUID = re.compile(
    r"\b[0-9A-Fa-f]{8}(?:-[0-9A-Fa-f]{4}){3}-[0-9A-Fa-f]{12}\b"
)
HOST_IDENTITY_KEYS = frozenset(
    {
        "device_uuid",
        "gpu_uuid",
        "host_name",
        "hostname",
        "machine_id",
        "machine_uuid",
    }
)
PROCESS_ID_KEYS = frozenset({"parent_pid", "pid", "process_id"})


@dataclass(frozen=True, order=True)
class Finding:
    path: str
    line: int
    rule: str
    detail: str

    def render(self) -> str:
        return f"{self.path}:{self.line}: {self.rule}: {self.detail}"


def _line_number(source: str, offset: int) -> int:
    return source.count("\n", 0, max(offset, 0)) + 1


def _text_findings(relative: str, source: str) -> list[Finding]:
    findings: list[Finding] = []
    rules = (
        (
            "private-issue-url",
            PRIVATE_ISSUE_URL,
            "replace private tracking history with stable RFC, evidence, or dependency text",
        ),
        (
            "personal-path",
            PERSONAL_PATH,
            "remove a user-specific filesystem path",
        ),
        (
            "bare-tracking-reference",
            BARE_TRACKING_REFERENCE,
            "replace repository-numbered history with a stable RFC or evidence link",
        ),
    )
    for rule, pattern, detail in rules:
        findings.extend(
            Finding(relative, _line_number(source, match.start()), rule, detail)
            for match in pattern.finditer(source)
        )
    for pattern in STALE_RELEASE_PHRASES:
        findings.extend(
            Finding(
                relative,
                _line_number(source, match.start()),
                "stale-release-state",
                "describe the current public repository rather than bootstrap state",
            )
            for match in pattern.finditer(source)
        )
    return findings


def _json_line(source: str, key: str, value: object) -> int:
    key_offset = source.find(json.dumps(key))
    if key_offset >= 0:
        return _line_number(source, key_offset)
    if isinstance(value, str):
        value_offset = source.find(json.dumps(value))
        if value_offset >= 0:
            return _line_number(source, value_offset)
    return 1


def _observation_findings(relative: str, source: str) -> list[Finding]:
    try:
        document = json.loads(source)
    except json.JSONDecodeError:
        # Syntax validity belongs to the evidence-schema checks. This check is
        # intentionally limited to public-release hygiene.
        return []

    findings: list[Finding] = []

    def visit(value: object, path: tuple[str, ...]) -> None:
        if isinstance(value, dict):
            for key, child in value.items():
                normalized = key.casefold()
                location = _json_line(source, key, child)
                if (
                    normalized in HOST_IDENTITY_KEYS
                    or normalized.startswith("load_average")
                ):
                    findings.append(
                        Finding(
                            relative,
                            location,
                            "host-identity",
                            f"remove host-identifying observation field {key!r}",
                        )
                    )
                if normalized in PROCESS_ID_KEYS:
                    findings.append(
                        Finding(
                            relative,
                            location,
                            "process-identity",
                            f"remove process identifier field {key!r}",
                        )
                    )
                if (
                    "uuid" in normalized
                    and isinstance(child, str)
                    and RAW_UUID.search(child)
                ):
                    findings.append(
                        Finding(
                            relative,
                            location,
                            "raw-uuid",
                            f"remove hardware identifier field {key!r}",
                        )
                    )
                visit(child, (*path, key))
        elif isinstance(value, list):
            for index, child in enumerate(value):
                visit(child, (*path, str(index)))
        elif isinstance(value, str):
            location = _json_line(source, path[-1] if path else "", value)
            if GPU_UUID.search(value):
                findings.append(
                    Finding(
                        relative,
                        location,
                        "gpu-uuid",
                        "replace the raw device UUID with a non-identifying ordinal",
                    )
                )
            if PCI_ADDRESS.search(value):
                findings.append(
                    Finding(
                        relative,
                        location,
                        "pci-address",
                        "remove the host-specific PCI address",
                    )
                )

    visit(document, ())
    return findings


def _candidate_files(root: Path) -> Iterable[Path]:
    for directory, child_directories, file_names in os.walk(root):
        child_directories[:] = sorted(
            name for name in child_directories if name not in IGNORED_DIRECTORIES
        )
        base = Path(directory)
        for name in sorted(file_names):
            path = base / name
            if path == root / ".git":
                continue
            if path.is_file() and not path.is_symlink():
                yield path


def _read_text(path: Path) -> str | None:
    try:
        data = path.read_bytes()
    except OSError:
        raise
    if b"\0" in data:
        return None
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return None


def check(root: Path) -> list[Finding]:
    root = root.resolve()
    findings: list[Finding] = []
    for path in _candidate_files(root):
        source = _read_text(path)
        if source is None:
            continue
        relative_path = path.relative_to(root)
        relative = relative_path.as_posix()
        findings.extend(_text_findings(relative, source))
        if (
            path.suffix == ".json"
            and "verify" in relative_path.parts
            and "observations" in relative_path.parts
        ):
            findings.extend(_observation_findings(relative, source))
    return sorted(set(findings))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("root", nargs="?", default=".")
    root = Path(parser.parse_args().root)
    try:
        findings = check(root)
    except OSError as error:
        print(f"public release tree check failed: {error}", file=sys.stderr)
        return 2
    if findings:
        for finding in findings:
            print(finding.render(), file=sys.stderr)
        return 1
    print("public release tree contains no repository-local release leaks")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
