use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleEntryV1, BundleRoleV1, ExactVersion,
    NormalizedRelativePath, QualifiedName, ResolutionRecordV1, SourceFileV1,
    prepare_package_release_v1,
};
use pyo3::ffi::c_str;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

const SOURCE: &str = r#"
public component PoissonRectangle {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  representation space = continuum;
  field potential on region as space: 1 = 0;
  relation balance continuous on region {
    -div(grad(potential))
      - source_scale * sin(wave_number * coordinate(0))
        * sin(wave_number * coordinate(1)) = 0;
  }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
  relation bottom_value continuous on bottom { trace(potential) = 0; }
  relation top_value continuous on top { trace(potential) = 0; }
}
"#;

static SCRATCH_SERIAL: AtomicU64 = AtomicU64::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn create() -> Self {
        let serial = SCRATCH_SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::var_os("HOME")
            .map(PathBuf::from)
            .expect("HOME-backed test scratch")
            .join(".cache/eqiora/python-package-geometry-tests")
            .join(format!("{}-{serial}", std::process::id()));
        fs::create_dir_all(&root).expect("create package test scratch");
        Self(root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn locked_store() -> (Scratch, Vec<u8>) {
    let path = NormalizedRelativePath::parse("src/poisson.eqi").expect("source path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.example.geometry_poisson").expect("package name"),
        ExactVersion::parse("1.0.0").expect("version"),
        vec![],
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .expect("manifest");
    let sources = AuthorPackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            SOURCE.as_bytes().to_vec(),
        )],
    )
    .expect("closed package sources");
    let release = prepare_package_release_v1(sources, &[]).expect("prepared package release");
    let resolution = ResolutionRecordV1::from_exact_releases(&release, &[])
        .expect("exact package resolution")
        .canonical_json()
        .expect("canonical resolution");
    let scratch = Scratch::create();
    let digest = release.source_digest().expect("source digest");
    fs::write(
        scratch.0.join(format!("{digest}.json")),
        release.canonical_json().expect("canonical release"),
    )
    .expect("publish exact release into selected store");
    (scratch, resolution)
}

#[test]
fn package_component_uses_caller_geometry_common_plan_and_run() -> PyResult<()> {
    let (store, resolution) = locked_store();
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let locals = PyDict::new(py);
        locals.set_item("package", native.bind(py))?;
        locals.set_item(
            "store",
            store.0.to_str().expect("Unicode HOME-backed scratch"),
        )?;
        locals.set_item("resolution", PyBytes::new(py, &resolution))?;
        py.run(
            c_str!(
                r#"
graph = package.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
geometry = graph.build(rectangle, named_topology={
    "region": rectangle.region,
    "left": rectangle.boundaries[0],
    "right": rectangle.boundaries[1],
    "bottom": rectangle.boundaries[2],
    "top": rectangle.boundaries[3],
})
model = package.compile_package(
    store,
    resolution,
    geometry=geometry,
    component="PoissonRectangle",
    parameters={"wave_number": 3.141592653589793, "source_scale": 19.739208802178716},
)
assert model.package_compilation_digest is not None
assert model.domain_ids

mesher = package.CartesianMesher(cells=(4, 4))
mesh_plan = package.resolve(geometry, mesher)
mesh = package.generate(geometry, plan=mesh_plan)
linear = package.Linear(
    relative_tolerance=1e-10,
    absolute_tolerance=1e-12,
    maximum_iterations=10000,
)
plan = package._resolve_plan(model, mesh=mesh, spatial=package.Q1(), solve=linear)
assert plan.model is model
assert plan.mesh is mesh
assert plan.package_compilation_digest == model.package_compilation_digest
run = package.submit_plan(plan)
assert run.package_compilation_digest == model.package_compilation_digest
result = run.result()
assert result.model_digest == model.digest

replayed = package.Model.from_bytes(model.to_bytes())
replayed_plan = package._resolve_plan(replayed, mesh=mesh, spatial=package.Q1(), solve=linear)
assert replayed_plan.identity == plan.identity
assert replayed_plan.package_compilation_digest is None

wide_rectangle = graph.rectangle(x_bounds=(0.0, 2.0), y_bounds=(0.0, 1.0))
wide_geometry = graph.build(wide_rectangle, named_topology={
    "region": wide_rectangle.region,
    "left": wide_rectangle.boundaries[0],
    "right": wide_rectangle.boundaries[1],
    "bottom": wide_rectangle.boundaries[2],
    "top": wide_rectangle.boundaries[3],
})
wide_mesh_plan = package.resolve(wide_geometry, mesher)
wide_mesh = package.generate(wide_geometry, plan=wide_mesh_plan)
try:
    package._resolve_plan(replayed, mesh=wide_mesh, spatial=package.Q1(), solve=linear)
except package.ValidationError:
    pass
else:
    raise AssertionError("replayed bound Model crossed into a foreign Geometry/Mesh lineage")

foreign_rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
foreign = graph.build(foreign_rectangle, named_topology={
    "other": foreign_rectangle.region,
    "left": foreign_rectangle.boundaries[0],
    "right": foreign_rectangle.boundaries[1],
    "bottom": foreign_rectangle.boundaries[2],
    "top": foreign_rectangle.boundaries[3],
})
try:
    package.compile_package(
        store,
        resolution,
        geometry=foreign,
        component="PoissonRectangle",
        parameters={"wave_number": 3.141592653589793, "source_scale": 19.739208802178716},
    )
except package.ValidationError as error:
    assert "region" in error.diagnostics[0].message
else:
    raise AssertionError("equal bounds bypassed exact support-name binding")
"#
            ),
            None,
            Some(&locals),
        )
    })
}

#[test]
fn package_compile_argument_and_resolution_boundaries_fail_closed() -> PyResult<()> {
    let (store, resolution) = locked_store();
    Python::initialize();
    Python::attach(|py| {
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let locals = PyDict::new(py);
        locals.set_item("package", native.bind(py))?;
        locals.set_item("store", store.0.to_str().expect("Unicode scratch"))?;
        locals.set_item("resolution", PyBytes::new(py, &resolution))?;
        py.run(
            c_str!(
                r#"
graph = package.GeometryGraph()
rectangle = graph.rectangle(x_bounds=(0.0, 1.0), y_bounds=(0.0, 1.0))
geometry = graph.build(rectangle, named_topology={
    "region": rectangle.region,
    "left": rectangle.boundaries[0],
    "right": rectangle.boundaries[1],
    "bottom": rectangle.boundaries[2],
    "top": rectangle.boundaries[3],
})
for args, kwargs in (
    ((store, bytearray(resolution)), dict(geometry=geometry, component="PoissonRectangle")),
    ((store, resolution), dict(component="PoissonRectangle")),
    ((store, resolution), dict(geometry=geometry)),
):
    try:
        package.compile_package(*args, **kwargs)
    except TypeError:
        pass
    else:
        raise AssertionError("invalid package compile arguments were admitted")

try:
    package.compile_package(
        store,
        b" " + resolution,
        geometry=geometry,
        component="PoissonRectangle",
    )
except package.CompatibilityError:
    pass
else:
    raise AssertionError("noncanonical resolution bytes were admitted")
"#
            ),
            None,
            Some(&locals),
        )
    })
}
