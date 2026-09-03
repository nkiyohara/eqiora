use super::*;
use eqiora_artifact::{ArtifactDigest, RunManifestV1};
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_package::{
    AuthorManifestV1, BundleEntryV1, DependencyRequirementV1, ExactVersion, InMemoryPackageStore,
    NormalizedRelativePath, PackageReleaseV1, SourceFileV1,
};

const VERSION: &str = "1.0.0";
const SOURCE_PATH: &str = "src/package.eqi";

fn author_sources(
    name: &str,
    source: &str,
    dependencies: &[(&str, &PackageReleaseV1)],
) -> AuthorPackageSourcesV1 {
    let mut requirements = Vec::new();
    for (alias, dependency) in dependencies {
        let target = dependency.package_identity().expect("dependency identity");
        requirements.push(
            DependencyRequirementV1::new(QualifiedName::parse(*alias).expect("alias"), target)
                .expect("dependency requirement"),
        );
    }
    let path = NormalizedRelativePath::parse(SOURCE_PATH).expect("source path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse(name).expect("package name"),
        ExactVersion::parse(VERSION).expect("version"),
        requirements,
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .expect("manifest");
    AuthorPackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            source.as_bytes().to_vec(),
        )],
    )
    .expect("admitted author sources")
}

fn release(
    name: &str,
    source: &str,
    dependencies: &[(&str, &PackageReleaseV1)],
) -> PackageReleaseV1 {
    let sources = author_sources(name, source, dependencies);
    let dependency_releases = dependencies
        .iter()
        .map(|(_, release)| (*release).clone())
        .collect::<Vec<_>>();
    prepare_package_release_v1(sources, &dependency_releases).expect("compiler-derived release")
}

fn caller_geometry(volume: &str) -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole_named_roles(
        [[0.0, 2.2], [0.0, 0.41]],
        [0.2, 0.2],
        0.05,
        1.0e-12,
        volume,
        "inlet",
        "outlet",
        "walls",
        "walls",
        "cylinder",
    )
    .expect("caller Geometry")
}

#[test]
fn locked_source_bundle_reconstructs_declared_module_graph() {
    let main_path = NormalizedRelativePath::parse("src/main.eqi").expect("main path");
    let library_path = NormalizedRelativePath::parse("sources/anywhere.eqi").expect("library path");
    let main_source = r#"
import library.parts as lib;
model Main { instance load: lib.Resistor(resistance = 2); }
"#;
    let library_source = r#"
module library.parts;
public component Resistor {
  public parameter resistance: 1;
  relation law continuous { resistance - 2 = 0; }
}
"#;
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.example.DeclaredModules").expect("package name"),
        ExactVersion::parse(VERSION).expect("version"),
        vec![],
        vec![
            BundleEntryV1::new(main_path.clone(), BundleRoleV1::ModelSource),
            BundleEntryV1::new(library_path.clone(), BundleRoleV1::ModelSource),
        ],
    )
    .expect("manifest");
    let sources = AuthorPackageSourcesV1::new(
        manifest,
        vec![
            SourceFileV1::new(
                main_path,
                BundleRoleV1::ModelSource,
                main_source.as_bytes().to_vec(),
            ),
            SourceFileV1::new(
                library_path,
                BundleRoleV1::ModelSource,
                library_source.as_bytes().to_vec(),
            ),
        ],
    )
    .expect("closed source bundle");
    let release = prepare_package_release_v1(sources, &[]).expect("prepared module package");
    let replayed =
        PackageReleaseV1::from_json(&release.canonical_json().expect("canonical package release"))
            .expect("replayed package release");
    assert_eq!(
        replayed.source_digest().unwrap(),
        release.source_digest().unwrap()
    );

    let mut store = InMemoryPackageStore::default();
    store.insert(&replayed).expect("store replayed package");
    let resolution =
        ResolutionRecordV1::from_exact_releases(&replayed, &[]).expect("exact resolution");
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
        .expect("reconstructed module graph compiles");
    packaged
        .compilation()
        .validate_against(&resolution)
        .expect("compilation binds exact source inventory");
    assert!(packaged.model().aliases().contains_key("load.law"));
}

