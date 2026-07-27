#!/usr/bin/env python3
"""Classify a pull-request diff into reviewed CI ownership surfaces.

The classifier is deliberately repository-owned and dependency-free. Unknown
paths fail closed by selecting every conditional job. Non-pull-request runs
are full compatibility runs.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import PurePosixPath
from typing import Iterable


SURFACES = (
    "rust",
    "msrv",
    "python",
    "studio",
    "dependency_policy",
    "cubecl_experiment",
)
PYTHON_HOST_EVIDENCE_SURFACES = ("rust", "python")

FULL_SHA = re.compile(r"[0-9a-f]{40}")


def documentation_path(path: str) -> bool:
    """Return whether a path is owned entirely by documentation/process."""
    name = PurePosixPath(path).name
    return (
        path.startswith(("docs/", "rfcs/", ".github/ISSUE_TEMPLATE/"))
        or path == ".github/pull_request_template.md"
        or name in {"README.md", "AGENTS.md"}
        or name
        in {
            "CHANGELOG.md",
            "CODE_OF_CONDUCT.md",
            "CONTRIBUTING.md",
            "GOVERNANCE.md",
            "LICENSE",
            "NOTICE",
            "SECURITY.md",
        }
    )


def root_rust_path(path: str) -> bool:
    return (
        path.startswith(("api/", "crates/", "tools/xtask/", "verify/"))
        or path
        in {
            "Cargo.toml",
            "Cargo.lock",
            "rustfmt.toml",
        }
    )


def msrv_path(path: str) -> bool:
    return path.startswith(("crates/", "tools/xtask/")) or path in {
        "Cargo.toml",
        "Cargo.lock",
    }


def public_facade_path(path: str) -> bool:
    return path.startswith(("api/", "crates/eqiora/")) or path in {
        "Cargo.toml",
        "Cargo.lock",
    }


def dependency_policy_path(path: str) -> bool:
    name = PurePosixPath(path).name
    return (
        path in {"Cargo.toml", "Cargo.lock"}
        or path
        in {
            "studio/src-tauri/Cargo.toml",
            "studio/src-tauri/Cargo.lock",
            "studio/src-tauri/deny.toml",
        }
        or (name == "Cargo.toml" and path.startswith(("crates/", "tools/xtask/")))
        or path == "deny.toml"
        or path.startswith(".cargo/")
        or path == ".github/dependabot.yml"
    )


def classify(paths: Iterable[str], *, full: bool = False) -> dict[str, bool]:
    """Map normalized repository paths to conditional CI surfaces."""
    selected = {surface: full for surface in SURFACES}
    if full:
        return selected

    normalized: list[str] = []
    for raw_path in paths:
        path = raw_path.replace("\\", "/")
        if path.startswith("./"):
            path = path[2:]
        if not path:
            continue
        normalized.append(path)

    if normalized and all(documentation_path(path) for path in normalized):
        return selected

    for path in normalized:

        if path.startswith((".github/workflows/", "tools/ci/")):
            return {surface: True for surface in SURFACES}

        known = False
        if documentation_path(path):
            known = True
        if root_rust_path(path):
            selected["rust"] = True
            known = True
        if msrv_path(path):
            selected["msrv"] = True
            known = True
        if path.startswith(("bindings/python/", "crates/eqiora-python/")) or public_facade_path(
            path
        ):
            selected["python"] = True
            known = True
        if path.startswith("studio/") or public_facade_path(path):
            selected["studio"] = True
            known = True
        if dependency_policy_path(path):
            selected["dependency_policy"] = True
            known = True
        if path.startswith("experiments/cubecl-local-action/"):
            selected["cubecl_experiment"] = True
            known = True
        if not known:
            return {surface: True for surface in SURFACES}

    return selected


def changed_paths(base: str, head: str) -> list[str]:
    """Read the complete merge-base diff without GitHub's path-filter limit."""
    output = subprocess.run(
        ["git", "diff", "--no-renames", "--name-only", "-z", f"{base}...{head}"],
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    return [entry.decode("utf-8") for entry in output.split(b"\0") if entry]


def exact_head(expected: str) -> str:
    """Resolve HEAD and, for dispatches, require the exact requested commit."""
    actual = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout.strip()
    if expected:
        if FULL_SHA.fullmatch(expected) is None:
            raise ValueError("manual full verification requires a full lowercase commit SHA")
        if actual != expected:
            raise ValueError(f"checked out {actual}, expected exact commit {expected}")
    return actual


def render_outputs(target_sha: str, selected: dict[str, bool], *, full: bool) -> str:
    """Render stable GitHub Actions outputs."""
    lines = [f"target_sha={target_sha}", f"full={'true' if full else 'false'}"]
    lines.extend(
        f"{surface}={'true' if selected[surface] else 'false'}" for surface in SURFACES
    )
    python_host_evidence = any(
        selected[surface] for surface in PYTHON_HOST_EVIDENCE_SURFACES
    )
    lines.append(
        f"python_host_evidence={'true' if python_host_evidence else 'false'}"
    )
    versions = ["3.11", "3.12", "3.13", "3.14"] if full else ["3.11", "3.14"]
    lines.append(f"python_versions={json.dumps(versions, separators=(',', ':'))}")
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--event", required=True)
    parser.add_argument("--base", default="")
    parser.add_argument("--head", default="")
    parser.add_argument("--requested-commit", default="")
    arguments = parser.parse_args()

    try:
        target_sha = exact_head(arguments.requested_commit)
        full = arguments.event != "pull_request"
        if full:
            paths: list[str] = []
        else:
            if not arguments.base or not arguments.head:
                raise ValueError("pull-request classification requires base and head SHAs")
            paths = changed_paths(arguments.base, arguments.head)
            if not paths:
                raise ValueError("pull-request diff is unexpectedly empty")
        selected = classify(paths, full=full)
    except (OSError, subprocess.CalledProcessError, UnicodeDecodeError, ValueError) as error:
        print(f"CI classification failed: {error}", file=sys.stderr)
        return 2

    print(render_outputs(target_sha, selected, full=full))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
