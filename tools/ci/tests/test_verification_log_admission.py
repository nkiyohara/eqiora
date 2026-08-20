import contextlib
import io
import itertools
import multiprocessing
import os
import signal
import stat
import sys
import tempfile
import unittest
from pathlib import Path

CI_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(CI_ROOT))

import resource_scheduler as resources  # noqa: E402
import verification_scheduler as scheduler  # noqa: E402

LANE, Budget, Command, Failure = scheduler.VerificationLane("admission", resources.ResourceRequest(1, 1)), resources.ResourceBudget, scheduler.PlannedCommand, scheduler.VerificationFailure  # fmt: skip
MARKER = "import signal,sys;signal.alarm(10);open(sys.argv[1],'x').write(sys.argv[2]+'\\n'+sys.argv[3])"
GUARD = "import os,sys;from pathlib import Path;lines=Path(sys.argv[1]).read_text().splitlines();assert len(lines)==1;run=Path(lines[0]);authority=Path(sys.argv[4]);assert run.is_absolute() and run==run.resolve() and run.parent.parent==authority;os.spawnv(os.P_WAIT,sys.executable,(sys.executable,'-c',sys.argv[5],sys.argv[2],sys.argv[3],str(run)))"
NESTED = "import sys;sys.path.insert(0,sys.argv[6]);from tests.test_verification_log_admission import _command,_invoke,_plan;inner=Path(sys.argv[8]);events,marker,receipt=map(Path,sys.argv[9:12]);output,error=_invoke(_plan(_command('inner',0,inner,events,marker,status=31)),inner,events);receipt.write_text(type(error).__name__+'\\n'+str(error)+'\\0'+output);receipt.chmod(0o444)"


# fmt: off
class _Capture(io.TextIOBase):
    def __init__(self, authority, events):
        assert not events.exists()
        self.authority, self.events, self.parts = authority, events, []

    def write(self, value):
        if len(value) <= 4096:
            self.parts.append(value)
        for word in value.split():
            path = Path(word)
            if path.is_absolute() and path == path.resolve() and path.parent.parent == self.authority:  # fmt: skip
                flags = os.O_WRONLY | os.O_CLOEXEC | (os.O_APPEND if self.events.exists() else os.O_CREAT | os.O_EXCL)  # fmt: skip
                with os.fdopen(os.open(self.events, flags, 0o600), "wb") as stream:
                    stream.write((word + "\n").encode())
        return len(value)
def _command(label, index, scratch, events, marker, action="", *args, status=0):
    argv = (sys.executable, "-c", ";".join(filter(None, (GUARD, action, f"sys.exit({status})"))), str(events), str(marker), str(index), str(scratch / "runs"), MARKER, *map(str, args))
    return Command(label, argv, lane=LANE)
def _plan(*commands):
    return scheduler.VerificationPlan("affected", (), (), (), commands, ())
def _invoke(plan, scratch, events):
    output = _Capture(scratch / "runs", events)
    with contextlib.redirect_stdout(output):
        try:
            scheduler.run_plan(plan, scratch.parent, budget=Budget(1, 1), scratch_root=scratch)
        except Exception as error:
            return "".join(output.parts), error
    return "".join(output.parts), None
def _deny(scratch, events, marker, count=1):
    output, error = _invoke(_plan(*([_command("must not start", 0, scratch, events, marker)] * count)), scratch, events)  # fmt: skip
    assert error is not None and not events.exists() and not marker.exists(), output
def _snapshot(path):
    mode = path.lstat().st_mode
    payload = (os.readlink(path), _snapshot(path.resolve())) if path.is_symlink() else path.read_bytes() if stat.S_ISREG(mode) else tuple((entry.name, _snapshot(entry)) for entry in sorted(path.iterdir())) if stat.S_ISDIR(mode) else None  # fmt: skip
    return mode, payload
def _state(path, kind):
    if kind == "extra":
        path.mkdir(mode=0o750)
        (path / "unexpected").write_bytes(b"frozen-extra")
        (path / "unexpected").chmod(0o640)
    elif kind == "symlink":
        target = path.parent.parent / f"{path.name}-target"
        target.mkdir()
        (target / "outside").write_bytes(b"outside")
        path.symlink_to(target, target_is_directory=True)
    elif kind == "file":
        path.write_bytes(b"frozen-file")
    else:
        os.mkfifo(path)
    return _snapshot(path)