#[test]
fn locked_root_can_select_one_direct_dependency_public_model() {
    let dependency = release(
        "org.example.Library",
        "public model Shared { parameter gain: 1 = 2; relation law continuous { gain - 2 = 0; } }",
        &[],
    );
    let root = release(
        "org.example.Root",
        "model Local {}",
        &[("library", &dependency)],
    );
    let resolution =
        ResolutionRecordV1::from_exact_releases(&root, std::slice::from_ref(&dependency))
            .expect("exact dependency lock");
    let mut store = InMemoryPackageStore::default();
    store.insert(&root).expect("store root");
    store.insert(&dependency).expect("store dependency");

    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "library.Shared")
        .expect("direct dependency public Model compiles");
    assert!(packaged.model().aliases().contains_key("law"));
    packaged
        .compilation()
        .validate_against(&resolution)
        .expect("imported entry retains exact package resolution");
}

#[test]
fn locked_scalar_property_replays_offline_with_inspectable_provenance() {
    const SOURCE: &str = r#"
public property contract Diffusivity {
  scalar value: m ^ 2 / s;
}
property release ReferenceDiffusivity implements Diffusivity {
  value = 25;
  source_unit: m ^ 2 / s = 1 / 1000;
  validity = unconditional;
  citation = org.example.measurement;
  license = spdx.CC0_1_0;
}
public component Diffusion {
  public property diffusivity: Diffusivity;
  relation law continuous { diffusivity = 0; }
}
model Main {
  instance domain: Diffusion(property diffusivity = ReferenceDiffusivity);
}
"#;
    let root = release("org.example.Property", SOURCE, &[]);
    assert!(
        root.semantic()
            .declarations()
            .iter()
            .any(|value| { value.kind() == DeclarationKindV1::PropertyContract })
    );
    assert!(
        root.semantic()
            .declarations()
            .iter()
            .any(|value| { value.kind() == DeclarationKindV1::PropertyRelease })
    );
    let mut store = InMemoryPackageStore::default();
    store.insert(&root).expect("store exact release");
    let resolution = ResolutionRecordV1::from_exact_releases(&root, &[]).expect("exact lock");
    let first = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
        .expect("first offline compile");
    let replay =
        PackagedModelDocument::compile_locked(&store, &resolution, "Main").expect("offline replay");
    assert_eq!(
        first.model().digest().unwrap(),
        replay.model().digest().unwrap()
    );
    assert_eq!(first.compilation(), replay.compilation());
    assert_eq!(
        first.property_bindings().collect::<Vec<_>>(),
        replay.property_bindings().collect::<Vec<_>>()
    );
    let binding = first.property_bindings().next().unwrap();
    assert_eq!(binding.0, None);
    assert_eq!(binding.5, 0.025);
    assert_eq!(binding.4, "diffusivity");
    assert_eq!(binding.6, "unconditional");
    assert_eq!(binding.7, "org.example.measurement");
    assert_eq!(binding.8, "spdx.CC0_1_0");

    let changed_source = SOURCE.replace("org.example.measurement", "org.example.remeasurement");
    let changed = release("org.example.Property", &changed_source, &[]);
    let mut changed_store = InMemoryPackageStore::default();
    changed_store
        .insert(&changed)
        .expect("store changed release");
    let changed_resolution =
        ResolutionRecordV1::from_exact_releases(&changed, &[]).expect("changed exact lock");
    let changed_model =
        PackagedModelDocument::compile_locked(&changed_store, &changed_resolution, "Main")
            .expect("changed provenance compiles");
    assert_ne!(
        first.model().digest().unwrap(),
        changed_model.model().digest().unwrap()
    );
}

