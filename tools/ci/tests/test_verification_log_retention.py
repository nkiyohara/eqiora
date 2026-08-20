import contextlib
import io
import multiprocessing
import os
import signal
import stat
import sys
import tempfile
import time
import unittest
from pathlib import Path

CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

import resource_scheduler as resources  # noqa: E402
import verification_scheduler as scheduler  # noqa: E402

MAX_BYTES = 16 * 1024 * 1024
LANE = scheduler.VerificationLane("retention", resources.ResourceRequest(1, 1))
Budget, Command, Failure = resources.ResourceBudget, scheduler.PlannedCommand, scheduler.VerificationFailure  # fmt: skip
MARKER = "import signal,sys;signal.alarm(10);open(sys.argv[1],'x').write(sys.argv[2]+'\\n'+sys.argv[3])"
GUARD = "import os,sys;from pathlib import Path;lines=Path(sys.argv[1]).read_text().splitlines();assert len(lines)==1;run=Path(lines[0]);authority=Path(sys.argv[4]);assert run.is_absolute() and run==run.resolve() and run.parent.parent==authority;os.spawnv(os.P_WAIT,sys.executable,(sys.executable,'-c',sys.argv[5],sys.argv[2],sys.argv[3],str(run)))"
EXPECTED = "1 verification command(s) failed: frozen failure (exit 23)"


class _Capture(io.TextIOBase):
    def __init__(self, authority, events, fail=False):
        assert authority == authority.resolve() and not events.exists()
        self.authority, self.events, self.fail, self.parts = authority, events, fail, []

    def write(self, value):
        if self.fail and value.startswith("==> report failure "):
            raise OSError("frozen report failure")
        if len(value) <= 4096:
            self.parts.append(value)
        for word in value.split():
            path = Path(word)
            if path.is_absolute() and path == path.resolve() and path.parent.parent == self.authority:  # fmt: skip
                flags = os.O_WRONLY | os.O_CLOEXEC | (os.O_APPEND if self.events.exists() else os.O_CREAT | os.O_EXCL)  # fmt: skip
                descriptor = os.open(self.events, flags, 0o600)
                try:
                    os.write(descriptor, (word + "\n").encode())
                finally:
                    os.close(descriptor)
        return len(value)


def _command(label, index, scratch, events, marker, action="", *args, status=0, env=()):
    source = ";".join(filter(None, (GUARD, action, f"sys.exit({status})")))
    argv = (sys.executable, "-c", source, str(events), str(marker), str(index), str(scratch / "runs"), MARKER, *map(str, args))  # fmt: skip
    return Command(label, argv, env=env, lane=LANE)


def _plan(*commands):
    return scheduler.VerificationPlan("affected", (), (), (), commands, ())


def _invoke(plan, scratch, events, output=None):
    output = output or _Capture(scratch / "runs", events)
    with contextlib.redirect_stdout(output):
        try:
            scheduler.run_plan(
                plan, scratch.parent, budget=Budget(1, 1), scratch_root=scratch
            )
        except Exception as error:
            return "".join(output.parts), error
    return "".join(output.parts), None


def _events(path):
    return tuple(map(Path, path.read_text().splitlines())) if path.exists() else ()


def _marker(path):
    index, identity = path.read_text().splitlines()
    return int(index), Path(identity)


def _retained(scratch, events, expected, output=""):
    authority, logs = scratch / "runs", sorted((scratch / "runs").rglob("*.log"))
    assert logs and [path.name for path in logs] == sorted(expected)
    assert len({path.parent for path in logs}) == 1
    run = logs[0].parent
    assert _events(events) == (run.resolve(),) and run.parent.parent == authority
    assert authority == authority.resolve() and not any(path.is_symlink() for path in (authority, run.parent, run))  # fmt: skip
    assert stat.S_IMODE(run.stat().st_mode) == 0o700
    assert all(path.is_file() and not path.is_symlink() and stat.S_IMODE(path.stat().st_mode) == 0o600 and (expected[path.name] is None or path.read_bytes() == expected[path.name]) for path in logs)  # fmt: skip
    assert not output or str(run.resolve()) in output
    return run


def _success(events, marker):
    (identity,) = _events(events)
    assert _marker(marker) == (0, identity) and not identity.exists() and not identity.parent.exists()  # fmt: skip
    return identity


def _session(plan, scratch, events, receipt):
    os.setsid()
    output, error = _invoke(plan, scratch, events)
    receipt.write_text(type(error).__name__ + "\n" + str(error) + "\0" + output)
    receipt.chmod(0o444)


def _reap(process, wait):
    process.join(wait)
    for action in (signal.SIGTERM, signal.SIGKILL):
        if not process.is_alive():
            break
        with contextlib.suppress(ProcessLookupError):
            os.killpg(process.pid, action)
        process.join(2)
    assert not process.is_alive()


