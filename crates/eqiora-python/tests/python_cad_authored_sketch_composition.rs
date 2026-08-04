use eqiora::geometry::{CadAuthoredGraph, CadAuthoredSketch, ConstrainedRectangleV1};
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyAnyMethods, PyBytes, PyDict, PyDictMethods, PyModule};

const V1_WIRE: &[u8] = br#"{"schema":"eqiora.cad-authored-operation-graph-envelope/v1","encoding":"eqiora.canonical-json/v1","length_unit":"metre","requested_modeling_tolerance_m":1e-9,"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.5},"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle","sketch_plane":"sketch-plane","constraint":"closed-by-construction","x_bounds_m":[-2.0,3.0],"y_bounds_m":[-1.0,2.0]},"face":{"id":"profile-face","kind":"one-closed-loop-face","profile":"rectangle-profile","region_count":1},"extrusion":{"id":"positive-z-extrusion","kind":"positive-z","face":"profile-face","depth_m":4.0,"repair":"none"},"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper","profile-y-lower","profile-y-upper"]}"#;
const V1_DIGEST: &str = "919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36";
const V2_WIRE: &[u8] = br#"{"schema":"eqiora.cad-authored-operation-graph-envelope/v2","encoding":"eqiora.canonical-json/v1","length_unit":"metre","requested_modeling_tolerance_m":1e-10,"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.0},"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle","sketch_plane":"sketch-plane","constraint":"closed-by-construction","x_bounds_m":[-0.04,0.04],"y_bounds_m":[-0.025,0.025]},"face":{"id":"profile-face","kind":"one-closed-loop-face","profile":"rectangle-profile","region_count":1},"extrusion":{"id":"positive-z-extrusion","kind":"positive-z","face":"profile-face","depth_m":0.02,"repair":"none"},"cut_sketch_plane":{"id":"cut-sketch-plane","kind":"on-face","face":"end-cap"},"cut_profile":{"id":"circle-profile","kind":"circle","sketch_plane":"cut-sketch-plane","constraint":"closed-by-construction","center_m":[0.02,0.0],"radius_m":0.008},"cut_face":{"id":"cut-profile-face","kind":"one-closed-loop-face","profile":"circle-profile","region_count":1},"cut":{"id":"circular-through-cut","kind":"difference-through-all-negative-z","target":"positive-z-extrusion","tool_face":"cut-profile-face","requested_tolerance_m":1e-9,"repair":"none"},"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper","profile-y-lower","profile-y-upper","cut-wall"]}"#;
const V2_DIGEST: &str = "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47";
const DFG_SECTION_DIGEST: &str = "b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9";