#[test]
fn locked_component_binds_caller_geometry_into_ordinary_model() {
    const SOURCE: &str = r#"
public component SpatialLaw {
  public support fluid: volume(ambient_dimension = 2);
  public parameter forcing: 1;
  representation space = continuum;
  field state on fluid as space: 1 = 0;
  relation balance continuous on fluid { state - forcing = 0; }
}
"#;
    let root = release("org.example.SpatialLaw", SOURCE, &[]);
    let mut store = InMemoryPackageStore::default();
    store.insert(&root).expect("store root");
    let resolution = ResolutionRecordV1::from_exact_releases(&root, &[]).expect("resolution");
    let geometry = caller_geometry("fluid");

    let packaged = PackagedModelDocument::compile_locked_with_geometry(
        &store,
        &resolution,
        "SpatialLaw",
        &geometry,
        &[("forcing", 2.0)],
    )
    .expect("Geometry-bound package compilation");

    packaged
        .compilation()
        .validate_against(&resolution)
        .expect("compilation retains exact resolution");
    assert_eq!(
        packaged.compilation().model_digest(),
        CanonicalModelDigest::parse(&packaged.model().digest().expect("Model digest"))
            .expect("canonical Model digest")
    );
    assert_eq!(
        packaged.model().aliases()["definition.forcing"].kind(),
        eqiora_core::EntityKind::Parameter
    );

    let foreign = caller_geometry("other");
    let error = PackagedModelDocument::compile_locked_with_geometry(
        &store,
        &resolution,
        "SpatialLaw",
        &foreign,
        &[("forcing", 2.0)],
    )
    .expect_err("support names cannot fall back to matching bounds");
    assert!(format!("{error:?}").contains("fluid"), "{error:?}");

    let duplicated_geometry = release(
        "org.example.DuplicatedGeometry",
        &format!("{SOURCE}\nmodel Legacy {{ domain fluid = box(0, 1, 0, 1); }}\n"),
        &[],
    );
    let mut duplicated_store = InMemoryPackageStore::default();
    duplicated_store
        .insert(&duplicated_geometry)
        .expect("store duplicated Geometry package");
    let duplicated_resolution = ResolutionRecordV1::from_exact_releases(&duplicated_geometry, &[])
        .expect("duplicated Geometry resolution");
    let error = PackagedModelDocument::compile_locked_with_geometry(
        &duplicated_store,
        &duplicated_resolution,
        "SpatialLaw",
        &geometry,
        &[("forcing", 2.0)],
    )
    .expect_err("package-authored root Geometry cannot coexist with caller Geometry");
    assert!(
        format!("{error:?}").contains("definitions-only root package"),
        "{error:?}"
    );
}

