use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use eqiora::kernel::BoundarySide;
use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleEntryV1, BundleRoleV1, DependencyRequirementV1,
    DirectoryPackageStore, ExactVersion, InMemoryPackageStore, NormalizedRelativePath,
    PackageReleaseV1, PackagedModelDocument, QualifiedName, ResolutionRecordV1, SourceFileV1,
    prepare_package_release_v1,
};
use eqiora_numerics::{
    common::{PhysicalBoundaryDisposition, PhysicalBoundaryQuantity},
    fluid::{
        SteadyIncompressibleStokesCartesianModel2d, lower_steady_incompressible_stokes_cartesian_2d,
    },
};

#[path = "support/embedded_package.rs"]
mod embedded_package;

const VERSION: &str = "0.1.0";
static NEXT_SCRATCH: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
enum Outlet {
    NoSlip,
    TractionFree,
    NormalPressure,
}

#[test]
fn curated_and_explicit_standard_stokes_share_the_package_neutral_path() {
    let (mechanics, fluid) = standard_releases();
    let curated = compile_root(&mechanics, &fluid, &root_source(true, Outlet::NoSlip));
    let explicit = compile_root(&mechanics, &fluid, &root_source(false, Outlet::NoSlip));
    let curated = lower(curated.model().program());
    let explicit = lower(explicit.model().program());

    assert_eq!(curated.bounds(), explicit.bounds());
    assert_eq!(curated.dynamic_viscosity(), explicit.dynamic_viscosity());
    assert_eq!(
        curated.force_potential_expression().evaluate(&[0.25, 0.75]),
        explicit
            .force_potential_expression()
            .evaluate(&[0.25, 0.75])
    );
    for (axis, side) in cartesian_sides() {
        assert_eq!(
            curated
                .boundary_inventory()
                .boundary(axis, side)
                .expect("curated boundary")
                .disposition(),
            PhysicalBoundaryDisposition::TraceZero
        );
        assert_eq!(
            explicit
                .boundary_inventory()
                .boundary(axis, side)
                .expect("explicit boundary")
                .disposition(),
            PhysicalBoundaryDisposition::TraceZero
        );
    }
}

#[test]
fn standard_outlets_retain_typed_traction_and_pressure_meaning() {
    let (mechanics, fluid) = standard_releases();
    let traction = compile_root(&mechanics, &fluid, &root_source(true, Outlet::TractionFree));
    let traction = lower(traction.model().program());
    assert_eq!(
        traction
            .boundary_inventory()
            .boundary(0, BoundarySide::Upper)
            .expect("traction outlet")
            .disposition(),
        PhysicalBoundaryDisposition::FluxZero
    );

    let pressure = compile_root(
        &mechanics,
        &fluid,
        &root_source(true, Outlet::NormalPressure),
    );
    let pressure = lower(pressure.model().program());
    let outlet = pressure
        .boundary_inventory()
        .boundary(0, BoundarySide::Upper)
        .expect("pressure outlet");
    assert!(matches!(
        outlet.disposition(),
        PhysicalBoundaryDisposition::Prescribed(law)
            if law.quantity() == PhysicalBoundaryQuantity::Flux
    ));
    let normal_pressure = pressure
        .normal_pressure(0, BoundarySide::Upper)
        .expect("recognized normal pressure");
    assert_eq!(
        normal_pressure.expression().evaluate(&[0.25, 0.75]),
        Ok(2.0)
    );
}

