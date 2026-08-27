"""Installed-wheel contract for the common structural Plan and observation."""

from __future__ import annotations

import subprocess
import sys
from importlib.resources import files
from pathlib import Path

import numpy as np
import pytest

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "mixed_boundary_elasticity.py"


def geometry_and_mesh() -> tuple[eqiora.geometry.Geometry, eqiora.meshing.Mesh]:
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
    geometry = graph.build(
        rectangle,
        named_topology={
            "body": rectangle.region,
            "x_lower": rectangle.boundaries[0],
            "x_upper": rectangle.boundaries[1],
            "y_lower": rectangle.boundaries[2],
            "y_upper": rectangle.boundaries[3],
        },
    )
    request = eqiora.meshing.MeshRequest(
        eqiora.meshing.CartesianMesher(cells=(16, 16))
    )
    plan = eqiora.meshing.resolve(geometry, request)
    return geometry, eqiora.meshing.generate(geometry, plan=plan)


def accepted() -> tuple[eqiora.Model, eqiora.Plan, eqiora.Result]:
    geometry, mesh = geometry_and_mesh()
    model = eqiora.compile(
        path=files(eqiora).joinpath("examples", "mixed-boundary-elasticity.eqi"),
        geometry=geometry,
        parameters={"mu": 3.0, "lambda": 0.0, "length_scale": 1.0},
    )
    plan = eqiora.resolve(
        model,
        mesh=mesh,
        spatial=eqiora.fem.Q1(),
        solve=eqiora.solve.Linear(
            relative_tolerance=1.0e-10,
            absolute_tolerance=1.0e-12,
            maximum_iterations=10_000,
        ),
    )
    return model, plan, eqiora.run(plan)


def test_common_plan_result_and_observation_close_exact_lineage() -> None:
    model, plan, result = accepted()
    displacement = plan.field
    assert displacement is not None
    output = result.output(displacement)
    evidence = eqiora.solid.linear_elasticity_evidence(result)

    assert result.model_digest == model.digest == plan.model_digest
    assert result.plan_key == plan.identity == evidence.plan_key
    assert output.field == displacement
    assert output.mesh is plan.mesh
    assert output.vertex_count == 289
    assert output.components == 2
    assert output.dimension == (0, 1, 0, 0, 0, 0, 0)
    values = output.vertex_values.numpy(copy=False).reshape(289, 2)
    assert values.shape == (289, 2)
    assert not values.flags.writeable
    assert np.isfinite(values).all()

    assert evidence.exact_bounds == ((0.0, 1.0), (0.0, 1.0))
    assert evidence.assembly_packets > 0
    assert evidence.assembly_targets > 0
    assert np.isfinite(evidence.constrained_reaction).all()
    assert np.isfinite(evidence.integrated_body_force).all()
    assert evidence.solve.true_residual_norm <= evidence.solve.residual_target
    np.testing.assert_allclose(
        np.asarray(evidence.constrained_reaction)
        + np.asarray(evidence.integrated_body_force),
        np.zeros(2),
        rtol=0.0,
        atol=1.0e-10,
    )


def test_root_plan_rejects_foreign_model_field_and_observation() -> None:
    model, plan, result = accepted()
    foreign_geometry, foreign_mesh = geometry_and_mesh()
    foreign = eqiora.compile(
        path=files(eqiora).joinpath("examples", "mixed-boundary-elasticity.eqi"),
        geometry=foreign_geometry,
        parameters={"mu": 4.0, "lambda": 0.0, "length_scale": 1.0},
    )
    foreign_plan = eqiora.resolve(
        foreign,
        mesh=foreign_mesh,
        spatial=eqiora.fem.Q1(),
        solve=eqiora.solve.Linear(
            relative_tolerance=1.0e-10,
            absolute_tolerance=1.0e-12,
            maximum_iterations=10_000,
        ),
    )
    assert foreign_plan.identity != plan.identity
    with pytest.raises(ValueError, match="different exact Model"):
        assert foreign_plan.field is not None
        result.output(foreign_plan.field)
    with pytest.raises(eqiora.ValidationError):
        eqiora.resolve(
            model,
            mesh=plan.mesh,
            spatial=eqiora.fem.MiniP1(),
            solve=eqiora.solve.Linear(
                relative_tolerance=1.0e-10,
                absolute_tolerance=1.0e-12,
                maximum_iterations=10_000,
            ),
        )


def test_displaced_structural_lifecycle_is_absent() -> None:
    for name in (
        "LinearElasticity",
        "LinearElasticityPlan",
        "MixedBoundaryElasticityResult",
        "resolve",
        "solve_mixed_boundary_elasticity",
    ):
        assert not hasattr(eqiora.solid, name)
        assert name not in eqiora.solid.__all__


def test_checked_in_python_demo_runs_with_packaged_component_resource() -> None:
    completed = subprocess.run(
        [sys.executable, str(PYTHON_DEMO)],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    assert "constrained reaction" in completed.stdout
    assert "integrated body force" in completed.stdout
