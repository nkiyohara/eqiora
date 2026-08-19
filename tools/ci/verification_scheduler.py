"""Resource-aware execution for deterministic local verification plans."""

from __future__ import annotations

import hashlib
import os
import shlex
import subprocess
import tempfile
from collections.abc import Callable, Iterator
from contextlib import AbstractContextManager, ExitStack, contextmanager
from dataclasses import dataclass, field
from pathlib import Path
from typing import Mapping, Sequence

from resource_scheduler import (
    ResourceBudget,
    ResourceRequest,
    ScheduledTask,
    run_tasks,
)


ROOT = Path(__file__).resolve().parents[2]
_CLI_FILESYSTEM_SOCKET_SUFFIX = Path("eqiora-cli-filesystem-4294967295-8") / "socket"
_UNIX_PATHNAME_MAX = 107


# The Cargo test profile the hosted workflow runs under. A local gate that does
# not reproduce it is not measuring the thing CI will measure: the registered
# preconditioner-scaling case takes 1150.8 s at the default `opt-level = 0` and
# 64.5 s at `opt-level = 1`, so a local run can be eighteen times slower than
# the hosted one and still be reported as the same evidence.
#
# Apply this to every planned command. The variables are test-profile scoped,
# so other commands ignore them, while Clippy and tests can share artifacts.
# `tools/ci/tests/test_ci_contracts.py` fails if this single value stops matching
# `.github/workflows/ci.yml`.
HOSTED_TEST_PROFILE = {
    "CARGO_PROFILE_TEST_DEBUG": "0",
    "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS": "true",
    "CARGO_PROFILE_TEST_INCREMENTAL": "false",
    "CARGO_PROFILE_TEST_OPT_LEVEL": "1",
    "CARGO_PROFILE_TEST_OVERFLOW_CHECKS": "true",
}


@dataclass(frozen=True)
class VerificationLane:
    name: str
    resources: ResourceRequest

    def __post_init__(self) -> None:
        if not self.name:
            raise ValueError("verification lane name cannot be empty")


