use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

#[test]
fn python_geometry_v2_uses_only_the_gmsh_product_path() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        py.run(
            c_str!(
                r#"
graph = eqiora.geometry.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
circle = graph.circle(center=(0.2, 0.2), radius=0.05)
fluid = graph.subtract(rectangle, circle)
geometry = graph.build(fluid, named_topology={
    "fluid": fluid.region,
    "inlet": rectangle.boundaries[0],
    "outlet": rectangle.boundaries[1],
    "walls": rectangle.boundaries[2:],
    "cylinder": circle.boundaries[0],
})
provider = eqiora.meshing.GmshMesher(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
plan = eqiora.meshing.resolve(geometry, provider)
assert plan.provider == provider
assert provider.maximum_target_size is None
assert plan.source_digest == geometry.digest
assert plan.boundary_facets <= provider.maximum_boundary_facets
assert not hasattr(plan, "production_lineage_digest")
assert not hasattr(eqiora.meshing, "ReferenceMesher")
assert not hasattr(eqiora.meshing, "GmshImport")
assert not hasattr(eqiora.meshing, "import_gmsh")

for invalid in (
    lambda: eqiora.meshing.GmshMesher(maximum_boundary_error=0.0),
    lambda: eqiora.meshing.GmshMesher(minimum_mean_ratio=0.0),
    lambda: eqiora.meshing.GmshMesher(maximum_boundary_facets=7),
    lambda: eqiora.meshing.GmshMesher(maximum_target_size=0.0),
    lambda: eqiora.meshing.GmshMesher(maximum_target_size=float("nan")),
):
    try:
        invalid()
    except eqiora.ValidationError:
        pass
    else:
        raise AssertionError("Gmsh policy accepted an invalid value")
"#
            ),
            Some(&locals),
            Some(&locals),
        )
    })
}

fn public_module(py: Python<'_>) -> PyResult<Bound<'_, PyModule>> {
    let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
    let package_directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
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
        .expect("public package must load")
        .cast_into::<PyModule>()?)
}
