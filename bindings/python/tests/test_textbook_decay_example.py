"""Focused product checks for the executable textbook decay listing."""

from __future__ import annotations

import math
import runpy
import shutil
import subprocess
import sys
from pathlib import Path


PROGRAM = Path(__file__).resolve().parents[3] / "examples/python/textbook_decay.py"
MODEL = PROGRAM.parents[1] / "decay.eqi"


def test_textbook_decay_runs_from_the_installed_public_surface() -> None:
    namespace = runpy.run_path(PROGRAM.as_posix())
    samples = namespace["solve"](MODEL)

    assert tuple(time_s for time_s, _ in samples) == (0.25, 0.5, 1.0)
    for time_s, computed in samples:
        assert abs(computed - math.exp(-time_s)) <= 2.0e-8


def test_textbook_decay_listing_has_no_repository_private_imports() -> None:
    source = PROGRAM.read_text(encoding="utf-8")
    compile(source, PROGRAM.as_posix(), "exec")
    assert "from examples" not in source
    assert "verify" not in source
    assert "eqiora._" not in source
    assert 'model decay() {' not in source


def test_downloaded_decay_runs_and_edits_without_a_checkout(tmp_path: Path) -> None:
    shutil.copyfile(PROGRAM, tmp_path / "run.py")
    model = tmp_path / "decay.eqi"
    source = MODEL.read_text(encoding="utf-8")
    for rate in (1, 2):
        model.write_text(source.replace("rate: 1 / s = 1;", f"rate: 1 / s = {rate};"), encoding="utf-8")
        result = subprocess.run(
            [sys.executable, "-I", "run.py"], cwd=tmp_path,
            capture_output=True, text=True, check=True,
        )
        lines = result.stdout.splitlines()
        assert len(lines) == 3
        for line, time_s in zip(lines, (0.25, 0.5, 1.0), strict=True):
            assert line.startswith(f"t={time_s:.2f}, x=")
            assert abs(float(line.split("x=")[1]) - math.exp(-rate * time_s)) <= 2.0e-8


def test_get_started_source_diagnostic_names_the_unresolved_symbol(tmp_path: Path) -> None:
    import eqiora

    source = MODEL.read_text(encoding="utf-8").replace("rate * x", "missing_rate * x")
    path = tmp_path / "invalid.eqi"
    path.write_text(source, encoding="utf-8")
    try:
        eqiora.compile(path=path)
    except eqiora.ValidationError as error:
        assert any(diagnostic.code == "EQ0603" for diagnostic in error.diagnostics)
        assert "unresolved expression symbol `missing_rate`" in str(error)
    else:
        raise AssertionError("unknown symbol accepted")
