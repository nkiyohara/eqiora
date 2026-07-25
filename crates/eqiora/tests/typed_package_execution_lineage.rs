use std::num::NonZeroUsize;

use eqiora::api::{ScalarEllipticExecutionEnvironment, ScalarEllipticIntent, ScalarEllipticMethod};
use eqiora::artifact::{
    ArtifactDigest, ExecutionProvenanceV1, ExecutionTopologyV1, RealizationEnvelopeV1,
    RunManifestV2,
};
use eqiora::compatibility::ExactModelCodec;
use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleRoleV1, InMemoryPackageStore,
    ModelPackageIdentityV1, PackageCompilationRecordV1, PackageExecutionBindingError,
    PackageExecutionBindingV1, PackageReleaseV1, PackagedModelDocument, ResolutionNodeV1,
    ResolutionRecordV1, SourceFileV1, prepare_package_release_v1,
};
use eqiora::realization::RealizationRevision;
use eqiora::solver::ExecutionTopology;
use eqiora::solver::ReductionPolicy;

const MANIFEST: &[u8] = include_bytes!("../../../packages/org.example.poisson/package.json");
const SOURCE: &str = include_str!("../../../packages/org.example.poisson/src/main.eqi");
const README: &[u8] = include_bytes!("../../../packages/org.example.poisson/README.md");
const EXPECTED: &[u8] =
    include_bytes!("../../../verify/packages/typed-execution-lineage/expected/identities.json");
const SOURCE_PATH: &str = "src/main.eqi";

fn package_release(readme: &[u8], reverse_files: bool) -> PackageReleaseV1 {
    let manifest = AuthorManifestV1::from_json(MANIFEST).expect("manifest");
    let mut files = vec![
        SourceFileV1::new(
            eqiora::package::NormalizedRelativePath::parse("README.md").expect("README path"),
            BundleRoleV1::Documentation,
            readme.to_vec(),
        ),
        SourceFileV1::new(
            eqiora::package::NormalizedRelativePath::parse(SOURCE_PATH).expect("source path"),
            BundleRoleV1::ModelSource,
            SOURCE.as_bytes().to_vec(),
        ),
    ];
    if reverse_files {
        files.reverse();
    }
    let sources =
        AuthorPackageSourcesV1::new(manifest, files).expect("closed package source inventory");
    prepare_package_release_v1(sources, &[])
        .expect("compiler-derived release with validated definitions")
}

fn install(
    release: &PackageReleaseV1,
) -> (
    InMemoryPackageStore,
    ResolutionRecordV1,
    ModelPackageIdentityV1,
) {
    let identity = release.package_identity().expect("package identity");
    let mut store = InMemoryPackageStore::default();
    let source = store.insert(release).expect("store release");
    let resolution = ResolutionRecordV1::new(
        identity.clone(),
        vec![ResolutionNodeV1::new(identity.clone(), source)],
        vec![],
    )
    .expect("resolution");
    (store, resolution, identity)
}

fn execution_provenance(result: &eqiora::api::ScalarEllipticRunResult) -> ExecutionProvenanceV1 {
    let report = result.solve();
    let ExecutionTopology::Host { workers } = report.execution().topology() else {
        panic!("registered reference execution must be host-local");
    };
    ExecutionProvenanceV1::new(
        report.execution().adapter().as_str(),
        env!("CARGO_PKG_VERSION"),
        report.backend().as_str(),
        env!("CARGO_PKG_VERSION"),
        ExecutionTopologyV1::Host { workers },
        report.reduction(),
    )
    .expect("execution provenance")
}

fn accepted_run(
    packaged: &PackagedModelDocument,
    method: ScalarEllipticMethod,
    cells_per_axis: usize,
) -> (RealizationEnvelopeV1, RunManifestV2) {
    let environment = ScalarEllipticExecutionEnvironment::host_serial();
    let intent = ScalarEllipticIntent::new(
        RealizationRevision::new(1),
        method,
        NonZeroUsize::new(cells_per_axis).expect("nonzero cells"),
        NonZeroUsize::MIN,
    );
    let plan = packaged
        .model()
        .preview_scalar_elliptic_run(intent, environment)
        .expect("typed Realization");
    let result = packaged
        .model()
        .run_scalar_elliptic_plan(plan, environment)
        .expect("accepted scalar elliptic run");
    assert!(result.solve().true_residual_norm() <= result.solve().residual_target());
    assert!(result.balance().relative_imbalance() <= 1.0e-10);
    let expected_values = match method {
        ScalarEllipticMethod::FiniteElement => (cells_per_axis + 1).pow(2),
        ScalarEllipticMethod::FiniteVolume => cells_per_axis.pow(2),
    };
    assert_eq!(result.field().value_count(), expected_values);

    let realization = result.plan().artifact().clone();
    let run =
        RunManifestV2::new(&realization, execution_provenance(&result)).expect("typed Run v2");
    (realization, run)
}

