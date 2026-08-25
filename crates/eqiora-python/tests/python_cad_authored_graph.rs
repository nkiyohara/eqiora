use eqiora::geometry::{CadAuthoredGraph, ConstrainedRectangleV1};
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyBytes, PyDict, PyDictMethods, PyModule};

const V2_WIRE: &[u8] = br#"{"schema":"eqiora.cad-authored-operation-graph-envelope/v2","encoding":"eqiora.canonical-json/v1","length_unit":"metre","requested_modeling_tolerance_m":1e-10,"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.0},"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle","sketch_plane":"sketch-plane","constraint":"closed-by-construction","x_bounds_m":[-0.04,0.04],"y_bounds_m":[-0.025,0.025]},"face":{"id":"profile-face","kind":"one-closed-loop-face","profile":"rectangle-profile","region_count":1},"extrusion":{"id":"positive-z-extrusion","kind":"positive-z","face":"profile-face","depth_m":0.02,"repair":"none"},"cut_sketch_plane":{"id":"cut-sketch-plane","kind":"on-face","face":"end-cap"},"cut_profile":{"id":"circle-profile","kind":"circle","sketch_plane":"cut-sketch-plane","constraint":"closed-by-construction","center_m":[0.02,0.0],"radius_m":0.008},"cut_face":{"id":"cut-profile-face","kind":"one-closed-loop-face","profile":"circle-profile","region_count":1},"cut":{"id":"circular-through-cut","kind":"difference-through-all-negative-z","target":"positive-z-extrusion","tool_face":"cut-profile-face","requested_tolerance_m":1e-9,"repair":"none"},"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper","profile-y-lower","profile-y-upper","cut-wall"]}"#;
const V2_DIGEST: &str = "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47";
const DFG_SECTION_WIRE: &[u8] = br#"{"schema":"eqiora.planar-circular-hole-envelope/v1","encoding":"eqiora.canonical-json/v1","kind":"axis-aligned-rectangle-with-circular-hole-v1","length_unit":"metre","tolerance_m":1e-12,"bounds":[[0.0,2.2],[0.0,0.41]],"circle":{"center":[0.2,0.2],"radius_m":0.05},"entity_sets":[{"name":"cylinder","dimension":1,"members":[4]},{"name":"inlet","dimension":1,"members":[0]},{"name":"outlet","dimension":1,"members":[1]},{"name":"walls","dimension":1,"members":[2,3]},{"name":"fluid","dimension":2,"members":[0]}]}"#;
const DFG_SECTION_DIGEST: &str = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9";

