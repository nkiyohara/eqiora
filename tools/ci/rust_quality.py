#!/usr/bin/env python3
"""Run hosted Rust checks over the existing conservative package closure."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path
from typing import Mapping, Sequence

from classify_changes import FULL_SHA, changed_paths
from local_verify import (
    ROOT,
    WorkspacePackage,
    direct_packages,
    load_workspace,
    reverse_dependency_closure,
)


def package_selectors(
    paths: Sequence[str],
    packages: Mapping[str, WorkspacePackage],
    *,
    unsafe_mode: bool = False,
) -> tuple[str, ...]:
    """Narrow only ordinary Rust source changes with unchanged Cargo topology."""
    if unsafe_mode:
        return ("--workspace",)
    sources = []
    for path in paths:
        if path.startswith(("docs/", "rfcs/")):
            continue
        if not path.endswith(".rs") or not direct_packages([path], packages):
            return ("--workspace",)
        sources.append(path)
    if not sources:
        return ("--workspace",)
    selected = reverse_dependency_closure(direct_packages(sources, packages), packages)
    return tuple(arg for name in sorted(selected) for arg in ("-p", name))


def hosted_selectors() -> tuple[str, ...]:
    if os.environ.get("GITHUB_EVENT_NAME") != "pull_request":
        return ("--workspace",)
    try:
        event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
        pull_request = event["pull_request"]
        base = pull_request["base"]["sha"]
        head = pull_request["head"]["sha"]
        if not isinstance(base, str) or not isinstance(head, str):
            raise ValueError("invalid pull-request commits")
        if FULL_SHA.fullmatch(base) is None or FULL_SHA.fullmatch(head) is None:
            raise ValueError("invalid pull-request commits")
        actual = subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True
        ).strip()
        if actual != head:
            raise ValueError("checkout does not match the pull-request head")
        paths, unsafe_mode = changed_paths(base, head)
        return package_selectors(paths, load_workspace(ROOT), unsafe_mode=unsafe_mode)
    except (
        KeyError,
        TypeError,
        ValueError,
        OSError,
        subprocess.CalledProcessError,
    ) as error:
        print(f"Rust scope unavailable; checking the workspace: {error}", flush=True)
        return ("--workspace",)


def cargo_command(check: str, selectors: Sequence[str]) -> list[str]:
    options = {
        "clippy": [
            "--all-targets",
            "--all-features",
            "--keep-going",
            "--",
            "-D",
            "warnings",
        ],
        "test": ["--all-targets"],
        "doc": ["--no-deps"],
    }
    return ["cargo", "+stable", check, "--locked", *selectors, *options[check]]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("check", choices=("clippy", "test", "doc"))
    args = parser.parse_args()
    os.chdir(ROOT)
    os.environ["RUSTUP_TOOLCHAIN"] = "stable"
    command = cargo_command(args.check, hosted_selectors())
    print("Rust quality: " + " ".join(command), flush=True)
    return subprocess.run(command, check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
