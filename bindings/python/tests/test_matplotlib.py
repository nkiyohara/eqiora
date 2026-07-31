from __future__ import annotations

import gc
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


def accepted_structural_result() -> eqiora.solid.MixedBoundaryElasticityResult:
    source = (
        files(eqiora)
        .joinpath("examples", "mixed-boundary-elasticity.eqi")
        .read_text(encoding="utf-8")
    )
    model = eqiora.compatibility.compile_exact(
        source,
        filename="mixed-boundary-elasticity.eqi",
        codec=eqiora.compatibility.ExactModelCodec.V4,
    )
    return eqiora.solid.solve_mixed_boundary_elasticity(model)


@pytest.fixture(scope="module")
def result() -> eqiora.fluid.CircularHoleSteadyStokesResult:
    return accepted_result()


@pytest.fixture(scope="module")
def structural_result() -> eqiora.solid.MixedBoundaryElasticityResult:
    return accepted_structural_result()


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
    field_position = figure.axes[0].get_position()
    colorbar_position = figure.axes[1].get_position()
    assert colorbar_position.y0 == pytest.approx(field_position.y0)
    assert colorbar_position.y1 == pytest.approx(field_position.y1)

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


def test_caller_owned_figure_keeps_its_render_data_alive() -> None:
    result = accepted_result()
    figure = eqplot.plot_pressure(result)

    del result
    gc.collect()

    encoded = io.BytesIO()
    figure.savefig(encoded, format="png")
    assert encoded.getvalue().startswith(b"\x89PNG\r\n\x1a\n")


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


def test_displacement_plot_rejects_foreign_inputs_before_rendering(
    result: eqiora.fluid.CircularHoleSteadyStokesResult,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    rendered = False

    def reject_render(*args: Any, **kwargs: Any) -> Any:
        nonlocal rendered
        rendered = True
        raise AssertionError("foreign input reached Matplotlib")

    monkeypatch.setattr(Axes, "add_collection", reject_render)
    for foreign in (object(), result):
        with pytest.raises(
            TypeError,
            match="MixedBoundaryElasticityResult",
        ):
            eqplot.plot_displacement(foreign)  # type: ignore[arg-type]
    assert not rendered


@pytest.mark.parametrize("scale", [0.0, 2.0])
def test_displacement_plot_preserves_canonical_edges_and_explicit_scale(
    structural_result: eqiora.solid.MixedBoundaryElasticityResult,
    scale: float,
) -> None:
    import matplotlib.pyplot as pyplot

    coordinates = structural_result.coordinates.copy()
    displacement = structural_result.displacement.copy()
    cells = structural_result.cells.copy()
    edges = sorted(
        {
            tuple(sorted((int(cell[first]), int(cell[second]))))
            for cell in cells
            for first, second in ((0, 1), (1, 3), (3, 2), (2, 0))
        }
    )
    assert len(edges) == 544
    expected_original = coordinates[edges]
    expected_deformed = (coordinates + scale * displacement)[edges]

    registered_figures = pyplot.get_fignums()
    figure = eqplot.plot_displacement(structural_result, scale=scale)
    assert pyplot.get_fignums() == registered_figures
    assert len(figure.axes) == 1
    axes = figure.axes[0]
    original, deformed = axes.collections
    np.testing.assert_array_equal(original.get_segments(), expected_original)
    np.testing.assert_array_equal(deformed.get_segments(), expected_deformed)
    assert original.get_label() == "Original mesh"
    assert deformed.get_label() == f"Displaced mesh (scale = {scale:g})"
    assert f"scale {scale:g}" in axes.get_title()
    assert axes.get_xlabel() == "x [m]"
    assert axes.get_ylabel() == "y [m]"
    assert axes.get_aspect() == 1.0
    assert axes.get_xlim()[0] <= min(
        coordinates[:, 0].min(),
        expected_deformed[..., 0].min(),
    )
    assert axes.get_xlim()[1] >= max(
        coordinates[:, 0].max(),
        expected_deformed[..., 0].max(),
    )
    assert axes.get_ylim()[0] <= min(
        coordinates[:, 1].min(),
        expected_deformed[..., 1].min(),
    )
    assert axes.get_ylim()[1] >= max(
        coordinates[:, 1].max(),
        expected_deformed[..., 1].max(),
    )
    assert not structural_result.coordinates.flags.writeable
    assert not structural_result.cells.flags.writeable
    assert not structural_result.displacement.flags.writeable


def test_structural_figure_is_headless_caller_owned_and_nonblank(
    tmp_path: Path,
) -> None:
    result = accepted_structural_result()
    figure = eqplot.plot_displacement(result, scale=1.0)
    del result
    gc.collect()

    encoded = io.BytesIO()
    figure.savefig(encoded, format="png")
    payload = encoded.getvalue()
    destination = tmp_path / "displacement.png"
    figure.savefig(destination)
    assert payload.startswith(b"\x89PNG\r\n\x1a\n")
    assert destination.read_bytes().startswith(b"\x89PNG\r\n\x1a\n")
    encoded.seek(0)
    pixels = image.imread(encoded, format="png")
    assert np.ptp(pixels[..., :3]) > 0.0


@pytest.mark.parametrize("scale", [-1.0, float("inf"), float("nan")])
def test_displacement_plot_rejects_invalid_scale_before_rendering(
    structural_result: eqiora.solid.MixedBoundaryElasticityResult,
    scale: float,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    rendered = False

    def reject_render(*args: Any, **kwargs: Any) -> Any:
        nonlocal rendered
        rendered = True
        raise AssertionError("invalid scale reached Matplotlib")

    monkeypatch.setattr(Axes, "add_collection", reject_render)
    with pytest.raises(ValueError, match="finite and nonnegative"):
        eqplot.plot_displacement(structural_result, scale=scale)
    assert not rendered