def _available_memory_mib() -> int:
    try:
        for line in Path("/proc/meminfo").read_text(encoding="ascii").splitlines():
            key, value = line.split(":", maxsplit=1)
            if key == "MemAvailable":
                kibibytes, unit = value.split()
                if unit != "kB":
                    raise ValueError("unexpected MemAvailable unit")
                return max(1, int(kibibytes) // 1024)
    except (OSError, UnicodeError, ValueError):
        pass
    try:
        pages = os.sysconf("SC_AVPHYS_PAGES")
        page_size = os.sysconf("SC_PAGE_SIZE")
    except (OSError, ValueError):
        return 8 * 1024
    return max(1, pages * page_size // (1024 * 1024))


def _cpu_request(maximum: int) -> int:
    return max(1, min(maximum, os.cpu_count() or 1))


REPOSITORY_LANE = VerificationLane("repository", ResourceRequest(1, 512))
ROOT_CARGO_LANE = VerificationLane(
    "root-cargo",
    ResourceRequest(_cpu_request(4), 8 * 1024, locks=("root-cargo",)),
)
PYTHON_LANE = VerificationLane(
    "python-candidate",
    ResourceRequest(_cpu_request(2), 4 * 1024, locks=("python-candidate",)),
)
STUDIO_LANE = VerificationLane(
    "studio",
    ResourceRequest(_cpu_request(2), 4 * 1024, locks=("studio",)),
)
CUBECL_LANE = VerificationLane(
    "cubecl",
    ResourceRequest(_cpu_request(2), 4 * 1024, locks=("cubecl",)),
)
DEPENDENCY_POLICY_LANE = VerificationLane(
    "dependency-policy",
    ResourceRequest(1, 512, locks=("cargo-deny",)),
)


@dataclass(frozen=True)
class PlannedCommand:
    label: str
    argv: tuple[str, ...]
    cwd: str = "."
    env: tuple[tuple[str, str], ...] = field(default_factory=tuple)
    lane: VerificationLane = REPOSITORY_LANE

    def render(self) -> str:
        prefix = " ".join(f"{key}={shlex.quote(value)}" for key, value in self.env)
        command = shlex.join(self.argv)
        invocation = f"{prefix} {command}" if prefix else command
        return (
            invocation
            if self.cwd == "."
            else f"(cd {shlex.quote(self.cwd)} && {invocation})"
        )


@dataclass(frozen=True)
class VerificationPlan:
    tier: str
    paths: tuple[str, ...]
    packages: tuple[str, ...]
    cases: tuple[str, ...]
    commands: tuple[PlannedCommand, ...]
    limitations: tuple[str, ...]


@dataclass(frozen=True)
class CommandFailure:
    command: PlannedCommand
    returncode: int | None
    detail: str | None = None


class VerificationFailure(RuntimeError):
    def __init__(self, failures: Sequence[CommandFailure]) -> None:
        self.failures = tuple(failures)
        summary = ", ".join(self._render_failure(failure) for failure in self.failures)
        super().__init__(
            f"{len(self.failures)} verification command(s) failed: {summary}"
        )

    @staticmethod
    def _render_failure(failure: CommandFailure) -> str:
        if failure.returncode is not None:
            return f"{failure.command.label} (exit {failure.returncode})"
        return f"{failure.command.label} ({failure.detail or 'execution error'})"


@dataclass(frozen=True)
class _LaneResult:
    failures: tuple[tuple[int, CommandFailure], ...]
    skipped: tuple[int, ...]


def default_budget() -> ResourceBudget:
    gpu_slots = int(os.environ.get("EQIORA_LOCAL_VERIFY_GPU_SLOTS", "0"))
    return ResourceBudget(os.cpu_count() or 1, _available_memory_mib(), gpu_slots)


def _scratch_base(root: Path, configured: Path | None) -> Path:
    if configured is not None:
        candidate = configured.expanduser().absolute()
        if not candidate.resolve().is_relative_to(Path.home().resolve()):
            raise ValueError("scratch root must remain below the home directory")
        return candidate
    fingerprint = hashlib.sha256(str(root.resolve()).encode()).hexdigest()[:16]
    return Path.home() / ".cache" / "eqiora" / "local-verify" / fingerprint


def _lane_directory(base: Path, lane: VerificationLane) -> Path:
    slug = "".join(character if character.isalnum() else "-" for character in lane.name)
    suffix = hashlib.sha256(lane.name.encode()).hexdigest()[:8]
    return base / "lanes" / f"{slug}-{suffix}"


def _lane_tmp_scope(
    authority: Path,
    admit: Callable[[Path], None],
) -> AbstractContextManager[Path]:
    @contextmanager
    def allocated_scope() -> Iterator[Path]:
        authority.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="r-", dir=authority) as directory:
            candidate = Path(directory)
            admit(candidate)
            yield candidate

    return allocated_scope()


def _validate_lanes(
    plan: VerificationPlan, budget: ResourceBudget
) -> tuple[tuple[VerificationLane, tuple[tuple[int, PlannedCommand], ...]], ...]:
    grouped: dict[str, tuple[VerificationLane, list[tuple[int, PlannedCommand]]]] = {}
    for index, item in enumerate(plan.commands):
        existing = grouped.get(item.lane.name)
        if existing is None:
            grouped[item.lane.name] = (item.lane, [(index, item)])
            continue
        lane, commands = existing
        if lane != item.lane:
            raise ValueError(f"lane {item.lane.name!r} has inconsistent resources")
        commands.append((index, item))

    lanes = tuple((lane, tuple(commands)) for lane, commands in grouped.values())
    for lane, _commands in lanes:
        request = lane.resources
        if request.cpu_slots > budget.cpu_slots:
            raise ValueError(
                f"cpu request for lane {lane.name!r} exceeds the configured budget"
            )
        if request.memory_mib > budget.memory_mib:
            raise ValueError(
                f"memory request for lane {lane.name!r} exceeds the configured budget"
            )
        if request.gpu_slots > budget.gpu_slots:
            raise ValueError(
                f"gpu request for lane {lane.name!r} exceeds the configured budget"
            )
    return lanes


def cpu_allocations(
    lanes: Sequence[VerificationLane], budget: ResourceBudget
) -> dict[str, int]:
    total_request = sum(lane.resources.cpu_slots for lane in lanes)
    if total_request >= budget.cpu_slots:
        return {lane.name: lane.resources.cpu_slots for lane in lanes}

    weighted = [
        (budget.cpu_slots * lane.resources.cpu_slots, order, lane)
        for order, lane in enumerate(lanes)
    ]
    allocations = {
        lane.name: numerator // total_request for numerator, _order, lane in weighted
    }
    remaining = budget.cpu_slots - sum(allocations.values())
    by_remainder = sorted(
        weighted,
        key=lambda item: (-(item[0] % total_request), item[1]),
    )
    for _numerator, _order, lane in by_remainder[:remaining]:
        allocations[lane.name] += 1
    return allocations


def _run_lane(
    lane: VerificationLane,
    commands: tuple[tuple[int, PlannedCommand], ...],
    root: Path,
    lane_root: Path,
    lane_tmp: Path,
    cargo_jobs: int,
    log_paths: Mapping[int, Path],
) -> _LaneResult:
    for position, (index, item) in enumerate(commands):
        environment = os.environ.copy()
        environment.update(HOSTED_TEST_PROFILE)
        environment.update(dict(item.env))
        environment.update(
            {
                "EQIORA_VERIFY_LANE_ROOT": str(lane_root),
                "TMPDIR": str(lane_tmp),
                "CARGO_TARGET_DIR": str(lane_root / "cargo-target"),
                "CARGO_BUILD_JOBS": str(cargo_jobs),
            }
        )
        with log_paths[index].open("wb") as output:
            try:
                subprocess.run(
                    item.argv,
                    cwd=root / item.cwd,
                    env=environment,
                    check=True,
                    stdout=output,
                    stderr=subprocess.STDOUT,
                )
            except subprocess.CalledProcessError as error:
                return _LaneResult(
                    ((index, CommandFailure(item, error.returncode)),),
                    tuple(
                        skipped_index for skipped_index, _ in commands[position + 1 :]
                    ),
                )
            except OSError as error:
                return _LaneResult(
                    ((index, CommandFailure(item, None, str(error))),),
                    tuple(
                        skipped_index for skipped_index, _ in commands[position + 1 :]
                    ),
                )
    return _LaneResult((), ())


def _emit_reports(
    plan: VerificationPlan,
    log_paths: Mapping[int, Path],
    skipped: frozenset[int],
) -> None:
    for index, item in enumerate(plan.commands):
        print(f"==> {item.label} [{item.lane.name}]: {item.render()}", flush=True)
        log_path = log_paths[index]
        if log_path.exists():
            trailing_character = "\n"
            with log_path.open(encoding="utf-8", errors="replace") as output:
                while chunk := output.read(64 * 1024):
                    print(chunk, end="", flush=True)
                    trailing_character = chunk[-1]
            if trailing_character != "\n":
                print(flush=True)
        if index in skipped:
            print("skipped: an earlier command in this lane failed", flush=True)


def run_plan(
    plan: VerificationPlan,
    root: Path = ROOT,
    *,
    budget: ResourceBudget | None = None,
    scratch_root: Path | None = None,
) -> None:
    admitted_budget = budget or default_budget()
    lanes = _validate_lanes(plan, admitted_budget)
    if not lanes:
        return

    base = _scratch_base(root, scratch_root)
    tmp_authority = (
        base
        if scratch_root is not None
        else Path.home() / ".cache" / "eqiora" / "local-verify-tmp"
    )

    def admit_lane_tmp(candidate: Path) -> None:
        if not candidate.is_absolute() or not candidate.is_relative_to(tmp_authority):
            raise ValueError("lane TMPDIR must be an absolute path below its authority")
        candidate_length = len(os.fsencode(candidate / _CLI_FILESYSTEM_SOCKET_SUFFIX))
        if candidate_length > _UNIX_PATHNAME_MAX:
            raise ValueError(
                "lane TMPDIR produces a "
                f"{candidate_length}-byte Unix-socket pathname; "
                f"maximum is {_UNIX_PATHNAME_MAX} bytes"
            )

    cargo_jobs = cpu_allocations(
        tuple(lane for lane, _commands in lanes), admitted_budget
    )

    with ExitStack() as lane_scopes:
        lane_tmp_directories = {
            lane.name: lane_scopes.enter_context(
                _lane_tmp_scope(tmp_authority, admit_lane_tmp)
            )
            for lane, _commands in lanes
        }

        run_parent = base / "runs"
        run_parent.mkdir(parents=True, exist_ok=True)
        lane_directories: dict[str, Path] = {}
        for lane, _commands in lanes:
            lane_root = _lane_directory(base, lane)
            (lane_root / "cargo-target").mkdir(parents=True, exist_ok=True)
            lane_directories[lane.name] = lane_root

        with tempfile.TemporaryDirectory(
            prefix="run-", dir=run_parent
        ) as run_directory:
            run_path = Path(run_directory)
            log_paths = {
                index: run_path / f"{index:04d}.log"
                for index, _item in enumerate(plan.commands)
            }
            results: list[tuple[int, _LaneResult]] = []
            unexpected: list[tuple[int, Exception]] = []

            print(
                "==> starting lanes: " + ", ".join(lane.name for lane, _ in lanes),
                flush=True,
            )
            tasks = tuple(
                ScheduledTask(
                    lane.name,
                    lane.resources,
                    lambda lane=lane, commands=commands: _run_lane(
                        lane,
                        commands,
                        root,
                        lane_directories[lane.name],
                        lane_tmp_directories[lane.name],
                        cargo_jobs[lane.name],
                        log_paths,
                    ),
                )
                for lane, commands in lanes
            )
            for order, outcome in enumerate(run_tasks(tasks, admitted_budget)):
                if outcome.error is not None:
                    unexpected.append((order, outcome.error))
                else:
                    assert outcome.value is not None
                    results.append((order, outcome.value))

            skipped = frozenset(
                index for _order, result in results for index in result.skipped
            )
            _emit_reports(plan, log_paths, skipped)
            if unexpected:
                raise min(unexpected, key=lambda item: item[0])[1]
            failures = tuple(
                failure
                for _index, failure in sorted(
                    (
                        indexed_failure
                        for _order, result in results
                        for indexed_failure in result.failures
                    ),
                    key=lambda item: item[0],
                )
            )
            if failures:
                raise VerificationFailure(failures)