#[test]
fn python_explicit_composition_replays_the_public_rust_authorities() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let native_base = CadAuthoredSketch::rectangle_xy(
            ConstrainedRectangleV1::new((-0.04, 0.04), (-0.025, 0.025), 0.0).unwrap(),
            1.0e-10,
        )
        .unwrap()
        .extrude_positive_z(0.02)
        .unwrap();
        let native_circle = CadAuthoredSketch::circle_on_face(
            native_base.face_handle("end-cap").unwrap(),
            [0.02, 0.0],
            0.008,
        )
        .unwrap();
        let native_cut = native_base.through_cut(&native_circle, 1.0e-9).unwrap();
        let native_rejection_messages = vec![
            native_circle
                .extrude_positive_z(0.02)
                .unwrap_err()
                .message()
                .to_owned(),
            native_base
                .through_cut(&native_circle, 0.0)
                .unwrap_err()
                .message()
                .to_owned(),
            native_cut
                .through_cut(&native_circle, 1.0e-9)
                .unwrap_err()
                .message()
                .to_owned(),
        ];
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        locals.set_item("v1_wire", PyBytes::new(py, V1_WIRE))?;
        locals.set_item("v1_digest", V1_DIGEST)?;
        locals.set_item("v2_wire", PyBytes::new(py, V2_WIRE))?;
        locals.set_item("v2_digest", V2_DIGEST)?;
        locals.set_item("dfg_section_digest", DFG_SECTION_DIGEST)?;
        locals.set_item("native_rejection_messages", native_rejection_messages)?;
        py.run(
            c_str!(
                r#"
Sketch = eqiora.geometry.CadAuthoredSketch
Graph = eqiora.geometry.CadAuthoredGraph

rectangle = Sketch.rectangle_xy(
    x_bounds=(-2.0, 3.0),
    y_bounds=(-1.0, 2.0),
    plane_z=0.5,
    modeling_tolerance=1e-9,
)
explicit_v1 = rectangle.extrude_positive_z(depth=4.0)
reused_v1 = rectangle.extrude_positive_z(depth=4.0)
compatibility_v1 = Graph.rectangle_extrusion(
    x_bounds=(-2.0, 3.0),
    y_bounds=(-1.0, 2.0),
    plane_z=0.5,
    depth=4.0,
    modeling_tolerance=1e-9,
)
assert explicit_v1 == reused_v1 == compatibility_v1
assert explicit_v1.canonical_bytes == v1_wire
assert explicit_v1.graph_digest == v1_digest

def four_routes(*, x_bounds, y_bounds, depth, center, radius, boolean_tolerance):
    explicit_base = Sketch.rectangle_xy(
        x_bounds=x_bounds,
        y_bounds=y_bounds,
        plane_z=0.0,
        modeling_tolerance=1e-10,
    ).extrude_positive_z(depth=depth)
    compatibility_base = Graph.rectangle_extrusion(
        x_bounds=x_bounds,
        y_bounds=y_bounds,
        plane_z=0.0,
        depth=depth,
        modeling_tolerance=1e-10,
    )
    explicit_circle = Sketch.circle_on_face(
        explicit_base.face_handle("end-cap"), center=center, radius=radius
    )
    compatibility_circle = Sketch.circle_on_face(
        compatibility_base.face_handle("end-cap"), center=center, radius=radius
    )
    return (
        explicit_base.through_cut(
            explicit_circle, boolean_tolerance=boolean_tolerance
        ),
        compatibility_base.circular_through_cut(
            center=center, radius=radius, boolean_tolerance=boolean_tolerance
        ),
        explicit_base.circular_through_cut(
            center=center, radius=radius, boolean_tolerance=boolean_tolerance
        ),
        compatibility_base.through_cut(
            compatibility_circle, boolean_tolerance=boolean_tolerance
        ),
    )

v2_routes = four_routes(
    x_bounds=(-0.04, 0.04),
    y_bounds=(-0.025, 0.025),
    depth=0.02,
    center=(0.02, 0.0),
    radius=0.008,
    boolean_tolerance=1e-9,
)
assert all(graph == v2_routes[0] for graph in v2_routes)
assert all(graph.canonical_bytes == v2_wire for graph in v2_routes)
assert all(graph.graph_digest == v2_digest for graph in v2_routes)
assert Graph.decode_canonical(v2_routes[0].canonical_bytes) == v2_routes[0]

for property_name in (
    "retained_unchanged",
    "retained_modified",
    "created",
    "deleted",
    "split",
    "merged",
):
    expected = getattr(v2_routes[1].build(), property_name)
    assert all(getattr(graph.build(), property_name) == expected for graph in v2_routes)

dfg_routes = four_routes(
    x_bounds=(0.0, 2.2),
    y_bounds=(0.0, 0.41),
    depth=1.0,
    center=(0.2, 0.2),
    radius=0.05,
    boolean_tolerance=1e-10,
)
assert all(graph == dfg_routes[0] for graph in dfg_routes)
sections = tuple(
    graph.planar_circular_section(
        classification_tolerance=1e-12,
        region="fluid",
        x_lower="inlet",
        x_upper="outlet",
        y_lower="walls",
        y_upper="walls",
        hole="cylinder",
    )
    for graph in dfg_routes
)
assert all(section == sections[0] for section in sections)
assert len(sections[0].canonical_bytes) == 511
assert sections[0].digest == dfg_section_digest

base = Sketch.rectangle_xy(
    x_bounds=(-0.04, 0.04),
    y_bounds=(-0.025, 0.025),
    plane_z=0.0,
    modeling_tolerance=1e-10,
).extrude_positive_z(depth=0.02)
circle = Sketch.circle_on_face(
    base.face_handle("end-cap"), center=(0.02, 0.0), radius=0.008
)
for operation, native_message in zip((
    lambda: circle.extrude_positive_z(depth=0.02),
    lambda: base.through_cut(circle, boolean_tolerance=0.0),
    lambda: v2_routes[0].through_cut(circle, boolean_tolerance=1e-9),
), native_rejection_messages, strict=True):
    try:
        operation()
    except eqiora.ValidationError as error:
        assert error.category == "validation"
        assert len(error.diagnostics) == 1
        diagnostic = error.diagnostics[0]
        assert diagnostic.source == "kernel"
        assert diagnostic.severity == "error"
        assert diagnostic.code == "EQ0901"
        assert diagnostic.message == native_message
    else:
        raise AssertionError("a frozen authored-sketch rejection returned a value")

python_v1 = explicit_v1.canonical_bytes
python_v2 = v2_routes[0].canonical_bytes
"#
            ),
            Some(&locals),
            Some(&locals),
        )?;

        for (name, expected) in [("python_v1", V1_WIRE), ("python_v2", V2_WIRE)] {
            let canonical = locals
                .get_item(name)?
                .expect("Python composition must expose canonical graph bytes")
                .extract::<Vec<u8>>()?;
            let replayed = CadAuthoredGraph::decode_canonical(&canonical)
                .expect("Python bytes must replay through the accepted Rust graph owner");
            assert_eq!(replayed.canonical_bytes(), expected);
        }
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
