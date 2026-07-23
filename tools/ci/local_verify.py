#!/usr/bin/env python3
"""Plan and run repository-owned local verification without hosted CI."""

from __future__ import annotations

import argparse
import json
import os
import shlex
import subprocess
import sys
import tomllib
from dataclasses import dataclass, field
from pathlib import Path, PurePosixPath
from typing import Iterable, Mapping, Sequence

from classify_changes import classify


ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class WorkspacePackage:
    name: str
    directory: str
    dependencies: frozenset[str]


@dataclass(frozen=True)
class PlannedCommand:
    label: str
    argv: tuple[str, ...]
    cwd: str = "."
    env: tuple[tuple[str, str], ...] = field(default_factory=tuple)

    def render(self) -> str:
        prefix = " ".join(f"{key}={shlex.quote(value)}" for key, value in self.env)
        command = shlex.join(self.argv)
        invocation = f"{prefix} {command}" if prefix else command
        return invocation if self.cwd == "." else f"(cd {shlex.quote(self.cwd)} && {invocation})"


@dataclass(frozen=True)
class VerificationPlan:
    tier: str
    paths: tuple[str, ...]
    packages: tuple[str, ...]
    cases: tuple[str, ...]
    commands: tuple[PlannedCommand, ...]
    limitations: tuple[str, ...]


def _git_paths(arguments: Sequence[str], root: Path = ROOT) -> set[str]:
    output = subprocess.run(
        ["git", *arguments],
        cwd=root,
        check=True,
        stdout=subprocess.PIPE,
    ).stdout
    return {
        entry.decode("utf-8").replace("\\", "/")
        for entry in output.split(b"\0")
        if entry
    }


def local_changed_paths(base: str, root: Path = ROOT) -> tuple[str, ...]:
    """Union committed, staged, unstaged, and untracked change paths."""
    paths = _git_paths(
        ["diff", "--no-renames", "--name-only", "-z", f"{base}...HEAD"], root
    )
    paths.update(_git_paths(["diff", "--no-renames", "--name-only", "-z"], root))
    paths.update(
        _git_paths(["diff", "--cached", "--no-renames", "--name-only", "-z"], root)
    )
    paths.update(_git_paths(["ls-files", "--others", "--exclude-standard", "-z"], root))
    return tuple(sorted(paths))


def load_workspace(root: Path = ROOT) -> dict[str, WorkspacePackage]:
    output = subprocess.run(
        ["cargo", "metadata", "--format-version=1", "--no-deps"],
        cwd=root,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
    ).stdout
    metadata = json.loads(output)
    members = set(metadata["workspace_members"])
    workspace_names = {
        package["name"]
        for package in metadata["packages"]
        if package["id"] in members
    }
    packages: dict[str, WorkspacePackage] = {}
    for package in metadata["packages"]:
        if package["id"] not in members:
            continue
        manifest = Path(package["manifest_path"]).resolve()
        directory = manifest.parent.relative_to(root.resolve()).as_posix()
        dependencies = frozenset(
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency["name"] in workspace_names
        )
        packages[package["name"]] = WorkspacePackage(
            package["name"], directory, dependencies
        )
    return packages


def direct_packages(
    paths: Iterable[str], packages: Mapping[str, WorkspacePackage]
) -> set[str]:
    normalized = set(paths)
    if normalized.intersection({"Cargo.toml", "Cargo.lock"}):
        return set(packages)
    selected: set[str] = set()
    for path in normalized:
        candidates = [
            package
            for package in packages.values()
            if path == package.directory or path.startswith(f"{package.directory}/")
        ]
        if candidates:
            selected.add(max(candidates, key=lambda package: len(package.directory)).name)
    return selected


def reverse_dependency_closure(
    selected: Iterable[str], packages: Mapping[str, WorkspacePackage]
) -> set[str]:
    closure = set(selected)
    changed = True
    while changed:
        changed = False
        for package in packages.values():
            if package.name not in closure and package.dependencies.intersection(closure):
                closure.add(package.name)
                changed = True
    return closure


def changed_case_ids(paths: Iterable[str]) -> set[str]:
    cases = set()
    for raw in paths:
        parts = PurePosixPath(raw).parts
        if len(parts) >= 3 and parts[0] == "verify":
            cases.add(f"{parts[1]}.{parts[2]}")
    return cases


def all_case_ids(root: Path = ROOT) -> set[str]:
    return {
        str(tomllib.loads(path.read_text(encoding="utf-8"))["id"])
        for path in sorted((root / "verify").glob("*/*/case.toml"))
    }


def command(label: str, *argv: str, cwd: str = ".", env: Mapping[str, str] | None = None) -> PlannedCommand:
    return PlannedCommand(
        label=label,
        argv=tuple(argv),
        cwd=cwd,
        env=tuple(sorted((env or {}).items())),
    )


