import contextlib
import io
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

import resource_scheduler as resources  # noqa: E402
import verification_scheduler as scheduler  # noqa: E402

MAX_BYTES = 16 * 1024 * 1024
LANE = scheduler.VerificationLane("retention", resources.ResourceRequest(1, 1))
Budget = resources.ResourceBudget
Command = scheduler.PlannedCommand
Failure = scheduler.VerificationFailure
MARK = "from pathlib import Path;import sys;Path(sys.argv[1]).write_bytes(b'x')"
STREAM = "import os,sys;chunk=b'Q'*65536;[(os.write(1,chunk)) for _ in range(256)];open(sys.argv[1],'wb').write(b'done')"
EXPECTED_FAILURE = "1 verification command(s) failed: frozen failure (exit 23)"


class _Capture(io.TextIOBase):
    def __init__(self) -> None:
        self.parts: list[str] = []

    def write(self, value: str) -> int:
        if len(value) <= 4096:
            self.parts.append(value)
        return len(value)


def _python(label, source, *args, env=()):
    return Command(label, (sys.executable, "-c", source, *args), env=env, lane=LANE)


def _source(stdout, stderr, status=0, guard=""):
    return f"import os,sys;{guard}os.write(1,bytes.fromhex('{stdout.hex()}'));os.write(2,bytes.fromhex('{stderr.hex()}'));sys.exit({status})"


def _plan(*commands):
    return scheduler.VerificationPlan("affected", (), (), (), commands, ())


def _invoke(plan, root, scratch):
    output = _Capture()
    with contextlib.redirect_stdout(output):
        try:
            scheduler.run_plan(plan, root, budget=Budget(1, 1), scratch_root=scratch)
        except Exception as error:  # admission STOP type is deliberately private
            return "".join(output.parts), error
    return "".join(output.parts), None


def _retained(scratch, expected, output=None):
    logs = sorted((scratch / "runs").rglob("*.log"))
    assert logs, "failed run was not retained"
    assert [path.name for path in logs] == sorted(expected)
    assert len({path.parent for path in logs}) == 1
    run = logs[0].parent
    assert run.parent.parent == scratch / "runs"
    assert stat.S_IMODE(run.stat().st_mode) == 0o700
    assert not run.is_symlink() and not run.parent.is_symlink()
    assert run.resolve().is_relative_to((scratch / "runs").resolve())
    for path in logs:
        assert path.is_file() and not path.is_symlink()
        assert stat.S_IMODE(path.stat().st_mode) == 0o600
        if expected[path.name] is not None:
            assert path.read_bytes() == expected[path.name]
    if output is not None:
        assert output.index(str(run.resolve())) < output.index("==> starting lanes:")
    return run


