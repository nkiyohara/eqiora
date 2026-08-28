use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyModule};

#[test]
fn python_rectangle_cartesian_meshing_owns_exact_common_resources() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let module = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", module)?;
        py.run(
            c_str!(
                r#"
import json
import eqiora

def rectangle(xmax=2.0):
    import eqiora
    graph = eqiora.geometry.GeometryGraph()
    source = graph.rectangle(x_bounds=(0.0, xmax), y_bounds=(-1.0, 2.0))
    return graph.build(source, named_topology={
        "region": source.region,
        "left": source.boundaries[0],
        "right": source.boundaries[1],
        "bottom": source.boundaries[2],
        "top": source.boundaries[3],
    })

provider = eqiora.meshing.CartesianMesher(cells=(2, 3))
assert provider.cells == (2, 3)
assert repr(provider) == "CartesianMesher(cells=(2, 3))"
request = provider
source = rectangle()
plan = eqiora.meshing.resolve(source, request)
assert plan.provider == provider
assert plan.boundary_facets == 10
try:
    plan.achieved_minimum_mean_ratio
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("Cartesian plan fabricated simplex quality")

mesh = eqiora.meshing.generate(source, plan=plan)
assert mesh.dimension == 2
assert mesh.vertex_count == 12
assert mesh.cell_count == 6
assert mesh.coordinates.shape == (12, 2)
assert mesh.cells.shape == (6, 4)
assert mesh.source_digest == mesh.realized_geometry_digest == source.digest
assert mesh.selection_entity_count("region") == 6
assert mesh.selection_entity_count("left") == 3
assert mesh.selection_entity_count("right") == 3
assert mesh.selection_entity_count("bottom") == 2
assert mesh.selection_entity_count("top") == 2
assert mesh.production_lineage_bytes == plan.production_lineage_bytes
lineage = json.loads(mesh.production_lineage_bytes)
assert lineage["provider"] == {
    "identity": "eqiora.structured-cartesian", "version": "1"
}
assert lineage["effective_policy"] == {
    "kind": "cartesian-cells", "cells": [2, 3]
}

for cells in ((0, 3), (2, 0), (2,), (True, 3)):
    try:
        eqiora.meshing.CartesianMesher(cells=cells)
    except (eqiora.ValidationError, TypeError):
        pass
    else:
        raise AssertionError("invalid Cartesian policy was admitted")

try:
    eqiora.meshing.generate(rectangle(3.0), plan=plan)
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("foreign Geometry replay was admitted")

graph = eqiora.geometry.GeometryGraph()
outer = graph.rectangle(x_bounds=(0.0, 2.0), y_bounds=(-1.0, 2.0))
hole = graph.circle(center=(1.0, 0.0), radius=0.1)
cut = graph.subtract(outer, hole)
not_rectangle = graph.build(cut, named_topology={
    "region": cut.region,
    "left": outer.boundaries[0],
    "right": outer.boundaries[1],
    "walls": outer.boundaries[2:4],
    "hole": hole.boundaries[0],
})
try:
    eqiora.meshing.resolve(not_rectangle, request)
except eqiora.ValidationError:
    pass
else:
    raise AssertionError("non-rectangle Geometry reached Cartesian production")
"#
            ),
            None,
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
    py.import("eqiora")
}
