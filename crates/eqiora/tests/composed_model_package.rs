use std::collections::BTreeSet;
use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use eqiora::Id;
use eqiora::entity::kinds;
use eqiora::graph::EdgeKind;
use eqiora::kernel::KernelNode;
use eqiora::package::{
    AuthorManifestV1, AuthorPackageDirectory, AuthorPackageSourcesV1, BundleRoleV1,
    DeclarationKindV1, DependencyRequirementV1, DirectoryPackageInstaller, DirectoryPackageStore,
    InMemoryPackageStore, PackageInstallDisposition, PackagePreparationError, PackageReleaseV1,
    PackageStageCleanup, PackagedModelDocument, ResolutionRecordV1, SourceFileV1, VisibilityV1,
    prepare_package_release_v1,
};
use eqiora::sem::PhysicalUnknown;
use eqiora::solver::{
    LinearSolveRequest, LinearSolver, PreconditionerPolicy, ReductionPolicy, SolverPlan,
};
use eqiora_backend_faer::FaerLinearSolver;
use eqiora_numerics::{
    scalar::lower_scalar_physical_affine, scalar::solve_scalar_physical_affine_with_initial_guess,
};

const PERMUTED_CIRCUITS_SOURCE: &str =
    include_str!("../../../verify/packages/composed-model-package/models/circuits-permuted.eqi");
const EXPECTED_IDENTITIES: &[u8] =
    include_bytes!("../../../verify/packages/composed-model-package/expected/identities.json");
const VALUE_TOLERANCE: f64 = 2.0e-11;
const RESIDUAL_TOLERANCE: f64 = 1.2e-11;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn create() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "eqiora-composed-package-{}-{nonce:x}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create composed-package store");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package_root(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages")
        .join(name)
}

fn sources(name: &str) -> AuthorPackageSourcesV1 {
    AuthorPackageDirectory::open_ambient(package_root(name))
        .expect("open explicit package root")
        .read_sources()
        .expect("read closed package inventory")
}

fn release(name: &str, dependencies: &[PackageReleaseV1]) -> PackageReleaseV1 {
    prepare_package_release_v1(sources(name), dependencies)
        .expect("prepare compiler-derived release")
}

fn replace_model_source(
    sources: &AuthorPackageSourcesV1,
    model_source: &str,
) -> AuthorPackageSourcesV1 {
    let files = sources
        .files()
        .iter()
        .map(|file| {
            if file.role() == BundleRoleV1::ModelSource {
                SourceFileV1::new(
                    file.path().clone(),
                    file.role(),
                    model_source.as_bytes().to_vec(),
                )
            } else {
                file.clone()
            }
        })
        .collect();
    AuthorPackageSourcesV1::new(sources.manifest().clone(), files)
        .expect("replace the one closed model source")
}

fn retarget_only_dependency(
    sources: &AuthorPackageSourcesV1,
    target: &PackageReleaseV1,
) -> AuthorPackageSourcesV1 {
    let manifest = sources.manifest();
    let requirement = manifest
        .dependencies()
        .first()
        .expect("fixture has one direct dependency");
    assert_eq!(manifest.dependencies().len(), 1);
    let target = target.package_identity().expect("target identity");
    let manifest = AuthorManifestV1::new(
        manifest.name().clone(),
        manifest.version().clone(),
        vec![
            DependencyRequirementV1::new(requirement.alias().clone(), target)
                .expect("retarget direct dependency"),
        ],
        manifest.bundle().to_vec(),
    )
    .expect("retargeted manifest");
    AuthorPackageSourcesV1::new(manifest, sources.files().to_vec())
        .expect("retargeted package sources")
}

fn assert_diagnostic_contains(diagnostics: &[eqiora::Diagnostic], expected: &str) {
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains(expected)),
        "expected a diagnostic containing {expected:?}, got {diagnostics:#?}"
    );
}

fn selected_connection(
    packaged: &PackagedModelDocument,
    member: Id<kinds::Port>,
) -> Id<kinds::Connection> {
    packaged
        .model()
        .program()
        .nodes()
        .find_map(|node| {
            let KernelNode::Connection(connection) = node else {
                return None;
            };
            packaged
                .model()
                .program()
                .edges()
                .iter()
                .any(|edge| {
                    edge.kind() == EdgeKind::Connects
                        && edge.from() == connection.id().erase()
                        && edge.to() == member.erase()
                })
                .then_some(connection.id())
        })
        .expect("selected Port belongs to one Connection")
}