def _rust_commands(packages: Iterable[str], *, rustdoc: bool) -> list[PlannedCommand]:
    selected = sorted(packages)
    if not selected:
        return []
    selectors = tuple(item for package in selected for item in ("-p", package))
    commands = [
        command("Rust tests", "cargo", "test", "--locked", *selectors, "--all-targets"),
        command(
            "Rust Clippy",
            "cargo",
            "clippy",
            *selectors,
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ),
    ]
    if rustdoc:
        commands.append(
            command(
                "Rustdoc",
                "cargo",
                "doc",
                *selectors,
                "--no-deps",
                "--locked",
                env={"RUSTDOCFLAGS": "-D warnings"},
            )
        )
    return commands


def _case_commands(cases: Iterable[str]) -> list[PlannedCommand]:
    return [
        command(
            f"Registered evidence {case}",
            "cargo",
            "run",
            "--locked",
            "-p",
            "eqiora-verify",
            "--",
            "run",
            "--case",
            case,
        )
        for case in sorted(set(cases))
    ]


def _surface_commands(surfaces: Mapping[str, bool]) -> list[PlannedCommand]:
    commands: list[PlannedCommand] = []
    if surfaces["dependency_policy"]:
        commands.extend(
            [
                command(
                    "Root dependency policy",
                    "cargo",
                    "deny",
                    "--locked",
                    "check",
                ),
                command(
                    "Studio dependency policy",
                    "cargo",
                    "deny",
                    "--all-features",
                    "--locked",
                    "--manifest-path",
                    "studio/src-tauri/Cargo.toml",
                    "--config",
                    "studio/src-tauri/deny.toml",
                    "check",
                ),
            ]
        )
    if surfaces["python"]:
        commands.append(
            command(
                "Python isolated wheel and tests",
                sys.executable,
                "tools/ci/python_package_gate.py",
            )
        )
    if surfaces["studio"]:
        commands.extend(
            [
                command("Studio quality", "npm", "run", "check", cwd="studio"),
                command("Studio unit tests", "npm", "test", cwd="studio"),
                command("Studio build", "npm", "run", "build", cwd="studio"),
                command(
                    "Studio interaction tests",
                    "npm",
                    "run",
                    "test:e2e",
                    "--",
                    "--workers=1",
                    cwd="studio",
                ),
                command(
                    "Studio native MSRV",
                    "cargo",
                    "+1.89.0",
                    "check",
                    "--manifest-path",
                    "studio/src-tauri/Cargo.toml",
                    "--locked",
                    "--all-targets",
                ),
                command(
                    "Studio native Clippy",
                    "cargo",
                    "clippy",
                    "--manifest-path",
                    "studio/src-tauri/Cargo.toml",
                    "--locked",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ),
                command(
                    "Studio native tests",
                    "cargo",
                    "test",
                    "--manifest-path",
                    "studio/src-tauri/Cargo.toml",
                    "--locked",
                ),
            ]
        )
    if surfaces["cubecl_experiment"]:
        manifest = "experiments/cubecl-local-action/Cargo.toml"
        commands.extend(
            [
                command("CubeCL Clippy", "cargo", "clippy", "--manifest-path", manifest, "--locked", "--all-targets", "--", "-D", "warnings"),
                command("CubeCL tests", "cargo", "test", "--manifest-path", manifest, "--locked"),
            ]
        )
    return commands