#[test]
fn locked_compilation_binds_exact_graph_model_and_package_provenance() {
    const LIBRARY_SOURCE: &str = r#"
public component Resistor {
  public parameter resistance: 1 = 2;
  relation law continuous { resistance - 2 = 0; }
}
"#;
    const ROOT_SOURCE: &str = r#"
model Main {
  instance load: electrical.Resistor(resistance = 3);
}
"#;

    let library = release("Eqiora.Electrical.Basic", LIBRARY_SOURCE, &[]);
    let root = release(
        "org.example.ParallelDc",
        ROOT_SOURCE,
        &[("electrical", &library)],
    );
    let mut store = InMemoryPackageStore::default();
    let library_source = store.insert(&library).expect("store library");
    let root_source = store.insert(&root).expect("store root");
    let resolution = ResolutionRecordV1::from_exact_releases(&root, std::slice::from_ref(&library))
        .expect("derived resolution");
    assert!(
        resolution
            .nodes()
            .iter()
            .any(|node| node.source_digest() == library_source)
    );
    assert!(
        resolution
            .nodes()
            .iter()
            .any(|node| node.source_digest() == root_source)
    );

    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
        .expect("locked compilation");

    packaged
        .compilation()
        .validate_against(&resolution)
        .expect("compilation binds the resolution");
    assert_eq!(
        packaged.compilation().model_digest(),
        CanonicalModelDigest::parse(&packaged.model().digest().expect("model digest"))
            .expect("canonical digest")
    );
    assert_eq!(
        packaged.model().aliases().get("load.resistance"),
        None,
        "a literal Component binding does not fabricate a Parameter alias"
    );
    let law = packaged.model().aliases()["load.law"];
    let provenance = packaged
        .provenance()
        .get_by_graph_id(law)
        .expect("imported Relation provenance");
    assert!(provenance.definition_span().file.ends_with(SOURCE_PATH));
    assert!(provenance.instance_span().file.ends_with(SOURCE_PATH));
    assert_ne!(
        provenance.definition_span().file,
        provenance.instance_span().file,
        "package-qualified labels disambiguate equal bundle paths"
    );
    assert!(
        provenance
            .definition_span()
            .file
            .contains("Eqiora.Electrical.Basic")
    );
    assert!(
        provenance
            .instance_span()
            .file
            .contains("org.example.ParallelDc")
    );

    let make_run = |executor: &str, topology: &str, reduction: &str| {
        RunManifestV1::new(
            ArtifactDigest::from_hex(packaged.model().digest().expect("model digest"))
                .expect("artifact digest"),
            packaged.model().program().revision().0,
            executor,
            env!("CARGO_PKG_VERSION"),
        )
        .expect("run manifest")
        .with_numerical_setting("execution.topology", topology)
        .expect("execution topology")
        .with_numerical_setting("solver.method", "reference")
        .expect("solver method")
        .with_numerical_setting("solver.reduction", reduction)
        .expect("solver reduction")
    };
    let run = make_run(
        "eqiora-reference",
        "one-host-process-one-worker",
        "reproducible",
    );
    let binding = packaged.bind_run_v1(&run).expect("package run binding");
    packaged
        .validate_run_v1_binding(&binding, &run, &resolution)
        .expect("exact package run replay");

    let changed_run = make_run("eqiora-reference", "one-host-process-one-worker", "fast");
    assert!(
        packaged
            .validate_run_v1_binding(&binding, &changed_run, &resolution)
            .is_err()
    );

    let changed_backend = make_run(
        "eqiora-other-backend",
        "one-host-process-one-worker",
        "reproducible",
    );
    assert!(
        packaged
            .validate_run_v1_binding(&binding, &changed_backend, &resolution)
            .is_err()
    );

    let changed_topology = make_run(
        "eqiora-reference",
        "one-host-process-two-workers",
        "reproducible",
    );
    assert!(
        packaged
            .validate_run_v1_binding(&binding, &changed_topology, &resolution)
            .is_err()
    );

    let changed_output = run
        .clone()
        .with_output(ArtifactDigest::from_hex("cd".repeat(32)).expect("output digest"));
    assert!(
        packaged
            .validate_run_v1_binding(&binding, &changed_output, &resolution)
            .is_err()
    );

    let wrong_run = RunManifestV1::new(
        ArtifactDigest::from_hex("ab".repeat(32)).expect("different model digest"),
        packaged.model().program().revision().0,
        "eqiora-reference",
        env!("CARGO_PKG_VERSION"),
    )
    .expect("wrong-model run");
    assert!(matches!(
        packaged.bind_run_v1(&wrong_run),
        Err(PackageRunBindingError::RunModelMismatch { .. })
    ));

    let wrong_revision_run = RunManifestV1::new(
        ArtifactDigest::from_hex(packaged.model().digest().expect("model digest"))
            .expect("artifact digest"),
        packaged.model().program().revision().0 + 1,
        "eqiora-reference",
        env!("CARGO_PKG_VERSION"),
    )
    .expect("wrong-revision run");
    assert!(matches!(
        packaged.bind_run_v1(&wrong_revision_run),
        Err(PackageRunBindingError::RunRevisionMismatch { .. })
    ));
}

#[test]
fn preparation_is_order_independent_over_one_transitive_exact_closure() {
    const LEAF: &str = r#"
public component Resistor {
  public parameter resistance: 1 = 2;
  relation law continuous { resistance - 2 = 0; }
}
"#;
    const MIDDLE: &str = r#"
public component Branch {
  instance load: leaf.Resistor(resistance = 3);
}
"#;
    const ROOT: &str = r#"
model Main {
  instance branch: middle.Branch;
}
"#;
    let leaf = release("org.example.Leaf", LEAF, &[]);
    let middle = release("org.example.Middle", MIDDLE, &[("leaf", &leaf)]);
    let sources = author_sources("org.example.Root", ROOT, &[("middle", &middle)]);

    let first = prepare_package_release_v1(sources.clone(), &[leaf.clone(), middle.clone()])
        .expect("forward dependency order");
    let second = prepare_package_release_v1(sources, &[middle.clone(), leaf.clone()])
        .expect("reverse dependency order");
    assert_eq!(first, second);
    assert_eq!(first.canonical_json(), second.canonical_json());
    assert_eq!(first.package_identity(), second.package_identity());

    let resolution =
        ResolutionRecordV1::from_exact_releases(&first, &[middle, leaf]).expect("exact lock");
    assert_eq!(resolution.nodes().len(), 3);
    assert_eq!(resolution.edges().len(), 2);
}

