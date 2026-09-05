use eqiora::package::{
    BundleRoleV1, InMemoryPackageStore, ModelPackageIdentityV1, NormalizedRelativePath,
    PackageCompilationRecordV2, PackageManifestV1, PackageReleaseV1, PackageSourcesV1,
    PackagedModelDocument, ResolutionNodeV1, ResolutionRecordV1, SourceFileV1,
    prepare_package_release_v1,
};

const MANIFEST: &[u8] = include_bytes!("../../../packages/org.example.poisson/package.json");
const SOURCE: &str = include_str!("../../../packages/org.example.poisson/src/main.eqi");
const README: &[u8] = include_bytes!("../../../packages/org.example.poisson/README.md");
const SOURCE_PATH: &str = "src/main.eqi";

fn package_release(source: &str, readme: &[u8], reverse_files: bool) -> PackageReleaseV1 {
    let manifest = PackageManifestV1::from_json(MANIFEST).expect("manifest");
    let mut files = vec![
        SourceFileV1::new(
            NormalizedRelativePath::parse("README.md").expect("README path"),
            BundleRoleV1::Documentation,
            readme.to_vec(),
        ),
        SourceFileV1::new(
            NormalizedRelativePath::parse(SOURCE_PATH).expect("source path"),
            BundleRoleV1::ModelSource,
            source.as_bytes().to_vec(),
        ),
    ];
    if reverse_files {
        files.reverse();
    }
    prepare_package_release_v1(
        PackageSourcesV1::new(manifest, files).expect("closed source inventory"),
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
    let release = package_release(SOURCE, README, false);
    let (store, resolution, identity) = install(&release);
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
        .expect("locked package compilation");

    let compilation_bytes = packaged.compilation().canonical_json().unwrap();
    let replayed_compilation = PackageCompilationRecordV2::from_json(&compilation_bytes).unwrap();
    replayed_compilation.validate_against(&resolution).unwrap();
    assert_eq!(&replayed_compilation, packaged.compilation());
    let model_bytes = packaged.model().canonical_json().unwrap();
    let replayed_model = eqiora::api::ModelDocument::replay(&model_bytes).unwrap();
    assert_eq!(replayed_model.canonical_json().unwrap(), model_bytes);
    assert_eq!(replayed_model.digest(), packaged.model().digest());

    let permuted_release = package_release(SOURCE, README, true);
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

    let changed_release = package_release(SOURCE, b"different documentation\n", false);
    let (changed_store, changed_resolution, changed_identity) = install(&changed_release);
    let changed =
        PackagedModelDocument::compile_locked(&changed_store, &changed_resolution, "Main").unwrap();
    assert_eq!(changed_identity, identity);
    assert_ne!(changed_release.source_digest(), release.source_digest());
    assert_eq!(changed.model().digest(), packaged.model().digest());
    assert_ne!(changed.compilation(), packaged.compilation());
}

#[test]
fn rational_package_dimensions_keep_model_meaning_separate_from_package_structure() {
    let source = "public model Main { parameter amplitude: m ^ (-1 / 2) = 1; relation r continuous { amplitude = 0; } }";
    let release = package_release(source, README, false);
    let equivalent = package_release(&source.replace("-1 / 2", "-2 / 4"), README, false);
    let changed = package_release(&source.replace("-1 / 2", "-1 / 3"), README, false);
    // Package canonicalization preserves expression structure (RFC 0022).
    // Exact dimension equivalence belongs to the compiled Model instead.
    assert_ne!(
        release.package_identity().unwrap(),
        equivalent.package_identity().unwrap()
    );
    assert_ne!(release.source_digest(), equivalent.source_digest());
    assert_ne!(
        release.package_identity().unwrap(),
        changed.package_identity().unwrap()
    );

    let (store, resolution, _) = install(&release);
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main").unwrap();
    let direct = eqiora::api::ModelDocument::compile("wave.eqi", source).unwrap();
    assert!(packaged.model().structurally_equivalent(&direct).unwrap());
    let (equivalent_store, equivalent_resolution, _) = install(&equivalent);
    let equivalent_model =
        PackagedModelDocument::compile_locked(&equivalent_store, &equivalent_resolution, "Main")
            .unwrap();
    assert!(
        packaged
            .model()
            .structurally_equivalent(equivalent_model.model())
            .unwrap()
    );
    let (changed_store, changed_resolution, _) = install(&changed);
    let changed_model =
        PackagedModelDocument::compile_locked(&changed_store, &changed_resolution, "Main").unwrap();
    assert!(
        !packaged
            .model()
            .structurally_equivalent(changed_model.model())
            .unwrap()
    );
    let replay =
        eqiora::api::ModelDocument::replay(&packaged.model().canonical_json().unwrap()).unwrap();
    assert!(replay.structurally_equivalent(&direct).unwrap());
}
