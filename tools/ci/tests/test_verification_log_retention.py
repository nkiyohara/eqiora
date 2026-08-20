import contextlib
import io
import os
import stat
import sys
import tempfile
import threading
import unittest
from pathlib import Path

CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

import resource_scheduler as resources  # noqa: E402
import verification_scheduler as scheduler  # noqa: E402

MAX_BYTES = 16 * 1024 * 1024
LANE = scheduler.VerificationLane("retention", resources.ResourceRequest(1, 1))
Budget = resources.ResourceBudget
Command = scheduler.PlannedCommand
Failure = scheduler.VerificationFailure
run_plan = scheduler.run_plan
MARK = "from pathlib import Path;import sys;Path(sys.argv[1]).write_bytes(b'x')"
COUNT = "import os,sys;open(sys.argv[1],'ab').write(bytes([int(sys.argv[2])]))"
STREAM = "import os,sys;chunk=b'Q'*65536;[(os.write(1,chunk)) for _ in range(256)];open(sys.argv[1],'wb').write(b'done')"
OVERFLOW = "import os,sys;chunk=b'Q'*65536;[(os.write(1,chunk)) for _ in range(256)];os.write(2,b'R');os.close(1);os.close(2);open(sys.argv[2],'wb').write(b'ready');open(sys.argv[1],'rb').read(1);open(sys.argv[3],'wb').write(b'done')"
NESTED = "import pathlib,sys;sys.path.insert(0,sys.argv[1]);import resource_scheduler as r,verification_scheduler as s;lane=s.VerificationLane('nested',r.ResourceRequest(1,1));child=s.PlannedCommand('second-concurrent',(sys.executable,'-c',\"import os,sys;os.write(1,b'inner\\n');sys.exit(31)\"),lane=lane);plan=s.VerificationPlan('affected',(),(),(),(child,),());s.run_plan(plan,pathlib.Path(sys.argv[2]),budget=r.ResourceBudget(1,1),scratch_root=pathlib.Path(sys.argv[3]))"
EXPECTED_FAILURE = "1 verification command(s) failed: frozen failure (exit 23)"


class _Capture(io.TextIOBase):
    def __init__(self, scratch, fail=False):
        self.parts = []
        self.fail = fail
        self.witness = scratch.parent / f".{scratch.name}-announcement"
        self.witness.unlink(missing_ok=True)
        self.prefix = str((scratch / "runs").absolute()) + os.sep

    def write(self, value):
        if self.fail and value.startswith("==> report failure "):
            raise OSError("frozen report failure")
        if len(value) <= 4096:
            self.parts.append(value)
        for word in value.split():
            if word.startswith(self.prefix):
                self.witness.write_text(word)
        return len(value)


def _python(label, source, *args, env=()):
    return Command(label, (sys.executable, "-c", source, *args), env=env, lane=LANE)


def _source(stdout, stderr, status=0, guard=""):
    return f"import os,sys;{guard}os.write(2,bytes.fromhex('{stderr.hex()}'));os.write(1,bytes.fromhex('{stdout.hex()}'));sys.exit({status})"


def _plan(*commands):
    return scheduler.VerificationPlan("affected", (), (), (), commands, ())


def _invoke(plan, scratch, output=None):
    output = output or _Capture(scratch)
    with contextlib.redirect_stdout(output):
        try:
            run_plan(plan, scratch.parent, budget=Budget(1, 1), scratch_root=scratch)
        except Exception as error:
            return "".join(output.parts), error
    return "".join(output.parts), None


def _retained(scratch, expected, output=None):
    authority = scratch / "runs"
    logs = sorted(authority.rglob("*.log"))
    assert logs and [path.name for path in logs] == sorted(expected)
    assert len({path.parent for path in logs}) == 1
    run = logs[0].parent
    assert run.parent.parent == authority and run.resolve().is_relative_to(authority)
    assert authority.resolve() == authority.absolute() and not authority.is_symlink()
    assert stat.S_IMODE(run.stat().st_mode) == 0o700 and not run.is_symlink()
    assert not run.parent.is_symlink()
    assert all(path.is_file() and not path.is_symlink() and stat.S_IMODE(path.stat().st_mode) == 0o600 and (expected[path.name] is None or path.read_bytes() == expected[path.name]) for path in logs)  # fmt: skip
    if output is not None:
        prefix = str(authority.resolve()) + os.sep
        assert run.resolve() in set(map(Path, (word for word in output.split() if word.startswith(prefix))))  # fmt: skip
    return run


