"""Ordinary installed-product coverage for Geometry v2 through Gmsh."""

from __future__ import annotations

import json

import numpy as np
import pytest

import eqiora


def geometry(*, x_max: float = 2.2, center: tuple[float, float] = (0.2, 0.2)) -> eqiora.geometry.Geometry:
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, x_max), y_bounds=(0.0, 0.41))
    circle = graph.circle(center=center, radius=0.05)
    fluid = graph.subtract(rectangle, circle)
    return graph.build(
        fluid,
        named_topology={
            "fluid": fluid.region,
            "inlet": rectangle.boundaries[0],
            "outlet": rectangle.boundaries[1],
            "walls": rectangle.boundaries[2:],
            "cylinder": circle.boundaries[0],
        },
    )


def provider(**changes: float | int | None) -> eqiora.meshing.GmshMesher:
    values: dict[str, float | int | None] = {
        "maximum_boundary_error": 1.0e-4,
        "minimum_mean_ratio": 1.0e-5,
        "maximum_boundary_facets": 50,
    }
    values.update(changes)
    return eqiora.meshing.GmshMesher(**values)


def test_gmsh_plan_publishes_complete_source_owned_mesh(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("EQIORA_GMSH", "gmsh")
    source = geometry()
    policy = provider()
    plan = eqiora.meshing.resolve(source, policy)
    mesh = eqiora.meshing.generate(plan)

    assert plan.provider == policy
    assert plan.source_digest == source.digest == mesh.source_digest
    assert mesh.realized_geometry_digest == source.digest
    assert mesh.dimension == 2
    assert mesh.vertex_count > 0
    assert mesh.cell_count > 0
    assert mesh.selection_entity_count(source.selection("fluid")) == mesh.cell_count
    assert set(mesh.selection_names) == {"fluid", "inlet", "outlet", "walls", "cylinder"}
    assert sum(
        mesh.selection_entity_count(source.selection(name))
        for name in ("inlet", "outlet", "walls", "cylinder")
    ) > 0
    assert not hasattr(plan, "production_lineage_bytes")
    assert not hasattr(plan, "production_lineage_digest")
    lineage = json.loads(mesh.production_lineage_bytes)
    assert lineage["provider"] == {"identity": "eqiora.gmsh-cli", "version": "4.15.2"}
    assert lineage["effective_policy"] == {
        "kind": "gmsh-mesh",
        "maximum_boundary_error_m": 1.0e-4,
        "minimum_mean_ratio": 1.0e-5,
        "maximum_boundary_facets": 50,
        "maximum_target_size_m": 0.2,
        "maximum_target_size_ownership": "automatic",
    }

    coordinates = mesh.coordinates
    cells = mesh.cells
    assert coordinates.shape == (mesh.vertex_count, 2)
    assert cells.shape == (mesh.cell_count, 3)
    assert np.isfinite(coordinates).all()
    assert np.all(cells < mesh.vertex_count)
    assert not coordinates.flags.writeable
    assert not cells.flags.writeable


def test_gmsh_resolve_is_planning_only(monkeypatch: pytest.MonkeyPatch) -> None:
    source = geometry()
    monkeypatch.setenv("EQIORA_GMSH", "/provider/must/not/be-launched-by-resolve")

    plan = eqiora.meshing.resolve(source, provider())

    assert plan.source_digest == source.digest
    with pytest.raises(AttributeError):
        plan.source_digest = "mutated"
    with pytest.raises(eqiora.ValidationError):
        eqiora.meshing.generate(plan)


def test_gmsh_plan_replay_and_crosswire_are_fail_closed(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("EQIORA_GMSH", "gmsh")
    source = geometry()
    plan = eqiora.meshing.resolve(source, provider())
    first = eqiora.meshing.generate(plan)
    second = eqiora.meshing.generate(plan)
    assert first.digest == second.digest
    assert first.correspondence_digest == second.correspondence_digest
    assert first.canonical_bytes == second.canonical_bytes

    foreign = geometry(center=(0.21, 0.2))
    with pytest.raises(TypeError):
        eqiora.meshing.generate(foreign, plan=plan)
    with pytest.raises(eqiora.ValidationError):
        first.selection_entity_count(foreign.selection("cylinder"))


def test_explicit_target_size_is_replayable_and_strictly_denser(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setenv("EQIORA_GMSH", "gmsh")
    source = geometry()
    automatic_provider = provider()
    explicit_provider = provider(maximum_target_size=0.05)
    assert automatic_provider.maximum_target_size is None
    assert explicit_provider.maximum_target_size == 0.05

    automatic = eqiora.meshing.generate(
        eqiora.meshing.resolve(source, automatic_provider)
    )
    explicit_plan = eqiora.meshing.resolve(source, explicit_provider)
    explicit = eqiora.meshing.generate(explicit_plan)
    replayed = eqiora.meshing.generate(explicit_plan)

    assert explicit.cell_count > automatic.cell_count
    assert explicit.vertex_count > automatic.vertex_count
    assert replayed.digest == explicit.digest
    lineage = json.loads(explicit.production_lineage_bytes)
    assert lineage["effective_policy"]["maximum_target_size_m"] == 0.05
    assert lineage["effective_policy"]["maximum_target_size_ownership"] == "explicit"


@pytest.mark.parametrize(
    "kwargs",
    [
        {"maximum_boundary_error": 0.0},
        {"maximum_target_size": 0.0},
        {"maximum_target_size": float("nan")},
        {"maximum_target_size": float("inf")},
        {"minimum_mean_ratio": 0.0},
        {"maximum_boundary_facets": 7},
    ],
)
def test_gmsh_policy_rejects_invalid_values(kwargs: dict[str, float | int | None]) -> None:
    with pytest.raises(eqiora.ValidationError):
        provider(**kwargs)


def test_gmsh_plan_rejects_target_smaller_than_boundary_chord() -> None:
    with pytest.raises(eqiora.ValidationError):
        eqiora.meshing.resolve(geometry(), provider(maximum_target_size=0.001))


def test_reference_and_import_products_are_absent() -> None:
    assert not hasattr(eqiora.meshing, "ReferenceMesher")
    assert not hasattr(eqiora.meshing, "GmshImport")
    assert not hasattr(eqiora.meshing, "import_gmsh")
