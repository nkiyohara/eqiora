use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods};

const POISSON: &str =
    include_str!("../../../verify/interfaces/python-native-modeling/models/poisson.eqi");

#[test]
fn common_plan_owns_exact_model_mesh_and_effective_policies() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let locals = PyDict::new(py);
        locals.set_item("eqiora", native.bind(py))?;
        locals.set_item("source", POISSON)?;
        py.run(
            c_str!(
                r#"
model = eqiora.compile(source)
mesh_request = eqiora.Cartesian(cells_per_axis=4)
assert mesh_request.cells_per_axis == 4
assert mesh_request == eqiora.Cartesian(cells_per_axis=4)
assert hash(mesh_request) == hash(eqiora.Cartesian(cells_per_axis=4))
assert mesh_request != eqiora.Cartesian(cells_per_axis=8)
mesh = eqiora.resolve_plan(
    model,
    mesh=mesh_request,
    spatial=eqiora.Q1(),
    solve=eqiora.Linear(),
).mesh
assert mesh.dimension == 1
assert mesh.cells_per_axis == 4
assert mesh.cell_count == 4
assert len(mesh.digest) == 64
assert len(mesh.canonical_bytes) > 0

fem = eqiora.resolve_plan(
    model,
    mesh=mesh_request,
    spatial=eqiora.Q1(),
    solve=eqiora.Linear(),
)
fvm = eqiora.resolve_plan(
    model,
    mesh=mesh_request,
    spatial=eqiora.CellCenteredTpfa(),
    solve=eqiora.Linear(),
)
assert fem.model_digest == fvm.model_digest == model.digest
assert fem.mesh_digest == fvm.mesh_digest == mesh.digest
assert fem.mesh == fvm.mesh == mesh
assert fem.realization_digest != fvm.realization_digest
assert fem == eqiora.resolve_plan(model, mesh=mesh_request, spatial=eqiora.Q1(), solve=eqiora.Linear())
assert hash(fem) == hash(eqiora.resolve_plan(model, mesh=mesh_request, spatial=eqiora.Q1(), solve=eqiora.Linear()))
assert fem.realization.digest == fem.realization_digest
assert fem.discretization == "q1"
assert fem.space == "continuous-lagrange-q1"
assert fem.quadrature == "gauss-legendre-2-per-axis"
assert fvm.discretization == "cell-centered-tpfa"
assert fvm.space == "cell-constant"
assert fvm.quadrature == "cell-centroid"
for plan in (fem, fvm):
    assert plan.mesh_kind == "generated-cartesian"
    assert plan.spatial_dimension == 1
    assert plan.scalar_type == "f64"
    assert plan.vector_layout == "replicated"
    assert plan.operator_properties == "symmetric-positive-definite"
    assert plan.schedule == "offline"
    assert plan.solver_algorithm == "conjugate-gradient"
    assert plan.solver_backend == "eqiora.reference"
    assert len(plan.solver_backend_version) > 0
    assert plan.placement == "host-cpu"
    assert plan.workers == 1
    assert len(plan.execution_provider) > 0
    assert len(plan.execution_provider_version) > 0
    assert plan.solve == eqiora.Linear()

fem_result = eqiora.submit_plan(fem).result()
fvm_result = eqiora.submit_plan(fvm).result()
assert fem_result.realization.digest == fem.realization_digest
assert fvm_result.realization.digest == fvm.realization_digest
assert fem_result.field.location == eqiora.ScalarFieldLocation.Vertex
assert fvm_result.field.location == eqiora.ScalarFieldLocation.CellCenter
assert fem_result.field.value_count == 5
assert fvm_result.field.value_count == 4
"#
            ),
            None,
            Some(&locals),
        )
    })
}

#[test]
fn common_plan_reuses_mesh_request_and_rejects_unsupported_shapes() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let locals = PyDict::new(py);
        locals.set_item("eqiora", native.bind(py))?;
        locals.set_item("source", POISSON)?;
        py.run(
            c_str!(
                r#"
model = eqiora.compile(source)
mesh_request = eqiora.Cartesian(cells_per_axis=4)
changed = model.commit(model.preview_value_edit("source_scale", 2.0))
changed_plan = eqiora.resolve_plan(
    changed,
    mesh=mesh_request,
    spatial=eqiora.Q1(),
    solve=eqiora.Linear(),
)
original_plan = eqiora.resolve_plan(
    model,
    mesh=mesh_request,
    spatial=eqiora.Q1(),
    solve=eqiora.Linear(),
)
assert changed_plan.model_digest == changed.digest
assert changed_plan.mesh_digest == original_plan.mesh_digest
assert changed_plan != original_plan

for spatial in (object(), eqiora.Linear()):
    try:
        eqiora.resolve_plan(
            model,
            mesh=mesh_request,
            spatial=spatial,
            solve=eqiora.Linear(),
        )
    except TypeError:
        pass
    else:
        raise AssertionError("an unsupported spatial policy must be rejected")

try:
    eqiora.resolve_plan(
        model,
        mesh=mesh_request,
        spatial=eqiora.Q1(),
        solve=object(),
    )
except TypeError:
    pass
else:
    raise AssertionError("an unsupported solve policy must be rejected")

try:
    eqiora.resolve_plan(
        model,
        mesh=object(),
        spatial=eqiora.Q1(),
        solve=eqiora.Linear(),
    )
except TypeError:
    pass
else:
    raise AssertionError("an unsupported Mesh type must be rejected")

temporal = eqiora.compile("""
model decay {
  field x: 1 = 1;
  relation flow continuous { derivative(x) = 0; }
}
""")
try:
    eqiora.resolve_plan(
        temporal,
        mesh=mesh_request,
        spatial=eqiora.Q1(),
        solve=eqiora.Linear(),
    )
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("a non-spatial operator must fail before Plan publication")
"#
            ),
            None,
            Some(&locals),
        )
    })
}
