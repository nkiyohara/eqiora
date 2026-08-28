use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleRoleV1, InMemoryPackageStore,
    ModelPackageIdentityV1, NormalizedRelativePath, PackageCompilationRecordV1, PackageReleaseV1,
    PackagedModelDocument, ResolutionNodeV1, ResolutionRecordV1, SourceFileV1,
    prepare_package_release_v1,
};

const MANIFEST: &[u8] = include_bytes!("../../../packages/org.example.poisson/package.json");
const SOURCE: &str = include_str!("../../../packages/org.example.poisson/src/main.eqi");
const README: &[u8] = include_bytes!("../../../packages/org.example.poisson/README.md");
const EXPECTED: &[u8] =
    include_bytes!("../../../verify/packages/typed-compilation-lineage/expected/identities.json");
const SOURCE_PATH: &str = "src/main.eqi";

fn package_release(readme: &[u8], reverse_files: bool) -> PackageReleaseV1 {
    let manifest = AuthorManifestV1::from_json(MANIFEST).expect("manifest");
    let mut files = vec![
        SourceFileV1::new(
            NormalizedRelativePath::parse("README.md").expect("README path"),
            BundleRoleV1::Documentation,
            readme.to_vec(),
        ),
        SourceFileV1::new(
            NormalizedRelativePath::parse(SOURCE_PATH).expect("source path"),
            BundleRoleV1::ModelSource,
            SOURCE.as_bytes().to_vec(),
        ),
    ];
    if reverse_files {
        files.reverse();
    }
    prepare_package_release_v1(
        AuthorPackageSourcesV1::new(manifest, files).expect("closed source inventory"),
        &[],
    )
    .expect("compiler-derived release")
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

#[test]
fn exact_package_compilation_and_model_replay_retain_identity() {
    let release = package_release(README, false);
    let (store, resolution, identity) = install(&release);
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
        .expect("locked package compilation");

    let compilation_bytes = packaged.compilation().canonical_json().unwrap();
    let replayed_compilation = PackageCompilationRecordV1::from_json(&compilation_bytes).unwrap();
    replayed_compilation.validate_against(&resolution).unwrap();
    assert_eq!(&replayed_compilation, packaged.compilation());
    let model_bytes = packaged.model().canonical_json().unwrap();
    let replayed_model = eqiora::api::ModelDocument::replay(&model_bytes).unwrap();
    assert_eq!(replayed_model.canonical_json().unwrap(), model_bytes);
    assert_eq!(replayed_model.digest(), packaged.model().digest());

    let permuted_release = package_release(README, true);
    assert_eq!(
        release.canonical_json().unwrap(),
        permuted_release.canonical_json().unwrap()
    );
    let (permuted_store, permuted_resolution, _) = install(&permuted_release);
    let permuted =
        PackagedModelDocument::compile_locked(&permuted_store, &permuted_resolution, "Main")
            .unwrap();
    assert_eq!(permuted.model().digest(), packaged.model().digest());
    assert_eq!(permuted.compilation(), packaged.compilation());

    let changed_release = package_release(b"different documentation\n", false);
    let (changed_store, changed_resolution, changed_identity) = install(&changed_release);
    let changed =
        PackagedModelDocument::compile_locked(&changed_store, &changed_resolution, "Main").unwrap();
    assert_eq!(changed_identity, identity);
    assert_ne!(changed_release.source_digest(), release.source_digest());
    assert_eq!(changed.model().digest(), packaged.model().digest());
    assert_ne!(changed.compilation(), packaged.compilation());

    let expected: serde_json::Value = serde_json::from_slice(EXPECTED).unwrap();
    assert_eq!(
        expected["schema"],
        "eqiora.verify.typed-package-compilation-lineage-identities.v1"
    );
    assert_eq!(
        expected["package_semantic_sha256"],
        identity.semantic_digest.to_hex()
    );
    assert_eq!(
        expected["source_bundle_sha256"],
        release.source_digest().unwrap().to_hex()
    );
    assert_eq!(
        expected["resolution_sha256"],
        resolution.digest().unwrap().to_hex()
    );
    assert_eq!(expected["model_sha256"], packaged.model().digest().unwrap());
    assert_eq!(
        expected["package_compilation_sha256"],
        packaged.compilation().digest().unwrap().to_hex()
    );
}