#[test]
fn preparation_rejects_incomplete_duplicate_and_unreachable_inputs() {
    const LIBRARY: &str = r#"
public component Resistor {
  public parameter resistance: 1 = 2;
  relation law continuous { resistance - 2 = 0; }
}
"#;
    const ROOT: &str = r#"
model Main {
  instance load: electrical.Resistor(resistance = 3);
}
"#;
    let library = release("org.example.Library", LIBRARY, &[]);
    let sources = author_sources("org.example.Root", ROOT, &[("electrical", &library)]);
    assert!(matches!(
        prepare_package_release_v1(sources.clone(), &[]),
        Err(PackagePreparationError::MissingDependency { .. })
    ));
    assert!(matches!(
        prepare_package_release_v1(sources, &[library.clone(), library.clone()]),
        Err(PackagePreparationError::DuplicateDependency(_))
    ));

    let independent = author_sources("org.example.Independent", "model Main {}\n", &[]);
    assert!(matches!(
        prepare_package_release_v1(independent, &[library]),
        Err(PackagePreparationError::Contract(_))
    ));
}

#[test]
fn dishonest_dependency_source_fails_before_root_release_is_returned() {
    const LIBRARY_SOURCE: &str = r#"
public component Resistor {
  public parameter resistance: 1 = 2;
  relation law continuous { resistance - 2 = 0; }
}
"#;
    const ROOT_SOURCE: &str = r#"
model Main {
  instance load: electrical.Resistor(resistance = 3);
}
"#;
    let path = NormalizedRelativePath::parse(SOURCE_PATH).expect("source path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.example.Dishonest").expect("name"),
        ExactVersion::parse(VERSION).expect("version"),
        vec![],
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .expect("manifest");
    let claimed = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
        QualifiedName::parse("Resistor").expect("declaration"),
        DeclarationKindV1::Component,
        VisibilityV1::Public,
        CanonicalDeclaration::new("eqiora.source-declaration.v1:sha256:deadbeef")
            .expect("false canonical claim"),
    )])
    .expect("semantic claim");
    let dishonest = PackageReleaseV1::new(
        manifest,
        claimed.clone(),
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            LIBRARY_SOURCE.as_bytes().to_vec(),
        )],
    )
    .expect("locally valid dishonest release");
    let identity = dishonest.package_identity().expect("dishonest identity");
    let sources = author_sources(
        "org.example.Root",
        ROOT_SOURCE,
        &[("electrical", &dishonest)],
    );

    let error = prepare_package_release_v1(sources, &[dishonest])
        .expect_err("dishonest dependency must not produce a root release");
    match error {
        PackagePreparationError::SemanticContentMismatch {
            package,
            release,
            compiler,
        } => {
            assert_eq!(*package, identity);
            assert_eq!(*release, claimed);
            assert_ne!(compiler, release);
        }
        other => panic!("unexpected preparation error: {other:?}"),
    }
}

#[test]
fn semantic_mismatch_fails_before_model_admission() {
    let path = NormalizedRelativePath::parse(SOURCE_PATH).expect("source path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.example.FalseClaim").expect("name"),
        ExactVersion::parse(VERSION).expect("version"),
        vec![],
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .expect("manifest");
    let claimed = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
        QualifiedName::parse("Main").expect("declaration"),
        DeclarationKindV1::Model,
        VisibilityV1::Private,
        CanonicalDeclaration::new("eqiora.source-declaration.v1:sha256:deadbeef")
            .expect("false canonical claim"),
    )])
    .expect("semantic claim");
    let release = PackageReleaseV1::new(
        manifest,
        claimed.clone(),
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            b"model Main {}\n".to_vec(),
        )],
    )
    .expect("release");
    let identity = release.package_identity().expect("identity");
    let mut store = InMemoryPackageStore::default();
    store.insert(&release).expect("store release");
    let resolution = ResolutionRecordV1::from_exact_releases(&release, &[]).expect("resolution");

    let error = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
        .expect_err("false semantic claim must fail");
    match error {
        PackageCompilationError::SemanticContentMismatch {
            package,
            release,
            compiler,
        } => {
            assert_eq!(*package, identity);
            assert_eq!(*release, claimed);
            assert_ne!(compiler, release);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}
