from __future__ import annotations

import gc
import inspect
import io
import json
import os
import struct
import sys
import warnings
from importlib.resources import files
from pathlib import Path
from typing import Any

import numpy as np
import numpy.typing as npt
import pytest

import eqiora


assert "matplotlib" not in sys.modules
matplotlib = pytest.importorskip("matplotlib")
matplotlib.use("Agg", force=True)

import matplotlib.image as image  # noqa: E402
from matplotlib.axes import Axes  # noqa: E402
from matplotlib.collections import LineCollection  # noqa: E402

import eqiora.matplotlib as eqplot  # noqa: E402
from test_fixed_reference_fsi import (  # noqa: E402
    PARAMETERS as FSI_PARAMETERS,
    admitted as admitted_fsi,
    geometry_and_mesh as fsi_geometry_and_mesh,
    initial as initial_fsi,
)


assert "matplotlib.pyplot" not in sys.modules
EXPECTED_MATPLOTLIB_VERSION = os.environ.get("EQIORA_TEST_MATPLOTLIB_VERSION")
if EXPECTED_MATPLOTLIB_VERSION is not None:
    assert matplotlib.__version__ == EXPECTED_MATPLOTLIB_VERSION

# Frozen structural facts of the admitted fixed-mesh affine-triangle 2D FSI
# trajectory, wired verbatim from registered evidence
# (interfaces.python-fixed-mesh-trajectory, interfaces.python-fixed-reference-
# fsi-demo). No solver value, extremum, or tolerance is frozen here.
ACCEPTED_CELLS = (
    (0, 3, 4),
    (0, 4, 1),
    (1, 4, 5),
    (1, 5, 2),
    (3, 6, 7),
    (3, 7, 4),
    (4, 7, 8),
    (4, 8, 5),
)
ACCEPTED_VERTEX_SUPPORT = {
    "fluid_pressure": (0, 1, 2, 3, 4, 5),
    "solid_displacement": (3, 4, 5, 6, 7, 8),
}
ACCEPTED_SUPPORT_CELLS = {
    "fluid_pressure": (0, 1, 2, 3),
    "solid_displacement": (4, 5, 6, 7),
}
ACCEPTED_SUPPORT_EDGES = {
    "solid_displacement": (
        (3, 4),
        (3, 6),
        (3, 7),
        (4, 5),
        (4, 7),
        (4, 8),
        (5, 8),
        (6, 7),
        (7, 8),
    ),
}
ACCEPTED_STEPS = (1, 2)
CONTRACT_VOCABULARY = r"value shape|frame|dimension|association"


def accepted_result() -> eqiora.Result:
    graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
    ).circular_through_cut(
        center=(0.2, 0.2),
        radius=0.05,
        boolean_tolerance=1e-10,
    )
    geometry = graph.planar_circular_section(
        classification_tolerance=1e-12,
        region="fluid",
        x_lower="inlet",
        x_upper="outlet",
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )
    request = eqiora.meshing.MeshRequest(
        maximum_boundary_error=1e-4,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=50,
    )
    plan = eqiora.meshing.resolve(geometry, request)
    mesh = eqiora.meshing.generate(geometry, plan=plan)
    model_bytes = (
        files(eqiora)
        .joinpath("examples", "steady-flow-past-cylinder.model.json")
        .read_bytes()
    )
    model = eqiora.replay(model_bytes)
    intent = eqiora.fluid.SteadyStokes(
        length_scale_m=0.41,
        velocity_scale_m_per_s=0.3,
        pressure_scale_pa=0.001 * 0.3 / 0.41,
        relative_tolerance=1e-6,
        absolute_tolerance=1e-13,
        maximum_iterations=10_000,
    )
    plan = eqiora.fluid.resolve(model, intent, mesh=mesh)
    return eqiora.run(model, plan=plan)


def parameter_value_variant(encoded: bytes) -> bytes:
    """Change one Parameter value while preserving every semantic Field ULID."""

    document = json.loads(encoded)
    parameter = next(
        node for node in document["nodes"] if node["id"]["kind"] == "parameter"
    )
    identifier = parameter["id"]["ulid"]
    original = parameter["definition"]["value"]["value"]
    replacement = original + 1.0
    parameter["definition"]["value"]["value"] = replacement
    value = next(
        item
        for item in document["values"]
        if item["target"] == {"kind": "parameter", "ulid": identifier}
    )
    assert value["value"]["value"] == original
    value["value"]["value"] = replacement
    return json.dumps(document, separators=(",", ":")).encode()


def accepted_reference_result() -> tuple[eqiora.Model, eqiora.Result]:
    state = eqiora.Field("x", initial=1.0)
    model = eqiora.Model.define(
        "hold",
        state,
        eqiora.Relation("hold", residual=eqiora.derivative(state)),
    )
    return model, eqiora.run(model, end_time=0.1, max_step=0.1)


def accepted_structural_model() -> eqiora.Model:
    source = (
        files(eqiora)
        .joinpath("examples", "mixed-boundary-elasticity.eqi")
        .read_text(encoding="utf-8")
    )
    return eqiora.compile(
        source=source,
        filename="mixed-boundary-elasticity.eqi",
    )


