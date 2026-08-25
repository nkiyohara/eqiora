use eqiora::artifact::GeometryMeshCorrespondenceEnvelopeV1;
use eqiora::geometry::CanonicalGeometryV1;
use eqiora::meshing::MeshQualityGate;
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyDict, PyDictMethods, PyModule};

#[test]
fn python_build_names_source_handles_and_feeds_source_owned_correspondence() -> PyResult<()> {
    Python::initialize();
    let canonical = Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        py.run(
            c_str!(
                r#"
Graph = eqiora.geometry.CadAuthoredGraph
channel = Graph.rectangle_extrusion(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    depth=1.0,
    modeling_tolerance=1e-10,
)
fluid = channel.circular_through_cut(
    center=(0.2, 0.2), radius=0.05, boolean_tolerance=1e-10
)
receipt = fluid.build()
assert type(receipt).__name__ == "CadAuthoredBuild"
assert receipt.graph_digest == fluid.graph_digest

named = {
    "fluid": channel.face_handle("end-cap"),
    "inlet": channel.face_handle("profile-x-lower"),
    "outlet": channel.face_handle("profile-x-upper"),
    "walls": (
        channel.face_handle("profile-y-lower"),
        channel.face_handle("profile-y-upper"),
    ),
    "cylinder": fluid.face_handle("cut-wall"),
}
geometry = fluid.build(named_topology=named)
assert type(geometry).__name__ == "Geometry"
assert geometry.classification_tolerance is None
assert geometry.selection("fluid").dimension == 2
assert geometry.selection("cylinder").dimension == 1
canonical = geometry.canonical_bytes

invalid_mappings = (
    [],
    {key: value for key, value in named.items() if key != "outlet"},
    {**named, "empty": ()},
    {**named, "duplicate": named["inlet"]},
    {**named, "walls": (*named["walls"], named["fluid"])},
    {**named, "fluid": "not a handle"},
)
for invalid in invalid_mappings:
    try:
        fluid.build(named_topology=invalid)
    except (TypeError, eqiora.ValidationError):
        pass
    else:
        raise AssertionError("invalid named topology reached Geometry publication")

foreign = Graph.rectangle_extrusion(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    depth=1.0,
    modeling_tolerance=2e-10,
)
for invalid_handle in (
    channel.face_handle("start-cap"),
    fluid.face_handle("profile-x-lower"),
    foreign.face_handle("profile-x-lower"),
):
    try:
        fluid.build(named_topology={**named, "inlet": invalid_handle})
    except eqiora.ValidationError:
        pass
    else:
        raise AssertionError("foreign, stale, or absent construction handle projected")

try:
    channel.build(named_topology=named)
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("a non-planar build published named Geometry")
"#
            ),
            None,
            Some(&locals),
        )?;
        locals
            .get_item("canonical")?
            .expect("the positive path must publish canonical Geometry")
            .extract::<Vec<u8>>()
    })?;

    let geometry = CanonicalGeometryV1::decode_planar_circular_hole_v2_canonical(
        &canonical,
        Default::default(),
    )
    .expect("the Python result must be the common canonical Geometry v2 owner");
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
            &geometry,
            1.0e-4,
            50,
            MeshQualityGate::new(1.0e-5).unwrap(),
        )
        .expect("the common Python Geometry must feed source-owned correspondence");
    correspondence
        .validate_against_planar_circular_hole_v2_reference(&geometry, &mesh, 1.0e-4, 50)
        .unwrap();
    assert!(
        !correspondence
            .planar_circular_hole_v2_entity_set_entities(&geometry, "fluid")
            .unwrap()
            .is_empty()
    );
    Ok(())
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
