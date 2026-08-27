use std::path::Path;

use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyModule};

#[test]
fn python_common_ode_route_matches_independent_exponential_decay() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item(
            "source",
            include_str!("../../../verify/interfaces/python-common-ode-lifecycle/models/decay.eqi"),
        )?;
        py.run(
            c_str!(
                r#"
import asyncio
import math

SOURCE = source

def resolve(model):
    field = model.field(model.field_ids[0])
    plan = eqiora.resolve(
        model,
        temporal=eqiora.time.Tsitouras45(
            initial_step_s=0.01,
            relative_tolerance=1.0e-9,
            absolute_tolerances={field: 1.0e-11},
        ),
    )
    return field, plan

model = eqiora.compile(source=SOURCE)
field, plan = resolve(model)
assert plan.mesh is None
assert plan.spatial is None
assert plan.solve is None
assert plan.scaling is None
assert plan.preconditioner is None
assert plan.temporal is not None
assert plan.temporal.absolute_tolerances == {field: 1.0e-11}
state = eqiora.State.initial(plan)
assert state.value(field) == 1.0
result = eqiora.run(
    plan,
    state=state,
    until_s=0.2,
    output_times_s=(0.1, 0.2),
)
series = result.series(field)
assert series.field == field
assert series.dimension == (0, 0, 0, 0, 0, 0, 0)
assert len(series) == 2
assert [time_s for time_s, _ in series] == [0.1, 0.2]
for time_s, value in series:
    assert math.isclose(value, math.exp(-time_s), rel_tol=2.0e-8, abs_tol=2.0e-10)

replayed = eqiora.replay(model.to_json())
replayed_field, replayed_plan = resolve(replayed)
assert replayed_plan.identity == plan.identity
assert replayed_plan.model_digest == model.digest

other = eqiora.compile(source=SOURCE)
other_field, other_plan = resolve(other)
assert other_plan.model_digest != plan.model_digest
foreign_temporal = eqiora.time.Tsitouras45(
    initial_step_s=0.01,
    relative_tolerance=1.0e-9,
    absolute_tolerances={other_field: 1.0e-11},
)
try:
    eqiora.resolve(
        model,
        temporal=foreign_temporal,
    )
except TypeError:
    pass
else:
    raise AssertionError("foreign absolute-tolerance FieldRef was admitted")
try:
    eqiora.run(
        plan,
        state=eqiora.State.initial(other_plan),
        until_s=0.2,
        output_times_s=(0.2,),
    )
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("foreign ODE State was admitted")
for selector in (other_field, "x"):
    try:
        result.series(selector)
    except (TypeError, ValueError):
        pass
    else:
        raise AssertionError("non-exact ODE Series selector was admitted")

changed = eqiora.resolve(
    model,
    temporal=eqiora.time.Tsitouras45(
        initial_step_s=0.02,
        relative_tolerance=1.0e-8,
        absolute_tolerances={field: 2.0e-11},
    ),
)
assert changed.identity != plan.identity
assert changed.fields == plan.fields

for outputs in ((), (0.0,), (0.2, 0.1), (0.3,), (float("nan"),), (float("inf"),)):
    try:
        eqiora.run(
            plan,
            state=state,
            until_s=0.2,
            output_times_s=outputs,
        )
    except eqiora.ValidationError:
        pass
    else:
        raise AssertionError(f"invalid ODE output schedule was admitted: {outputs!r}")
for kwargs in (
    dict(initial_step_s=True, relative_tolerance=1.0e-9, absolute_tolerances={field: 1.0e-11}),
    dict(initial_step_s=0.0, relative_tolerance=1.0e-9, absolute_tolerances={field: 1.0e-11}),
    dict(initial_step_s=float("nan"), relative_tolerance=1.0e-9, absolute_tolerances={field: 1.0e-11}),
    dict(initial_step_s=0.01, relative_tolerance=0.0, absolute_tolerances={field: 1.0e-11}),
    dict(initial_step_s=0.01, relative_tolerance=1.0e-9, absolute_tolerances={}),
    dict(initial_step_s=0.01, relative_tolerance=1.0e-9, absolute_tolerances={field: -1.0e-11}),
):
    try:
        eqiora.time.Tsitouras45(**kwargs)
    except (TypeError, eqiora.ValidationError):
        pass
    else:
        raise AssertionError(f"invalid Tsitouras45 controls were admitted: {kwargs!r}")
try:
    eqiora.resolve(model, temporal=eqiora.time.BackwardEuler(0.01))
except TypeError:
    pass
else:
    raise AssertionError("BackwardEuler entered the no-Mesh ODE arm")
try:
    eqiora.resolve(model, spatial=object(), temporal=plan.temporal)
except TypeError:
    pass
else:
    raise AssertionError("spatial policy entered the no-Mesh ODE arm")

async def await_same_result():
    submitted = eqiora.submit(
        replayed_plan,
        state=eqiora.State.initial(replayed_plan),
        until_s=0.2,
        output_times_s=(0.2,),
    )
    assert submitted.adapter_version == "0.16.1"
    assert submitted.cancel() is False
    awaited = await submitted
    assert awaited is submitted.result()
    assert math.isclose(
        awaited.series(replayed_field).values[0],
        math.exp(-0.2),
        rel_tol=2.0e-8,
        abs_tol=2.0e-10,
    )

asyncio.run(await_same_result())
"#
            ),
            Some(&locals),
            Some(&locals),
        )
    })
}

fn public_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
    let package_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../bindings/python/python/eqiora")
        .canonicalize()?;
    let locals = PyDict::new(py);
    locals.set_item("native", native.bind(py))?;
    locals.set_item("package_directory", package_directory.to_string_lossy())?;
    py.run(
        c_str!(
            r#"
import importlib.util
import pathlib
import sys

package_path = pathlib.Path(package_directory)
spec = importlib.util.spec_from_file_location(
    "eqiora",
    package_path / "__init__.py",
    submodule_search_locations=[str(package_path)],
)
assert spec is not None and spec.loader is not None
package = importlib.util.module_from_spec(spec)
sys.modules["eqiora"] = package
sys.modules["eqiora._eqiora"] = native
spec.loader.exec_module(package)
"#
        ),
        None,
        Some(&locals),
    )?;
    Ok(locals
        .get_item("package")?
        .expect("the package loader must bind eqiora")
        .cast_into::<PyModule>()?)
}