def accepted_structural_result() -> tuple[eqiora.Model, eqiora.Result]:
    """Resolve and run the accepted structural Plan through the ordinary path."""

    model = accepted_structural_model()
    intent = eqiora.solid.LinearElasticity(
        cells_per_axis=16,
        relative_tolerance=1e-12,
        absolute_tolerance=1e-14,
        maximum_iterations=10_000,
    )
    plan = eqiora.solid.resolve(model, intent)
    return model, eqiora.run(model, plan=plan)


def foreign_fsi_model() -> tuple[eqiora.Model, eqiora.Plan]:
    """Compile a structurally equivalent Model with a different exact digest.

    Independent compilation allocates fresh semantic field ids, so this fixture
    shows that structural equivalence alone never admits a `FieldRef`.
    """

    geometry, mesh = fsi_geometry_and_mesh()
    model = eqiora.compile(
        path=files(eqiora).joinpath("examples", "fixed-reference-fsi.eqi"),
        geometry=geometry,
        component="FixedReferenceFsi2d",
        parameters={**FSI_PARAMETERS, "fluid_density": 4.0},
    )
    plan = eqiora.resolve(
        model,
        mesh=mesh,
        spatial=(
            eqiora.fem.MiniP1().at(model.domain("fluid")),
            eqiora.fem.P1().at(model.domain("solid")),
        ),
        temporal=eqiora.time.BackwardEuler(0.05),
        solve=eqiora.solve.Linear(
            relative_tolerance=1e-11,
            absolute_tolerance=1e-13,
            maximum_iterations=20_000,
        ),
    )
    return model, plan


def accepted_fsi_trajectory() -> tuple[
    eqiora.Model,
    eqiora.Plan,
    eqiora.Result,
    eqiora.trajectory.Trajectory,
]:
    model, mesh, plan = admitted_fsi()
    result = eqiora.run(
        plan,
        state=initial_fsi(model, mesh, plan),
        steps=2,
        output_steps=(1, 2),
    )
    return model, plan, result, result.trajectory


def fsi_field(plan: eqiora.Plan, name: str) -> eqiora.FieldRef:
    return plan.fields[
        {
            "fluid_velocity": 0,
            "fluid_pressure": 1,
            "solid_velocity": 2,
            "solid_displacement": 3,
        }[name]
    ]


def expanded_vertex_values(
    trajectory: eqiora.trajectory.Trajectory,
    snapshot: eqiora.trajectory.FieldSnapshot,
) -> npt.NDArray[np.float64]:
    values = snapshot.values("vertex")
    support = snapshot.support_indices("vertex")
    expanded = np.zeros((len(trajectory.coordinates), *values.shape[1:]))
    expanded[support] = values
    return expanded


@pytest.fixture(scope="module")
def result() -> eqiora.Result:
    return accepted_result()


@pytest.fixture(scope="module")
def structural() -> tuple[eqiora.Model, eqiora.Result]:
    return accepted_structural_result()


@pytest.fixture(scope="module")
def fsi() -> tuple[
    eqiora.Model,
    eqiora.Plan,
    eqiora.Result,
    eqiora.trajectory.Trajectory,
]:
    return accepted_fsi_trajectory()


