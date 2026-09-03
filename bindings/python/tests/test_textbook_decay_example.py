"""Focused product checks for the executable textbook decay listing."""

from __future__ import annotations

import math
import runpy
from pathlib import Path


PROGRAM = Path(__file__).resolve().parents[3] / "examples/python/textbook_decay.py"


def test_textbook_decay_runs_from_the_installed_public_surface() -> None:
    namespace = runpy.run_path(PROGRAM.as_posix())
    samples = namespace["solve"]()

    assert tuple(sample.time_s for sample in samples) == (0.25, 0.5, 1.0)
    for sample in samples:
        assert sample.closed_form == math.exp(-sample.time_s)
        assert sample.absolute_error <= 2.0e-8


def test_textbook_decay_listing_has_no_repository_private_imports() -> None:
    source = PROGRAM.read_text(encoding="utf-8")
    compile(source, PROGRAM.as_posix(), "exec")
    assert "from examples" not in source
    assert "verify" not in source
    assert "eqiora._" not in source
