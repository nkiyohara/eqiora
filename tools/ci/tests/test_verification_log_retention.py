import contextlib
import io
import os
import stat
import sys
import tempfile
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
NESTED = "import pathlib,sys;sys.path.insert(0,sys.argv[1]);import resource_scheduler as r,verification_scheduler as s;lane=s.VerificationLane('nested',r.ResourceRequest(1,1));child=s.PlannedCommand('second-concurrent',(sys.executable,'-c',\"import os,sys;os.write(1,b'inner\\n');sys.exit(31)\"),lane=lane);plan=s.VerificationPlan('affected',(),(),(),(child,),());s.run_plan(plan,pathlib.Path(sys.argv[2]),budget=r.ResourceBudget(1,1),scratch_root=pathlib.Path(sys.argv[3]))"
EXPECTED_FAILURE = "1 verification command(s) failed: frozen failure (exit 23)"


class _Capture(io.TextIOBase):
    def __init__(self, fail=False):
        self.parts = []
        self.fail = fail

    def write(self, value):
        if self.fail and value.startswith("==> report failure "):
            raise OSError("frozen report failure")
        if len(value) <= 4096:
            self.parts.append(value)
        return len(value)


def _python(label, source, *args, env=()):
    return Command(label, (sys.executable, "-c", source, *args), env=env, lane=LANE)


def _source(stdout, stderr, status=0, guard=""):
    return f"import os,sys;{guard}os.write(1,bytes.fromhex('{stdout.hex()}'));os.write(2,bytes.fromhex('{stderr.hex()}'));sys.exit({status})"


def _plan(*commands):
    return scheduler.VerificationPlan("affected", (), (), (), commands, ())


def _invoke(plan, scratch, output=None):
    output = output or _Capture()
    with contextlib.redirect_stdout(output):
        try:
            run_plan(plan, scratch.parent, budget=Budget(1, 1), scratch_root=scratch)
        except Exception as error:
            return "".join(output.parts), error
    return "".join(output.parts), None


def _announced(scratch, output):
    prefix = str((scratch / "runs").resolve()) + os.sep
    return set(map(Path, (word for word in output.split() if word.startswith(prefix))))


def _retained(scratch, expected, output=None):
    logs = sorted((scratch / "runs").rglob("*.log"))
    assert logs and [path.name for path in logs] == sorted(expected)
    assert len({path.parent for path in logs}) == 1
    run = logs[0].parent
    assert run.parent.parent == scratch / "runs"
    assert stat.S_IMODE(run.stat().st_mode) == 0o700
    assert not (run.is_symlink() or run.parent.is_symlink())
    for path in logs:
        assert path.is_file() and not path.is_symlink()
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        if expected[path.name] is not None:
            assert path.read_bytes() == expected[path.name]
    if output is not None:
        assert run.resolve() in _announced(scratch, output)
        assert output.index(str(run.resolve())) < output.index("==> starting lanes:")
    return run


def _stop(plan, scratch, marker=None):
    output, error = _invoke(plan, scratch)
    assert error is not None, output
    assert marker is None or not marker.exists()