fn compile_from_memory(
    releases: &[PackageReleaseV1],
    resolution: &ResolutionRecordV1,
) -> PackagedModelDocument {
    let mut store = InMemoryPackageStore::default();
    for release in releases {
        store.insert(release).expect("insert exact release");
    }
    PackagedModelDocument::compile_locked(&store, resolution, "Main")
        .expect("compile exact package graph")
}

fn assert_file_contains(path: &Path, fragment: &str) {
    assert!(
        path.to_string_lossy().contains(fragment),
        "expected {} to contain {fragment}",
        path.display()
    );
}

#[test]
fn transitive_composed_component_installs_flattens_and_solves() {
    let basic = release("Eqiora.Electrical.Basic", &[]);
    let circuits = release("Eqiora.Electrical.Circuits", std::slice::from_ref(&basic));
    assert_eq!(circuits.semantic().declarations().len(), 1);
    let exported = &circuits.semantic().declarations()[0];
    assert_eq!(exported.path().as_str(), "ParallelDc");
    assert_eq!(exported.kind(), DeclarationKindV1::Component);
    assert_eq!(exported.visibility(), VisibilityV1::Public);
    let root_sources = sources("org.example.closed_circuit");
    let root = prepare_package_release_v1(root_sources.clone(), &[basic.clone(), circuits.clone()])
        .expect("prepare leaf-first closure");
    let permuted_root =
        prepare_package_release_v1(root_sources, &[circuits.clone(), basic.clone()])
            .expect("prepare intermediate-first closure");
    assert_eq!(
        root.canonical_json().expect("root wire"),
        permuted_root.canonical_json().expect("permuted root wire")
    );

    let resolution =
        ResolutionRecordV1::from_exact_releases(&root, &[circuits.clone(), basic.clone()])
            .expect("derive transitive exact lock");
    let permuted_resolution =
        ResolutionRecordV1::from_exact_releases(&root, &[basic.clone(), circuits.clone()])
            .expect("derive permuted transitive exact lock");
    assert_eq!(resolution, permuted_resolution);
    assert_eq!(resolution.nodes().len(), 3);
    assert_eq!(resolution.edges().len(), 2);
    let root_identity = root.package_identity().expect("root identity");
    let circuits_identity = circuits.package_identity().expect("circuits identity");
    let basic_identity = basic.package_identity().expect("basic identity");
    assert!(resolution.edges().iter().any(|edge| {
        edge.declaring() == &root_identity
            && edge.alias().as_str() == "circuits"
            && edge.target() == &circuits_identity
    }));
    assert!(resolution.edges().iter().any(|edge| {
        edge.declaring() == &circuits_identity
            && edge.alias().as_str() == "basic"
            && edge.target() == &basic_identity
    }));

    let directory = TestDirectory::create();
    let installer =
        DirectoryPackageInstaller::open_ambient(&directory.0).expect("open package installer");
    for release in [&basic, &circuits, &root] {
        let receipt = installer.install(release).expect("install exact release");
        assert_eq!(receipt.disposition(), PackageInstallDisposition::Installed);
        assert_eq!(receipt.staging_cleanup(), PackageStageCleanup::Removed);
    }
    drop(installer);

    let store = DirectoryPackageStore::open_ambient(&directory.0).expect("open installed store");
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
        .expect("compile installed transitive closure");
    packaged
        .compilation()
        .validate_against(&resolution)
        .expect("validate exact compilation provenance");
    assert_eq!(packaged.compilation().packages().len(), 3);

    let memory_permuted = compile_from_memory(
        &[root.clone(), circuits.clone(), basic.clone()],
        &permuted_resolution,
    );
    assert_eq!(
        packaged.model().canonical_json().expect("installed Model"),
        memory_permuted
            .model()
            .canonical_json()
            .expect("permuted-memory Model")
    );
    assert_eq!(
        packaged
            .compilation()
            .canonical_json()
            .expect("installed compilation"),
        memory_permuted
            .compilation()
            .canonical_json()
            .expect("permuted-memory compilation")
    );

    let identity = |release: &PackageReleaseV1| {
        let package = release.package_identity().expect("package identity");
        serde_json::json!({
            "name": package.name.as_str(),
            "version": package.version.to_string(),
            "semantic_digest": package.semantic_digest.to_hex(),
            "source_digest": release.source_digest().expect("source digest").to_hex(),
        })
    };
    let actual_identities = serde_json::json!({
        "schema": "eqiora.verify.composed-model-package-identities.v1",
        "basic": identity(&basic),
        "circuits": identity(&circuits),
        "root": identity(&root),
        "resolution_digest": resolution.digest().expect("resolution digest").to_hex(),
        "model_digest": packaged.model().digest().expect("model digest"),
        "compilation_digest": packaged.compilation().digest().expect("compilation digest").to_hex(),
    });
    let expected_identities: serde_json::Value =
        serde_json::from_slice(EXPECTED_IDENTITIES).expect("expected identity oracle");
    assert_eq!(actual_identities, expected_identities);

    let program = packaged.model().program();
    let counts = program.nodes().fold([0_usize; 9], |mut counts, node| {
        let index = match node {
            KernelNode::Domain(_) => 0,
            KernelNode::Representation(_) => 1,
            KernelNode::Field(_) => 2,
            KernelNode::Parameter(_) => 3,
            KernelNode::Port(_) => 4,
            KernelNode::Relation(_) => 5,
            KernelNode::Activation(_) => 6,
            KernelNode::Connection(_) => 7,
            KernelNode::ClockDomain(_) => 8,
            _ => panic!("fixture contains an unclassified Kernel node kind"),
        };
        counts[index] += 1;
        counts
    });
    assert_eq!(counts, [1, 0, 0, 0, 7, 4, 4, 2, 0]);
    assert_eq!(program.nodes().count(), counts.iter().sum::<usize>());
    assert_eq!(packaged.provenance().len(), 19);
    assert!(
        program
            .nodes()
            .all(|node| packaged.provenance().get_by_graph_id(node.id()).is_some())
    );

    let resistor_two: Id<kinds::Port> = packaged.model().aliases()["circuit.resistor_two.positive"]
        .downcast()
        .expect("two-ohm resistor Port");
    let resistor_four: Id<kinds::Port> =
        packaged.model().aliases()["circuit.resistor_four.positive"]
            .downcast()
            .expect("four-ohm resistor Port");
    assert_ne!(resistor_two, resistor_four);

    let resistor_two_provenance = packaged
        .provenance()
        .get_by_graph_id(resistor_two.erase())
        .expect("two-ohm resistor Port provenance");
    let resistor_four_provenance = packaged
        .provenance()
        .get_by_graph_id(resistor_four.erase())
        .expect("four-ohm resistor Port provenance");
    assert_eq!(
        resistor_two_provenance.definition_span(),
        resistor_four_provenance.definition_span()
    );
    assert_ne!(
        resistor_two_provenance.instance_span(),
        resistor_four_provenance.instance_span()
    );

    let provenance = resistor_two_provenance;
    assert_file_contains(
        Path::new(&provenance.definition_span().file),
        "Eqiora.Electrical.Basic",
    );
    assert_file_contains(
        Path::new(&provenance.instance_span().file),
        "Eqiora.Electrical.Circuits",
    );
    let binding_files = provenance
        .binding_spans()
        .iter()
        .map(|span| span.file.as_str())
        .collect::<BTreeSet<_>>();
    assert!(
        binding_files
            .iter()
            .any(|file| file.contains("Eqiora.Electrical.Circuits"))
    );
    assert!(
        binding_files
            .iter()
            .any(|file| file.contains("org.example.closed_circuit"))
    );
    assert_eq!(binding_files.len(), 2);
    for alias in [
        "circuit.resistance_two",
        "circuit.resistance_four",
        "circuit.supply_voltage",
        "circuit.resistor_two.resistance",
        "circuit.resistor_four.resistance",
        "circuit.source.voltage",
    ] {
        assert!(
            packaged.model().aliases().get(alias).is_none(),
            "literal Parameter term `{alias}` must not become a Kernel Parameter"
        );
    }

    let problem =
        lower_scalar_physical_affine(program, selected_connection(&packaged, resistor_two), None)
            .expect("lower complete transitive physical closure");
    assert_eq!(
        (
            problem.canonical_system().rows(),
            problem.canonical_system().columns()
        ),
        (14, 14)
    );
    let mut junction_roots = problem
        .composed_system()
        .junctions()
        .iter()
        .map(|junction| junction.dag().roots().len())
        .collect::<Vec<_>>();
    junction_roots.sort_unstable();
    assert_eq!(junction_roots, [3, 4]);
    let plan = SolverPlan::new(
        LinearSolver::BiConjugateGradientStabilized,
        1.0e-12,
        1.0e-14,
        NonZeroUsize::new(100).expect("nonzero iterations"),
    )
    .expect("solver plan")
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Fast);
    let solution = solve_scalar_physical_affine_with_initial_guess(
        &problem,
        &vec![1.0; problem.canonical_system().columns()],
        LinearSolveRequest::new(&FaerLinearSolver, plan),
    )
    .expect("solve transitive composed circuit");
    let value = |name: &str, unknown: fn(Id<kinds::Port>) -> PhysicalUnknown| {
        let port = packaged.model().aliases()[name]
            .downcast()
            .expect("fixture Port");
        solution.value(unknown(port)).expect("physical value")
    };
    for (name, across, through) in [
        ("circuit.source.positive", 12.0, -9.0),
        ("circuit.resistor_two.positive", 12.0, 6.0),
        ("circuit.resistor_four.positive", 12.0, 3.0),
        ("circuit.ground.terminal", 0.0, 0.0),
    ] {
        assert!((value(name, PhysicalUnknown::Across) - across).abs() <= VALUE_TOLERANCE);
        assert!((value(name, PhysicalUnknown::Through) - through).abs() <= VALUE_TOLERANCE);
    }
    assert!(solution.reference_residual_norm() <= RESIDUAL_TOLERANCE);
}