class VerificationLogRetentionOracle(unittest.TestCase):
    def test_positive_then_complete_causal_boundary(self) -> None:
        assert (32 * MAX_BYTES, 64, 64 * MAX_BYTES) == (536870912, 64, 1073741824)
        first_out, first_err = b"first-out:\xff\n", b"first-err:\x80\n"
        failed_out, failed_err = b"failed-out:\xfe\n", b"failed-err:\x81\n"
        with tempfile.TemporaryDirectory(prefix="o-log-", dir=Path.home()) as temporary:
            root, scratch = Path(temporary) / "root", Path(temporary) / "scratch"
            root.mkdir()
            sentinel = Path(temporary) / "sentinel"
            guard = "assert os.path.realpath(os.getcwd())==os.path.realpath(sys.argv[1]);assert os.environ['O_LOG_ORACLE']=='frozen';"
            ordinary = _plan(
                _python(
                    "frozen success",
                    _source(first_out, first_err, guard=guard),
                    str(root),
                    env=(("O_LOG_ORACLE", "frozen"),),
                ),
                _python("frozen failure", _source(failed_out, failed_err, 23)),
                _python("must be skipped", MARK, str(sentinel)),
            )
            output, error = _invoke(ordinary, root, scratch)
            assert isinstance(error, Failure)
            assert str(error) == EXPECTED_FAILURE
            failure = error.failures[0]
            assert (failure.command.label, failure.returncode) == ("frozen failure", 23)
            expected = {
                "0000.log": first_out + first_err,
                "0001.log": failed_out + failed_err,
            }
            run = _retained(scratch, expected, output)
            assert not sentinel.exists() and not (run / "0002.log").exists()
            success_output, success_error = _invoke(
                _plan(_python("clean success", _source(b"clean\n", b""))), root, scratch
            )
            assert success_error is None, success_output
            _retained(scratch, expected)
            boundary, done = Path(temporary) / "boundary", Path(temporary) / "done"
            boundary_output, boundary_error = _invoke(
                _plan(_python("exact boundary", STREAM, str(done))), root, boundary
            )
            assert boundary_error is None, boundary_output
            assert done.read_bytes() == b"done"
            overflow_root = Path(temporary) / "overflow"
            drained = Path(temporary) / "drained"
            overflow = STREAM.replace("open(", "os.write(2,b'R');open(")
            overflow_output, overflow_error = _invoke(
                _plan(_python("overflow witness", overflow, str(drained))),
                root,
                overflow_root,
            )
            assert isinstance(overflow_error, Failure)
            detail = str(overflow_error).replace(",", "").replace("_", "").lower()
            assert str(MAX_BYTES) in detail and str(MAX_BYTES + 1) in detail
            assert "incomplete" in detail and drained.read_bytes() == b"done"
            overflow_run = _retained(overflow_root, {"0000.log": None}, overflow_output)
            overflow_log = overflow_run / "0000.log"
            assert overflow_log.stat().st_size == MAX_BYTES
            marker = Path(temporary) / "must-not-start"
            mark = _python("must not start", MARK, str(marker))
            count = Path(temporary) / "count"
            count_output, count_error = _invoke(
                _plan(mark, *([mark] * 32)), root, count
            )
            assert count_error is not None, count_output
            assert not marker.exists() and not (count / "runs").exists()
            concurrent = Path(temporary) / "concurrent"
            started, nested_outputs = [], []

            def fail_nested(argv, **kwargs):
                started.append(argv[0])
                kwargs["stdout"].write((argv[0] + "\n").encode())
                kwargs["stdout"].flush()
                if argv[0] == "first-concurrent":
                    nested = _plan(
                        Command("second-concurrent", ("second-concurrent",), lane=LANE)
                    )
                    nested_output, nested_error = _invoke(nested, root, concurrent)
                    assert isinstance(nested_error, Failure)
                    nested_outputs.append(nested_output)
                raise subprocess.CalledProcessError(31, argv)

            patched = mock.patch(
                "verification_scheduler.subprocess.run", side_effect=fail_nested
            )
            first = _plan(Command("first-concurrent", ("first-concurrent",), lane=LANE))
            with patched:
                concurrent_output, concurrent_error = _invoke(first, root, concurrent)
            assert isinstance(concurrent_error, Failure)
            assert sorted(started) == ["first-concurrent", "second-concurrent"]
            runs = concurrent / "runs"
            run_paths = sorted({path.parent for path in runs.rglob("*.log")})
            assert len(run_paths) == len({path.parent for path in run_paths}) == 2
            emitted = concurrent_output + "".join(nested_outputs)
            assert all(str(path.resolve()) in emitted for path in run_paths)
            occupied = {path: path.read_bytes() for path in runs.rglob("*.log")}
            for stale in (run_paths[0], run_paths[0].parent):
                os.utime(stale, ns=(1, 1))
            third_output, third_error = _invoke(_plan(mark), root, concurrent)
            assert third_error is not None, third_output
            assert not marker.exists()
            assert {path: path.read_bytes() for path in runs.rglob("*.log")} == occupied
            slot_names = sorted(path.parent.name for path in run_paths)
            for kind in ("symlink", "special"):
                hostile = Path(temporary) / kind
                slots, target = hostile / "runs", hostile / "foreign"
                slots.mkdir(parents=True)
                target.mkdir()
                for name in slot_names:
                    path = slots / name
                    if kind == "symlink":
                        path.symlink_to(target, target_is_directory=True)
                    else:
                        path.write_bytes(b"foreign")
                hostile_output, hostile_error = _invoke(_plan(mark), root, hostile)
                assert hostile_error is not None, hostile_output
                if kind == "symlink":
                    assert all((slots / name).is_symlink() for name in slot_names)
                else:
                    values = {(slots / name).read_bytes() for name in slot_names}
                    assert values == {b"foreign"}
            extra = Path(temporary) / "extra"
            foreign = extra / "runs" / slot_names[0] / "unexpected"
            foreign.parent.mkdir(parents=True)
            foreign.write_bytes(b"preserve")
            extra_output, extra_error = _invoke(
                _plan(_python("other-slot success", _source(b"ok", b""))), root, extra
            )
            assert extra_error is None, extra_output
            assert foreign.read_bytes() == b"preserve"
            assert set((extra / "runs").iterdir()) == {foreign.parent}
