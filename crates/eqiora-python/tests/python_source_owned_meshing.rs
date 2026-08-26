use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

#[test]
fn python_geometry_v2_uses_the_existing_source_owned_mesh_lifecycle() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        py.run(
            c_str!(
                r#"
def geometry(center=(0.2, 0.2)):
    Graph = eqiora.geometry.CadAuthoredGraph
    channel = Graph.rectangle_extrusion(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=1e-10,
    )
    fluid = channel.circular_through_cut(
        center=center, radius=0.05, boolean_tolerance=1e-10
    )
    return fluid.build(named_topology={
        "fluid": channel.face_handle("end-cap"),
        "inlet": channel.face_handle("profile-x-lower"),
        "outlet": channel.face_handle("profile-x-upper"),
        "walls": (
            channel.face_handle("profile-y-lower"),
            channel.face_handle("profile-y-upper"),
        ),
        "cylinder": fluid.face_handle("cut-wall"),
    })

request = eqiora.meshing.MeshRequest(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
source = geometry()
plan = eqiora.meshing.resolve(source, request)
same_plan = eqiora.meshing.resolve(source, request)
assert plan.provider == "eqiora.source-owned-planar-circular-hole-v2/1"
assert plan.source_digest == source.digest == same_plan.source_digest
assert plan.boundary_facets == same_plan.boundary_facets
for unavailable in (
    lambda: plan.canonical_bytes,
    lambda: plan.boundary_error_bound,
    lambda: plan.boundary_evaluation_allowance,
):
    try:
        unavailable()
    except eqiora.CapabilityError:
        pass
    else:
        raise AssertionError("Geometry v2 fabricated a legacy realization observation")

first = eqiora.meshing.generate(source, plan=plan)
second = eqiora.meshing.generate(source, plan=plan)
assert type(first).__name__ == type(second).__name__ == "Mesh"
assert first.source_digest == first.realized_geometry_digest == source.digest
assert first.digest == second.digest
assert first.correspondence_digest == second.correspondence_digest
assert first.canonical_bytes == second.canonical_bytes
assert first.selection_names == source.selection_names
assert first.selection_entity_count(source.selection("fluid")) == first.cell_count

boundary_count = sum(
    first.selection_entity_count(source.selection(name))
    for name in ("inlet", "outlet", "walls", "cylinder")
)
assert boundary_count > plan.boundary_facets
assert first.selection_entity_count(source.selection("cylinder")) == plan.boundary_facets

for mesh in (first, second):
    try:
        mesh.realization_digest
    except eqiora.CapabilityError:
        pass
    else:
        raise AssertionError("Geometry v2 fabricated a chordal realization identity")

bundle = first._repr_mimebundle_()
assert "text/plain" in bundle
assert "application/vnd.jupyter.widget-view+json" not in bundle
assert "Notebook view unavailable" in bundle["text/plain"]

foreign = geometry((0.21, 0.2))
for candidate_source, candidate_plan in ((foreign, plan), (source, eqiora.meshing.resolve(foreign, request))):
    try:
        eqiora.meshing.generate(candidate_source, plan=candidate_plan)
    except eqiora.ValidationError:
        pass
    else:
        raise AssertionError("a foreign Geometry/plan pair published a Mesh")

try:
    first.selection_entity_count(foreign.selection("cylinder"))
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("a stale or foreign GeometrySelection reached correspondence lookup")

try:
    first.selection_entity_count("unknown")
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("an unknown selection name resolved")
"#
            ),
            Some(&locals),
            Some(&locals),
        )?;
        Ok(())
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
    "eqiora", package_path / "__init__.py",
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