#[test]
fn semantic_permutation_changes_only_the_intermediate_source_lineage() {
    let basic = release("Eqiora.Electrical.Basic", &[]);
    let circuits_sources = sources("Eqiora.Electrical.Circuits");
    let circuits =
        prepare_package_release_v1(circuits_sources.clone(), std::slice::from_ref(&basic))
            .expect("prepare canonical circuits release");
    let permuted_circuits = prepare_package_release_v1(
        replace_model_source(&circuits_sources, PERMUTED_CIRCUITS_SOURCE),
        std::slice::from_ref(&basic),
    )
    .expect("prepare permuted circuits release");
    assert_eq!(
        circuits.package_identity().expect("circuits identity"),
        permuted_circuits
            .package_identity()
            .expect("permuted circuits identity")
    );
    assert_ne!(
        circuits.source_digest().expect("circuits source"),
        permuted_circuits
            .source_digest()
            .expect("permuted circuits source")
    );

    let root_sources = sources("org.example.closed_circuit");
    let root = prepare_package_release_v1(root_sources.clone(), &[basic.clone(), circuits.clone()])
        .expect("prepare canonical root");
    let permuted_root =
        prepare_package_release_v1(root_sources, &[permuted_circuits.clone(), basic.clone()])
            .expect("prepare root against semantically equal dependency");
    assert_eq!(root, permuted_root);

    let resolution =
        ResolutionRecordV1::from_exact_releases(&root, &[basic.clone(), circuits.clone()])
            .expect("canonical resolution");
    let permuted_resolution = ResolutionRecordV1::from_exact_releases(
        &permuted_root,
        &[basic.clone(), permuted_circuits.clone()],
    )
    .expect("permuted resolution");
    assert_ne!(resolution, permuted_resolution);
    assert_ne!(
        resolution.digest().expect("canonical resolution digest"),
        permuted_resolution
            .digest()
            .expect("permuted resolution digest")
    );
    let changed_resolution_packages = resolution
        .nodes()
        .iter()
        .zip(permuted_resolution.nodes())
        .filter_map(|(canonical, permuted)| {
            assert_eq!(canonical.identity(), permuted.identity());
            (canonical.source_digest() != permuted.source_digest())
                .then(|| canonical.identity().name.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(changed_resolution_packages, ["Eqiora.Electrical.Circuits"]);

    let canonical = compile_from_memory(&[basic.clone(), circuits, root], &resolution);
    let permuted = compile_from_memory(
        &[basic, permuted_circuits, permuted_root],
        &permuted_resolution,
    );
    canonical
        .compilation()
        .validate_against(&resolution)
        .expect("canonical compilation matches its exact lock");
    permuted
        .compilation()
        .validate_against(&permuted_resolution)
        .expect("permuted compilation matches its exact lock");
    assert_eq!(
        canonical.model().canonical_json().expect("canonical Model"),
        permuted.model().canonical_json().expect("permuted Model")
    );
    assert_ne!(
        canonical
            .compilation()
            .digest()
            .expect("canonical compilation"),
        permuted
            .compilation()
            .digest()
            .expect("permuted compilation")
    );
    let changed_compilation_packages = canonical
        .compilation()
        .packages()
        .iter()
        .zip(permuted.compilation().packages())
        .filter_map(|(canonical, permuted)| {
            assert_eq!(canonical.package(), permuted.package());
            (canonical.source_digest() != permuted.source_digest())
                .then(|| canonical.package().name.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(changed_compilation_packages, ["Eqiora.Electrical.Circuits"]);
}

#[test]
fn root_cannot_escape_its_direct_typed_component_contract() {
    let basic = release("Eqiora.Electrical.Basic", &[]);
    let circuits = release("Eqiora.Electrical.Circuits", std::slice::from_ref(&basic));
    let root_sources = sources("org.example.closed_circuit");

    assert!(matches!(
        prepare_package_release_v1(root_sources.clone(), std::slice::from_ref(&circuits)),
        Err(PackagePreparationError::MissingDependency { .. })
    ));

    let transitive_alias = replace_model_source(
        &root_sources,
        "model Main { instance forbidden: basic.Resistor(resistance = 2); }",
    );
    let transitive_alias_error =
        prepare_package_release_v1(transitive_alias, &[basic.clone(), circuits.clone()])
            .expect_err("root cannot address a transitive alias");
    let PackagePreparationError::Diagnostics(diagnostics) = transitive_alias_error else {
        panic!("expected typed diagnostics, got {transitive_alias_error}");
    };
    assert_diagnostic_contains(&diagnostics, "unknown direct package alias");

    let circuits_sources = sources("Eqiora.Electrical.Circuits");
    let public_source = circuits_sources
        .files()
        .iter()
        .find(|file| file.role() == BundleRoleV1::ModelSource)
        .expect("circuits model source");
    let public_source = std::str::from_utf8(public_source.bytes()).expect("UTF-8 model source");
    let private_source = public_source.replacen(
        "public component ParallelDc",
        "private component ParallelDc",
        1,
    );
    assert_ne!(private_source, public_source);
    let private_circuits = prepare_package_release_v1(
        replace_model_source(&circuits_sources, &private_source),
        std::slice::from_ref(&basic),
    )
    .expect("prepare private-component dependency");
    let private_root_sources = retarget_only_dependency(&root_sources, &private_circuits);
    let private_import_error =
        prepare_package_release_v1(private_root_sources, &[basic.clone(), private_circuits])
            .expect_err("root cannot import a private component");
    let PackagePreparationError::Diagnostics(diagnostics) = private_import_error else {
        panic!("expected typed diagnostics, got {private_import_error}");
    };
    assert_diagnostic_contains(&diagnostics, "private component");

    let dimension_mismatch = replace_model_source(
        &root_sources,
        r#"
model Main {
  parameter duration: s = 1;
  instance circuit: circuits.ParallelDc(
    supply_voltage = duration,
    resistance_two = 2,
    resistance_four = 4
  );
}
"#,
    );
    let dimension_error =
        prepare_package_release_v1(dimension_mismatch, &[circuits.clone(), basic.clone()])
            .expect_err("dimension mismatch cannot produce a package release");
    let PackagePreparationError::Diagnostics(diagnostics) = dimension_error else {
        panic!("expected typed diagnostics, got {dimension_error}");
    };
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].code(),
        eqiora::diagnostic::codes::LANGUAGE_TYPE_ERROR
    );
    assert_eq!(
        diagnostics[0].message(),
        "Parameter binding has dimension [T], expected [M·L^2·T^-3·I^-1]"
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.graph_path().is_none()),
        "publication-time definition diagnostics must not synthesize Model or Transaction graph identity"
    );
}
