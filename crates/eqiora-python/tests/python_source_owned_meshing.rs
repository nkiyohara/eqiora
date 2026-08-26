use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyModule};

#[test]
fn python_geometry_v2_uses_the_existing_source_owned_mesh_lifecycle() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item(
            "raw_msh",
            PyBytes::new(
                py,
                include_bytes!(
                    "../../../verify/fluid/flow-past-cylinder-mesh-family-private/references/primary-l0.msh"
                ),
            ),
        )?;
        py.run(
            c_str!(
                r#"
import json

def geometry(center=(0.2, 0.2)):
    graph = eqiora.geometry.GeometryGraph()
    rectangle = graph.rectangle(x_bounds=(0.0, 2.2), y_bounds=(0.0, 0.41))
    circle = graph.circle(center=center, radius=0.05)
    fluid = graph.subtract(rectangle, circle)
    return graph.build(fluid, named_topology={
        "fluid": fluid.region,
        "inlet": rectangle.boundaries[0],
        "outlet": rectangle.boundaries[1],
        "walls": rectangle.boundaries[2:4],
        "cylinder": circle.boundaries[0],
    })

reference_provider = eqiora.meshing.ReferenceMesher(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
request = eqiora.meshing.MeshRequest(reference_provider)
source = geometry()
plan = eqiora.meshing.resolve(source, request)
same_plan = eqiora.meshing.resolve(source, request)
assert plan.provider == reference_provider
assert plan.source_digest == source.digest == same_plan.source_digest
assert plan.boundary_facets == same_plan.boundary_facets
first = eqiora.meshing.generate(source, plan=plan)
second = eqiora.meshing.generate(source, plan=plan)
assert type(first).__name__ == type(second).__name__ == "Mesh"
assert first.source_digest == first.realized_geometry_digest == source.digest
assert first.digest == second.digest
assert first.correspondence_digest == second.correspondence_digest
assert plan.production_lineage_bytes == first.production_lineage_bytes == second.production_lineage_bytes
assert plan.production_lineage_digest == first.production_lineage_digest == second.production_lineage_digest
assert len(plan.production_lineage_digest) == 64
reference_lineage = json.loads(plan.production_lineage_bytes)
assert reference_lineage["provider"] == {
    "identity": "eqiora.reference-planar-circular-hole", "version": "1"
}
assert reference_lineage["effective_policy"] == {
    "maximum_boundary_error_m": 1e-4,
    "minimum_mean_ratio": 1e-5,
    "maximum_boundary_facets": 50,
}
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

gmsh_provider = eqiora.meshing.GmshMesher(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
gmsh_request = eqiora.meshing.MeshRequest(gmsh_provider)
gmsh_plan = eqiora.meshing.resolve(source, gmsh_request)
assert gmsh_plan.provider == gmsh_provider
assert gmsh_plan.boundary_facets == plan.boundary_facets <= 50
generated = eqiora.meshing.generate(source, plan=gmsh_plan)
assert generated.source_digest == generated.realized_geometry_digest == source.digest
assert generated.digest != first.digest
assert generated.correspondence_digest != first.correspondence_digest
assert gmsh_plan.production_lineage_bytes == generated.production_lineage_bytes
assert gmsh_plan.production_lineage_digest == generated.production_lineage_digest
assert gmsh_plan.production_lineage_digest != plan.production_lineage_digest
gmsh_lineage = json.loads(gmsh_plan.production_lineage_bytes)
assert gmsh_lineage["provider"] == {"identity": "eqiora.gmsh-cli", "version": "4.15.2"}
assert gmsh_lineage["effective_policy"] == reference_lineage["effective_policy"]
assert generated.selection_names == source.selection_names
assert generated.selection_entity_count(source.selection("fluid")) == generated.cell_count
assert generated.selection_entity_count(source.selection("cylinder")) == gmsh_plan.boundary_facets

legacy = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    depth=1.0,
    modeling_tolerance=1e-10,
).circular_through_cut(
    center=(0.2, 0.2), radius=0.05, boolean_tolerance=1e-10
).planar_circular_section(
    classification_tolerance=1e-12,
    region="fluid",
    x_lower="inlet",
    x_upper="outlet",
    y_lower="walls",
    y_upper="walls",
    hole="cylinder",
)
try:
    eqiora.meshing.resolve(legacy, gmsh_request)
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("GmshMesher silently fell back to classification-bearing Geometry v1")

imported = eqiora.meshing.import_gmsh(
    legacy,
    raw_msh,
    policy=eqiora.meshing.GmshImport(
        maximum_boundary_error=4e-3,
        minimum_mean_ratio=1e-5,
        maximum_boundary_facets=8,
    ),
)
assert imported.external_import_manifest_bytes is not None
assert imported.external_import_manifest_digest is not None

for invalid_provider in (
    lambda: eqiora.meshing.GmshMesher(maximum_boundary_error=0.0),
    lambda: eqiora.meshing.ReferenceMesher(minimum_mean_ratio=0.0),
    lambda: eqiora.meshing.GmshMesher(maximum_boundary_facets=7),
):
    try:
        invalid_provider()
    except eqiora.ValidationError:
        pass
    else:
        raise AssertionError("provider-local numerical policy accepted an invalid value")

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
