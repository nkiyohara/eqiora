from __future__ import annotations

import io
import os
import struct
import sys
from importlib.resources import files
from pathlib import Path
from typing import Any

import numpy as np
import pytest

import eqiora


assert "matplotlib" not in sys.modules
matplotlib = pytest.importorskip("matplotlib")
matplotlib.use("Agg", force=True)

import matplotlib.image as image  # noqa: E402
from matplotlib.axes import Axes  # noqa: E402

import eqiora.matplotlib as eqplot  # noqa: E402


assert "matplotlib.pyplot" not in sys.modules
EXPECTED_MATPLOTLIB_VERSION = os.environ.get("EQIORA_TEST_MATPLOTLIB_VERSION")
if EXPECTED_MATPLOTLIB_VERSION is not None:
    assert matplotlib.__version__ == EXPECTED_MATPLOTLIB_VERSION


def accepted_result() -> eqiora.fluid.CircularHoleSteadyStokesResult:
    geometry = eqiora.geometry.RectangleWithCircularHole(
        bounds=((0.0, 2.2), (0.0, 0.41)),
        circle_center=(0.2, 0.2),
        circle_radius=0.05,
        tolerance=1e-12,
        region="fluid",
        x_lower="inlet",
        x_upper="outlet",
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )
    mesh = eqiora.meshing.circular_hole_chordal(
        geometry,
        max_boundary_error=1e-4,
        required_minimum_mean_ratio=1e-5,
        max_segments=50,
    )
    model_v7 = (
        files(eqiora)
        .joinpath("examples", "steady-flow-past-cylinder.model-v7.json")
        .read_bytes()
    )
    return eqiora.fluid.solve_exact_cylinder_stokes(
        model_v7=model_v7,
        geometry=geometry,
        mesh=mesh,
    )


@pytest.fixture(scope="module")
def result() -> eqiora.fluid.CircularHoleSteadyStokesResult:
    return accepted_result()


def test_plot_passes_the_accepted_p1_field_unchanged_to_matplotlib(
    result: eqiora.fluid.CircularHoleSteadyStokesResult,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import matplotlib.pyplot as pyplot

    expected_coordinates = result.coordinates.copy()
    expected_triangles = result.triangles.copy()
    expected_pressure = result.pressure.numpy(copy=True)
    identity = (
        result.model_digest,
        result.mesh_digest,
        result.snapshot_digest,
        result.run_digest,
    )
    observed: dict[str, Any] = {}
    original = Axes.tripcolor

    def capture(axes: Axes, *args: Any, **kwargs: Any) -> Any:
        observed["x"] = np.asarray(args[0]).copy()
        observed["y"] = np.asarray(args[1]).copy()
        observed["values"] = np.asarray(args[2]).copy()
        observed["triangles"] = np.asarray(kwargs["triangles"]).copy()
        artist = original(axes, *args, **kwargs)
        observed["artist"] = artist
        return artist

    monkeypatch.setattr(Axes, "tripcolor", capture)
    registered_figures = pyplot.get_fignums()
    figure = eqplot.plot_pressure(result)
    axes = figure.axes[0]

    assert pyplot.get_fignums() == registered_figures
    np.testing.assert_array_equal(observed["x"], expected_coordinates[:, 0])
    np.testing.assert_array_equal(observed["y"], expected_coordinates[:, 1])
    np.testing.assert_array_equal(observed["triangles"], expected_triangles)
    np.testing.assert_array_equal(observed["values"], expected_pressure)
    assert expected_coordinates.shape == (104, 2)
    assert expected_triangles.shape == (104, 3)
    assert expected_pressure.shape == (104,)
    assert np.isfinite(expected_coordinates).all()
    assert np.isfinite(expected_pressure).all()
    assert expected_triangles.max() < expected_coordinates.shape[0]
    assert observed["artist"].get_clim() == (
        result.pressure_minimum,
        result.pressure_maximum,
    )
    assert axes.get_xlabel() == "x [m]"
    assert axes.get_ylabel() == "y [m]"
    assert figure.axes[1].get_ylabel() == "Pressure [Pa]"
    assert axes.get_aspect() == 1.0
    assert axes.get_xlim() == result.bounds[0]
    assert axes.get_ylim() == result.bounds[1]

    assert identity == (
        result.model_digest,
        result.mesh_digest,
        result.snapshot_digest,
        result.run_digest,
    )
    np.testing.assert_array_equal(result.coordinates, expected_coordinates)
    np.testing.assert_array_equal(result.triangles, expected_triangles)
    np.testing.assert_array_equal(
        result.pressure.numpy(copy=False),
        expected_pressure,
    )
    assert not result.coordinates.flags.writeable
    assert not result.triangles.flags.writeable


def test_headless_figure_is_caller_saveable_and_nonblank(
    result: eqiora.fluid.CircularHoleSteadyStokesResult,
    tmp_path: Path,
) -> None:
    figure = eqplot.plot_pressure(result)
    encoded = io.BytesIO()
    figure.savefig(encoded, format="png")
    payload = encoded.getvalue()
    destination = tmp_path / "pressure.png"
    figure.savefig(destination)

    assert payload.startswith(b"\x89PNG\r\n\x1a\n")
    width, height = struct.unpack(">II", payload[16:24])
    assert width > 0
    assert height > 0
    assert (width, height) == figure.canvas.get_width_height()
    assert destination.read_bytes().startswith(b"\x89PNG\r\n\x1a\n")

    encoded.seek(0)
    pixels = image.imread(encoded, format="png")
    assert pixels.shape[:2] == (height, width)
    assert pixels.shape[2] in (3, 4)
    assert np.ptp(pixels[..., :3]) > 0.0
    if pixels.shape[2] == 4:
        assert np.any(pixels[..., 3] > 0.0)

    high_resolution = io.BytesIO()
    figure.savefig(high_resolution, format="png", dpi=180)
    high_resolution_payload = high_resolution.getvalue()
    high_resolution_width, high_resolution_height = struct.unpack(
        ">II",
        high_resolution_payload[16:24],
    )
    assert high_resolution_width > width
    assert high_resolution_height > height


def test_foreign_inputs_fail_before_rendering(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    rendered = False

    def reject_render(*args: Any, **kwargs: Any) -> Any:
        nonlocal rendered
        rendered = True
        raise AssertionError("foreign input reached Matplotlib")

    monkeypatch.setattr(Axes, "tripcolor", reject_render)
    with pytest.raises(
        TypeError,
        match="CircularHoleSteadyStokesResult",
    ):
        eqplot.plot_pressure(object())  # type: ignore[arg-type]
    assert not rendered