#[test]
fn standard_package_reopens_from_the_project_lock_and_offline_store() {
    let scratch = Scratch::create();
    let mechanics_sources =
        embedded_package::release_sources("Eqiora.Mechanics.Interfaces", "0.2.0");
    let mechanics =
        prepare_package_release_v1(mechanics_sources.clone(), &[]).expect("mechanics release");
    let fluid_sources = embedded_package::release_sources("Eqiora.Fluid", VERSION);
    let fluid = prepare_package_release_v1(fluid_sources.clone(), std::slice::from_ref(&mechanics))
        .expect("standard fluid release");
    let root_sources = root_sources(&fluid, &root_source(true, Outlet::NoSlip));

    write_package(&scratch.child("root"), &root_sources);
    write_package(&scratch.child("fluid"), &fluid_sources);
    write_package(&scratch.child("mechanics"), &mechanics_sources);
    let store_path = scratch.child("store");
    fs::write(
        scratch.0.join("eqiora.toml"),
        r#"schema = "eqiora.project.v1"
root = "root"

[dependencies]
fluid = "fluid"

[sources.root]
path = "root"

[sources.fluid]
path = "fluid"

[sources.mechanics]
path = "mechanics"
"#,
    )
    .expect("write project manifest");

    let resolution =
        PackagedModelDocument::resolve_local_package_project_v1(&scratch.0, &store_path)
            .expect("resolve standard package project");
    let lock_bytes = fs::read(scratch.0.join("eqiora.lock")).expect("read exact lock");
    assert_eq!(
        lock_bytes,
        resolution.canonical_json().expect("canonical resolution")
    );

    let reopened = ResolutionRecordV1::from_json(&lock_bytes).expect("reopen exact lock");
    let store = DirectoryPackageStore::open_ambient(store_path).expect("open offline store");
    let document = PackagedModelDocument::compile_locked(&store, &reopened, "Main")
        .expect("compile from exact offline store");
    assert_eq!(lower(document.model().program()).dynamic_viscosity(), 2.0);
}

fn standard_releases() -> (PackageReleaseV1, PackageReleaseV1) {
    let mechanics = prepare_package_release_v1(
        embedded_package::release_sources("Eqiora.Mechanics.Interfaces", "0.2.0"),
        &[],
    )
    .expect("mechanics release");
    let fluid = prepare_package_release_v1(
        embedded_package::release_sources("Eqiora.Fluid", VERSION),
        std::slice::from_ref(&mechanics),
    )
    .expect("standard fluid release");
    (mechanics, fluid)
}

fn compile_root(
    mechanics: &PackageReleaseV1,
    fluid: &PackageReleaseV1,
    source: &str,
) -> PackagedModelDocument {
    let root = prepare_package_release_v1(
        root_sources(fluid, source),
        &[fluid.clone(), mechanics.clone()],
    )
    .expect("standard fluid project root");
    let resolution =
        ResolutionRecordV1::from_exact_releases(&root, &[fluid.clone(), mechanics.clone()])
            .expect("standard fluid lock");
    let mut store = InMemoryPackageStore::default();
    store.insert(mechanics).expect("store mechanics");
    store.insert(fluid).expect("store fluid");
    store.insert(&root).expect("store root");
    PackagedModelDocument::compile_locked(&store, &resolution, "Main")
        .expect("compile standard fluid project")
}

fn root_sources(fluid: &PackageReleaseV1, source: &str) -> AuthorPackageSourcesV1 {
    let path = NormalizedRelativePath::parse("src/main.eqi").expect("root path");
    let dependency = DependencyRequirementV1::new(
        QualifiedName::parse("fluid").expect("fluid alias"),
        fluid.package_identity().expect("fluid identity"),
    )
    .expect("fluid dependency");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.example.StandardFluid").expect("root name"),
        ExactVersion::parse(VERSION).expect("root version"),
        vec![dependency],
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .expect("root manifest");
    AuthorPackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            source.as_bytes().to_vec(),
        )],
    )
    .expect("root sources")
}

fn lower(program: &eqiora::sem::KernelProgram) -> SteadyIncompressibleStokesCartesianModel2d {
    lower_steady_incompressible_stokes_cartesian_2d(program)
        .expect("standard package reaches common Stokes lowering")
}

fn cartesian_sides() -> [(usize, BoundarySide); 4] {
    [
        (0, BoundarySide::Lower),
        (0, BoundarySide::Upper),
        (1, BoundarySide::Lower),
        (1, BoundarySide::Upper),
    ]
}