class VerificationLogRetentionOracle(unittest.TestCase):
    def test_positive_then_complete_causal_boundary(self):
        home = Path.home().resolve(strict=True)
        assert home.is_absolute() and home.is_dir() and not home.is_symlink()
        with tempfile.TemporaryDirectory(
            prefix="o-log-retention-", dir=home
        ) as temporary:
            base = Path(temporary)
            assert base == base.resolve()
            scratch, events = base / "ordinary", base / "ordinary.events"
            markers = [base / f"marker-{index}" for index in range(7)]
            first_out, first_err = b"first-out:\xff\n", b"first-err:\x80\n"
            failed_out, failed_err = b"failed-out:\xfe\n", b"failed-err:\x81\n"
            first_raw = f"assert os.path.realpath(os.getcwd())==os.path.realpath(sys.argv[6]);assert os.environ['O_LOG_ORACLE']=='frozen';os.write(2,bytes.fromhex('{first_err.hex()}'));os.write(1,bytes.fromhex('{first_out.hex()}'))"
            failed_raw = f"os.write(2,bytes.fromhex('{failed_err.hex()}'));os.write(1,bytes.fromhex('{failed_out.hex()}'))"
            ordinary = _plan(_command("frozen success", 0, scratch, events, markers[0], first_raw, base, env=(("O_LOG_ORACLE", "frozen"),)), _command("frozen failure", 1, scratch, events, markers[1], failed_raw, status=23), _command("must be skipped", 2, scratch, events, base / "skipped"))  # fmt: skip
            output, error = _invoke(ordinary, scratch, events)
            assert isinstance(error, Failure) and str(error) == EXPECTED, output
            assert (error.failures[0].command.label, error.failures[0].returncode) == ("frozen failure", 23)  # fmt: skip
            expected = {"0000.log": first_err + first_out, "0001.log": failed_err + failed_out}  # fmt: skip
            failed_run = _retained(scratch, events, expected, output)
            assert (_marker(markers[0]), _marker(markers[1])) == ((0, failed_run), (1, failed_run)) and not (base / "skipped").exists()  # fmt: skip
            reused = []
            for offset in range(2):
                reuse_events = base / f"reuse-{offset}.events"
                reuse_output, reuse_error = _invoke(_plan(_command(f"reuse {offset}", 0, scratch, reuse_events, markers[2 + offset], "os.write(1,b'clean\\n')")), scratch, reuse_events)  # fmt: skip
                assert reuse_error is None, reuse_output
                reused.append(_success(reuse_events, markers[2 + offset]))
                _retained(scratch, events, expected)
            assert reused[0].parent == reused[1].parent != failed_run.parent and reused[0] != reused[1]  # fmt: skip
            boundary, boundary_events, done = base / "boundary", base / "boundary.events", base / "boundary.done"  # fmt: skip
            stream = "chunk=b'Q'*65536;[(os.write(1,chunk)) for _ in range(256)];open(sys.argv[6],'wb').write(b'done')"
            boundary_output, boundary_error = _invoke(_plan(_command("exact boundary", 0, boundary, boundary_events, markers[4], stream, done)), boundary, boundary_events)  # fmt: skip
            assert boundary_error is None and done.read_bytes() == b"done", boundary_output  # fmt: skip
            _success(boundary_events, markers[4])
            overflow, overflow_events = base / "overflow", base / "overflow.events"
            ready, release, drained, receipt = base / "ready", base / "release", base / "drained", base / "overflow.receipt"  # fmt: skip
            os.mkfifo(release)
            control = os.open(release, os.O_RDWR | os.O_NONBLOCK)
            overflow_source = "chunk=b'Q'*65536;[(os.write(1,chunk)) for _ in range(256)];os.write(2,b'R');os.close(1);os.close(2);open(sys.argv[6],'wb').write(b'ready');open(sys.argv[7],'rb').read(1);open(sys.argv[8],'wb').write(b'done')"
            overflow_plan = _plan(_command("overflow witness", 0, overflow, overflow_events, markers[5], overflow_source, ready, release, drained))  # fmt: skip
            process = multiprocessing.get_context("fork").Process(target=_session, args=(overflow_plan, overflow, overflow_events, receipt))  # fmt: skip
            process.start()
            caught, deadline = None, time.monotonic() + 30
            try:
                while not ready.exists() and process.is_alive() and time.monotonic() < deadline:  # fmt: skip
                    time.sleep(0.01)
                assert ready.exists()
                live = list((overflow / "runs").rglob("0000.log"))
                assert len(live) == 1 and live[0].stat().st_size <= MAX_BYTES
            except BaseException as error:
                caught = error
            finally:
                with contextlib.suppress(OSError):
                    os.write(control, b"x")
                os.close(control)
                _reap(process, 5)
            if caught:
                raise caught
            terminal, overflow_output = receipt.read_text().split("\0", 1)
            assert terminal.startswith("VerificationFailure\n") and all(value in terminal.lower() for value in (str(MAX_BYTES), str(MAX_BYTES + 1), "incomplete"))  # fmt: skip
            assert drained.read_bytes() == b"done"
            overflow_run = _retained(overflow, overflow_events, {"0000.log": None}, overflow_output)  # fmt: skip
            assert (overflow_run / "0000.log").stat().st_size == MAX_BYTES and _marker(markers[5]) == (0, overflow_run)  # fmt: skip
            launch, launch_events, launch_marker = base / "launch", base / "launch.events", base / "launch.marker"  # fmt: skip
            missing = Command("execution failure", (str(base / "absent"), str(launch_events), str(launch_marker), "0", str(launch / "runs")), lane=LANE)  # fmt: skip
            launch_output, launch_error = _invoke(_plan(missing), launch, launch_events)
            assert isinstance(launch_error, Failure) and launch_error.failures[0].returncode is None and not launch_marker.exists()  # fmt: skip
            _retained(launch, launch_events, {"0000.log": b""}, launch_output)
            reporting, report_events = base / "report", base / "report.events"
            report_output = _Capture(reporting / "runs", report_events, fail=True)
            _, report_error = _invoke(_plan(_command("report failure", 0, reporting, report_events, markers[6], "os.write(1,b'reported\\n')")), reporting, report_events, report_output)  # fmt: skip
            assert type(report_error) is OSError
            report_run = _retained(reporting, report_events, {"0000.log": b"reported\n"}, "".join(report_output.parts))  # fmt: skip
            assert _marker(markers[6]) == (0, report_run) and len([path for path in markers if path.exists()]) == 7  # fmt: skip
            assert 2 * MAX_BYTES + 1 + sum(len(value or b"") for value in expected.values()) + 21 < 33_555_000  # fmt: skip