#[test]
fn python_projection_is_byte_transparent_to_the_public_rust_owner() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item("expected_wire", PyBytes::new(py, V2_WIRE))?;
        locals.set_item("expected_digest", V2_DIGEST)?;
        locals.set_item("expected_section_wire", PyBytes::new(py, DFG_SECTION_WIRE))?;
        locals.set_item("expected_section_digest", DFG_SECTION_DIGEST)?;
        py.run(
            c_str!(
                r#"
base = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
    x_bounds=(-0.04, 0.04),
    y_bounds=(-0.025, 0.025),
    plane_z=0.0,
    depth=0.02,
    modeling_tolerance=1e-10,
)
graph = base.circular_through_cut(
    center=(0.02, 0.0), radius=0.008, boolean_tolerance=1e-9
)
assert graph.canonical_bytes == expected_wire
assert graph.graph_digest == expected_digest
assert not hasattr(graph, "geometry_digest")
replayed = eqiora.geometry.CadAuthoredGraph.decode_canonical(graph.canonical_bytes)
assert replayed == graph

handle = graph.face_handle("cut-wall")
assert handle.provenance_key == "cut-wall"
assert graph.resolve_face(handle) == "cut-wall"
assert graph.face_boundary_loop_count(handle) == 2
assert graph.rectangular_face_vertices(handle) is None

build = graph.build()
assert build.graph_digest == graph.graph_digest
assert build.provider_profile == "eqiora.cad.analytic-circular-through-cut-v1"
assert tuple(item.provenance_key for item in build.retained_modified) == (
    "start-cap", "end-cap"
)
assert tuple(item.provenance_key for item in build.created) == ("cut-wall",)
assert build.deleted == build.split == build.merged == ()

changed = base.circular_through_cut(
    center=(0.02, 0.0), radius=0.008, boolean_tolerance=2e-9
)
try:
    changed.resolve_face(handle)
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("a stale graph-bound handle rebound through Python")

try:
    base.planar_circular_section(
        classification_tolerance=1e-12,
        region="fluid",
        x_lower="inlet",
        x_upper="outlet",
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("a rectangle-only graph produced a circular section")

channel = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    plane_z=0.0,
    depth=1.0,
    modeling_tolerance=1e-10,
)
channel_handles = {
    name: channel.face_handle(name)
    for name in (
        "end-cap",
        "profile-x-lower",
        "profile-x-upper",
        "profile-y-lower",
        "profile-y-upper",
    )
}
dfg_graph = channel.circular_through_cut(
    center=(0.2, 0.2), radius=0.05, boolean_tolerance=1e-10
)
section = dfg_graph.planar_circular_section(
    classification_tolerance=1e-12,
    region="fluid",
    x_lower="inlet",
    x_upper="outlet",
    y_lower="walls",
    y_upper="walls",
    hole="cylinder",
)
assert type(section).__name__ == "Geometry"
assert section.canonical_bytes == expected_section_wire
assert section.digest == expected_section_digest
assert section.selection_names == ("cylinder", "inlet", "outlet", "walls", "fluid")

result = dfg_graph.planar_result()
named_topology = {
    "fluid": result.project(channel_handles["end-cap"]),
    "inlet": result.project(channel_handles["profile-x-lower"]),
    "outlet": result.project(channel_handles["profile-x-upper"]),
    "walls": (
        result.project(channel_handles["profile-y-lower"]),
        result.project(channel_handles["profile-y-upper"]),
    ),
    "cylinder": result.project(dfg_graph.face_handle("cut-wall")),
}
common_section = result.with_named_topology(named_topology)
assert common_section.classification_tolerance is None
assert common_section.selection("fluid").dimension == 2
assert common_section.selection("cylinder").dimension == 1
request = eqiora.meshing.MeshRequest(
    maximum_boundary_error=1e-4,
    minimum_mean_ratio=1e-5,
    maximum_boundary_facets=50,
)
try:
    eqiora.meshing.resolve(common_section, request)
except eqiora.ValidationError as error:
    assert "source-owned mesh correspondence" in str(error)
else:
    raise AssertionError("Geometry v2 reached v1 mesh realization")
assert hash(named_topology["inlet"]) == hash(result.project(channel_handles["profile-x-lower"]))
try:
    hash(result)
except TypeError:
    pass
else:
    raise AssertionError("value-equal planar results must remain explicitly unhashable")

arbitrary_names = dict(named_topology)
arbitrary_names["left boundary"] = arbitrary_names.pop("inlet")
renamed = result.with_named_topology(arbitrary_names)
assert "left boundary" in renamed.selection_names
assert "inlet" not in renamed.selection_names

for invalid in (
    [],
    {**named_topology, "empty": ()},
    {key: value for key, value in named_topology.items() if key != "outlet"},
    {**named_topology, "fluid": dfg_graph.face_handle("start-cap")},
    {**named_topology, "walls": (named_topology["walls"],)},
):
    try:
        result.with_named_topology(invalid)
    except (TypeError, eqiora.ValidationError):
        pass
    else:
        raise AssertionError("invalid topology mapping reached Geometry publication")

try:
    result.project(dfg_graph.face_handle("start-cap"))
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("deleted construction topology reached result publication")

for stale in (
    dfg_graph.face_handle("profile-x-lower"),
    eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(
        x_bounds=(0.0, 2.2),
        y_bounds=(0.0, 0.41),
        plane_z=0.0,
        depth=1.0,
        modeling_tolerance=2e-10,
    ).face_handle("profile-x-lower"),
):
    try:
        result.project(stale)
    except eqiora.ValidationError:
        pass
    else:
        raise AssertionError("foreign or wrong-generation predecessor handle projected")

canonical = graph.canonical_bytes
digest = graph.graph_digest
"#
            ),
            Some(&locals),
            Some(&locals),
        )?;

        let canonical = locals
            .get_item("canonical")?
            .expect("Python graph must expose canonical bytes")
            .extract::<Vec<u8>>()?;
        let digest = locals
            .get_item("digest")?
            .expect("Python graph must expose graph identity")
            .extract::<String>()?;
        let replayed = CadAuthoredGraph::decode_canonical(&canonical)
            .expect("Python bytes must replay through the public Rust owner");
        assert_eq!(replayed.canonical_bytes(), V2_WIRE);
        assert_eq!(digest, V2_DIGEST);
        Ok(())
    })
}

#[test]
fn python_tau_only_witness_does_not_mislabel_graph_identity_as_geometry() -> PyResult<()> {
    let first = CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((-2.0, 3.0), (-1.0, 2.0), 0.5).unwrap(),
        4.0,
        1.0e-9,
    )
    .unwrap();
    let changed = CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((-2.0, 3.0), (-1.0, 2.0), 0.5).unwrap(),
        4.0,
        2.0e-9,
    )
    .unwrap();
    assert_eq!(first.output(), changed.output());
    assert_eq!(first.volume_m3(), changed.volume_m3());
    assert_ne!(first.digest_bytes(), changed.digest_bytes());

    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let graph_type = module.getattr("geometry")?.getattr("CadAuthoredGraph")?;
        assert!(!graph_type.hasattr("geometry_digest")?);
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