fn write_package(path: &Path, sources: &AuthorPackageSourcesV1) {
    fs::create_dir_all(path).expect("create package directory");
    fs::write(
        path.join("package.json"),
        sources
            .manifest()
            .canonical_json()
            .expect("canonical package manifest"),
    )
    .expect("write package manifest");
    for file in sources.files() {
        let destination = path.join(file.path().as_str());
        fs::create_dir_all(destination.parent().expect("package file parent"))
            .expect("create package source directory");
        fs::write(destination, file.bytes()).expect("write package source");
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn create() -> Self {
        let sequence = NEXT_SCRATCH.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "eqiora-standard-fluid-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create scratch directory");
        Self(path)
    }

    fn child(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).expect("create scratch child");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove scratch directory");
    }
}

fn root_source(curated: bool, outlet: Outlet) -> String {
    let governing = if curated {
        r#"  instance governing: fluid.SteadyStokes2d(
    support body = body,
    support exterior = boundaries(x_lower, x_upper, y_lower, y_upper),
    field velocity = velocity,
    field pressure = pressure,
    field force_potential = force_potential,
    dynamic_viscosity = dynamic_viscosity
  );"#
    } else {
        r#"  instance balance: fluid.SteadyNewtonianBalance2d(
    support body = body,
    field velocity = velocity,
    field pressure = pressure,
    field force_potential = force_potential,
    dynamic_viscosity = dynamic_viscosity
  );
  instance interface: fluid.VelocityTractionInterface2d(
    support body = body,
    support exterior = boundaries(x_lower, x_upper, y_lower, y_upper),
    field velocity = velocity,
    field pressure = pressure,
    dynamic_viscosity = dynamic_viscosity
  );"#
    };
    let interface = if curated { "governing" } else { "interface" };
    let (outlet_field, outlet_instance) = match outlet {
        Outlet::NoSlip => (
            "",
            "  instance x_upper_condition: fluid.NoSlip2d(\n    support body = body, support face = x_upper\n  );",
        ),
        Outlet::TractionFree => (
            "",
            "  instance x_upper_condition: fluid.TractionFree2d(\n    support body = body, support face = x_upper\n  );",
        ),
        Outlet::NormalPressure => (
            r#"  field exterior_pressure on body as space: kg / (m * s ^ 2) = 0;
  parameter ambient_pressure: kg / (m * s ^ 2) = 2;
  relation exterior_pressure_definition continuous on body {
    exterior_pressure - ambient_pressure = 0;
  }
"#,
            r#"  instance x_upper_condition: fluid.NormalPressureOutlet2d(
    support body = body,
    support face = x_upper,
    field exterior_pressure = exterior_pressure
  );"#,
        ),
    };
    format!(
        r#"model Main {{
  domain body = box(0, 4, 0, 2);
  domain x_lower = boundary(body, axis = 0, side = lower);
  domain x_upper = boundary(body, axis = 0, side = upper);
  domain y_lower = boundary(body, axis = 1, side = lower);
  domain y_upper = boundary(body, axis = 1, side = upper);
  representation space = continuum;
  field velocity on body as space: m / s shape spatial_vector;
  field pressure on body as space: kg / (m * s ^ 2) = 0;
  field force_potential on body as space: kg / (m * s ^ 2) = 0;
  parameter dynamic_viscosity: kg / (m * s) = 2;
  parameter zero_pressure: kg / (m * s ^ 2) = 0;
  relation force_definition continuous on body {{
    force_potential - zero_pressure = 0;
  }}
{outlet_field}{governing}
  instance x_lower_condition: fluid.NoSlip2d(
    support body = body, support face = x_lower
  );
{outlet_instance}
  instance y_lower_condition: fluid.NoSlip2d(
    support body = body, support face = y_lower
  );
  instance y_upper_condition: fluid.NoSlip2d(
    support body = body, support face = y_upper
  );

  connect conserving {interface}.mechanical[boundary = x_lower],
    x_lower_condition.mechanical;
  connect conserving {interface}.mechanical[boundary = x_upper],
    x_upper_condition.mechanical;
  connect conserving {interface}.mechanical[boundary = y_lower],
    y_lower_condition.mechanical;
  connect conserving {interface}.mechanical[boundary = y_upper],
    y_upper_condition.mechanical;
}}
"#
    )
}
