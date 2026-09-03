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
use pyo3::types::{PyBytes, PyDict, PyModule};

const SOURCE: &str = r#"
public property contract Diffusivity { scalar value: 1; }
public property release ReferenceDiffusivity implements Diffusivity {
  value = 25;
  source_unit: 1 = 1 / 1000;
  validity = unconditional;
  citation = org.example.measurement;
  license = spdx.CC0_1_0;
}

public component PoissonLaw {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  public property diffusivity: Diffusivity;
  representation space = continuum;
  field potential on region as space: 1 = 0;
  relation balance continuous on region {
    -div(diffusivity * grad(potential))
      - source_scale * math.sin(wave_number * coordinate(0))
        * math.sin(wave_number * coordinate(1)) = 0;
  }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
  relation bottom_value continuous on bottom { trace(potential) = 0; }
  relation top_value continuous on top { trace(potential) = 0; }
}

public component PoissonRectangle {
  public support region: volume(ambient_dimension = 2);
  public support left: boundary(parent = region);
  public support right: boundary(parent = region);
  public support bottom: boundary(parent = region);
  public support top: boundary(parent = region);
  public parameter wave_number: 1 / m;
  public parameter source_scale: 1 / m ^ 2;
  instance equation: PoissonLaw(
    support region = region,
    support left = left,
    support right = right,
    support bottom = bottom,
    support top = top,
    wave_number = wave_number,
    source_scale = source_scale,
    property diffusivity = ReferenceDiffusivity
  );
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

fn locked_store(source: &str) -> (Scratch, Vec<u8>, String) {
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
            source.as_bytes().to_vec(),
        )],
    )
    .expect("closed package sources");
    let release = prepare_package_release_v1(sources, &[]).expect("prepared package release");
    let identity = release.package_identity().expect("exact package identity");
    let namespace = format!(
        "{}/{}/{}",
        identity.name,
        identity.version,
        identity.semantic_digest.to_hex()
    );
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
    (scratch, resolution, namespace)
}

#[test]
fn local_package_project_locks_and_compiles_a_model_through_python() -> PyResult<()> {
    let scratch = Scratch::create();
    let package_root = scratch.0.join("root");
    let store_root = scratch.0.join("store");
    fs::create_dir_all(package_root.join("src")).expect("create package source directory");
    fs::create_dir(&store_root).expect("create package store");
    let path = NormalizedRelativePath::parse("src/main.eqi").expect("source path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.example.LocalRoot").expect("package name"),
        ExactVersion::parse("1.0.0").expect("version"),
        vec![],
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .expect("manifest");
    fs::write(
        package_root.join("package.json"),
        manifest.canonical_json().expect("canonical manifest"),
    )
    .expect("write package manifest");
    fs::write(
        package_root.join(path.as_str()),
        "public model Main { parameter gain: 1 = 2; relation law continuous { gain - 2 = 0; } }",
    )
    .expect("write package source");
    fs::write(
        scratch.0.join("eqiora.toml"),
        "schema = \"eqiora.project.v1\"\nroot = \"root\"\n\n[dependencies]\n\n[sources.root]\npath = \"root\"\n",
    )
    .expect("write project manifest");

    Python::initialize();
    Python::attach(|py| {
        let public = public_module(py)?;
        let locals = PyDict::new(py);
        locals.set_item("eqiora", public)?;
        locals.set_item("project", scratch.0.to_string_lossy())?;
        locals.set_item("store", store_root.to_string_lossy())?;
        py.run(
            c_str!(
                r#"
resolution = eqiora.resolve_local_project(project, store)
assert type(resolution) is bytes
assert open(project + "/eqiora.lock", "rb").read() == resolution
model = eqiora.compile_package(store, resolution, entry_model="Main")
assert model.package_compilation_digest is not None
assert len(model.parameter_ids) == 1
"#
            ),
            None,
            Some(&locals),
        )
    })
}