def _stop(plan, scratch, marker):
    output, error = _invoke(plan, scratch)
    assert error is not None, output
    assert not marker.exists()


class VerificationLogRetentionOracle(unittest.TestCase):
    def test_positive_then_complete_causal_boundary(self):
        assert (32 * MAX_BYTES, 64, 64 * MAX_BYTES) == (536870912, 64, 1073741824)
        first_out, first_err = b"first-out:\xff\n", b"first-err:\x80\n"
        failed_out, failed_err = b"failed-out:\xfe\n", b"failed-err:\x81\n"
        with tempfile.TemporaryDirectory(prefix="o-log-", dir=Path.home()) as temporary:
            base, scratch = Path(temporary), Path(temporary) / "scratch"
            witness, started, sentinel = base / ".scratch-announcement", base / "started", base / "sentinel"  # fmt: skip
            guard = "assert os.path.realpath(os.getcwd())==os.path.realpath(sys.argv[1]);assert os.environ['O_LOG_ORACLE']=='frozen';from pathlib import Path;assert Path(sys.argv[2]).is_file();os.spawnv(os.P_WAIT,sys.executable,(sys.executable,'-c',sys.argv[3],sys.argv[4]));"
            first = _source(first_out, first_err, guard=guard)
            ordinary = _plan(_python("frozen success", first, str(base), str(witness), MARK, str(started), env=(("O_LOG_ORACLE", "frozen"),)), _python("frozen failure", _source(failed_out, failed_err, 23)), _python("must be skipped", MARK, str(sentinel)))  # fmt: skip
            output, error = _invoke(ordinary, scratch)
            assert isinstance(error, Failure)
            assert str(error) == EXPECTED_FAILURE
            assert (error.failures[0].command.label, error.failures[0].returncode) == ("frozen failure", 23)  # fmt: skip
            expected = {"0000.log": first_err + first_out, "0001.log": failed_err + failed_out}  # fmt: skip
            failed_run = _retained(scratch, expected, output)
            assert witness.read_text() == str(failed_run.resolve())
            assert started.exists() and not sentinel.exists()
            assert not (failed_run / "0002.log").exists()
            reused = []
            for label in ("first reuse", "second reuse"):
                success = _plan(_python(label, _source(b"clean\n", b"")))
                success_output, success_error = _invoke(success, scratch)
                assert success_error is None, success_output
                prefix = str((scratch / "runs").resolve()) + os.sep
                paths = set(map(Path, (word for word in success_output.split() if word.startswith(prefix))))  # fmt: skip
                assert len(paths) == 1
                reused.append(paths.pop())
                _retained(scratch, expected)
            assert reused[0].parent == reused[1].parent != failed_run.parent
            assert reused[0] != reused[1] and not any(path.exists() for path in reused)
            boundary, done = base / "boundary", base / "done"
            boundary_output, boundary_error = _invoke(_plan(_python("exact boundary", STREAM, str(done))), boundary)  # fmt: skip
            assert boundary_error is None and done.read_bytes() == b"done", boundary_output  # fmt: skip
            overflow_root, drained, release, ready = base / "overflow", base / "drained", base / "release", base / "ready"  # fmt: skip
            os.mkfifo(release)
            control = os.open(release, os.O_RDWR | os.O_NONBLOCK)
            overflow_plan = _plan(_python("overflow witness", OVERFLOW, str(release), str(ready), str(drained)))  # fmt: skip
            result = []
            worker = threading.Thread(target=lambda: result.append(_invoke(overflow_plan, overflow_root)))  # fmt: skip
            worker.start()
            while not ready.exists():
                assert worker.is_alive()
            live = list((overflow_root / "runs").rglob("0000.log"))
            assert len(live) == 1 and live[0].stat().st_size <= MAX_BYTES
            os.write(control, b"x")
            os.close(control)
            worker.join()
            overflow_output, overflow_error = result[0]
            assert isinstance(overflow_error, Failure)
            detail = str(overflow_error).replace(",", "").replace("_", "").lower()
            assert all(value in detail for value in (str(MAX_BYTES), str(MAX_BYTES + 1), "incomplete")) and drained.read_bytes() == b"done"  # fmt: skip
            assert (_retained(overflow_root, {"0000.log": None}, overflow_output) / "0000.log").stat().st_size == MAX_BYTES  # fmt: skip
            count_file, count_root = base / "counted", base / "capacity"
            commands = tuple(_python(f"count {index}", COUNT + (";sys.exit(7)" if index == 31 else ""), str(count_file), str(index)) for index in range(32))  # fmt: skip
            for turn in range(2):
                count_output, count_error = _invoke(_plan(*commands), count_root)
                assert isinstance(count_error, Failure), count_output
                assert count_file.read_bytes() == bytes(range(32)) * (turn + 1)
            capacity_logs = sorted((count_root / "runs").rglob("*.log"))
            capacity_runs = {path.parent for path in capacity_logs}
            assert (len(capacity_logs), len(capacity_runs), len({run.parent for run in capacity_runs})) == (64, 2, 2)  # fmt: skip
            assert {sum(path.parent == run for path in capacity_logs) for run in capacity_runs} == {32}  # fmt: skip
            marker = base / "must-not-start"
            mark = _python("must not start", MARK, str(marker))
            overcount, empty = base / "overcount", base / "empty"
            _stop(_plan(mark, *([mark] * 32)), overcount, marker)
            assert not (overcount / "runs").exists()
            _stop(_plan(), empty, marker)
            assert not (empty / "runs").exists()
            concurrent = base / "concurrent"
            nested_args = str(CI_ROOT), str(base), str(concurrent)
            concurrent_output, concurrent_error = _invoke(_plan(_python("first-concurrent", NESTED, *nested_args)), concurrent)  # fmt: skip
            assert isinstance(concurrent_error, Failure)
            logs = sorted((concurrent / "runs").rglob("*.log"))
            run_paths = sorted({path.parent for path in logs})
            assert len(logs) == len(run_paths) == len({path.parent for path in run_paths}) == 2  # fmt: skip
            prefix = str((concurrent / "runs").resolve()) + os.sep
            assert set(map(Path, (word for word in concurrent_output.split() if word.startswith(prefix)))) == {path.resolve() for path in run_paths}  # fmt: skip
            occupied = {path: path.read_bytes() for path in capacity_logs}
            for stale in (old := next(iter(capacity_runs)), old.parent):
                os.utime(stale, ns=(1, 1))
            _stop(_plan(mark), count_root, marker)
            fresh = sorted((count_root / "runs").rglob("*.log"))
            assert fresh == capacity_logs and {path: path.read_bytes() for path in fresh} == occupied  # fmt: skip
            slot_names = sorted(path.parent.name for path in run_paths)
            for name in slot_names:
                for kind in ("symlink", "special", "fifo"):
                    hostile = base / f"{name}-{kind}"
                    slots, target = hostile / "runs", hostile / "foreign"
                    slots.mkdir(parents=True)
                    target.mkdir()
                    path = slots / name
                    if kind == "symlink":
                        path.symlink_to(target, target_is_directory=True)
                    elif kind == "fifo":
                        os.mkfifo(path)
                    else:
                        path.write_bytes(b"foreign")
                    identity = path.lstat().st_mode, os.readlink(path) if path.is_symlink() else path.read_bytes() if path.is_file() else None  # fmt: skip
                    _stop(_plan(mark), hostile, marker)
                    assert (path.lstat().st_mode, os.readlink(path) if path.is_symlink() else path.read_bytes() if path.is_file() else None) == identity  # fmt: skip
            authority, outside = base / "authority", base / "outside-runs"
            authority.mkdir()
            outside.mkdir()
            (authority / "runs").symlink_to(outside, target_is_directory=True)
            _stop(_plan(mark), authority, marker)
            assert os.readlink(authority / "runs") == str(outside)
            extra = base / "extra"
            foreign = extra / "runs" / slot_names[0] / "unexpected"
            foreign.parent.mkdir(parents=True)
            foreign.write_bytes(b"preserve")
            _stop(_plan(mark), extra, marker)
            assert foreign.read_bytes() == b"preserve"
            assert set((extra / "runs").iterdir()) == {foreign.parent}
            execution = base / "execution-error"
            execution_output, execution_error = _invoke(_plan(Command("execution failure", (str(base / "absent"),), lane=LANE)), execution)  # fmt: skip
            assert isinstance(execution_error, Failure)
            assert execution_error.failures[0].returncode is None
            _retained(execution, {"0000.log": b""}, execution_output)
            reporting = base / "reporting-error"
            report_output = _Capture(reporting, fail=True)
            _, report_error = _invoke(_plan(_python("report failure", _source(b"reported\n", b""))), reporting, report_output)  # fmt: skip
            assert type(report_error) is OSError
            _retained(reporting, {"0000.log": b"reported\n"}, "".join(report_output.parts))  # fmt: skip
