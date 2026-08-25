from __future__ import annotations

import pytest

import eqiora


POISSON = """
model common_plan_poisson {
  domain interval = box(0, 1);
  domain lower_end = boundary(interval, axis = 0, side = lower);
  domain upper_end = boundary(interval, axis = 0, side = upper);
  representation scalar_space = continuum;
  field potential on interval as scalar_space: 1 = 0;
  parameter source_scale: 1 / m ^ 2 = 1;
  relation balance continuous on interval {
    -div(grad(potential)) - source_scale = 0;
  }
  relation lower_value continuous on lower_end { trace(potential) = 0; }
  relation upper_value continuous on upper_end { trace(potential) = 0; }
}
"""


@pytest.mark.parametrize(
    ("spatial", "discretization", "location"),
    [
        (eqiora.fem.Q1(), "q1", eqiora.ScalarFieldLocation.Vertex),
        (
            eqiora.fvm.CellCenteredTpfa(),
            "cell-centered-tpfa",
            eqiora.ScalarFieldLocation.CellCenter,
        ),
    ],
)
def test_common_plan_is_the_complete_public_run_input(
    spatial: eqiora.fem.Q1 | eqiora.fvm.CellCenteredTpfa,
    discretization: str,
    location: eqiora.ScalarFieldLocation,
) -> None:
    model = eqiora.compile(POISSON)
    mesh_request = eqiora.meshing.Cartesian(cells_per_axis=4)
    plan = eqiora.resolve(
        model,
        mesh=mesh_request,
        spatial=spatial,
        solve=eqiora.solve.Linear(),
    )

    assert plan.model_digest == model.digest
    assert plan.mesh_digest == plan.mesh.digest
    assert plan.mesh.cells_per_axis == mesh_request.cells_per_axis
    assert plan.discretization == discretization
    assert plan.spatial == spatial
    assert plan.solve == eqiora.solve.Linear()
    assert plan.realization.digest == plan.realization_digest
    assert plan.placement == "host-cpu"
    assert plan.workers == 1

    result = eqiora.run(plan)
    assert result.realization.digest == plan.realization_digest
    assert result.field.location == location

    with pytest.raises(TypeError, match="accepts no additional"):
        eqiora.run(plan, end_time=1.0, max_step=0.1)


def test_common_plan_reuses_request_across_values_and_rejects_untyped_policies() -> None:
    model = eqiora.compile(POISSON)
    mesh_request = eqiora.meshing.Cartesian(cells_per_axis=4)
    changed = model.commit(model.preview_value_edit("source_scale", 2.0))
    original_plan = eqiora.resolve(
        model,
        mesh=mesh_request,
        spatial=eqiora.fem.Q1(),
        solve=eqiora.solve.Linear(),
    )
    changed_plan = eqiora.resolve(
        changed,
        mesh=mesh_request,
        spatial=eqiora.fem.Q1(),
        solve=eqiora.solve.Linear(),
    )
    assert changed_plan.mesh_digest == original_plan.mesh_digest
    assert changed_plan.model_digest != original_plan.model_digest
    with pytest.raises(TypeError):
        eqiora.resolve(
            model,
            mesh=mesh_request,
            spatial=object(),
            solve=eqiora.solve.Linear(),
        )
    with pytest.raises(TypeError):
        eqiora.resolve(
            model,
            mesh=mesh_request,
            spatial=eqiora.fem.Q1(),
            solve=object(),
        )
    with pytest.raises(TypeError):
        eqiora.resolve(
            model,
            mesh=object(),
            spatial=eqiora.fem.Q1(),
            solve=eqiora.solve.Linear(),
        )

    temporal = eqiora.compile(
        """
model decay {
  field x: 1 = 1;
  relation flow continuous { derivative(x) = 0; }
}
"""
    )
    with pytest.raises(eqiora.ValidationError):
        eqiora.resolve(
            temporal,
            mesh=mesh_request,
            spatial=eqiora.fem.Q1(),
            solve=eqiora.solve.Linear(),
        )