def _session(plan, scratch, events, receipt):
    os.setsid()
    output, error = _invoke(plan, scratch, events)
    (receipt.write_text(type(error).__name__ + "\n" + str(error) + "\0" + output), receipt.chmod(0o444))
def _reap(process, wait):
    for timeout, action in ((wait, None), (2, signal.SIGTERM), (2, signal.SIGKILL)):
        if action and process.is_alive():
            with contextlib.suppress(ProcessLookupError):
                os.killpg(process.pid, action)
        process.join(timeout)
    assert not process.is_alive()
class VerificationLogAdmissionOracle(unittest.TestCase):
    def test_positive_then_complete_causal_boundary(self):
        home = Path.home().resolve(strict=True)
        assert home.is_absolute() and home.is_dir() and not home.is_symlink()
        with tempfile.TemporaryDirectory(
            prefix="o-log-admission-", dir=home
        ) as temporary:
            base, scratch = Path(temporary), Path(temporary) / "capacity"
            assert base == base.resolve() and scratch == scratch.resolve() and scratch / "runs" == (scratch / "runs").resolve()
            runs, all_markers = [], []
            for turn in range(2):
                events, counted = base / f"capacity-{turn}.events", base / f"counted-{turn}"  # fmt: skip
                markers = [base / f"capacity-{turn}-{index}.marker" for index in range(32)]  # fmt: skip
                action = "open(sys.argv[6],'ab').write(bytes([int(sys.argv[3])]))"
                commands = tuple(_command(f"count {index}", index, scratch, events, markers[index], action, counted, status=7 if index == 31 else 0) for index in range(32))  # fmt: skip
                output, error = _invoke(_plan(*commands), scratch, events)
                assert isinstance(error, Failure) and counted.exists() and counted.read_bytes() == bytes(range(32)), output  # fmt: skip
                assert (error.failures[0].command.label, error.failures[0].returncode) == ("count 31", 7)  # fmt: skip
                (identity,) = map(Path, events.read_text().splitlines())
                assert all(marker.read_text().splitlines() == [str(index), str(identity)] for index, marker in enumerate(markers))  # fmt: skip
                (runs.append(identity), all_markers.extend(markers))
            logs = sorted((scratch / "runs").rglob("*.log"))
            assert (len(logs), len(runs), len({run.parent for run in runs})) == (64, 2, 2)  # fmt: skip
            assert {sum(path.parent == run for path in logs) for run in runs} == {32}
            assert set(runs) == {path.parent.resolve() for path in logs}
            assert (32 * 16 * 1024 * 1024, 64 * 16 * 1024 * 1024) == (536_870_912, 1_073_741_824)
            _deny(base / "overcount", base / "overcount.events", base / "overcount.marker", 33)  # fmt: skip
            _deny(base / "empty", base / "empty.events", base / "empty.marker", 0)
            inventory = {path: path.read_bytes() for path in logs}
            for stale in (runs[0], runs[0].parent):
                os.utime(stale, ns=(1, 1))
            _deny(scratch, base / "full.events", base / "full.marker")
            assert {path: path.read_bytes() for path in logs} == inventory
            concurrent, outer_events, inner_events = base / "concurrent", base / "outer.events", base / "inner.events"  # fmt: skip
            outer_marker, inner_marker, nested_receipt = base / "outer.marker", base / "inner.marker", base / "nested.receipt"  # fmt: skip
            nested = _command("outer", 0, concurrent, outer_events, outer_marker, NESTED, CI_ROOT, base, concurrent, inner_events, inner_marker, nested_receipt, status=31)  # fmt: skip
            controller_receipt = base / "controller.receipt"
            process = multiprocessing.get_context("fork").Process(target=_session, args=(_plan(nested), concurrent, outer_events, controller_receipt))  # fmt: skip
            (process.start(), _reap(process, 60))
            terminal, outer_output = controller_receipt.read_text().split("\0", 1)
            inner_terminal, inner_output = nested_receipt.read_text().split("\0", 1)
            (outer_identity,) = map(Path, outer_events.read_text().splitlines())
            (inner_identity,) = map(Path, inner_events.read_text().splitlines())
            assert terminal.startswith("VerificationFailure\n") and inner_terminal.startswith("VerificationFailure\n") and all(stat.S_IMODE(path.stat().st_mode) == 0o444 for path in (controller_receipt, nested_receipt))  # fmt: skip
            assert (outer_marker.read_text().splitlines(), inner_marker.read_text().splitlines()) == (["0", str(outer_identity)], ["0", str(inner_identity)])  # fmt: skip
            assert str(outer_identity) in outer_output and str(inner_identity) not in outer_output and str(inner_identity) in inner_output  # fmt: skip
            overlap_logs = sorted((concurrent / "runs").rglob("*.log"))
            assert {path.parent.resolve() for path in overlap_logs} == {outer_identity, inner_identity}  # fmt: skip
            assert len({identity.parent for identity in (outer_identity, inner_identity)}) == 2  # fmt: skip
            all_markers.extend((outer_marker, inner_marker))
            slot_names = sorted(run.parent.name for run in runs)
            for occupied_name in slot_names:
                root, events, marker = base / f"extra-{occupied_name}", base / f"extra-{occupied_name}.events", base / f"extra-{occupied_name}.marker"  # fmt: skip
                authority = root / "runs"
                authority.mkdir(parents=True)
                occupied = authority / occupied_name
                before = _state(occupied, "extra")
                os.utime(occupied, ns=(1, 1))
                safe_name = next(name for name in slot_names if name != occupied_name)
                success_output, success_error = _invoke(_plan(_command("use safe sibling", 0, root, events, marker)), root, events)  # fmt: skip
                assert success_error is None, success_output
                (identity,) = map(Path, events.read_text().splitlines())
                assert marker.read_text().splitlines() == ["0", str(identity)] and identity.parent == authority / safe_name  # fmt: skip
                assert not identity.exists() and not identity.parent.exists() and _snapshot(occupied) == before  # fmt: skip
                assert set(authority.iterdir()) == {occupied}
                all_markers.append(marker)
            assert len(all_markers) == 68 and all(marker.exists() for marker in all_markers)  # fmt: skip
            structural = ("symlink", "file", "fifo")
            for name, kind in itertools.product(slot_names, structural):
                root = base / f"hostile-{name}-{kind}"
                (root / "runs").mkdir(parents=True)
                hostile = root / "runs" / name
                before = _state(hostile, kind)
                _deny(root, base / f"{root.name}.events", base / f"{root.name}.marker")
                assert _snapshot(hostile) == before
            (authority_root := base / "authority-alias").mkdir()
            authority_before = _state(authority_root / "runs", "symlink")
            _deny(authority_root, base / "authority.events", base / "authority.marker")
            assert _snapshot(authority_root / "runs") == authority_before
            alias_target, alias = base / "alias-target", base / "scratch-alias"
            (alias_target.mkdir(), (alias_target / "frozen").write_bytes(b"alias"), alias.symlink_to(alias_target, target_is_directory=True))
            alias_before = _snapshot(alias)
            _deny(alias, base / "alias.events", base / "alias.marker")
            assert _snapshot(alias) == alias_before
            states = ("extra", *structural)
            for serial, kinds in enumerate(itertools.product(states, repeat=2)):
                root = base / f"no-safe-{serial}"
                (root / "runs").mkdir(parents=True)
                paths = [root / "runs" / name for name in slot_names]
                before = tuple(_state(path, kind) for path, kind in zip(paths, kinds, strict=True))  # fmt: skip
                _deny(root, base / f"no-safe-{serial}.events", base / f"no-safe-{serial}.marker")  # fmt: skip
                assert tuple(map(_snapshot, paths)) == before
            assert not (base / "overcount" / "runs").exists() and not (base / "empty" / "runs").exists()  # fmt: skip
            assert sum(path.stat().st_size for path in logs + overlap_logs) < 1024 * 1024  # fmt: skip
