"""Installed-wheel contract for the root exact-cylinder Stokes lifecycle."""

from __future__ import annotations

import subprocess
import sys
from importlib.resources import files
from pathlib import Path

import numpy as np
import pytest

import eqiora


REPOSITORY_ROOT = Path(__file__).resolve().parents[3]
PYTHON_DEMO = REPOSITORY_ROOT / "examples" / "python" / "exact_cylinder_stokes.py"


def geometry_and_mesh() -> tuple[eqiora.geometry.Geometry, eqiora.meshing.Mesh]:
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    circle = graph.circle(center=(0.2, 0.2), radius=0.05)
    fluid = graph.subtract(rectangle, circle)
    geometry = graph.build(
        fluid,
        named_topology={
            "fluid": fluid.region,
            "inlet": rectangle.boundaries[0],
            "outlet": rectangle.boundaries[1],
            "walls": rectangle.boundaries[2:],
            "cylinder": circle.boundaries[0],
        },
    )
    request = eqiora.meshing.GmshMesher(
        maximum_boundary_error=1.0e-4,
        minimum_mean_ratio=1.0e-5,
        maximum_boundary_facets=50,
    )
    mesh_plan = eqiora.meshing.resolve(geometry, request)
    return geometry, eqiora.meshing.generate(geometry, plan=mesh_plan)


def accepted() -> tuple[eqiora.geometry.Geometry, eqiora.Model, eqiora.Plan, eqiora.Result]:
    geometry, mesh = geometry_and_mesh()
    model = eqiora.compile(
        path=files(eqiora).joinpath("examples", "steady-flow-past-cylinder.eqi"),
        geometry=geometry,
        parameters={
            "dynamic_viscosity": 1.0e-3,
            "zero_pressure": 0.0,
            "inlet_speed": 0.3,
            "channel_height": geometry.bounds[1][1] - geometry.bounds[1][0],
        },
    )
    plan = eqiora.resolve(
        model,
        mesh=mesh,
        spatial=eqiora.fem.MiniP1(),
        solve=eqiora.solve.Linear(
            relative_tolerance=1.0e-6,
            absolute_tolerance=1.0e-13,
            maximum_iterations=10_000,
        ),
        scaling=None,
    )
    return geometry, model, plan, eqiora.run(plan)


def test_root_plan_result_and_observation_close_exact_lineage() -> None:
    geometry, model, plan, result = accepted()
    pressure = result.output(plan.capability.pressure)
    evidence = eqiora.fluid.steady_stokes_evidence(result)

    assert result.model_digest == model.digest == plan.model_digest
    assert result.plan_key == plan.identity == evidence.plan_key
    assert pressure.field == plan.capability.pressure
    assert pressure.mesh is plan.mesh
    assert pressure.coefficient_count("vertex") == plan.mesh.vertex_count
    assert pressure.value_shape == ()
    values = pressure.values("vertex").numpy(copy=False)
    assert values.shape == (plan.mesh.vertex_count,)
    assert not values.flags.writeable
    assert np.isfinite(values).all()
    assert float(values.min()) == evidence.pressure_minimum
    assert float(values.max()) == evidence.pressure_maximum

    cylinder_force = result.boundary_force(geometry.selection("cylinder"))
    inlet_flux = result.boundary_flux(geometry.selection("inlet"))
    outlet_flux = result.boundary_flux(geometry.selection("outlet"))
    assert cylinder_force.on_domain == evidence.cylinder_force_on_fluid
    assert cylinder_force.source_digest == result.plan_key
    assert cylinder_force.source_kind == "result"
    assert inlet_flux.value == evidence.inlet_flux
    assert outlet_flux.value == evidence.outlet_flux
    assert inlet_flux.value + outlet_flux.value == evidence.net_flux

    assert evidence.exact_bounds == ((0.0, 2.2), (0.0, 0.41))
    assert evidence.net_flux == evidence.inlet_flux + evidence.outlet_flux
    assert np.isfinite(evidence.net_flux)
    assert np.isfinite(evidence.momentum_closure).all()
    assert evidence.solve.true_residual_norm <= evidence.solve.residual_target


def test_fresh_and_replayed_models_use_the_same_root_resolver() -> None:
    _, model, plan, _ = accepted()
    replayed = eqiora.Model.from_bytes(model.to_bytes())
    again = eqiora.resolve(
        replayed,
        mesh=plan.mesh,
        spatial=eqiora.fem.MiniP1(),
        solve=eqiora.solve.Linear(
            relative_tolerance=1.0e-6,
            absolute_tolerance=1.0e-13,
            maximum_iterations=10_000,
        ),
        scaling=None,
    )
    assert again.identity == plan.identity
    assert again.model_digest == replayed.digest == model.digest
    assert eqiora.run(again).plan_key == again.identity


def test_displaced_fluid_lifecycle_is_absent_and_cross_physics_fails() -> None:
    for name in ("SteadyStokes", "SteadyStokesPlan", "resolve"):
        assert not hasattr(eqiora.fluid, name)
        assert name not in eqiora.fluid.__all__

    ode = eqiora.compile(source="""
model decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous { derivative(x) + rate * x = 0; }
}
""")
    field = ode.field(ode.field_ids[0])
    ode_plan = eqiora.resolve(
        ode,
        temporal=eqiora.time.Tsitouras45(
            initial_step_s=0.01,
            relative_tolerance=1.0e-9,
            absolute_tolerances={field: 1.0e-11},
        ),
    )
    ode_result = eqiora.run(
        ode_plan,
        state=eqiora.State.initial(ode_plan),
        until_s=0.1,
        output_times_s=(0.1,),
    )
    with pytest.raises(eqiora.CapabilityError):
        eqiora.fluid.steady_stokes_evidence(ode_result)


def test_checked_in_python_demo_runs_with_packaged_component_resource() -> None:
    completed = subprocess.run(
        [sys.executable, str(PYTHON_DEMO)],
        cwd=REPOSITORY_ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    assert "cylinder force on fluid" in completed.stdout
    assert "net flux" in completed.stdout
