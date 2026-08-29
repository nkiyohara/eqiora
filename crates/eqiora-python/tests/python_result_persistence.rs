use std::path::Path;

use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyDictMethods, PyModule};

#[test]
fn python_ode_result_round_trips_complete_canonical_content() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        py.run(
            c_str!(
                r#"
source = """
model result_decay {
  field x: 1 = 1;
  parameter rate: 1 / s = 1;
  relation flow continuous {
    derivative(x) + rate * x = 0;
  }
}
"""

model = eqiora.compile(source=source)
field = model.field(model.field_ids[0])
plan = eqiora.resolve(
    model,
    temporal=eqiora.time.Tsitouras45(
        initial_step_s=0.01,
        relative_tolerance=1.0e-9,
        absolute_tolerances={field: 1.0e-11},
    ),
)
result = eqiora.run(
    plan,
    state=eqiora.State.initial(plan),
    until_s=0.2,
    output_times_s=(0.1, 0.2),
)
result_bytes = result.to_bytes()
replayed = eqiora.Result.from_bytes(plan, result_bytes)
assert replayed.to_bytes() == result_bytes
assert replayed.plan_key == result.plan_key
assert replayed.elapsed_seconds == result.elapsed_seconds
assert replayed.series(field).values.numpy().tolist() == result.series(field).values.numpy().tolist()

try:
    eqiora.Result.from_bytes(plan, result_bytes + b"\n")
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("noncanonical Result bytes were admitted")

changed = eqiora.resolve(
    model,
    temporal=eqiora.time.Tsitouras45(
        initial_step_s=0.02,
        relative_tolerance=1.0e-8,
        absolute_tolerances={field: 2.0e-11},
    ),
)
try:
    eqiora.Result.from_bytes(changed, result_bytes)
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("Result bytes were crossed with a different Plan")
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