#[test]
fn package_component_uses_caller_geometry_common_plan_and_run() -> PyResult<()> {
    Python::initialize();
    Python::attach(|py| {
        let public = public_module(py)?;
        let author_locals = PyDict::new(py);
        author_locals.set_item("eqiora", public)?;
        py.run(
            c_str!(
                r#"
q = eqiora.lang
u = q.units
source = q.Source()
contract = source.scalar_property_contract("Diffusivity", unit=u.one)
release = source.scalar_property_release(
    "ReferenceDiffusivity",
    implements=contract,
    value=25,
    source_unit=u.one,
    source_scale=0.001,
    citation="org.example.measurement",
    license="spdx.CC0_1_0",
)
law = source.component("PoissonLaw")
region = law.volume("region", dimensions=2)
left = law.boundary("left", parent=region)
right = law.boundary("right", parent=region)
bottom = law.boundary("bottom", parent=region)
top = law.boundary("top", parent=region)
source_scale = law.parameter("source_scale", unit=u.one / u.m**2)
diffusivity = law.property("diffusivity", contract=contract)
potential = law.field("potential", on=region, unit=u.one, initial=0)
law.relation(
    "balance",
    on=region,
    residual=-q.div(diffusivity * q.grad(potential)) - source_scale,
)
law.relation("left_value", on=left, residual=q.trace(potential))
law.relation("right_value", on=right, residual=q.trace(potential))
law.relation("bottom_value", on=bottom, residual=q.trace(potential))
law.relation("top_value", on=top, residual=q.trace(potential))

root = source.component("PoissonRectangle")
root_region = root.volume("region", dimensions=2)
root_left = root.boundary("left", parent=root_region)
root_right = root.boundary("right", parent=root_region)
root_bottom = root.boundary("bottom", parent=root_region)
root_top = root.boundary("top", parent=root_region)
root_source_scale = root.parameter("source_scale", unit=u.one / u.m**2)
root.instance(
    "equation",
    component=law,
    supports={
        region: root_region,
        left: root_left,
        right: root_right,
        bottom: root_bottom,
        top: root_top,
    },
    parameters={source_scale: root_source_scale},
    properties={diffusivity: release},
)
authored_source = source.to_eqi()
"#
            ),
            None,
            Some(&author_locals),
        )?;
        let authored_source = author_locals
            .get_item("authored_source")?
            .expect("authored source")
            .extract::<String>()?;
        let (store, resolution, namespace) = locked_store(&authored_source);
        let native = pyo3::wrap_pymodule!(_eqiora::_eqiora)(py);
        let locals = PyDict::new(py);
        locals.set_item("package", native.bind(py))?;
        locals.set_item(
            "store",
            store.0.to_str().expect("Unicode HOME-backed scratch"),
        )?;
        locals.set_item("resolution", PyBytes::new(py, &resolution))?;
        locals.set_item("namespace", namespace)?;
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
    parameters={"source_scale": 1.0},
)
assert model.package_compilation_digest is not None
assert model.domain_ids
assert isinstance(model.property_bindings, tuple)
assert len(model.property_bindings) == 1
binding = model.property_bindings[0]
assert binding.composition is None
assert binding.contract == f"{namespace}::Diffusivity"
assert binding.release == f"{namespace}::ReferenceDiffusivity"
assert binding.component == f"{namespace}::PoissonLaw"
assert binding.requirement == "diffusivity"
assert binding.normalized_value == 0.025
assert binding.validity == "unconditional"
assert binding.citation == "org.example.measurement"
assert binding.license == "spdx.CC0_1_0"
try:
    package.PropertyBinding()
except TypeError:
    pass
else:
    raise AssertionError("Python forged a package-owned property binding")
try:
    binding.citation = "org.example.rewritten"
except AttributeError:
    pass
else:
    raise AssertionError("Python mutated a package-owned property binding")

mesher = package.CartesianMesher(cells=(4, 4))
mesh_plan = package.resolve(geometry, mesher)
mesh = package.generate(mesh_plan)
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
assert model.property_bindings[0].citation == "org.example.measurement"

replayed = package.Model.from_bytes(model.to_bytes())
assert replayed.property_bindings == ()
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
wide_mesh = package.generate(wide_mesh_plan)
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
        parameters={"source_scale": 1.0},
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
    let (store, resolution, _) = locked_store(SOURCE);
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
    ((store, resolution), dict(entry_model="Main", geometry=geometry, component="PoissonRectangle")),
    ((store, resolution), dict(entry_model="Main", parameters={})),
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
