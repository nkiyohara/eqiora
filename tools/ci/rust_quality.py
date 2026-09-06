#!/usr/bin/env python3
"""Run hosted Rust checks over the existing conservative package closure."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
from pathlib import Path, PurePosixPath
from typing import Mapping, Sequence

from classify_changes import FULL_SHA, changed_paths, documentation_path
from local_verify import (
    ROOT,
    WorkspacePackage,
    cli_feature_args,
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
    """Select known source owners with unchanged Cargo topology; otherwise widen."""
    if unsafe_mode:
        return ("--workspace",)
    sources = []
    source_consumers = set()
    for path in paths:
        # Git paths must be canonical before prefix-based ownership is trusted.
        if (
            not isinstance(path, str)
            or not path
            or "\\" in path
            or any(part in {"", ".", ".."} for part in path.split("/"))
            or any(ord(char) < 32 for char in path)
        ):
            return ("--workspace",)
        name = PurePosixPath(path).name
        if path.startswith(("docs/", "rfcs/")):
            continue
        owners = direct_packages([path], packages)
        if documentation_path(path) and (
            "/" not in path or owners or path == "examples/README.md"
        ):
            # Crate documentation can be a rustdoc include; package/verify
            # READMEs can be executable fixture inputs, so do not skip those.
            sources.extend([path] if owners else [])
            continue
        if (
            path.startswith("bindings/python/") and name.endswith((".py", ".pyi"))
        ) or (path.startswith("examples/python/") and path.endswith(".py")):
            # Native tests import Python source and include the public stub.
            # Python example tests exercise that same installed adapter owner.
            if not {"eqiora-python", "eqiora"}.issubset(packages):
                return ("--workspace",)
            source_consumers.update({"eqiora-python", "eqiora"})
            continue
        # .eqi examples feed build.rs and include_str! across multiple crates;
        # they need the workspace until an existing complete owner covers them.
        if name == "build.rs" or not path.endswith(".rs") or not owners:
            return ("--workspace",)
        # The facade's control-plane test reads Python adapter source directly,
        # without a Cargo dependency. Include that consumer and its dependents.
        if path.startswith("crates/eqiora-python/"):
            if "eqiora" not in packages:
                return ("--workspace",)
            source_consumers.add("eqiora")
        sources.append(path)
    if not sources and not source_consumers:
        return ("--workspace",)
    selected = reverse_dependency_closure(
        direct_packages(sources, packages) | source_consumers, packages
    )
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
    packages = ("eqiora",) if "--workspace" in selectors else selectors[1::2]
    options = {
        "clippy": [
            "--all-targets",
            "--all-features",
            "--keep-going",
            "--",
            "-D",
            "warnings",
        ],
        "test": ["--timings", *cli_feature_args(packages), "--all-targets"],
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
