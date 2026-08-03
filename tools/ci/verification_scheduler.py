"""Resource-aware execution for deterministic local verification plans."""

from __future__ import annotations

import concurrent.futures
import hashlib
import os
import shlex
import shutil
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path
from typing import Mapping, Sequence


ROOT = Path(__file__).resolve().parents[2]


# Keep this identical to the environment in `.github/workflows/ci.yml`. The CI
# contract tests compare the hosted workflow with this single Python value.
HOSTED_TEST_PROFILE = {
    "CARGO_PROFILE_TEST_DEBUG": "0",
    "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS": "true",
    "CARGO_PROFILE_TEST_INCREMENTAL": "false",
    "CARGO_PROFILE_TEST_OPT_LEVEL": "1",
    "CARGO_PROFILE_TEST_OVERFLOW_CHECKS": "true",
}


@dataclass(frozen=True)
class ResourceBudget:
    cpu_slots: int
    memory_mib: int
    gpu_slots: int = 0

    def __post_init__(self) -> None:
        if self.cpu_slots < 1:
            raise ValueError("cpu budget must contain at least one slot")
        if self.memory_mib < 1:
            raise ValueError("memory budget must contain at least one MiB")
        if self.gpu_slots < 0:
            raise ValueError("gpu budget cannot be negative")


@dataclass(frozen=True)
class ResourceRequest:
    cpu_slots: int
    memory_mib: int
    gpu_slots: int = 0
    locks: tuple[str, ...] = field(default_factory=tuple)

    def __post_init__(self) -> None:
        if self.cpu_slots < 1:
            raise ValueError("cpu request must contain at least one slot")
        if self.memory_mib < 1:
            raise ValueError("memory request must contain at least one MiB")
        if self.gpu_slots < 0:
            raise ValueError("gpu request cannot be negative")
        if any(not lock for lock in self.locks):
            raise ValueError("resource lock names cannot be empty")
        if tuple(sorted(set(self.locks))) != self.locks:
            raise ValueError("resource lock names must be sorted and unique")


@dataclass(frozen=True)
class VerificationLane:
    name: str
    resources: ResourceRequest

    def __post_init__(self) -> None:
        if not self.name:
            raise ValueError("verification lane name cannot be empty")


def _available_memory_mib() -> int:
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
    returncode: int


class VerificationFailure(RuntimeError):
    def __init__(self, failures: Sequence[CommandFailure]) -> None:
        self.failures = tuple(failures)
        summary = ", ".join(
            f"{failure.command.label} (exit {failure.returncode})"
            for failure in self.failures
        )
        super().__init__(
            f"{len(self.failures)} verification command(s) failed: {summary}"
        )


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


def _run_lane(
    lane: VerificationLane,
    commands: tuple[tuple[int, PlannedCommand], ...],
    root: Path,
    lane_root: Path,
    lane_tmp: Path,
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
                "CARGO_BUILD_JOBS": str(lane.resources.cpu_slots),
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
    return _LaneResult((), ())


def _fits(
    request: ResourceRequest,
    available_cpu: int,
    available_memory: int,
    available_gpu: int,
    active_locks: frozenset[str],
) -> bool:
    return (
        request.cpu_slots <= available_cpu
        and request.memory_mib <= available_memory
        and request.gpu_slots <= available_gpu
        and active_locks.isdisjoint(request.locks)
    )


def _emit_reports(
    plan: VerificationPlan,
    log_paths: Mapping[int, Path],
    skipped: frozenset[int],
) -> None:
    for index, item in enumerate(plan.commands):
        print(f"==> {item.label} [{item.lane.name}]: {item.render()}", flush=True)
        log_path = log_paths[index]
        if log_path.exists():
            output = log_path.read_bytes().decode("utf-8", errors="replace")
            if output:
                print(output, end="" if output.endswith("\n") else "\n", flush=True)
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
    run_parent = base / "runs"
    run_parent.mkdir(parents=True, exist_ok=True)
    lane_directories: dict[str, Path] = {}
    lane_tmp_directories: dict[str, Path] = {}

    with tempfile.TemporaryDirectory(prefix="run-", dir=run_parent) as run_directory:
        run_path = Path(run_directory)
        log_paths = {
            index: run_path / f"{index:04d}.log"
            for index, _item in enumerate(plan.commands)
        }
        for lane, _commands in lanes:
            lane_root = _lane_directory(base, lane)
            (lane_root / "cargo-target").mkdir(parents=True, exist_ok=True)
            tmp_parent = lane_root / "tmp"
            tmp_parent.mkdir(parents=True, exist_ok=True)
            lane_tmp = Path(tempfile.mkdtemp(prefix="run-", dir=tmp_parent))
            lane_directories[lane.name] = lane_root
            lane_tmp_directories[lane.name] = lane_tmp

        available_cpu = admitted_budget.cpu_slots
        available_memory = admitted_budget.memory_mib
        available_gpu = admitted_budget.gpu_slots
        active_locks: set[str] = set()
        pending = list(enumerate(lanes))
        active: dict[
            concurrent.futures.Future[_LaneResult], tuple[int, VerificationLane]
        ] = {}
        results: list[tuple[int, _LaneResult]] = []
        unexpected: list[tuple[int, Exception]] = []

        print(
            "==> starting lanes: " + ", ".join(lane.name for lane, _ in lanes),
            flush=True,
        )
        try:
            with concurrent.futures.ThreadPoolExecutor(
                max_workers=len(lanes), thread_name_prefix="eqiora-verify"
            ) as executor:
                while pending or active:
                    for pending_item in tuple(pending):
                        order, (lane, commands) = pending_item
                        if not _fits(
                            lane.resources,
                            available_cpu,
                            available_memory,
                            available_gpu,
                            frozenset(active_locks),
                        ):
                            continue
                        pending.remove(pending_item)
                        request = lane.resources
                        available_cpu -= request.cpu_slots
                        available_memory -= request.memory_mib
                        available_gpu -= request.gpu_slots
                        active_locks.update(request.locks)
                        future = executor.submit(
                            _run_lane,
                            lane,
                            commands,
                            root,
                            lane_directories[lane.name],
                            lane_tmp_directories[lane.name],
                            log_paths,
                        )
                        active[future] = (order, lane)

                    if not active:
                        raise RuntimeError("verification scheduler made no progress")

                    completed, _ = concurrent.futures.wait(
                        active, return_when=concurrent.futures.FIRST_COMPLETED
                    )
                    for future in sorted(completed, key=lambda item: active[item][0]):
                        order, lane = active.pop(future)
                        request = lane.resources
                        available_cpu += request.cpu_slots
                        available_memory += request.memory_mib
                        available_gpu += request.gpu_slots
                        active_locks.difference_update(request.locks)
                        try:
                            results.append((order, future.result()))
                        except Exception as error:
                            unexpected.append((order, error))

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
        finally:
            for lane_tmp in lane_tmp_directories.values():
                shutil.rmtree(lane_tmp, ignore_errors=True)