class VerificationLogRetentionOracle(unittest.TestCase):
    def test_positive_then_complete_causal_boundary(self):
        assert (32 * MAX_BYTES, 64, 64 * MAX_BYTES) == (536870912, 64, 1073741824)
        first_out, first_err = b"first-out:\xff\n", b"first-err:\x80\n"
        failed_out, failed_err = b"failed-out:\xfe\n", b"failed-err:\x81\n"
        with tempfile.TemporaryDirectory(prefix="o-log-", dir=Path.home()) as temporary:
            base, scratch = Path(temporary), Path(temporary) / "scratch"
            sentinel = base / "sentinel"
            guard = "assert os.path.realpath(os.getcwd())==os.path.realpath(sys.argv[1]);assert os.environ['O_LOG_ORACLE']=='frozen';"
            first = _source(first_out, first_err, guard=guard)
            ordinary = _plan(
                _python(
                    "frozen success",
                    first,
                    str(base),
                    env=(("O_LOG_ORACLE", "frozen"),),
                ),
                _python("frozen failure", _source(failed_out, failed_err, 23)),
                _python("must be skipped", MARK, str(sentinel)),
            )
            output, error = _invoke(ordinary, scratch)
            assert isinstance(error, Failure)
            assert str(error) == EXPECTED_FAILURE
            failure = error.failures[0]
            assert (failure.command.label, failure.returncode) == ("frozen failure", 23)
            expected = {"0000.log": first_out + first_err}
            expected["0001.log"] = failed_out + failed_err
            failed_run = _retained(scratch, expected, output)
            assert not sentinel.exists() and not (failed_run / "0002.log").exists()
            reused = []
            for label in ("first reuse", "second reuse"):
                success = _plan(_python(label, _source(b"clean\n", b"")))
                success_output, success_error = _invoke(success, scratch)
                assert success_error is None, success_output
                paths = _announced(scratch, success_output)
                assert len(paths) == 1
                reused.append(paths.pop())
                _retained(scratch, expected)
            assert reused[0].parent == reused[1].parent != failed_run.parent
            assert reused[0] != reused[1] and not any(path.exists() for path in reused)
            boundary, done = base / "boundary", base / "done"
            boundary_plan = _plan(_python("exact boundary", STREAM, str(done)))
            boundary_output, boundary_error = _invoke(boundary_plan, boundary)
            assert boundary_error is None, boundary_output
            assert done.read_bytes() == b"done"
            overflow_root, drained = base / "overflow", base / "drained"
            overflow = STREAM.replace("open(", "os.write(2,b'R');open(")
            overflow_plan = _plan(_python("overflow witness", overflow, str(drained)))
            overflow_output, overflow_error = _invoke(overflow_plan, overflow_root)
            assert isinstance(overflow_error, Failure)
            detail = str(overflow_error).replace(",", "").replace("_", "").lower()
            assert str(MAX_BYTES) in detail and str(MAX_BYTES + 1) in detail
            assert "incomplete" in detail and drained.read_bytes() == b"done"
            overflow_run = _retained(overflow_root, {"0000.log": None}, overflow_output)
            overflow_log = overflow_run / "0000.log"
            assert overflow_log.stat().st_size == MAX_BYTES
            count_file, count_root = base / "counted", base / "count"
            commands = tuple(
                _python(f"count {index}", COUNT, str(count_file), str(index))
                for index in range(32)
            )
            count_output, count_error = _invoke(_plan(*commands), count_root)
            assert count_error is None, count_output
            assert count_file.read_bytes() == bytes(range(32))
            marker = base / "must-not-start"
            mark = _python("must not start", MARK, str(marker))
            overcount = base / "overcount"
            _stop(_plan(mark, *([mark] * 32)), overcount, marker)
            assert not (overcount / "runs").exists()
            empty = base / "empty"
            _stop(_plan(), empty)
            assert not (empty / "runs").exists()
            concurrent = base / "concurrent"
            nested_args = (str(CI_ROOT), str(base), str(concurrent))
            nested = _python("first-concurrent", NESTED, *nested_args)
            concurrent_output, concurrent_error = _invoke(_plan(nested), concurrent)
            assert isinstance(concurrent_error, Failure)
            logs = sorted((concurrent / "runs").rglob("*.log"))
            run_paths = sorted({path.parent for path in logs})
            assert len(logs) == len(run_paths) == 2
            assert len({path.parent for path in run_paths}) == 2
            resolved = {path.resolve() for path in run_paths}
            assert _announced(concurrent, concurrent_output) == resolved
            occupied = {path: path.read_bytes() for path in logs}
            for stale in (run_paths[0], run_paths[0].parent):
                os.utime(stale, ns=(1, 1))
            _stop(_plan(mark), concurrent, marker)
            fresh = sorted((concurrent / "runs").rglob("*.log"))
            assert fresh == logs
            assert {path: path.read_bytes() for path in fresh} == occupied
            slot_names = sorted(path.parent.name for path in run_paths)
            for kind in ("symlink", "special"):
                hostile = base / kind
                slots, target = hostile / "runs", hostile / "foreign"
                slots.mkdir(parents=True)
                target.mkdir()
                for name in slot_names:
                    path = slots / name
                    path.symlink_to(
                        target, target_is_directory=True
                    ) if kind == "symlink" else path.write_bytes(b"foreign")
                _stop(_plan(mark), hostile, marker)
                paths = [slots / name for name in slot_names]
                if kind == "symlink":
                    assert all(path.is_symlink() for path in paths)
                else:
                    assert {path.read_bytes() for path in paths} == {b"foreign"}
            extra = base / "extra"
            foreign = extra / "runs" / slot_names[0] / "unexpected"
            foreign.parent.mkdir(parents=True)
            foreign.write_bytes(b"preserve")
            _stop(_plan(mark), extra, marker)
            assert foreign.read_bytes() == b"preserve"
            assert set((extra / "runs").iterdir()) == {foreign.parent}
            execution = base / "execution-error"
            missing = Command("execution failure", (str(base / "absent"),), lane=LANE)
            execution_output, execution_error = _invoke(_plan(missing), execution)
            assert isinstance(execution_error, Failure)
            assert execution_error.failures[0].returncode is None
            _retained(execution, {"0000.log": b""}, execution_output)
            reporting = base / "reporting-error"
            report_output = _Capture(fail=True)
            report_plan = _plan(_python("report failure", _source(b"reported\n", b"")))
            _, report_error = _invoke(report_plan, reporting, report_output)
            assert type(report_error) is OSError
            report_text = "".join(report_output.parts)
            _retained(reporting, {"0000.log": b"reported\n"}, report_text)
