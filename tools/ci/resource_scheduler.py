"""Small resource-aware scheduler for private in-process verification tasks."""

from __future__ import annotations

import concurrent.futures
from collections.abc import Callable, Sequence
from dataclasses import dataclass, field
from typing import Generic, TypeVar


T = TypeVar("T")


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
class ScheduledTask(Generic[T]):
    """One private callable and the resources held for its full lifetime."""

    name: str
    resources: ResourceRequest
    execute: Callable[[], T]

    def __post_init__(self) -> None:
        if not self.name:
            raise ValueError("scheduled task name cannot be empty")


@dataclass(frozen=True)
class TaskOutcome(Generic[T]):
    """A task result retained in declaration order, including failures."""

    name: str
    value: T | None
    error: Exception | None


def _validate_tasks(tasks: Sequence[ScheduledTask[T]], budget: ResourceBudget) -> None:
    names = tuple(task.name for task in tasks)
    if len(set(names)) != len(names):
        raise ValueError("duplicate scheduled task identity")
    for task in tasks:
        request = task.resources
        if request.cpu_slots > budget.cpu_slots:
            raise ValueError(
                f"cpu request for task {task.name!r} exceeds the configured budget"
            )
        if request.memory_mib > budget.memory_mib:
            raise ValueError(
                f"memory request for task {task.name!r} exceeds the configured budget"
            )
        if request.gpu_slots > budget.gpu_slots:
            raise ValueError(
                f"gpu request for task {task.name!r} exceeds the configured budget"
            )


def _fits(
    request: ResourceRequest,
    *,
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


def run_tasks(
    tasks: Sequence[ScheduledTask[T]], budget: ResourceBudget
) -> tuple[TaskOutcome[T], ...]:
    """Run every admitted task once and return outcomes in declaration order."""

    admitted = tuple(tasks)
    _validate_tasks(admitted, budget)
    if not admitted:
        return ()

    available_cpu = budget.cpu_slots
    available_memory = budget.memory_mib
    available_gpu = budget.gpu_slots
    active_locks: set[str] = set()
    pending = list(enumerate(admitted))
    active: dict[concurrent.futures.Future[T], tuple[int, ScheduledTask[T]]] = {}
    outcomes: list[TaskOutcome[T] | None] = [None] * len(admitted)

    with concurrent.futures.ThreadPoolExecutor(
        max_workers=len(admitted), thread_name_prefix="eqiora-resource"
    ) as executor:
        while pending or active:
            for candidate in tuple(pending):
                order, task = candidate
                request = task.resources
                if not _fits(
                    request,
                    available_cpu=available_cpu,
                    available_memory=available_memory,
                    available_gpu=available_gpu,
                    active_locks=frozenset(active_locks),
                ):
                    continue
                pending.remove(candidate)
                available_cpu -= request.cpu_slots
                available_memory -= request.memory_mib
                available_gpu -= request.gpu_slots
                active_locks.update(request.locks)
                active[executor.submit(task.execute)] = (order, task)

            if not active:
                raise RuntimeError("resource scheduler made no progress")

            completed, _ = concurrent.futures.wait(
                active, return_when=concurrent.futures.FIRST_COMPLETED
            )
            for future in sorted(completed, key=lambda item: active[item][0]):
                order, task = active.pop(future)
                request = task.resources
                available_cpu += request.cpu_slots
                available_memory += request.memory_mib
                available_gpu += request.gpu_slots
                active_locks.difference_update(request.locks)
                try:
                    outcomes[order] = TaskOutcome(task.name, future.result(), None)
                except Exception as error:
                    outcomes[order] = TaskOutcome(task.name, None, error)

    if any(outcome is None for outcome in outcomes):  # pragma: no cover
        raise RuntimeError("resource scheduler omitted a task outcome")
    return tuple(outcome for outcome in outcomes if outcome is not None)
