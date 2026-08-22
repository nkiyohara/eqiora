from __future__ import annotations

import contextlib
import hashlib
import importlib
import os
import signal
import subprocess
import sys
import tempfile
import time
import types
import unittest
from collections.abc import Callable
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))

import python_candidate as python_candidate_module  # noqa: E402


CandidateError = python_candidate_module.CandidateError


class NotebookZombieReapEvidence(unittest.TestCase):
    """Independent evidence for notebook zombie membership and reaping."""

    HOST = """
import os
import time

release_fd = int(os.environ["EQIORA_RELEASE_FD"])
mode = os.environ["EQIORA_HOST_MODE"]
if mode == "live":
    while True:
        time.sleep(1.0)
os.read(release_fd, 1)
os._exit(int(os.environ["EQIORA_EXIT_STATUS"]))
"""

    DESCENDANT_HOST = """
import os
import time

release_fd = int(os.environ["EQIORA_RELEASE_FD"])
child_pid_path = os.environ["EQIORA_CHILD_PID_PATH"]
reaped_path = os.environ["EQIORA_REAPED_PATH"]
child = os.fork()
if child == 0:
    os._exit(0)
with open(child_pid_path, "x", encoding="ascii") as stream:
    stream.write(str(child))
os.read(release_fd, 1)
os.waitpid(child, 0)
with open(reaped_path, "x", encoding="ascii") as stream:
    stream.write("reaped\\n")
while True:
    time.sleep(1.0)
"""

    @staticmethod
    def _proc_identity(pid: int) -> tuple[str, int] | None:
        try:
            record = (Path("/proc") / str(pid) / "stat").read_text(encoding="ascii")
        except (FileNotFoundError, ProcessLookupError):
            return None
        comm_end = record.rfind(")")
        if comm_end < 0:
            raise AssertionError(f"malformed /proc identity for PID {pid}")
        fields = record[comm_end + 2 :].split()
        if len(fields) <= 19:
            raise AssertionError(f"incomplete /proc identity for PID {pid}")
        return fields[0], int(fields[19])

    @classmethod
    def _wait_for_state(cls, pid: int, expected: str) -> tuple[str, int]:
        deadline = time.monotonic() + 2.0
        while time.monotonic() < deadline:
            identity = cls._proc_identity(pid)
            if identity is not None and identity[0] == expected:
                return identity
            time.sleep(0.005)
        raise AssertionError(
            f"PID {pid} did not causally reach Linux state {expected!r}"
        )

    @classmethod
    def _wait_for_absence(cls, pid: int) -> None:
        deadline = time.monotonic() + 2.0
        while time.monotonic() < deadline:
            if cls._proc_identity(pid) is None:
                return
            time.sleep(0.005)
        raise AssertionError(f"PID {pid} remained present after parent acknowledgement")

    @staticmethod
    def _survivor(*, state: str = "Z") -> dict[str, object]:
        return {
            "scenario": "jupyterlab-4.6.2",
            "role": "host",
            "pid": 41_029,
            "start_time": 908_172,
            "state": state,
            "requested_stages": (),
            "stage_results": (),
            "authority_denied": False,
        }

    def _run_outer_host(
        self,
        root: Path,
        *,
        host_mode: str,
        exit_status: int = 0,
        lifecycle_trigger: str | None = None,
        primary_error: BaseException | None = None,
    ) -> dict[str, object]:
        """Capture and execute the real callbacks supplied by the outer seam."""

        executor = importlib.import_module("python_candidate_h2")
        profiles = importlib.import_module("python_candidate_profiles")
        extracted = root / "extracted"
        fixture = (
            extracted
            / "bindings/python/tests/fixtures/rich_mesh_display/jupyterlab.ipynb"
        )
        fixture.parent.mkdir(parents=True)
        fixture.write_text("{}", encoding="utf-8")
        rich_test = extracted / "bindings/python/tests/test_rich_mesh_display.py"
        rich_test.parent.mkdir(parents=True, exist_ok=True)
        rich_test.write_text("def test_placeholder():\n    pass\n", encoding="utf-8")

        browser = root / "browser"
        npm = root / "npm"
        node = root / "node"
        for path in (browser, npm, node):
            path.write_bytes(path.name.encode("ascii"))
        acquired = types.SimpleNamespace(
            browser_archive_sha256="a" * 64,
            browser_executable_sha256="b" * 64,
            browser_platform="linux-x86_64",
            browser_executable=browser,
            python_wheels=(),
            npm=npm,
            node=node,
        )
        receipt = {
            "browser": {
                "downloaded_archive_sha256": acquired.browser_archive_sha256,
                "executable_sha256": acquired.browser_executable_sha256,
                "platform": acquired.browser_platform,
            },
            "python_host": {
                "resolved_environment_sha256": executor.structured_sha256(())
            },
        }
        frontend = {
            "h2_receipt_sha256": hashlib.sha256(
                executor.canonical_json_bytes(receipt)
            ).hexdigest()
        }
        workspace_root = root / "profile"
        workspace = types.SimpleNamespace(
            root=workspace_root,
            environment=workspace_root / "environment",
            consumer=workspace_root / "consumer",
        )
        release_read, release_write = os.pipe()
        os.set_inheritable(release_read, True)
        trace: dict[str, object] = {
            "events": [],
            "observations": [],
            "requests": [],
            "waits": [],
            "pidfd_opens": [],
            "pidfd_sends": [],
            "fallback_signals": [],
            "popen_waits": [],
            "error": None,
        }
        events = trace["events"]
        observations = trace["observations"]
        requests = trace["requests"]
        waits = trace["waits"]
        process: subprocess.Popen[str] | None = None
        harness_cleanup = False
        real_popen = subprocess.Popen
        real_pidfd_open = os.pidfd_open
        real_pidfd_send_signal = signal.pidfd_send_signal
        real_os_kill = os.kill
        real_lifecycle = python_candidate_module._notebook_cleanup_lifecycle

        def launch_host(
            _argv: list[str],
            **kwargs: object,
        ) -> subprocess.Popen[str]:
            nonlocal process
            environment = dict(kwargs["env"])
            environment.update(
                {
                    "EQIORA_RELEASE_FD": str(release_read),
                    "EQIORA_HOST_MODE": host_mode,
                    "EQIORA_EXIT_STATUS": str(exit_status),
                }
            )
            process = real_popen(
                [sys.executable, "-I", "-c", self.HOST],
                cwd=kwargs["cwd"],
                env=environment,
                stdout=kwargs["stdout"],
                stderr=kwargs["stderr"],
                text=kwargs["text"],
                start_new_session=True,
                pass_fds=(release_read,),
            )
            trace["process"] = process
            original_send_signal = process.send_signal
            original_wait = process.wait

            def record_fallback_signal(signum: int) -> None:
                if not harness_cleanup:
                    trace["fallback_signals"].append(signum)
                original_send_signal(signum)

            def record_wait(*args: object, **wait_kwargs: object) -> int:
                if not harness_cleanup:
                    trace["popen_waits"].append(wait_kwargs.get("timeout"))
                return original_wait(*args, **wait_kwargs)

            process.send_signal = record_fallback_signal
            process.wait = record_wait
            return process

        def checked_run(argv: list[str], **_kwargs: object) -> str:
            if tuple(argv[:3]) == ("npm", "run", "test:hosts"):
                if primary_error is not None:
                    raise primary_error
            return ""

        def run_first_host(
            offered: tuple[tuple[str, Callable[[], None]], ...],
            *,
            emit: Callable[[str], None],
        ) -> tuple[str, ...]:
            selected = offered[:6]
            for name, operation in selected:
                operation()
                emit(name)
            return tuple(name for name, _operation in selected)

        def stage_frontend(_source: Path, build: object) -> None:
            Path(build.frontend).mkdir(parents=True)

        def release_to_zombie(trigger: str) -> None:
            if process is None:
                raise AssertionError("host was not launched before cleanup")
            if trigger in {"initial-zero", "initial-nonzero", "pidfd-zero"}:
                os.write(release_write, b"x")
            elif trigger in {"initial-sigterm", "pidfd-sigterm"}:
                real_os_kill(process.pid, signal.SIGTERM)
            else:
                raise AssertionError(f"unknown causal zombie trigger: {trigger}")
            state, start_time = self._wait_for_state(process.pid, "Z")
            events.append(("causal-z", trigger, process.pid, state, start_time))

        def capture_lifecycle(*args: object, **kwargs: object) -> None:
            events.append(("lifecycle-enter",))
            if lifecycle_trigger is not None and lifecycle_trigger.startswith(
                "initial-"
            ):
                release_to_zombie(lifecycle_trigger)
            wrapped = dict(kwargs)
            observe = kwargs["observe"]
            observe_identity = kwargs["observe_identity"]
            request_stage = kwargs["request_stage"]
            wait = kwargs["wait"]

            def record_observe(
                *, stage: str, deadline: float, timeout: float
            ) -> tuple[str, tuple[dict[str, object], ...]]:
                events.append(("observe-enter", stage, deadline, timeout))
                result = observe(stage=stage, deadline=deadline, timeout=timeout)
                observations.append(result)
                events.append(("observe-exit", stage, result))
                return result

            def record_identity(
                *, expected: dict[str, object]
            ) -> dict[str, object] | None:
                result = observe_identity(expected=expected)
                events.append(("identity", dict(expected), result))
                return result

            def record_request(**request_kwargs: object) -> str:
                events.append(("request-enter", dict(request_kwargs)))
                result = request_stage(**request_kwargs)
                requests.append((dict(request_kwargs), result))
                events.append(("request-exit", result))
                return result

            def record_wait(**wait_kwargs: object) -> tuple[str, object] | str:
                events.append(("wait-enter", dict(wait_kwargs)))
                result = wait(**wait_kwargs)
                waits.append((dict(wait_kwargs), result))
                events.append(("wait-exit", result))
                return result

            wrapped.update(
                {
                    "observe": record_observe,
                    "observe_identity": record_identity,
                    "request_stage": record_request,
                    "wait": record_wait,
                }
            )
            try:
                real_lifecycle(*args, **wrapped)
            finally:
                events.append(("lifecycle-exit",))

        acquired_once = False

        def controlled_pidfd_open(pid: int, flags: int) -> int:
            nonlocal acquired_once
            fd = real_pidfd_open(pid, flags)
            trace["pidfd_opens"].append((pid, flags, fd))
            if not acquired_once and lifecycle_trigger in {
                "pidfd-zero",
                "pidfd-sigterm",
            }:
                acquired_once = True
                release_to_zombie(lifecycle_trigger)
            return fd

        def record_pidfd_send(
            pidfd: int,
            signum: int,
            siginfo: object,
            flags: int,
        ) -> None:
            trace["pidfd_sends"].append((pidfd, signum, siginfo, flags))
            real_pidfd_send_signal(pidfd, signum, siginfo, flags)

        try:
            with contextlib.ExitStack() as stack:
                stack.enter_context(
                    mock.patch.object(
                        profiles,
                        "run_notebook_profile",
                        side_effect=run_first_host,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        profiles,
                        "install_environment",
                        return_value=root / "python",
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module,
                        "checked_run",
                        side_effect=checked_run,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module.subprocess,
                        "Popen",
                        side_effect=launch_host,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module.socket,
                        "create_connection",
                        return_value=mock.MagicMock(),
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        executor,
                        "stage_frontend",
                        side_effect=stage_frontend,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        executor,
                        "acquire_inputs",
                        return_value=acquired,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module,
                        "_notebook_cleanup_lifecycle",
                        side_effect=capture_lifecycle,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module.os,
                        "pidfd_open",
                        side_effect=controlled_pidfd_open,
                    )
                )
                stack.enter_context(
                    mock.patch.object(
                        python_candidate_module.signal,
                        "pidfd_send_signal",
                        side_effect=record_pidfd_send,
                    )
                )
                try:
                    python_candidate_module.run_notebook_profile(
                        uv="/reviewed/uv",
                        interpreter="/reviewed/python3.13",
                        wheel=root / "candidate.whl",
                        extracted=extracted,
                        workspace=workspace,
                        config=python_candidate_module.load_config(),
                        receipt=receipt,
                        frontend=frontend,
                    )
                except BaseException as error:
                    trace["error"] = error
        finally:
            harness_cleanup = True
            for fd in (release_read, release_write):
                try:
                    os.close(fd)
                except OSError:
                    pass
            if process is not None:
                if process.poll() is None:
                    try:
                        real_os_kill(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                try:
                    process.wait(timeout=2.0)
                except (ChildProcessError, subprocess.TimeoutExpired):
                    pass
        return trace

    def _assert_ordinary_positive(self, trace: dict[str, object]) -> None:
        self.assertIsNone(trace["error"])
        requests = trace["requests"]
        self.assertTrue(requests)
        self.assertEqual(requests[0][0]["stage"], "sigterm")
        self.assertEqual(requests[0][1], "sigterm=sent")
        self.assertTrue(trace["popen_waits"], "production must call bounded Popen.wait")
        self.assertTrue(trace["waits"])
        events = trace["events"]
        wait_exit = next(i for i, event in enumerate(events) if event[0] == "wait-exit")
        later_empty = [
            i
            for i, event in enumerate(events)
            if i > wait_exit
            and event[0] == "observe-exit"
            and event[2][0] == "complete-empty"
        ]
        self.assertTrue(
            later_empty, "success needs observation after Popen acknowledgement"
        )
        self.assertFalse(
            any(request[0]["stage"] == "sigkill" for request in requests),
            "the ordinary path may not require forced escalation",
        )

    def _assert_actual_bounded_popen_wait(self, trace: dict[str, object]) -> None:
        timeouts = trace["popen_waits"]
        self.assertTrue(timeouts, "direct-host Z requires actual Popen.wait authority")
        for timeout in timeouts:
            self.assertIn(type(timeout), (int, float))
            self.assertTrue(0.0 <= float(timeout) <= 35.0)

    def test_00_ordinary_controlled_live_host_reaps_before_later_empty(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            trace = self._run_outer_host(Path(temporary), host_mode="live")
        self._assert_ordinary_positive(trace)

    def _assert_initial_z_reap(
        self,
        trace: dict[str, object],
        *,
        status: int,
    ) -> None:
        observations = trace["observations"]
        self.assertTrue(observations)
        first_terminal, first_survivors = observations[0]
        self.assertEqual(first_terminal, "complete-nonempty")
        self.assertEqual(len(first_survivors), 1)
        self.assertEqual(first_survivors[0]["state"], "Z")
        self.assertEqual(trace["requests"], [])
        self._assert_actual_bounded_popen_wait(trace)
        self.assertTrue(trace["waits"])
        wait_kwargs, wait_result = trace["waits"][0]
        self.assertEqual(wait_kwargs["stage"], "reap")
        self.assertEqual(wait_result, ("reaped-complete-empty", status))
        events = trace["events"]
        wait_exit = next(i for i, event in enumerate(events) if event[0] == "wait-exit")
        self.assertTrue(
            any(
                i > wait_exit
                and event[0] == "observe-exit"
                and event[2] == ("complete-empty", ())
                for i, event in enumerate(events)
            )
        )

    def test_10_real_zero_exit_initial_z_is_reaped_before_success(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            trace = self._run_outer_host(
                Path(temporary),
                host_mode="controlled",
                lifecycle_trigger="initial-zero",
            )
        self.assertIsNone(trace["error"])
        self._assert_initial_z_reap(trace, status=0)

    def test_11_real_nonzero_initial_z_is_sticky_with_or_without_primary(self) -> None:
        for primary in (None, RuntimeError("host-payload-failed")):
            with self.subTest(primary=primary is not None):
                with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
                    trace = self._run_outer_host(
                        Path(temporary),
                        host_mode="controlled",
                        exit_status=7,
                        lifecycle_trigger="initial-nonzero",
                        primary_error=primary,
                    )
                self.assertEqual(trace["requests"], [])
                self._assert_actual_bounded_popen_wait(trace)
                error = trace["error"]
                self.assertIsInstance(error, CandidateError)
                self.assertIn("cleanup=incomplete(wait-invalid-status:7)", str(error))
                if primary is None:
                    self.assertIsNone(error.__cause__)
                else:
                    self.assertIs(error.__cause__, primary)
                    self.assertEqual(
                        str(error).splitlines()[0],
                        "primary=RuntimeError: host-payload-failed",
                    )

    def test_12_real_unsolicited_sigterm_initial_z_is_invalid(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            trace = self._run_outer_host(
                Path(temporary),
                host_mode="controlled",
                lifecycle_trigger="initial-sigterm",
            )
        self.assertEqual(trace["requests"], [])
        self._assert_actual_bounded_popen_wait(trace)
        error = trace["error"]
        self.assertIsInstance(error, CandidateError)
        self.assertIn("cleanup=incomplete(wait-invalid-status:-15)", str(error))

    def test_20_real_live_to_z_after_pidfd_open_never_sends(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            trace = self._run_outer_host(
                Path(temporary),
                host_mode="controlled",
                lifecycle_trigger="pidfd-zero",
            )
        self.assertIsNone(trace["error"])
        self.assertEqual(len(trace["pidfd_opens"]), 1)
        self.assertEqual(trace["pidfd_sends"], [])
        self.assertEqual(trace["fallback_signals"], [])
        self.assertEqual(trace["requests"][0][1], "sigterm=pending-reap")
        self._assert_actual_bounded_popen_wait(trace)
        self.assertEqual(trace["waits"][0][1], ("reaped-complete-empty", 0))

    def test_21_real_live_to_unsolicited_sigterm_after_pidfd_open_is_invalid(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            trace = self._run_outer_host(
                Path(temporary),
                host_mode="controlled",
                lifecycle_trigger="pidfd-sigterm",
            )
        error = trace["error"]
        self.assertIsInstance(error, CandidateError)
        self.assertIn("cleanup=incomplete(wait-invalid-status:-15)", str(error))
        self.assertEqual(len(trace["pidfd_opens"]), 1)
        self.assertEqual(trace["pidfd_sends"], [])
        self.assertEqual(trace["fallback_signals"], [])
        self.assertEqual(trace["requests"][0][1], "sigterm=pending-reap")
        self._assert_actual_bounded_popen_wait(trace)

    def test_31_current_skip_z_mutant_is_rejected(self) -> None:
        release_read, release_write = os.pipe()
        os.set_inheritable(release_read, True)
        environment = os.environ.copy()
        environment.update(
            {
                "EQIORA_RELEASE_FD": str(release_read),
                "EQIORA_HOST_MODE": "controlled",
                "EQIORA_EXIT_STATUS": "0",
            }
        )
        process = subprocess.Popen(
            [sys.executable, "-I", "-c", self.HOST],
            env=environment,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
            pass_fds=(release_read,),
        )
        try:
            observer = python_candidate_module._NotebookOwnedProcessObserver(
                scenario="jupyterlab-4.6.2",
                process=process,
            )
            start_time = self._proc_identity(process.pid)[1]
            os.write(release_write, b"x")
            self._wait_for_state(process.pid, "Z")
            terminal, survivors = observer.observe(deadline=time.monotonic() + 2.0)
            self.assertEqual(terminal, "complete-nonempty")
            self.assertEqual(
                tuple(
                    (item["pid"], item["start_time"], item["state"])
                    for item in survivors
                ),
                ((process.pid, start_time, "Z"),),
            )
        finally:
            os.close(release_read)
            os.close(release_write)
            process.wait(timeout=2.0)
            if process.stdout is not None:
                process.stdout.close()

    def test_30_real_descendant_zombie_remains_until_its_parent_reaps(self) -> None:
        with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
            root = Path(temporary)
            child_pid_path = root / "child.pid"
            reaped_path = root / "reaped"
            release_read, release_write = os.pipe()
            os.set_inheritable(release_read, True)
            environment = os.environ.copy()
            environment.update(
                {
                    "EQIORA_RELEASE_FD": str(release_read),
                    "EQIORA_CHILD_PID_PATH": str(child_pid_path),
                    "EQIORA_REAPED_PATH": str(reaped_path),
                }
            )
            process = subprocess.Popen(
                [sys.executable, "-I", "-c", self.DESCENDANT_HOST],
                env=environment,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                start_new_session=True,
                pass_fds=(release_read,),
            )
            try:
                observer = python_candidate_module._NotebookOwnedProcessObserver(
                    scenario="jupyterlab-4.6.2",
                    process=process,
                )
                deadline = time.monotonic() + 2.0
                child_pid: int | None = None
                while time.monotonic() < deadline:
                    try:
                        child_pid = int(child_pid_path.read_text(encoding="ascii"))
                    except (FileNotFoundError, ValueError):
                        pass
                    else:
                        break
                    time.sleep(0.005)
                if child_pid is None:
                    self.fail("descendant host did not publish its child PID")
                child_state, child_start = self._wait_for_state(child_pid, "Z")
                terminal, survivors = observer.observe(deadline=time.monotonic() + 2.0)
                self.assertEqual(terminal, "complete-nonempty")
                self.assertIn(
                    (child_pid, child_start, child_state),
                    tuple(
                        (item["pid"], item["start_time"], item["state"])
                        for item in survivors
                    ),
                )

                os.write(release_write, b"r")
                self._wait_for_absence(child_pid)
                deadline = time.monotonic() + 2.0
                while time.monotonic() < deadline and not reaped_path.is_file():
                    time.sleep(0.005)
                self.assertTrue(reaped_path.is_file())
                terminal, survivors = observer.observe(deadline=time.monotonic() + 2.0)
                self.assertEqual(terminal, "complete-nonempty")
                self.assertNotIn(child_pid, tuple(item["pid"] for item in survivors))
            finally:
                os.close(release_read)
                os.close(release_write)
                if process.poll() is None:
                    process.kill()
                process.wait(timeout=2.0)
                if process.stdout is not None:
                    process.stdout.close()

    def _run_lifecycle_result(
        self,
        wait_result: object,
        *,
        primary_error: BaseException | None,
        post_ack: tuple[str, tuple[dict[str, object], ...]] = (
            "complete-empty",
            (),
        ),
    ) -> tuple[BaseException | None, dict[str, object]]:
        survivor = self._survivor()
        observations = iter(
            (
                ("complete-nonempty", (survivor,)),
                post_ack,
            )
        )
        trace: dict[str, object] = {
            "observe": [],
            "identity": [],
            "request": [],
            "wait": [],
        }

        def observe(
            *, stage: str, deadline: float, timeout: float
        ) -> tuple[str, tuple[dict[str, object], ...]]:
            trace["observe"].append((stage, deadline, timeout))
            try:
                return next(observations)
            except StopIteration:
                return post_ack

        def observe_identity(*, expected: dict[str, object]) -> dict[str, object]:
            trace["identity"].append(dict(expected))
            return dict(expected, state="Z")

        def request_stage(**kwargs: object) -> str:
            trace["request"].append(dict(kwargs))
            return f"{kwargs['stage']}=pending-reap"

        def wait(**kwargs: object) -> object:
            trace["wait"].append(dict(kwargs))
            if isinstance(wait_result, BaseException):
                raise wait_result
            return wait_result

        error: BaseException | None = None
        try:
            python_candidate_module._notebook_cleanup_lifecycle(
                scenario="jupyterlab-4.6.2",
                primary_error=primary_error,
                observe=observe,
                observe_identity=observe_identity,
                request_stage=request_stage,
                wait=wait,
                monotonic=lambda: 100.0,
            )
        except BaseException as caught:
            error = caught
        return error, trace

    def test_40_positive_first_closed_wait_table_is_sticky(self) -> None:
        positives = (
            ("reaped-complete-empty", 0),
            ("reaped-complete-empty", -15),
        )
        negatives = (
            (("invalid-status", 7), "incomplete(wait-invalid-status:7)"),
            (("status-unavailable", None), "incomplete(wait-status-unavailable)"),
            (("deadline-exhausted", None), "incomplete(cleanup-deadline)"),
            (("host-still-running", None), "incomplete(wait-host-still-running)"),
            (("owned-survivors", None), "incomplete(wait-owned-survivors)"),
            (
                ("incomplete", "incomplete(authority-denied)"),
                "incomplete(authority-denied)",
            ),
            (
                ("incomplete", "incomplete(observer-unavailable)"),
                "incomplete(observer-unavailable)",
            ),
            (
                ("incomplete", "incomplete(observation-overflow)"),
                "incomplete(observation-overflow)",
            ),
            (
                ("incomplete", "incomplete(malformed-observation)"),
                "incomplete(malformed-observation)",
            ),
            (
                ("incomplete", "incomplete(stable-identity-mismatch)"),
                "incomplete(stable-identity-mismatch)",
            ),
            (
                ("incomplete", "incomplete(cleanup-deadline)"),
                "incomplete(cleanup-deadline)",
            ),
            ("graceful=host-exited:0", "incomplete(malformed-wait-result)"),
            (("unknown", None), "incomplete(malformed-wait-result)"),
            (("reaped-complete-empty", True), "incomplete(malformed-wait-result)"),
            (("invalid-status", True), "incomplete(malformed-wait-result)"),
            (
                ("incomplete", "incomplete(action-handle-unavailable)"),
                "incomplete(malformed-wait-result)",
            ),
            (
                ("reaped-complete-empty", 0, "extra"),
                "incomplete(malformed-wait-result)",
            ),
        )

        for result in positives:
            with self.subTest(result=result, primary=False):
                error, trace = self._run_lifecycle_result(
                    result,
                    primary_error=None,
                )
                self.assertIsNone(error)
                self.assertEqual(trace["request"], [])
                self.assertEqual(trace["wait"][0]["stage"], "reap")
                self.assertGreaterEqual(len(trace["observe"]), 2)
            primary = RuntimeError("primary-before-cleanup")
            with self.subTest(result=result, primary=True):
                error, trace = self._run_lifecycle_result(
                    result,
                    primary_error=primary,
                )
                self.assertIsInstance(error, CandidateError)
                self.assertIs(error.__cause__, primary)
                self.assertEqual(trace["request"], [])
                self.assertEqual(trace["wait"][0]["stage"], "reap")

        for result, terminal in negatives:
            for primary in (None, RuntimeError("primary-before-cleanup")):
                with self.subTest(result=result, primary=primary is not None):
                    error, trace = self._run_lifecycle_result(
                        result,
                        primary_error=primary,
                    )
                    self.assertIsInstance(error, CandidateError)
                    self.assertIn(f"cleanup={terminal}", str(error))
                    self.assertEqual(trace["request"], [])
                    self.assertEqual(trace["wait"][0]["stage"], "reap")
                    if primary is None:
                        self.assertIsNone(error.__cause__)
                    else:
                        self.assertIs(error.__cause__, primary)
                        self.assertEqual(
                            str(error).splitlines()[0],
                            "primary=RuntimeError: primary-before-cleanup",
                        )

        for primary in (None, RuntimeError("primary-before-cleanup")):
            with self.subTest(result="callback-error", primary=primary is not None):
                error, trace = self._run_lifecycle_result(
                    RuntimeError("wait-callback-broke"),
                    primary_error=primary,
                )
                self.assertIsInstance(error, CandidateError)
                self.assertIn(
                    "cleanup=incomplete(cleanup-callback-error)",
                    str(error),
                )
                self.assertEqual(trace["request"], [])
                self.assertEqual(trace["wait"][0]["stage"], "reap")
                if primary is None:
                    self.assertIsNone(error.__cause__)
                else:
                    self.assertIs(error.__cause__, primary)

    def test_50_acknowledgement_requires_a_distinct_later_empty_observation(
        self,
    ) -> None:
        error, trace = self._run_lifecycle_result(
            ("reaped-complete-empty", 0),
            primary_error=None,
        )
        self.assertIsNone(error)
        self.assertGreaterEqual(len(trace["observe"]), 2)
        self.assertEqual(len(trace["wait"]), 1)

        survivor = self._survivor(state="S")
        error, trace = self._run_lifecycle_result(
            ("reaped-complete-empty", 0),
            primary_error=None,
            post_ack=("complete-nonempty", (survivor,)),
        )
        self.assertIsInstance(error, CandidateError)
        self.assertIn("cleanup=complete-nonempty", str(error))
        self.assertGreaterEqual(len(trace["observe"]), 2)
        self.assertEqual(len(trace["wait"]), 1)

    def test_60_exact_deadline_starts_no_observe_action_wait_or_sleep(self) -> None:
        survivor = self._survivor()
        clock = types.SimpleNamespace(now=100.0)
        calls: list[tuple[str, object]] = []

        def observe(
            *, stage: str, deadline: float, timeout: float
        ) -> tuple[str, tuple[dict[str, object], ...]]:
            calls.append(("observe", stage))
            if len([call for call in calls if call[0] == "observe"]) == 1:
                clock.now = 135.0
                return "complete-nonempty", (survivor,)
            return "complete-empty", ()

        def forbidden(name: str) -> Callable[..., object]:
            def record(*_args: object, **_kwargs: object) -> object:
                calls.append((name, None))
                if name == "wait":
                    return "deadline-exhausted", None
                if name == "request":
                    return "sigterm=cleanup-deadline"
                if name == "identity":
                    return dict(survivor)
                return None

            return record

        with mock.patch.object(
            python_candidate_module.time,
            "sleep",
            side_effect=forbidden("sleep"),
        ):
            with self.assertRaises(CandidateError) as raised:
                python_candidate_module._notebook_cleanup_lifecycle(
                    scenario="jupyterlab-4.6.2",
                    primary_error=None,
                    observe=observe,
                    observe_identity=forbidden("identity"),
                    request_stage=forbidden("request"),
                    wait=forbidden("wait"),
                    monotonic=lambda: clock.now,
                )
        self.assertIn("cleanup=incomplete(cleanup-deadline)", str(raised.exception))
        self.assertEqual(calls, [("observe", "graceful")])


if __name__ == "__main__":
    unittest.main()