#[test]
fn exact_package_compilation_composes_with_typed_realization_and_run_v2() {
    let release = package_release(README, false);
    let (store, resolution, identity) = install(&release);
    let packaged =
        PackagedModelDocument::compile_locked(&store, &resolution, "Main", ExactModelCodec::V1)
            .expect("locked package compilation");
    let (realization, run) = accepted_run(&packaged, ScalarEllipticMethod::FiniteElement, 8);
    let realization_bytes = realization.canonical_json().expect("Realization JSON");
    let run_bytes = run.canonical_json().expect("Run JSON");

    let binding = packaged
        .bind_execution_v2(&realization, &run)
        .expect("package execution binding");
    assert_eq!(
        realization.canonical_json().expect("unchanged Realization"),
        realization_bytes
    );
    assert_eq!(run.canonical_json().expect("unchanged Run"), run_bytes);
    packaged
        .validate_execution_v2_binding(&binding, &realization, &run, &resolution)
        .expect("complete lineage replay");

    let compilation_bytes = packaged
        .compilation()
        .canonical_json()
        .expect("compilation JSON");
    let replayed_compilation =
        PackageCompilationRecordV1::from_json(&compilation_bytes).expect("decoded compilation");
    assert_eq!(&replayed_compilation, packaged.compilation());
    replayed_compilation
        .validate_against(&resolution)
        .expect("compilation replay");
    let model_bytes = packaged.model().canonical_json().expect("Model JSON");
    let replayed_model = ExactModelCodec::V1
        .replay(&model_bytes)
        .expect("decoded Model");
    assert_eq!(replayed_model.canonical_json().unwrap(), model_bytes);
    assert_eq!(replayed_model.digest(), packaged.model().digest());

    let binding_bytes = binding.canonical_json().expect("binding JSON");
    let replayed_binding =
        PackageExecutionBindingV1::from_json(&binding_bytes).expect("decoded binding");
    let replayed_realization =
        RealizationEnvelopeV1::from_json(&realization_bytes, Default::default())
            .expect("decoded Realization");
    let replayed_run =
        RunManifestV2::from_json(&run_bytes, Default::default()).expect("decoded Run");
    replayed_run
        .validate_against(&replayed_realization)
        .expect("typed Run replay");
    packaged
        .validate_execution_v2_binding(
            &replayed_binding,
            &replayed_realization,
            &replayed_run,
            &resolution,
        )
        .expect("canonical lineage replay");

    // File insertion order is outside package meaning and source identity.
    let permuted_release = package_release(README, true);
    assert_eq!(
        release.canonical_json().expect("release JSON"),
        permuted_release.canonical_json().expect("permuted JSON")
    );
    let (permuted_store, permuted_resolution, _) = install(&permuted_release);
    let permuted = PackagedModelDocument::compile_locked(
        &permuted_store,
        &permuted_resolution,
        "Main",
        ExactModelCodec::V1,
    )
    .expect("permuted compilation");
    let (permuted_realization, permuted_run) =
        accepted_run(&permuted, ScalarEllipticMethod::FiniteElement, 8);
    let permuted_binding = permuted
        .bind_execution_v2(&permuted_realization, &permuted_run)
        .expect("permuted binding");
    assert_eq!(permuted.model().digest(), packaged.model().digest());
    assert_eq!(permuted.compilation(), packaged.compilation());
    assert_eq!(permuted_realization, realization);
    assert_eq!(permuted_run, run);
    assert_eq!(permuted_binding, binding);

    // Documentation changes source and compilation lineage, but not package
    // semantics or the canonical Model/Realization/Run artifacts.
    let changed_release = package_release(b"different documentation\n", false);
    assert_eq!(
        changed_release
            .package_identity()
            .expect("changed semantic identity"),
        identity
    );
    assert_ne!(
        changed_release.source_digest().expect("changed source"),
        release.source_digest().expect("source")
    );
    let (changed_store, changed_resolution, _) = install(&changed_release);
    let changed = PackagedModelDocument::compile_locked(
        &changed_store,
        &changed_resolution,
        "Main",
        ExactModelCodec::V1,
    )
    .expect("changed-source compilation");
    assert_eq!(changed.model().digest(), packaged.model().digest());
    assert_ne!(changed.compilation(), packaged.compilation());
    let (changed_realization, changed_run) =
        accepted_run(&changed, ScalarEllipticMethod::FiniteElement, 8);
    assert_eq!(changed_realization, realization);
    assert_eq!(changed_run, run);
    let changed_binding = changed
        .bind_execution_v2(&changed_realization, &changed_run)
        .expect("changed-source binding");
    assert_ne!(
        changed_binding.digest().expect("changed binding digest"),
        binding.digest().expect("binding digest")
    );
    assert!(
        changed
            .validate_execution_v2_binding(&binding, &realization, &run, &changed_resolution,)
            .is_err()
    );

    // The concrete composition barrier distinguishes the three Model links
    // carried by a locally valid Realization wire.
    let realization_json: serde_json::Value =
        serde_json::from_slice(&realization_bytes).expect("Realization value");
    let mutate_realization = |field: &str, value: serde_json::Value| {
        let mut changed = realization_json.clone();
        changed[field] = value;
        RealizationEnvelopeV1::from_json(
            &serde_json::to_vec(&changed).expect("changed Realization JSON"),
            Default::default(),
        )
        .expect("locally valid changed Realization")
    };
    let changed_model_digest =
        mutate_realization("model_sha256", serde_json::Value::String("cd".repeat(32)));
    assert!(matches!(
        packaged.bind_execution_v2(&changed_model_digest, &run),
        Err(PackageExecutionBindingError::RealizationModelMismatch { .. })
    ));
    let changed_revision = mutate_realization(
        "semantic_revision",
        serde_json::Value::from(realization.semantic_revision().get() + 1),
    );
    assert!(matches!(
        packaged.bind_execution_v2(&changed_revision, &run),
        Err(PackageExecutionBindingError::RealizationRevisionMismatch { .. })
    ));
    let changed_ontology = mutate_realization(
        "model_ulid",
        serde_json::Value::String("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned()),
    );
    assert_ne!(
        changed_ontology.model().expect("changed ontology"),
        realization.model().expect("original ontology")
    );
    assert!(matches!(
        packaged.bind_execution_v2(&changed_ontology, &run),
        Err(PackageExecutionBindingError::RealizationOntologyMismatch)
    ));

    // A different valid Realization/Run chain is not interchangeable with the
    // one named by the binding.
    let (different_realization, different_run) =
        accepted_run(&packaged, ScalarEllipticMethod::FiniteVolume, 8);
    assert!(
        packaged
            .validate_execution_v2_binding(
                &binding,
                &different_realization,
                &different_run,
                &resolution,
            )
            .is_err()
    );

    // Output and producer identity are Run meaning. Each valid substitution
    // therefore changes the Run digest and fails exact lineage replay.
    let changed_output = run
        .clone()
        .with_output(ArtifactDigest::from_hex("ab".repeat(32)).expect("output digest"));
    changed_output
        .validate_against(&realization)
        .expect("changed-output Run remains locally valid");
    assert!(
        packaged
            .validate_execution_v2_binding(&binding, &realization, &changed_output, &resolution,)
            .is_err()
    );
    let changed_execution = ExecutionProvenanceV1::new(
        "eqiora.host.alternate",
        env!("CARGO_PKG_VERSION"),
        "eqiora.alternate.cg",
        env!("CARGO_PKG_VERSION"),
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::MIN,
        },
        ReductionPolicy::Reproducible,
    )
    .expect("alternate execution");
    let changed_run =
        RunManifestV2::new(&realization, changed_execution).expect("alternate valid Run");
    assert!(
        packaged
            .validate_execution_v2_binding(&binding, &realization, &changed_run, &resolution,)
            .is_err()
    );
    let changed_topology = ExecutionProvenanceV1::new(
        "eqiora.host.alternate",
        env!("CARGO_PKG_VERSION"),
        "eqiora.alternate.cg",
        env!("CARGO_PKG_VERSION"),
        ExecutionTopologyV1::Host {
            workers: NonZeroUsize::new(2).expect("two workers"),
        },
        ReductionPolicy::Reproducible,
    )
    .expect("alternate topology evidence");
    assert!(RunManifestV2::new(&realization, changed_topology).is_err());

    let expected: serde_json::Value =
        serde_json::from_slice(EXPECTED).expect("expected identities");
    let expected_fields = expected
        .as_object()
        .expect("expected identities must be an object")
        .keys()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        expected_fields,
        [
            "model_sha256",
            "package_compilation_sha256",
            "package_execution_binding_sha256",
            "package_semantic_sha256",
            "realization_sha256",
            "resolution_sha256",
            "run_sha256",
            "schema",
            "source_bundle_sha256",
        ]
        .into_iter()
        .collect()
    );
    assert_eq!(
        expected["schema"],
        "eqiora.verify.typed-package-execution-lineage-identities.v1"
    );
    assert_eq!(
        expected["package_semantic_sha256"],
        identity.semantic_digest.to_hex()
    );
    assert_eq!(
        expected["source_bundle_sha256"],
        release.source_digest().expect("source digest").to_hex()
    );
    assert_eq!(
        expected["resolution_sha256"],
        resolution.digest().expect("resolution digest").to_hex()
    );
    assert_eq!(
        expected["model_sha256"],
        packaged.model().digest().expect("Model digest")
    );
    assert_eq!(
        expected["package_compilation_sha256"],
        packaged
            .compilation()
            .digest()
            .expect("compilation digest")
            .to_hex()
    );
    assert_eq!(
        expected["realization_sha256"],
        realization
            .digest()
            .expect("Realization digest")
            .to_string()
    );
    assert_eq!(
        expected["run_sha256"],
        run.digest().expect("Run digest").to_string()
    );
    assert_eq!(
        expected["package_execution_binding_sha256"],
        binding.digest().expect("binding digest").to_hex()
    );
}
