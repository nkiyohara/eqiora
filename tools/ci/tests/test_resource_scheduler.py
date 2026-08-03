from __future__ import annotations

import importlib
import sys
import threading
import time
import unittest
from pathlib import Path


CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))


def scheduler_api() -> tuple[object, object, object, object]:
    """Import inside each test so the pre-implementation suite still collects."""

    scheduler = importlib.import_module("resource_scheduler")
    return (
        scheduler.ResourceBudget,
        scheduler.ResourceRequest,
        scheduler.ScheduledTask,
        scheduler.run_tasks,
    )


class CallableResourceSchedulerTests(unittest.TestCase):
    def test_disjoint_callables_overlap_once_and_return_in_plan_order(self) -> None:
        ResourceBudget, ResourceRequest, ScheduledTask, run_tasks = scheduler_api()
        rendezvous = threading.Barrier(2)
        calls: list[str] = []
        completions: list[str] = []
        lock = threading.Lock()

        def execute(name: str, delay: float) -> str:
            with lock:
                calls.append(name)
            rendezvous.wait(timeout=1.0)
            time.sleep(delay)
            with lock:
                completions.append(name)
            return f"receipt:{name}"

        outcomes = run_tasks(
            (
                ScheduledTask(
                    "slow",
                    ResourceRequest(1, 1),
                    lambda: execute("slow", 0.05),
                ),
                ScheduledTask(
                    "fast",
                    ResourceRequest(1, 1),
                    lambda: execute("fast", 0.0),
                ),
            ),
            ResourceBudget(2, 2),
        )

        self.assertCountEqual(calls, ["slow", "fast"])
        self.assertEqual(completions, ["fast", "slow"])
        self.assertEqual([outcome.name for outcome in outcomes], ["slow", "fast"])
        self.assertEqual(
            [outcome.value for outcome in outcomes],
            ["receipt:slow", "receipt:fast"],
        )
        self.assertTrue(all(outcome.error is None for outcome in outcomes))

    def test_cpu_memory_and_named_locks_each_prevent_overlap(self) -> None:
        ResourceBudget, ResourceRequest, ScheduledTask, run_tasks = scheduler_api()
        scenarios = {
            "cpu": (
                ResourceRequest(2, 1),
                ResourceRequest(2, 1),
                ResourceBudget(2, 2),
            ),
            "memory": (
                ResourceRequest(1, 2),
                ResourceRequest(1, 2),
                ResourceBudget(2, 2),
            ),
            "named lock": (
                ResourceRequest(1, 1, locks=("exclusive",)),
                ResourceRequest(1, 1, locks=("exclusive",)),
                ResourceBudget(2, 2),
            ),
        }
        for name, (first_request, second_request, budget) in scenarios.items():
            with self.subTest(resource=name):
                active = 0
                maximum_active = 0
                calls: list[str] = []
                lock = threading.Lock()

                def execute(task_name: str) -> str:
                    nonlocal active, maximum_active
                    with lock:
                        calls.append(task_name)
                        active += 1
                        maximum_active = max(maximum_active, active)
                    time.sleep(0.02)
                    with lock:
                        active -= 1
                    return task_name

                outcomes = run_tasks(
                    (
                        ScheduledTask("first", first_request, lambda: execute("first")),
                        ScheduledTask(
                            "second", second_request, lambda: execute("second")
                        ),
                    ),
                    budget,
                )

                self.assertEqual(calls, ["first", "second"])
                self.assertEqual(maximum_active, 1)
                self.assertEqual(
                    [outcome.value for outcome in outcomes], ["first", "second"]
                )

    def test_already_running_failures_are_joined_and_frozen_ordered(self) -> None:
        ResourceBudget, ResourceRequest, ScheduledTask, run_tasks = scheduler_api()
        rendezvous = threading.Barrier(2)

        def fail(message: str, delay: float) -> None:
            rendezvous.wait(timeout=1.0)
            time.sleep(delay)
            raise RuntimeError(message)

        outcomes = run_tasks(
            (
                ScheduledTask(
                    "first-profile",
                    ResourceRequest(1, 1),
                    lambda: fail("first diagnostic", 0.05),
                ),
                ScheduledTask(
                    "second-profile",
                    ResourceRequest(1, 1),
                    lambda: fail("second diagnostic", 0.0),
                ),
            ),
            ResourceBudget(2, 2),
        )

        self.assertEqual(
            [outcome.name for outcome in outcomes],
            ["first-profile", "second-profile"],
        )
        self.assertEqual(
            [str(outcome.error) for outcome in outcomes],
            ["first diagnostic", "second diagnostic"],
        )
        self.assertTrue(all(outcome.value is None for outcome in outcomes))

    def test_oversized_request_rejects_before_any_callable_starts(self) -> None:
        ResourceBudget, ResourceRequest, ScheduledTask, run_tasks = scheduler_api()
        calls: list[str] = []
        scenarios = {
            "cpu": (ResourceRequest(2, 1), ResourceBudget(1, 1)),
            "memory": (ResourceRequest(1, 2), ResourceBudget(1, 1)),
        }
        for name, (request, budget) in scenarios.items():
            with (
                self.subTest(resource=name),
                self.assertRaisesRegex(ValueError, name),
            ):
                run_tasks(
                    (ScheduledTask("forbidden", request, lambda: calls.append(name)),),
                    budget,
                )
        self.assertEqual(calls, [])

    def test_duplicate_task_identity_rejects_before_execution(self) -> None:
        ResourceBudget, ResourceRequest, ScheduledTask, run_tasks = scheduler_api()
        calls: list[str] = []
        tasks = (
            ScheduledTask(
                "duplicate", ResourceRequest(1, 1), lambda: calls.append("first")
            ),
            ScheduledTask(
                "duplicate", ResourceRequest(1, 1), lambda: calls.append("second")
            ),
        )

        with self.assertRaisesRegex(ValueError, "duplicate"):
            run_tasks(tasks, ResourceBudget(2, 2))
        self.assertEqual(calls, [])


if __name__ == "__main__":
    unittest.main()