def test_plot_passes_the_accepted_p1_field_unchanged_to_matplotlib(
    result: eqiora.Result,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import matplotlib.pyplot as pyplot

    snapshot = result.snapshots[0]
    mesh = result.mesh(snapshot.field)
    evidence = eqiora.fluid.steady_stokes_evidence(result)
    expected_coordinates = mesh.coordinates.copy()
    expected_triangles = mesh.cells.copy()
    expected_pressure = snapshot.values("vertex").copy()
    expected_support = snapshot.support_indices("vertex").copy()
    bytes_before = (
        mesh.canonical_bytes,
        result.run_manifest().to_json(),
    )
    identity = (
        result.model_id,
        result.model_digest,
        result.plan_key,
        mesh.digest,
        snapshot.field,
        snapshot.digest,
        snapshot.block_digests,
        snapshot.mesh_digest,
        result.run_manifest().digest,
        evidence.run_digest,
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
    figure = eqplot.plot_scalar_field(result, field=snapshot.field)
    axes = figure.axes[0]

    assert pyplot.get_fignums() == registered_figures
    np.testing.assert_array_equal(observed["x"], expected_coordinates[:, 0])
    np.testing.assert_array_equal(observed["y"], expected_coordinates[:, 1])
    np.testing.assert_array_equal(observed["triangles"], expected_triangles)
    np.testing.assert_array_equal(observed["values"], expected_pressure)
    np.testing.assert_array_equal(expected_support, np.arange(662, dtype=np.uint32))
    assert expected_coordinates.shape == (662, 2)
    assert expected_triangles.shape == (1210, 3)
    assert expected_pressure.shape == (662,)
    assert np.isfinite(expected_coordinates).all()
    assert np.isfinite(expected_pressure).all()
    assert expected_triangles.max() < expected_coordinates.shape[0]
    assert observed["artist"].get_clim() == (
        evidence.pressure_minimum,
        evidence.pressure_maximum,
    )
    assert axes.get_xlabel() == "x [m]"
    assert axes.get_ylabel() == "y [m]"
    assert figure.axes[1].get_ylabel() == "Pressure [Pa]"
    assert axes.get_aspect() == 1.0
    assert axes.get_xlim() == evidence.exact_bounds[0]
    assert axes.get_ylim() == evidence.exact_bounds[1]

    assert identity == (
        result.model_id,
        result.model_digest,
        result.plan_key,
        mesh.digest,
        snapshot.field,
        snapshot.digest,
        snapshot.block_digests,
        snapshot.mesh_digest,
        result.run_manifest().digest,
        evidence.run_digest,
    )
    assert bytes_before == (
        mesh.canonical_bytes,
        result.run_manifest().to_json(),
    )
    assert result.snapshots[0] is snapshot
    assert result.field(snapshot.field) is snapshot
    assert result.mesh(snapshot.field) is mesh
    np.testing.assert_array_equal(mesh.coordinates, expected_coordinates)
    np.testing.assert_array_equal(mesh.cells, expected_triangles)
    np.testing.assert_array_equal(
        snapshot.values("vertex"),
        expected_pressure,
    )
    np.testing.assert_array_equal(
        snapshot.support_indices("vertex"),
        expected_support,
    )
    assert not mesh.coordinates.flags.writeable
    assert not mesh.cells.flags.writeable
    assert not snapshot.values("vertex").flags.writeable
    assert not snapshot.support_indices("vertex").flags.writeable


def test_headless_figure_is_caller_saveable_and_nonblank(
    result: eqiora.Result,
    tmp_path: Path,
) -> None:
    figure = eqplot.plot_scalar_field(result, field=result.snapshots[0].field)
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
    figure = eqplot.plot_scalar_field(result, field=result.snapshots[0].field)

    del result
    gc.collect()

    encoded = io.BytesIO()
    figure.savefig(encoded, format="png")
    assert encoded.getvalue().startswith(b"\x89PNG\r\n\x1a\n")


def test_static_scalar_still_rejects_wrong_call_shape_and_identity_before_rendering(
    result: eqiora.Result,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    snapshot = result.snapshots[0]
    field = snapshot.field
    model_bytes = (
        files(eqiora)
        .joinpath("examples", "steady-flow-past-cylinder.model.json")
        .read_bytes()
    )
    current = eqiora.replay(model_bytes)
    absent_id = next(
        identifier for identifier in current.field_ids if identifier != field.id
    )
    absent = current.field(absent_id)
    foreign_artifact = eqiora.replay(parameter_value_variant(model_bytes))
    same_id_foreign_model = foreign_artifact.field(field.id)
    reference_model, reference = accepted_reference_result()

    assert same_id_foreign_model.id == field.id
    assert same_id_foreign_model.model_digest != field.model_digest
    forbid_rendering(monkeypatch)
    with pytest.raises(TypeError, match="Result|Trajectory"):
        eqplot.plot_scalar_field(object(), field=field)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        eqplot.plot_scalar_field(result, step=0, field=field)
    with pytest.raises(ValueError, match="different exact Model"):
        eqplot.plot_scalar_field(result, field=same_id_foreign_model)
    with pytest.raises(KeyError):
        eqplot.plot_scalar_field(result, field=absent)
    with pytest.raises(eqiora.CapabilityError):
        eqplot.plot_scalar_field(
            reference,
            field=reference_model.field("x"),
        )

    assert result.field(field) is snapshot
    assert result.mesh(field).digest == snapshot.mesh_digest


def quadrilateral_edges(cells: np.ndarray) -> list[tuple[int, int]]:
    """Canonical unique undirected edges of the Z-ordered Q1 connectivity."""

    return sorted(
        {
            tuple(sorted((int(cell[first]), int(cell[second]))))
            for cell in cells
            for first, second in ((0, 1), (1, 3), (3, 2), (2, 0))
        }
    )


def test_deformed_still_rejects_foreign_structural_inputs_before_rendering(
    structural: tuple[eqiora.Model, eqiora.Result],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    model, result = structural
    displacement = model.field("displacement")
    reference_model, reference = accepted_reference_result()
    forbid_rendering(monkeypatch)

    for foreign in (object(), result.run_manifest()):
        with pytest.raises(TypeError, match="Result|Trajectory"):
            eqplot.plot_deformed_field(foreign, field=displacement)
    # The general adapter owns exactly two arms: a static Result without step
    # and a Trajectory with step. Neither borrows the other's call shape.
    with pytest.raises(TypeError):
        eqplot.plot_deformed_field(result, step=1, field=displacement)
    with pytest.raises(ValueError, match="different exact Model"):
        eqplot.plot_deformed_field(
            result,
            field=accepted_structural_model().field("displacement"),
        )
    with pytest.raises(KeyError):
        eqplot.plot_deformed_field(result, field=model.field("load_potential"))
    with pytest.raises(eqiora.CapabilityError):
        eqplot.plot_deformed_field(reference, field=reference_model.field("x"))


def test_deformed_still_keeps_trajectory_topology_triangle_only(
    structural: tuple[eqiora.Model, eqiora.Result],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """A future quad-backed Trajectory cannot silently enter this adapter arm."""

    model, result = structural
    field = model.field("displacement")
    snapshot = result.field(field)
    mesh = result.mesh(field)

    class QuadState:
        step = 1
        time_s = 0.0

        @staticmethod
        def field(selected: eqiora.FieldRef) -> Any:
            assert selected == field
            return snapshot

    class QuadTrajectory:
        dimension = mesh.dimension
        coordinates = mesh.coordinates
        cells = mesh.cells

        @staticmethod
        def state(step: int) -> QuadState:
            assert step == 1
            return QuadState()

    # Native Trajectory construction currently admits only affine triangles.
    # Replacing only the adapter's runtime type guard lets this regression test
    # exercise a prospective quad-backed value without inventing a public
    # constructor or modifying exact trajectory evidence.
    monkeypatch.setattr(eqplot, "Trajectory", QuadTrajectory)
    assert QuadTrajectory.cells.shape[1] == 4
    forbid_rendering(monkeypatch)
    with pytest.raises(ValueError, match="affine triangle topology"):
        eqplot.plot_deformed_field(QuadTrajectory(), step=1, field=field)


@pytest.mark.parametrize("scale", [0.0, 2.0])
def test_deformed_still_preserves_canonical_q1_edges_and_explicit_scale(
    structural: tuple[eqiora.Model, eqiora.Result],
    scale: float,
) -> None:
    import matplotlib.pyplot as pyplot

    model, result = structural
    field = model.field("displacement")
    snapshot = result.field(field)
    mesh = result.mesh(field)
    coordinates = mesh.coordinates.copy()
    displacement = snapshot.values("vertex").copy()
    edges = quadrilateral_edges(mesh.cells)

    assert snapshot.value_shape == (2,)
    assert snapshot.frame == "spatial-cartesian"
    assert snapshot.dimension == (0, 1, 0, 0, 0, 0, 0)
    assert snapshot.associations == ("vertex",)
    assert mesh.cells.shape == (256, 4)
    assert len(edges) == 544
    expected_original = coordinates[edges]
    expected_deformed = (coordinates + scale * displacement)[edges]

    registered_figures = pyplot.get_fignums()
    figure = eqplot.plot_deformed_field(result, field=field, scale=scale)
    assert pyplot.get_fignums() == registered_figures
    assert len(figure.axes) == 1
    axes = figure.axes[0]
    original, deformed = wireframes(figure)
    np.testing.assert_array_equal(original, expected_original)
    np.testing.assert_array_equal(deformed, expected_deformed)
    labels = [artist.get_label() for artist in axes.collections]
    assert any(f"{scale:g}" in text for text in (axes.get_title(), *labels))
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
    assert not mesh.coordinates.flags.writeable
    assert not mesh.cells.flags.writeable
    assert not snapshot.values("vertex").flags.writeable


def test_structural_figure_is_headless_caller_owned_and_nonblank(
    tmp_path: Path,
) -> None:
    model, result = accepted_structural_result()
    figure = eqplot.plot_deformed_field(
        result,
        field=model.field("displacement"),
        scale=1.0,
    )
    del model, result
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
def test_deformed_still_rejects_invalid_structural_scale_before_rendering(
    structural: tuple[eqiora.Model, eqiora.Result],
    scale: float,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    model, result = structural
    forbid_rendering(monkeypatch)
    with pytest.raises(ValueError, match="finite and nonnegative"):
        eqplot.plot_deformed_field(
            result,
            field=model.field("displacement"),
            scale=scale,
        )


def test_predecessor_displacement_still_only_delegates_with_a_deprecation(
    structural: tuple[eqiora.Model, eqiora.Result],
    result: eqiora.Result,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    model, structural_result = structural
    field = model.field("displacement")
    converged = eqplot.plot_deformed_field(
        structural_result,
        field=field,
        scale=2.0,
    )

    with pytest.warns(DeprecationWarning):
        shim_result = eqiora.solid.solve_mixed_boundary_elasticity(
            accepted_structural_model()
        )
    with pytest.warns(DeprecationWarning, match="plot_deformed_field"):
        delegated = eqplot.plot_displacement(shim_result, scale=2.0)

    for converged_segments, delegated_segments in zip(
        wireframes(converged),
        wireframes(delegated),
        strict=True,
    ):
        np.testing.assert_array_equal(converged_segments, delegated_segments)

    # The retained result type name resolves to common Result, so the shim must
    # discriminate with the closed structural-evidence arm rather than an
    # isinstance check that would also admit this steady-Stokes Result.
    forbid_rendering(monkeypatch)
    with pytest.warns(DeprecationWarning, match="plot_deformed_field"):
        with pytest.raises(eqiora.CapabilityError):
            eqplot.plot_displacement(result, scale=2.0)


def test_predecessor_displacement_wrong_type_rejects_before_deprecation() -> None:
    with warnings.catch_warnings(record=True) as caught:
        warnings.simplefilter("always")
        with pytest.raises(TypeError, match="eqiora.Result"):
            eqplot.plot_displacement(object())  # type: ignore[arg-type]
    assert caught == []


# --------------------------------------------------------------------------
# interfaces.python-trajectory-field-stills
#
# Pre-committed oracle for the common trajectory stills, frozen before the
# adapters existed. Every relation below is owned by
# verify/interfaces/python-trajectory-field-stills/case.toml; an implementer
# wires it and returns a proof rather than relaxing it.
# --------------------------------------------------------------------------


def admitted_cells(support: tuple[int, ...]) -> tuple[int, ...]:
    """Cells whose complete vertex tuple lies in the accepted support."""

    inside = set(support)
    return tuple(
        index
        for index, cell in enumerate(ACCEPTED_CELLS)
        if all(vertex in inside for vertex in cell)
    )


def admitted_edges(cells: tuple[int, ...]) -> tuple[tuple[int, int], ...]:
    """Sorted unique undirected edges of the admitted affine triangles."""

    return tuple(
        sorted(
            {
                tuple(sorted((ACCEPTED_CELLS[cell][first], ACCEPTED_CELLS[cell][last])))
                for cell in cells
                for first, last in ((0, 1), (1, 2), (2, 0))
            }
        )
    )


def capture_tripcolor(monkeypatch: pytest.MonkeyPatch) -> dict[str, Any]:
    """Record the public triangular renderer inputs without suppressing it."""

    observed: dict[str, Any] = {}
    original = Axes.tripcolor

    def capture(axes: Axes, *args: Any, **kwargs: Any) -> Any:
        observed["positional"] = tuple(np.asarray(value).copy() for value in args)
        observed["triangles"] = (
            np.asarray(kwargs["triangles"]).copy() if "triangles" in kwargs else None
        )
        observed["shading"] = kwargs.get("shading")
        artist = original(axes, *args, **kwargs)
        observed["artist"] = artist
        return artist

    monkeypatch.setattr(Axes, "tripcolor", capture)
    return observed


def forbid_rendering(monkeypatch: pytest.MonkeyPatch) -> None:
    """Fail closed if a rejected still reaches a Figure or a renderer."""

    def reject(*args: Any, **kwargs: Any) -> Any:
        raise AssertionError("a rejected still reached Matplotlib")

    monkeypatch.setattr(eqplot, "Figure", reject)
    monkeypatch.setattr(Axes, "tripcolor", reject)
    monkeypatch.setattr(Axes, "add_collection", reject)


def wireframes(figure: Any) -> tuple[np.ndarray, np.ndarray]:
    """Extract the frozen reference and deformed wireframes from one Figure."""

    axes = figure.axes[0]
    assert len(axes.collections) == 2
    reference, deformed = axes.collections
    assert isinstance(reference, LineCollection)
    assert isinstance(deformed, LineCollection)
    return np.asarray(reference.get_segments()), np.asarray(deformed.get_segments())


def test_still_signatures_are_the_frozen_keyword_only_contract() -> None:
    scalar = inspect.signature(eqplot.plot_scalar_field).parameters
    deformed = inspect.signature(eqplot.plot_deformed_field).parameters

    assert list(scalar) == ["trajectory", "step", "field"]
    assert list(deformed) == ["trajectory", "step", "field", "scale"]
    assert scalar["trajectory"].kind is inspect.Parameter.POSITIONAL_ONLY
    assert deformed["trajectory"].kind is inspect.Parameter.POSITIONAL_ONLY
    for parameters in (scalar, deformed):
        for name in ("step", "field"):
            assert parameters[name].kind is inspect.Parameter.KEYWORD_ONLY
    # Both adapters are now general over one static Result and one Trajectory,
    # so `step` is optional in the signature and required by the Trajectory arm.
    assert scalar["step"].default is not inspect.Parameter.empty
    assert deformed["step"].default is not inspect.Parameter.empty
    assert scalar["field"].default is inspect.Parameter.empty
    assert deformed["field"].default is inspect.Parameter.empty
    assert deformed["scale"].kind is inspect.Parameter.KEYWORD_ONLY
    assert deformed["scale"].default == 1.0


def test_withdrawn_demo_stills_are_absent_without_alias_shims_or_exports() -> None:
    stub = files(eqiora).joinpath("matplotlib.pyi").read_text(encoding="utf-8")

    for removed in ("plot_fixed_reference_fsi", "plot_pressure"):
        assert not hasattr(eqplot, removed)
        with pytest.raises(AttributeError):
            getattr(eqplot, removed)
        assert removed not in dir(eqplot)
        assert removed not in stub
    assert eqplot.__all__ == [
        "plot_deformed_field",
        "plot_displacement",
        "plot_scalar_field",
    ]
    assert "from typing import overload" in stub
    assert "result: Result" in stub
    assert "trajectory: Trajectory" in stub
    assert stub.count("def plot_scalar_field(") == 2
    assert stub.count("def plot_deformed_field(") == 2
    for name in ("plot_scalar_field", "plot_deformed_field"):
        assert name in stub
        assert callable(getattr(eqplot, name))
    # The predecessor still is retained by the pre-1.0 compatibility rule for
    # one subsequent prerelease and owns no plotting implementation.
    assert "def plot_displacement(" in stub
    assert callable(eqplot.plot_displacement)


@pytest.mark.parametrize("step", ACCEPTED_STEPS)
def test_scalar_still_draws_exactly_the_accepted_support_restriction(
    fsi: tuple[eqiora.Model, eqiora.Plan, eqiora.Result, eqiora.trajectory.Trajectory],
    step: int,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    import matplotlib.pyplot as pyplot

    _, plan, _, trajectory = fsi
    field = fsi_field(plan, "fluid_pressure")
    snapshot = trajectory.state(step).field(field)
    support = ACCEPTED_VERTEX_SUPPORT["fluid_pressure"]
    cells = ACCEPTED_SUPPORT_CELLS["fluid_pressure"]
    np.testing.assert_array_equal(trajectory.cells, ACCEPTED_CELLS)
    np.testing.assert_array_equal(snapshot.support_indices("vertex"), support)
    assert admitted_cells(support) == cells
    assert snapshot.value_shape == ()
    assert snapshot.frame == "invariant"
    assert snapshot.associations == ("vertex",)

    coordinates = trajectory.coordinates.copy()
    values = expanded_vertex_values(trajectory, snapshot)
    restricted = values[list(support)]
    observed = capture_tripcolor(monkeypatch)
    registered_figures = pyplot.get_fignums()
    figure = eqplot.plot_scalar_field(trajectory, step=step, field=field)

    assert pyplot.get_fignums() == registered_figures
    assert len(observed["positional"]) == 3
    horizontal, vertical, drawn_values = observed["positional"]
    np.testing.assert_array_equal(horizontal, coordinates[:, 0])
    np.testing.assert_array_equal(vertical, coordinates[:, 1])
    np.testing.assert_array_equal(drawn_values, values)
    assert observed["shading"] == "gouraud"
    drawn_cells = observed["triangles"]
    assert drawn_cells is not None
    np.testing.assert_array_equal(drawn_cells, np.asarray(ACCEPTED_CELLS)[list(cells)])
    np.testing.assert_array_equal(np.unique(drawn_cells), support)
    assert drawn_cells.size > 0
    assert int(np.asarray(drawn_cells).max()) < coordinates.shape[0]

    # Only support-restricted values set the scalar limits. The outside-support
    # entries are exactly +0.0, so a whole-block implementation is caught
    # whenever the accepted extrema exclude zero; no sign is assumed here.
    outside = sorted(set(range(coordinates.shape[0])) - set(support))
    np.testing.assert_array_equal(values[outside], 0.0)
    assert not np.signbit(values[outside]).any()
    assert (float(values.min()), float(values.max())) == (
        min(float(restricted.min()), 0.0),
        max(float(restricted.max()), 0.0),
    )
    assert observed["artist"].get_clim() == (
        float(restricted.min()),
        float(restricted.max()),
    )

    axes = figure.axes[0]
    assert axes.get_xlabel() == "x [m]"
    assert axes.get_ylabel() == "y [m]"
    assert figure.axes[1].get_ylabel() == "Value [kg·m^-1·s^-2]"
    assert axes.get_aspect() == 1.0


@pytest.mark.parametrize("step", ACCEPTED_STEPS)
@pytest.mark.parametrize("scale", [0.0, 1.0, 12.0])
def test_deformed_still_draws_reference_and_scaled_support_edges(
    fsi: tuple[eqiora.Model, eqiora.Plan, eqiora.Result, eqiora.trajectory.Trajectory],
    step: int,
    scale: float,
) -> None:
    import matplotlib.pyplot as pyplot

    _, plan, _, trajectory = fsi
    field = fsi_field(plan, "solid_displacement")
    snapshot = trajectory.state(step).field(field)
    support = ACCEPTED_VERTEX_SUPPORT["solid_displacement"]
    cells = ACCEPTED_SUPPORT_CELLS["solid_displacement"]
    edges = ACCEPTED_SUPPORT_EDGES["solid_displacement"]
    np.testing.assert_array_equal(snapshot.support_indices("vertex"), support)
    assert admitted_cells(support) == cells
    assert admitted_edges(cells) == edges
    assert tuple(sorted({vertex for edge in edges for vertex in edge})) == support
    assert snapshot.value_shape == (trajectory.dimension,)
    assert snapshot.frame == "spatial-cartesian"
    assert snapshot.dimension == (0, 1, 0, 0, 0, 0, 0)
    assert snapshot.associations == ("vertex",)

    coordinates = trajectory.coordinates.copy()
    values = expanded_vertex_values(trajectory, snapshot)
    selection = list(edges)
    expected_reference = coordinates[selection]
    expected_deformed = (coordinates + scale * values)[selection]

    registered_figures = pyplot.get_fignums()
    figure = eqplot.plot_deformed_field(
        trajectory,
        step=step,
        field=field,
        scale=scale,
    )

    assert pyplot.get_fignums() == registered_figures
    reference, deformed = wireframes(figure)
    assert reference.shape == (len(edges), 2, 2)
    np.testing.assert_array_equal(reference, expected_reference)
    np.testing.assert_array_equal(deformed, expected_deformed)
    if scale == 0.0:
        np.testing.assert_array_equal(deformed, expected_reference)

    axes = figure.axes[0]
    labels = [artist.get_label() for artist in axes.collections]
    assert any(f"{scale:g}" in text for text in (axes.get_title(), *labels))
    assert axes.get_xlabel() == "x [m]"
    assert axes.get_ylabel() == "y [m]"
    assert axes.get_aspect() == 1.0


def test_deformed_still_defaults_to_unit_scale_and_addresses_accepted_steps(
    fsi: tuple[eqiora.Model, eqiora.Plan, eqiora.Result, eqiora.trajectory.Trajectory],
) -> None:
    _, plan, _, trajectory = fsi
    field = fsi_field(plan, "solid_displacement")
    selection = list(ACCEPTED_SUPPORT_EDGES["solid_displacement"])
    coordinates = trajectory.coordinates

    default = eqplot.plot_deformed_field(trajectory, step=1, field=field)
    explicit = eqplot.plot_deformed_field(trajectory, step=1, field=field, scale=1.0)
    later = eqplot.plot_deformed_field(trajectory, step=2, field=field)

    values = expanded_vertex_values(trajectory, trajectory.state(1).field(field))
    np.testing.assert_array_equal(
        wireframes(default)[1],
        (coordinates + 1.0 * values)[selection],
    )
    np.testing.assert_array_equal(wireframes(default)[1], wireframes(explicit)[1])
    later_values = expanded_vertex_values(
        trajectory, trajectory.state(2).field(field)
    )
    np.testing.assert_array_equal(
        wireframes(later)[1],
        (coordinates + later_values)[selection],
    )


def test_stills_reject_foreign_identity_and_contract_violations_before_a_figure(
    fsi: tuple[eqiora.Model, eqiora.Plan, eqiora.Result, eqiora.trajectory.Trajectory],
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    model, plan, result, trajectory = fsi
    pressure = fsi_field(plan, "fluid_pressure")
    displacement = fsi_field(plan, "solid_displacement")
    solid_velocity = fsi_field(plan, "solid_velocity")
    fluid_velocity = fsi_field(plan, "fluid_velocity")
    # Exact identity is the (Model artifact, field) pair. This separately
    # compiled parameter mutant deliberately retains the same semantic Field
    # IDs, so an ID alone supplies no accepted authority.
    foreign, foreign_plan = foreign_fsi_model()
    assert model.digest != foreign.digest
    assert trajectory.model_digest == model.digest == pressure.model_digest
    assert model.field_ids == foreign.field_ids
    for name, accepted_field in (
        ("fluid_pressure", pressure),
        ("solid_displacement", displacement),
    ):
        foreign_field = fsi_field(foreign_plan, name)
        assert foreign_field.id == accepted_field.id
        assert foreign_field.model_digest == foreign.digest
        assert foreign_field != accepted_field
        assert model.field(accepted_field.id) == accepted_field
        assert hash(model.field(accepted_field.id)) == hash(accepted_field)

    # Only the SI length dimension separates solid_velocity from the admitted
    # deformation field, so rejecting it cannot be a shape check in disguise.
    accepted = trajectory.state(1).field(displacement)
    velocity = trajectory.state(1).field(solid_velocity)
    assert velocity.value_shape == accepted.value_shape
    assert velocity.frame == accepted.frame
    assert velocity.associations == accepted.associations
    assert velocity.dimension == (0, 1, -1, 0, 0, 0, 0)
    assert velocity.dimension != accepted.dimension

    forbid_rendering(monkeypatch)
    with pytest.raises(TypeError):
        eqplot.plot_scalar_field(trajectory, field=pressure)
    with pytest.raises(TypeError):
        eqplot.plot_deformed_field(trajectory, field=displacement)
    for foreign_input in (object(), result, trajectory.coordinates):
        with pytest.raises(TypeError, match="Result|Trajectory|step"):
            eqplot.plot_scalar_field(foreign_input, step=1, field=pressure)
        with pytest.raises(TypeError, match="Result|Trajectory"):
            eqplot.plot_deformed_field(foreign_input, step=1, field=displacement)
    for other_pressure, other_displacement in ((
        fsi_field(foreign_plan, "fluid_pressure"),
        fsi_field(foreign_plan, "solid_displacement"),
    ),):
        with pytest.raises(ValueError, match="different exact Model"):
            eqplot.plot_scalar_field(
                trajectory,
                step=1,
                field=other_pressure,
            )
        with pytest.raises(ValueError, match="different exact Model"):
            eqplot.plot_deformed_field(
                trajectory,
                step=1,
                field=other_displacement,
            )
    absent = next(
        model.field(identifier)
        for identifier in model.field_ids
        if identifier not in {field.id for field in plan.fields}
    )
    with pytest.raises(KeyError):
        eqplot.plot_scalar_field(
            trajectory,
            step=1,
            field=absent,
        )
    for absent in (0, 3):
        with pytest.raises(IndexError):
            eqplot.plot_scalar_field(trajectory, step=absent, field=pressure)
        with pytest.raises(IndexError):
            eqplot.plot_deformed_field(trajectory, step=absent, field=displacement)
    for rejected in (displacement, solid_velocity, fluid_velocity):
        with pytest.raises(ValueError, match=CONTRACT_VOCABULARY):
            eqplot.plot_scalar_field(trajectory, step=1, field=rejected)
    for rejected in (pressure, solid_velocity, fluid_velocity):
        with pytest.raises(ValueError, match=CONTRACT_VOCABULARY):
            eqplot.plot_deformed_field(trajectory, step=1, field=rejected)
    for scale in (-1.0, float("inf"), float("nan")):
        with pytest.raises(ValueError, match="finite and nonnegative"):
            eqplot.plot_deformed_field(
                trajectory,
                step=1,
                field=displacement,
                scale=scale,
            )


def test_stills_leave_digests_arrays_and_support_membership_untouched(
    fsi: tuple[eqiora.Model, eqiora.Plan, eqiora.Result, eqiora.trajectory.Trajectory],
) -> None:
    _, plan, _, trajectory = fsi
    fields = {name: fsi_field(plan, name) for name in ACCEPTED_VERTEX_SUPPORT}
    snapshots = {
        (step, name): trajectory.state(step).field(field)
        for step in ACCEPTED_STEPS
        for name, field in fields.items()
    }
    digests_before = (
        trajectory.digest,
        trajectory.model_digest,
        trajectory.geometry_digest,
        trajectory.correspondence_digest,
        trajectory.mesh_digest,
        trajectory.realization_digest,
        trajectory.run_digest,
        tuple(state.digest for state in trajectory.states),
        tuple(snapshot.digest for snapshot in snapshots.values()),
        tuple(snapshot.block_digests for snapshot in snapshots.values()),
    )
    arrays_before = {
        "coordinates": trajectory.coordinates,
        "cells": trajectory.cells,
        **{
            f"values-{key}": snapshot.values("vertex")
            for key, snapshot in snapshots.items()
        },
        **{
            f"support-{key}": snapshot.support_indices("vertex")
            for key, snapshot in snapshots.items()
        },
    }
    copies_before = {name: array.copy() for name, array in arrays_before.items()}

    for step in ACCEPTED_STEPS:
        eqplot.plot_scalar_field(trajectory, step=step, field=fields["fluid_pressure"])
        eqplot.plot_deformed_field(
            trajectory,
            step=step,
            field=fields["solid_displacement"],
            scale=12.0,
        )

    assert digests_before == (
        trajectory.digest,
        trajectory.model_digest,
        trajectory.geometry_digest,
        trajectory.correspondence_digest,
        trajectory.mesh_digest,
        trajectory.realization_digest,
        trajectory.run_digest,
        tuple(state.digest for state in trajectory.states),
        tuple(snapshot.digest for snapshot in snapshots.values()),
        tuple(snapshot.block_digests for snapshot in snapshots.values()),
    )
    assert trajectory.coordinates is arrays_before["coordinates"]
    assert trajectory.cells is arrays_before["cells"]
    for key, snapshot in snapshots.items():
        assert trajectory.state(key[0]).field(fields[key[1]]) is snapshot
        assert snapshot.values("vertex") is arrays_before[f"values-{key}"]
        assert snapshot.support_indices("vertex") is arrays_before[f"support-{key}"]
    for name, array in arrays_before.items():
        np.testing.assert_array_equal(array, copies_before[name])
        assert array.flags.writeable is False
        with pytest.raises(ValueError):
            array.setflags(write=True)


def test_stills_are_headless_caller_owned_and_survive_trajectory_release(
    tmp_path: Path,
) -> None:
    import matplotlib.pyplot as pyplot

    _, plan, result, trajectory = accepted_fsi_trajectory()
    pressure = fsi_field(plan, "fluid_pressure")
    displacement = fsi_field(plan, "solid_displacement")
    selection = list(ACCEPTED_SUPPORT_EDGES["solid_displacement"])
    expected_deformed = (
        trajectory.coordinates
        + 12.0
        * expanded_vertex_values(
            trajectory, trajectory.state(2).field(displacement)
        )
    )[selection]

    registered_figures = pyplot.get_fignums()
    scalar = eqplot.plot_scalar_field(trajectory, step=2, field=pressure)
    deformed = eqplot.plot_deformed_field(
        trajectory,
        step=2,
        field=displacement,
        scale=12.0,
    )
    repeated = eqplot.plot_deformed_field(
        trajectory,
        step=2,
        field=displacement,
        scale=12.0,
    )
    assert pyplot.get_fignums() == registered_figures
    np.testing.assert_array_equal(wireframes(deformed)[1], wireframes(repeated)[1])

    del plan, result, trajectory, pressure, displacement
    gc.collect()

    np.testing.assert_array_equal(wireframes(deformed)[1], expected_deformed)
    for figure, name in ((scalar, "scalar.png"), (deformed, "deformed.png")):
        encoded = io.BytesIO()
        figure.savefig(encoded, format="png")
        payload = encoded.getvalue()
        destination = tmp_path / name
        figure.savefig(destination)
        assert payload.startswith(b"\x89PNG\r\n\x1a\n")
        assert destination.read_bytes().startswith(b"\x89PNG\r\n\x1a\n")
        encoded.seek(0)
        pixels = image.imread(encoded, format="png")
        assert pixels.shape[2] in (3, 4)
        assert np.ptp(pixels[..., :3]) > 0.0
