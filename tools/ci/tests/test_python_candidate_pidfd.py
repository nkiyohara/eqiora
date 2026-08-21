from __future__ import annotations

import ast
import errno
import inspect
import os
import signal
import subprocess
import sys
import tempfile
import textwrap
import types
import unittest
from collections.abc import Callable
from pathlib import Path
from unittest import mock


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
TESTS_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(REPOSITORY_ROOT / "tools/release"))
sys.path.insert(0, str(TESTS_ROOT))

import python_candidate as candidate  # noqa: E402
import test_python_candidate as existing_evidence  # noqa: E402


CandidateError = candidate.CandidateError
SCENARIO = "jupyterlab-4.6.2"
DEFAULT_OBSERVATION = object()


class SequenceClock:
    def __init__(self, *values: float) -> None:
        self.values = iter(values)
        self.last = values[-1]
        self.calls = 0

    def __call__(self) -> float:
        self.calls += 1
        try:
            self.last = next(self.values)
        except StopIteration:
            pass
        return self.last


class MutableClock:
    def __init__(self, now: float) -> None:
        self.now = now
        self.calls = 0

    def __call__(self) -> float:
        self.calls += 1
        return self.now


class NotebookPidfdActionEvidence(unittest.TestCase):
    """Independent stable-action evidence for the private Issue #495 seam."""

    @staticmethod
    def identity(
        *,
        pid: int = 41_502,
        start_time: int = 908_172,
        role: str = "host",
        state: str = "S",
    ) -> dict[str, object]:
        return {
            "scenario": SCENARIO,
            "role": role,
            "pid": pid,
            "start_time": start_time,
            "state": state,
            "requested_stages": (),
            "stage_results": (),
            "authority_denied": False,
        }

    def action(self) -> Callable[..., tuple[str, bool]]:
        observer_type = getattr(candidate, "_NotebookOwnedProcessObserver", None)
        self.assertTrue(callable(observer_type))
        action = getattr(observer_type, "request_stage", None)
        self.assertTrue(
            callable(action),
            "#495 requires the precommitted stable pidfd action seam",
        )
        self.assertEqual(
            tuple(inspect.signature(action).parameters),
            ("self", "stage", "identity", "deadline", "monotonic"),
        )
        return action

    def synthetic_observer(
        self,
        *,
        identity: dict[str, object] | None = None,
        known_record: dict[str, object] | None = None,
    ) -> tuple[object, dict[str, object]]:
        expected = self.identity() if identity is None else dict(identity)
        observer_type = candidate._NotebookOwnedProcessObserver
        observer = object.__new__(observer_type)
        observer.scenario = SCENARIO
        observer.process = types.SimpleNamespace(pid=int(expected["pid"]))
        observer.root_pid = int(expected["pid"])
        observer.initial_error = None
        observer.owned_sessions = {}
        observer.last_survivors = ()
        record = dict(expected if known_record is None else known_record)
        observer.known = {(int(record["pid"]), int(record["start_time"])): record}
        return observer, expected

    @staticmethod
    def bounded_reap(process: subprocess.Popen[bytes]) -> None:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=2.0)

    def invoke_synthetic(
        self,
        *,
        observer: object | None = None,
        identity: dict[str, object] | None = None,
        clock: Callable[[], float] | None = None,
        observe: object = DEFAULT_OBSERVATION,
        open_effect: object = 701,
        send_effect: object = None,
        close_effect: object = None,
        stage: str = "sigterm",
    ) -> tuple[
        tuple[str, bool],
        mock.Mock,
        mock.Mock,
        mock.Mock,
        mock.Mock,
        object,
        dict[str, object],
    ]:
        if observer is None:
            observer, expected = self.synthetic_observer(identity=identity)
        else:
            expected = self.identity() if identity is None else dict(identity)
        clock_read = SequenceClock(10.0, 10.0, 10.0) if clock is None else clock
        observed = dict(expected) if observe is DEFAULT_OBSERVATION else observe
        observation = mock.Mock()
        if isinstance(observed, BaseException):
            observation.side_effect = observed
        elif isinstance(observed, mock.Mock):
            observation = observed
        else:
            observation.return_value = observed
        open_pidfd = mock.Mock()
        if isinstance(open_effect, BaseException):
            open_pidfd.side_effect = open_effect
        elif callable(open_effect):
            open_pidfd.side_effect = open_effect
        else:
            open_pidfd.return_value = open_effect
        send_pidfd = mock.Mock()
        if isinstance(send_effect, BaseException):
            send_pidfd.side_effect = send_effect
        elif callable(send_effect):
            send_pidfd.side_effect = send_effect
        else:
            send_pidfd.return_value = send_effect
        close_pidfd = mock.Mock()
        if isinstance(close_effect, BaseException):
            close_pidfd.side_effect = close_effect
        elif callable(close_effect):
            close_pidfd.side_effect = close_effect
        else:
            close_pidfd.return_value = close_effect

        with (
            mock.patch.object(observer, "observe_identity", observation),
            mock.patch.object(candidate.os, "pidfd_open", open_pidfd, create=True),
            mock.patch.object(
                candidate.signal,
                "pidfd_send_signal",
                send_pidfd,
                create=True,
            ),
            mock.patch.object(candidate.os, "close", close_pidfd),
        ):
            result = self.action()(
                observer,
                stage=stage,
                identity=expected,
                deadline=20.0,
                monotonic=clock_read,
            )
        return (
            result,
            open_pidfd,
            observation,
            send_pidfd,
            close_pidfd,
            observer,
            expected,
        )

    def test_00_ordinary_real_host_reaches_reap_and_post_ack_empty(self) -> None:
        action = self.action()
        self.assertTrue(callable(getattr(os, "pidfd_open", None)))
        self.assertTrue(callable(getattr(signal, "pidfd_send_signal", None)))
        helper = existing_evidence.NotebookOwnedProcessARealPathTests("runTest")
        observer_type = candidate._NotebookOwnedProcessObserver
        real_open = os.pidfd_open
        real_send = signal.pidfd_send_signal
        real_close = os.close
        capture: dict[str, object] = {}
        action_traces: list[list[tuple[object, ...]]] = []
        root_pid = -1
        child_pid = -1

        def traced_request(
            observer: object,
            *,
            stage: str,
            identity: dict[str, object],
            deadline: float,
            monotonic: Callable[[], float],
        ) -> tuple[str, bool]:
            trace: list[tuple[object, ...]] = []
            real_read = observer.observe_identity

            def open_pidfd(pid: int, flags: int) -> int:
                fd = real_open(pid, flags)
                trace.append(("open", pid, flags, fd))
                return fd

            def observe_identity(
                *, expected: dict[str, object]
            ) -> dict[str, object] | None:
                observed = real_read(expected=expected)
                trace.append(
                    (
                        "read",
                        expected["pid"],
                        expected["start_time"],
                        None if observed is None else observed.get("state"),
                    )
                )
                return observed

            def send_pidfd(
                fd: int,
                signum: int,
                siginfo: object,
                flags: int,
            ) -> None:
                trace.append(("send", fd, signum, siginfo, flags))
                real_send(fd, signum, siginfo, flags)

            def close_pidfd(fd: int) -> None:
                trace.append(("close", fd))
                real_close(fd)

            with (
                mock.patch.object(candidate.os, "pidfd_open", side_effect=open_pidfd),
                mock.patch.object(
                    observer,
                    "observe_identity",
                    side_effect=observe_identity,
                ),
                mock.patch.object(
                    candidate.signal,
                    "pidfd_send_signal",
                    side_effect=send_pidfd,
                ),
                mock.patch.object(candidate.os, "close", side_effect=close_pidfd),
            ):
                result = action(
                    observer,
                    stage=stage,
                    identity=identity,
                    deadline=deadline,
                    monotonic=monotonic,
                )
            trace.append(("return", *result))
            action_traces.append(trace)
            return result

        try:
            with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
                with mock.patch.object(
                    observer_type,
                    "request_stage",
                    autospec=True,
                    side_effect=traced_request,
                ):
                    child_pid = helper.run_one_real_host(
                        Path(temporary),
                        cascade=True,
                        capture=capture,
                        guard_lifecycle_actions=True,
                    )
            root_pid = int(capture["root_pid"])
            self.assertFalse(helper.process_is_live(root_pid))
            self.assertFalse(helper.process_is_live(child_pid))
            self.assertTrue(action_traces)
            for trace in action_traces:
                self.assertEqual(
                    [event[0] for event in trace],
                    ["open", "read", "send", "close", "return"],
                )
                opened, _read, sent, closed, returned = trace
                self.assertEqual(opened[2], 0)
                self.assertEqual(sent[1], opened[3])
                self.assertEqual(closed[1], opened[3])
                self.assertEqual(sent[2:], (signal.SIGTERM, None, 0))
                self.assertEqual(returned, ("return", "sigterm=sent", True))

            events = capture["events"]
            event_names = [event[0] for event in events]
            self.assertNotIn("bypass-signal", event_names)
            self.assertNotIn("bypass-os-signal", event_names)
            self.assertNotIn("bypass-wait", event_names)
            self.assertIn("host-wait", event_names)
            acknowledgement = next(
                index
                for index, event in enumerate(events)
                if event[0] == "wait-exit" and event[2][0] == "reaped-complete-empty"
            )
            last_request = max(
                index
                for index, event in enumerate(events)
                if event[0] == "request-exit"
            )
            post_ack_empty = next(
                index
                for index, event in enumerate(
                    events[acknowledgement + 1 :], start=acknowledgement + 1
                )
                if event[:3] == ("observe-exit", "complete-empty", ())
            )
            self.assertLess(last_request, acknowledgement)
            self.assertLess(acknowledgement, post_ack_empty)
            decision = next(event for event in events if event[0] == "decision")
            self.assertEqual(decision[-1], "complete-empty")
        finally:
            root_pid = int(capture.get("root_pid", root_pid))
            child_pid = int(capture.get("child_pid", child_pid))
            helper.bounded_test_cleanup(
                *(pid for pid in (root_pid, child_pid) if pid > 0)
            )

    def test_01_post_open_identity_and_state_mismatches_fail_closed(self) -> None:
        expected = self.identity()
        without_state = dict(expected)
        del without_state["state"]
        variants = (
            (
                "start-time",
                dict(expected, start_time=908_173),
                "sigterm=stable-identity-mismatch",
            ),
            (
                "scenario",
                dict(expected, scenario="marimo-0.23.16"),
                "sigterm=stable-identity-mismatch",
            ),
            (
                "role",
                dict(expected, role="foreign"),
                "sigterm=stable-identity-mismatch",
            ),
            (
                "pid",
                dict(expected, pid=int(expected["pid"]) + 1),
                "sigterm=stable-identity-mismatch",
            ),
            ("missing-state", without_state, "sigterm=observer-unavailable"),
        )
        for label, observed, expected_result in variants:
            with self.subTest(label=label):
                observer, requested = self.synthetic_observer(identity=expected)
                result, opened, read, sent, closed, _, _ = self.invoke_synthetic(
                    observer=observer,
                    identity=requested,
                    observe=observed,
                )
                self.assertEqual(result, (expected_result, False))
                opened.assert_called_once_with(int(expected["pid"]), 0)
                read.assert_called_once_with(expected=requested)
                sent.assert_not_called()
                closed.assert_called_once_with(701)

    def test_02_post_match_numeric_replacement_cannot_retarget_send(self) -> None:
        observer, expected = self.synthetic_observer()
        occupant = {"pid": "admitted"}
        bound: dict[int, str] = {}
        delivered: list[str] = []
        fallback: list[tuple[object, ...]] = []

        def open_pidfd(pid: int, flags: int) -> int:
            self.assertEqual((pid, flags), (expected["pid"], 0))
            bound[702] = occupant["pid"]
            return 702

        def observe_identity(*, expected: dict[str, object]) -> dict[str, object]:
            occupant["pid"] = "replacement"
            return dict(expected)

        def send_pidfd(fd: int, *_args: object) -> None:
            delivered.append(bound[fd])

        with (
            mock.patch.object(
                observer, "observe_identity", side_effect=observe_identity
            ),
            mock.patch.object(candidate.os, "pidfd_open", side_effect=open_pidfd),
            mock.patch.object(
                candidate.signal, "pidfd_send_signal", side_effect=send_pidfd
            ),
            mock.patch.object(candidate.os, "close") as close_pidfd,
            mock.patch.object(
                candidate.os, "kill", side_effect=lambda *args: fallback.append(args)
            ),
            mock.patch.object(
                candidate.os, "killpg", side_effect=lambda *args: fallback.append(args)
            ),
            mock.patch.object(
                subprocess.Popen,
                "send_signal",
                side_effect=lambda *args: fallback.append(args),
            ),
        ):
            result = self.action()(
                observer,
                stage="sigterm",
                identity=expected,
                deadline=20.0,
                monotonic=SequenceClock(10.0, 10.0, 10.0),
            )
        self.assertEqual(result, ("sigterm=sent", True))
        self.assertEqual(delivered, ["admitted"])
        self.assertEqual(occupant["pid"], "replacement")
        self.assertEqual(fallback, [])
        close_pidfd.assert_called_once_with(702)

    def test_03_exact_known_authority_is_required_before_open(self) -> None:
        variants: list[tuple[str, object, dict[str, object]]] = []
        expected = self.identity()
        missing, _ = self.synthetic_observer(identity=expected)
        missing.known = {}
        variants.append(("missing", missing, expected))
        for field, value in (
            ("scenario", "marimo-0.23.16"),
            ("role", "foreign"),
            ("pid", int(expected["pid"]) + 1),
            ("start_time", int(expected["start_time"]) + 1),
        ):
            changed = dict(expected, **{field: value})
            observer, _ = self.synthetic_observer(
                identity=expected,
                known_record=changed,
            )
            variants.append((field, observer, expected))

        for label, observer, requested in variants:
            with self.subTest(label=label):
                (
                    result,
                    open_pidfd,
                    observation,
                    send_pidfd,
                    close_pidfd,
                    _,
                    _,
                ) = self.invoke_synthetic(
                    observer=observer,
                    identity=requested,
                )
                self.assertEqual(
                    result,
                    ("sigterm=stable-identity-mismatch", False),
                )
                open_pidfd.assert_not_called()
                observation.assert_not_called()
                send_pidfd.assert_not_called()
                close_pidfd.assert_not_called()

    def test_04_operation_specific_errno_and_observer_matrix(self) -> None:
        unavailable = "sigterm=action-handle-unavailable"
        for missing_feature in ("pidfd_open", "pidfd_send_signal"):
            with self.subTest(missing_feature=missing_feature):
                observer, expected = self.synthetic_observer()
                observation = mock.Mock(return_value=dict(expected))
                opened = mock.Mock(return_value=703)
                sent = mock.Mock()
                closed = mock.Mock()
                open_feature: object = opened
                send_feature: object = sent
                if missing_feature == "pidfd_open":
                    open_feature = None
                else:
                    send_feature = None
                with (
                    mock.patch.object(observer, "observe_identity", observation),
                    mock.patch.object(
                        candidate.os,
                        "pidfd_open",
                        open_feature,
                        create=True,
                    ),
                    mock.patch.object(
                        candidate.signal,
                        "pidfd_send_signal",
                        send_feature,
                        create=True,
                    ),
                    mock.patch.object(candidate.os, "close", closed),
                ):
                    result = self.action()(
                        observer,
                        stage="sigterm",
                        identity=expected,
                        deadline=20.0,
                        monotonic=SequenceClock(10.0),
                    )
                self.assertEqual(result, (unavailable, False))
                opened.assert_not_called()
                observation.assert_not_called()
                sent.assert_not_called()
                closed.assert_not_called()

        cases = (
            (
                "open-eperm",
                PermissionError(errno.EPERM, "denied"),
                None,
                None,
                unavailable,
                0,
            ),
            (
                "open-eacces",
                PermissionError(errno.EACCES, "denied"),
                None,
                None,
                unavailable,
                0,
            ),
            (
                "open-esrch",
                ProcessLookupError(errno.ESRCH, "gone"),
                None,
                None,
                "sigterm=not-found",
                0,
            ),
            (
                "open-enosys",
                OSError(errno.ENOSYS, "missing"),
                None,
                None,
                unavailable,
                0,
            ),
            ("open-emfile", OSError(errno.EMFILE, "limit"), None, None, unavailable, 0),
            (
                "open-einval",
                OSError(errno.EINVAL, "invalid"),
                None,
                None,
                unavailable,
                0,
            ),
            ("open-enfile", OSError(errno.ENFILE, "limit"), None, None, unavailable, 0),
            (
                "open-enodev",
                OSError(errno.ENODEV, "device"),
                None,
                None,
                unavailable,
                0,
            ),
            (
                "open-enomem",
                OSError(errno.ENOMEM, "memory"),
                None,
                None,
                unavailable,
                0,
            ),
            ("observe-absent", 703, None, None, "sigterm=not-found", 1),
            (
                "observe-denied",
                703,
                PermissionError(errno.EACCES, "denied"),
                None,
                "sigterm=authority-denied",
                1,
            ),
            (
                "observe-flag",
                703,
                dict(self.identity(), authority_denied=True),
                None,
                "sigterm=authority-denied",
                1,
            ),
            (
                "observe-io",
                703,
                OSError(errno.EIO, "io"),
                None,
                "sigterm=observer-unavailable",
                1,
            ),
            (
                "observe-candidate-error",
                703,
                CandidateError("malformed process data"),
                None,
                "sigterm=observer-unavailable",
                1,
            ),
            (
                "observe-malformed",
                703,
                ["not", "a", "record"],
                None,
                "sigterm=observer-unavailable",
                1,
            ),
            (
                "send-eperm",
                703,
                self.identity(),
                PermissionError(errno.EPERM, "denied"),
                "sigterm=authority-denied",
                1,
            ),
            (
                "send-eacces",
                703,
                self.identity(),
                PermissionError(errno.EACCES, "denied"),
                "sigterm=authority-denied",
                1,
            ),
            (
                "send-esrch",
                703,
                self.identity(),
                ProcessLookupError(errno.ESRCH, "gone"),
                "sigterm=not-found",
                1,
            ),
            (
                "send-ebadf",
                703,
                self.identity(),
                OSError(errno.EBADF, "bad fd"),
                unavailable,
                1,
            ),
            (
                "send-einval",
                703,
                self.identity(),
                OSError(errno.EINVAL, "invalid"),
                unavailable,
                1,
            ),
            (
                "send-enosys",
                703,
                self.identity(),
                OSError(errno.ENOSYS, "missing"),
                unavailable,
                1,
            ),
        )
        for (
            label,
            open_effect,
            observation,
            send_effect,
            expected_result,
            closes,
        ) in cases:
            with self.subTest(label=label):
                result, opened, read, sent, closed, _, _ = self.invoke_synthetic(
                    open_effect=open_effect,
                    observe=observation,
                    send_effect=send_effect,
                )
                self.assertEqual(result, (expected_result, False))
                self.assertEqual(opened.call_count, 1)
                self.assertEqual(closed.call_count, closes)
                self.assertLessEqual(sent.call_count, 1)
                if closes:
                    closed.assert_called_once_with(703)
                if label.startswith("open-"):
                    read.assert_not_called()
                    sent.assert_not_called()

    def test_05_deadline_equality_blocks_each_operation_start(self) -> None:
        cases = (
            ("before-open", SequenceClock(20.0), 0, 0, 0, 0),
            ("before-read", SequenceClock(19.0, 20.0), 1, 0, 0, 1),
            ("before-send", SequenceClock(19.0, 19.0, 20.0), 1, 1, 0, 1),
        )
        for label, clock, opens, reads, sends, closes in cases:
            with self.subTest(label=label):
                result, opened, read, sent, closed, _, _ = self.invoke_synthetic(
                    clock=clock
                )
                self.assertEqual(result, ("sigterm=cleanup-deadline", False))
                self.assertEqual(opened.call_count, opens)
                self.assertEqual(read.call_count, reads)
                self.assertEqual(sent.call_count, sends)
                self.assertEqual(closed.call_count, closes)

        clock = MutableClock(19.0)

        def accepted_after_deadline(*_args: object) -> None:
            clock.now = 21.0

        result, _, _, sent, closed, _, _ = self.invoke_synthetic(
            clock=clock,
            send_effect=accepted_after_deadline,
        )
        self.assertEqual(result, ("sigterm=sent", True))
        sent.assert_called_once()
        closed.assert_called_once_with(701)
        self.assertGreaterEqual(clock.now, 20.0)

        observer, expected = self.synthetic_observer()
        lifecycle_clock = MutableClock(100.0)
        observations = mock.Mock(return_value=("complete-nonempty", (expected,)))
        wait = mock.Mock(return_value=("reaped-complete-empty", 0))
        action_result: list[tuple[str, bool]] = []

        def finish_send_after_deadline(*_args: object) -> None:
            lifecycle_clock.now = 136.0

        def request_stage(
            *,
            stage: str,
            identity: dict[str, object],
            deadline: float,
            monotonic: Callable[[], float],
        ) -> str:
            self.assertEqual(deadline, 135.0)
            self.assertIs(monotonic, lifecycle_clock)
            action_result.append(
                self.action()(
                    observer,
                    stage=stage,
                    identity=identity,
                    deadline=deadline,
                    monotonic=monotonic,
                )
            )
            return action_result[-1][0]

        with (
            mock.patch.object(
                observer,
                "observe_identity",
                return_value=dict(expected),
            ),
            mock.patch.object(candidate.os, "pidfd_open", return_value=704),
            mock.patch.object(
                candidate.signal,
                "pidfd_send_signal",
                side_effect=finish_send_after_deadline,
            ),
            mock.patch.object(candidate.os, "close") as close_pidfd,
            self.assertRaises(CandidateError) as raised,
        ):
            candidate._notebook_cleanup_lifecycle(
                scenario=SCENARIO,
                primary_error=None,
                observe=observations,
                observe_identity=lambda *, expected: dict(expected),
                request_stage=request_stage,
                wait=wait,
                monotonic=lifecycle_clock,
            )
        self.assertEqual(action_result, [("sigterm=sent", True)])
        close_pidfd.assert_called_once_with(704)
        wait.assert_not_called()
        self.assertEqual(observations.call_count, 1)
        self.assertIn("cleanup-deadline", str(raised.exception))

    def test_06_close_failure_is_one_attempt_and_preserves_send_fact(self) -> None:
        replacement = dict(self.identity(), start_time=908_173)
        result, _, _, sent, closed, _, _ = self.invoke_synthetic(
            observe=replacement,
            close_effect=OSError(errno.EINTR, "interrupted"),
        )
        self.assertEqual(result, ("sigterm=action-handle-unavailable", False))
        sent.assert_not_called()
        self.assertEqual(closed.call_count, 1)

        for close_error in (
            OSError(errno.EIO, "close failed"),
            RuntimeError("reported close failure"),
        ):
            with self.subTest(close_error=type(close_error).__name__):
                result, _, _, sent, closed, _, _ = self.invoke_synthetic(
                    close_effect=close_error,
                )
                self.assertEqual(
                    result,
                    ("sigterm=action-handle-unavailable", True),
                )
                sent.assert_called_once()
                self.assertEqual(closed.call_count, 1)

    def test_07_unknown_stage_and_malformed_action_result_fail_closed(self) -> None:
        observer, expected = self.synthetic_observer()
        clock = mock.Mock(return_value=10.0)
        with (
            mock.patch.object(candidate.os, "pidfd_open", create=True) as opened,
            mock.patch.object(
                candidate.signal, "pidfd_send_signal", create=True
            ) as sent,
            mock.patch.object(candidate.os, "close") as closed,
            self.assertRaises(CandidateError),
        ):
            self.action()(
                observer,
                stage="shutdown",
                identity=expected,
                deadline=20.0,
                monotonic=clock,
            )
        clock.assert_not_called()
        opened.assert_not_called()
        sent.assert_not_called()
        closed.assert_not_called()

        lifecycle = candidate._notebook_cleanup_lifecycle
        for malformed in ("sigterm=accepted-unique-41", "sigkill=sent", object()):
            with self.subTest(malformed=repr(malformed)):
                survivor = self.identity()
                observations = iter(
                    (("complete-nonempty", (survivor,)), ("complete-empty", ()))
                )
                waits: list[tuple[str, float, float]] = []
                clock_read = MutableClock(100.0)

                def request_stage(**_kwargs: object) -> object:
                    return malformed

                def wait(
                    *, stage: str, deadline: float, timeout: float
                ) -> tuple[str, int | str | None]:
                    waits.append((stage, deadline, timeout))
                    return "reaped-complete-empty", 0

                with self.assertRaises(CandidateError) as raised:
                    lifecycle(
                        scenario=SCENARIO,
                        primary_error=None,
                        observe=lambda **_kwargs: next(observations),
                        observe_identity=lambda *, expected: dict(expected),
                        request_stage=request_stage,
                        wait=wait,
                        monotonic=clock_read,
                    )
                self.assertIn(
                    "incomplete(malformed-action-result)", str(raised.exception)
                )
                self.assertEqual(len(waits), 1)

    def test_09_post_open_z_is_pending_reap_without_a_claimed_signal(self) -> None:
        zombie = dict(self.identity(), state="Z")
        result, _, read, sent, closed, _, _ = self.invoke_synthetic(observe=zombie)
        self.assertEqual(result, ("sigterm=pending-reap", False))
        read.assert_called_once()
        sent.assert_not_called()
        closed.assert_called_once_with(701)

        lifecycle = candidate._notebook_cleanup_lifecycle
        survivor = self.identity()
        survivor["requested_stages"] = ()
        survivor["stage_results"] = ()
        observations = iter(
            (("complete-nonempty", (survivor,)), ("complete-empty", ()))
        )
        requested: list[str] = []
        waits: list[str] = []
        clock_read = MutableClock(100.0)

        def request_stage(**kwargs: object) -> str:
            requested.append(str(kwargs["stage"]))
            return "sigterm=pending-reap"

        def wait(**kwargs: object) -> tuple[str, int | str | None]:
            waits.append(str(kwargs["stage"]))
            return "reaped-complete-empty", 0

        lifecycle(
            scenario=SCENARIO,
            primary_error=None,
            observe=lambda **_kwargs: next(observations),
            observe_identity=lambda *, expected: dict(expected),
            request_stage=request_stage,
            wait=wait,
            monotonic=clock_read,
        )
        self.assertEqual(requested, ["sigterm"])
        self.assertEqual(waits, ["reap"])
        self.assertEqual(survivor["requested_stages"], ())
        self.assertEqual(survivor["stage_results"], ())

    def test_10_no_fallback_and_at_most_one_action_handle(self) -> None:
        observer, expected = self.synthetic_observer()
        active = 0
        maximum = 0
        opens: list[int] = []
        closes: list[int] = []

        def open_pidfd(_pid: int, _flags: int) -> int:
            nonlocal active, maximum
            active += 1
            maximum = max(maximum, active)
            handle = 800 + len(opens)
            opens.append(handle)
            return handle

        def close_pidfd(fd: int) -> None:
            nonlocal active
            closes.append(fd)
            active -= 1

        forbidden = mock.Mock(side_effect=AssertionError("numeric signal fallback"))
        with (
            mock.patch.object(
                observer, "observe_identity", return_value=dict(expected)
            ),
            mock.patch.object(candidate.os, "pidfd_open", side_effect=open_pidfd),
            mock.patch.object(candidate.signal, "pidfd_send_signal") as send_pidfd,
            mock.patch.object(candidate.os, "close", side_effect=close_pidfd),
            mock.patch.object(candidate.os, "kill", forbidden),
            mock.patch.object(candidate.os, "killpg", forbidden),
            mock.patch.object(subprocess.Popen, "send_signal", forbidden),
        ):
            for stage in ("sigterm", "sigkill", "sigterm"):
                self.assertEqual(
                    self.action()(
                        observer,
                        stage=stage,
                        identity=expected,
                        deadline=20.0,
                        monotonic=SequenceClock(10.0, 10.0, 10.0),
                    ),
                    (f"{stage}=sent", True),
                )
        self.assertEqual(maximum, 1)
        self.assertEqual(active, 0)
        self.assertEqual(closes, opens)
        self.assertEqual(send_pidfd.call_count, 3)
        forbidden.assert_not_called()

        tree = ast.parse(textwrap.dedent(inspect.getsource(self.action())))
        forbidden_calls = [
            node.attr
            for node in ast.walk(tree)
            if isinstance(node, ast.Attribute)
            and node.attr in {"kill", "killpg", "send_signal", "syscall"}
        ]
        self.assertEqual(forbidden_calls, [])

    def test_08_nested_direct_host_send_fact_is_not_inferred_from_result(self) -> None:
        helper = existing_evidence.NotebookOwnedProcessARealPathTests("runTest")
        handler_install = "signal.signal(signal.SIGTERM, stop)\n"
        self.assertEqual(helper.ROOT.count(handler_install), 1)
        helper.ROOT = helper.ROOT.replace(handler_install, "")
        observer_type = candidate._NotebookOwnedProcessObserver
        real_observe = observer_type.observe
        real_kill = os.kill
        cases: tuple[tuple[str, bool, type[BaseException] | None, str], ...] = (
            ("sigterm=not-found", False, None, "wait-invalid-status:-15"),
            (
                "sigterm=stable-identity-mismatch",
                False,
                None,
                "stable-identity-mismatch",
            ),
            ("sigterm=authority-denied", False, None, "authority-denied"),
            ("sigterm=observer-unavailable", False, None, "observer-unavailable"),
            (
                "sigterm=action-handle-unavailable",
                False,
                None,
                "action-handle-unavailable",
            ),
            ("sigterm=cleanup-deadline", False, None, "cleanup-deadline"),
            ("sigterm=pending-reap", False, None, "wait-invalid-status:-15"),
            ("", False, OSError, "cleanup-callback-error"),
            (
                "sigterm=action-handle-unavailable",
                True,
                None,
                "action-handle-unavailable",
            ),
        )

        def host_only_observe(
            observer: object,
            *,
            deadline: float | None = None,
        ) -> tuple[str, tuple[dict[str, object], ...]]:
            terminal, survivors = real_observe(observer, deadline=deadline)
            if terminal.startswith("incomplete("):
                return terminal, survivors
            hosts = tuple(item for item in survivors if item.get("role") == "host")
            return ("complete-nonempty", hosts) if hosts else ("complete-empty", ())

        for result, accepted, raised_type, expected_diagnostic in cases:
            with self.subTest(result=result, accepted=accepted, raised=raised_type):
                capture: dict[str, object] = {}
                root_pid = -1
                child_pid = -1
                action_calls = 0

                def request_stage(
                    *,
                    stage: str,
                    identity: dict[str, object],
                    deadline: float,
                    monotonic: Callable[[], float],
                ) -> tuple[str, bool]:
                    nonlocal action_calls
                    action_calls += 1
                    self.assertEqual(stage, "sigterm")
                    self.assertLess(monotonic(), deadline)
                    real_kill(int(identity["pid"]), signal.SIGTERM)
                    if raised_type is not None:
                        raise raised_type("pre-send failure")
                    return result, accepted

                try:
                    with tempfile.TemporaryDirectory(dir=Path.home()) as temporary:
                        with (
                            mock.patch.object(
                                observer_type,
                                "observe",
                                autospec=True,
                                side_effect=host_only_observe,
                            ),
                            mock.patch.object(
                                observer_type,
                                "request_stage",
                                side_effect=request_stage,
                                create=True,
                            ),
                            self.assertRaises(CandidateError) as raised,
                        ):
                            helper.run_one_real_host(
                                Path(temporary),
                                capture=capture,
                                guard_lifecycle_actions=True,
                            )
                    self.assertEqual(action_calls, 1)
                    event_names = [event[0] for event in capture["events"]]
                    self.assertIn("wait-enter", event_names)
                    self.assertIn("wait-exit", event_names)
                    self.assertIn(expected_diagnostic, str(raised.exception))
                    if not accepted:
                        self.assertNotIn("reaped-complete-empty", str(raised.exception))
                finally:
                    root_pid = int(capture.get("root_pid", -1))
                    child_pid = int(capture.get("child_pid", -1))
                    helper.bounded_test_cleanup(
                        *(pid for pid in (root_pid, child_pid) if pid > 0)
                    )


if __name__ == "__main__":
    unittest.main()