def build_plan(
    tier: str,
    paths: Sequence[str],
    explicit_cases: Sequence[str],
    packages: Mapping[str, WorkspacePackage],
    root: Path = ROOT,
) -> VerificationPlan:
    if tier == "periodic":
        surfaces = classify([], full=True)
        selected_packages = set(packages)
        cases = all_case_ids(root)
        commands = [
            command("CI contract tests", sys.executable, "-m", "unittest", "discover", "-s", "tools/ci/tests", "-v"),
            command("Documentation contract", sys.executable, "tools/ci/check_docs.py", "."),
            command("Formatting", "cargo", "fmt", "--all", "--", "--check"),
            command("Workspace Clippy", "cargo", "clippy", "--workspace", "--all-targets", "--all-features", "--keep-going", "--", "-D", "warnings"),
            command("Workspace tests", "cargo", "test", "--workspace", "--all-targets", "--all-features", "--locked"),
            command("All registered evidence", "cargo", "run", "--locked", "-p", "eqiora-verify", "--", "verify"),
            command("Dependency layers", "cargo", "xtask", "check-layers"),
            command("Public facade", "cargo", "xtask", "check-facade"),
            command("Workspace Rustdoc", "cargo", "doc", "--workspace", "--no-deps", "--locked", env={"RUSTDOCFLAGS": "-D warnings"}),
            command("MSRV", "cargo", "+1.89.0", "check", "--workspace", "--all-targets", "--all-features", "--locked"),
        ]
        commands.extend(_surface_commands(surfaces))
        limitations = (
            "Python coverage is the current interpreter, not the complete 3.11-3.14 matrix.",
            "Physical multi-node MPI and GPU evidence requires an explicit matching environment run.",
            "Studio native and browser commands require their documented system dependencies.",
        )
    else:
        surfaces = classify(paths)
        direct = direct_packages(paths, packages)
        selected_packages = (
            reverse_dependency_closure(direct, packages) if tier == "affected" else direct
        )
        if tier == "affected" and surfaces["rust"] and not direct:
            selected_packages = set(packages)
        cases = changed_case_ids(paths).union(explicit_cases)
        commands = []
        if surfaces["rust"] or selected_packages:
            commands.append(command("Formatting", "cargo", "fmt", "--all", "--", "--check"))
        commands.extend(_rust_commands(selected_packages, rustdoc=tier == "affected"))
        if tier == "affected":
            commands.append(
                command(
                    "Evidence manifest inventory",
                    "cargo",
                    "run",
                    "--locked",
                    "-p",
                    "eqiora-verify",
                    "--",
                    "check",
                )
            )
        commands.extend(_case_commands(cases))
        if any(path.endswith(".md") or path in {"AGENTS.md", "CONTRIBUTING.md"} for path in paths):
            commands.append(command("Documentation contract", sys.executable, "tools/ci/check_docs.py", "."))
        if any(
            PurePosixPath(path).name == "Cargo.toml"
            or path in {"Cargo.lock", "tools/xtask/src/main.rs"}
            for path in paths
        ):
            commands.append(command("Dependency layers", "cargo", "xtask", "check-layers"))
        if any(
            path
            in {
                "api/eqiora-facade-v1.json",
                "crates/eqiora/src/lib.rs",
                "tools/xtask/Cargo.toml",
                "tools/xtask/src/facade.rs",
                "tools/xtask/src/main.rs",
            }
            for path in paths
        ):
            commands.append(command("Public facade", "cargo", "xtask", "check-facade"))
        ci_contract_inputs = {
            "Cargo.toml",
            "Cargo.lock",
            "crates/eqiora-backend-faer/Cargo.toml",
            "crates/eqiora-backend-faer/src/lib.rs",
            "crates/eqiora-fabric/Cargo.toml",
            "crates/eqiora-fabric/src/lib.rs",
        }
        if any(path.startswith("tools/ci/") or path in ci_contract_inputs for path in paths):
            commands.append(command("CI contract tests", sys.executable, "-m", "unittest", "discover", "-s", "tools/ci/tests", "-v"))
        if tier == "affected":
            commands.extend(_surface_commands(surfaces))
        limitations = (
            "Affected evidence runs changed and explicitly named cases; semantic owners must name every affected case with --case.",
            "Default-feature Clippy is the local code gate; optional backend features require their registered case or an explicit environment-specific check.",
            "Environment-dependent hardware and multi-node evidence is not implied unless explicitly run and recorded.",
        )

    deduplicated = tuple(dict.fromkeys(commands))
    return VerificationPlan(
        tier=tier,
        paths=tuple(sorted(paths)),
        packages=tuple(sorted(selected_packages)),
        cases=tuple(sorted(cases)),
        commands=deduplicated,
        limitations=limitations,
    )


def render_plan(plan: VerificationPlan) -> str:
    lines = [
        f"tier: {plan.tier}",
        f"changed paths: {len(plan.paths)}",
        f"packages: {', '.join(plan.packages) if plan.packages else 'none'}",
        f"cases: {', '.join(plan.cases) if plan.cases else 'none'}",
        "commands:",
    ]
    lines.extend(f"  {index}. [{item.label}] {item.render()}" for index, item in enumerate(plan.commands, 1))
    lines.append("limitations:")
    lines.extend(f"  - {limitation}" for limitation in plan.limitations)
    return "\n".join(lines)


def run_plan(plan: VerificationPlan, root: Path = ROOT) -> None:
    for item in plan.commands:
        print(f"==> {item.label}: {item.render()}", flush=True)
        environment = os.environ.copy()
        environment.update(dict(item.env))
        subprocess.run(item.argv, cwd=root / item.cwd, env=environment, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tier", choices=("fast", "affected", "periodic"))
    parser.add_argument("--base", default="origin/main", help="merge-base comparison ref")
    parser.add_argument("--case", action="append", default=[], help="exact affected case ID")
    parser.add_argument("--plan", action="store_true", help="print without executing")
    arguments = parser.parse_args()
    try:
        paths = () if arguments.tier == "periodic" else local_changed_paths(arguments.base)
        if arguments.tier != "periodic" and not paths and not arguments.case:
            raise ValueError("no local changes or explicit verification cases were selected")
        plan = build_plan(
            arguments.tier,
            paths,
            arguments.case,
            load_workspace(),
        )
        print(render_plan(plan))
        if not arguments.plan:
            run_plan(plan)
    except (OSError, subprocess.CalledProcessError, UnicodeError, ValueError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"local verification failed: {error}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
