use eqiora::package::{
    BundleEntryV1, BundleRoleV1, ExactVersion, InMemoryPackageStore, NormalizedRelativePath,
    PackageDependencyV1, PackageManifestV1, PackageReleaseV1, PackageSourcesV1,
    PackagedModelDocument, QualifiedName, ResolutionRecordV1, SourceFileV1,
    prepare_package_release_v1,
};

fn sources(name: &str, source: &str, dependencies: Vec<PackageDependencyV1>) -> PackageSourcesV1 {
    let path = NormalizedRelativePath::parse("src/main.eqi").unwrap();
    let manifest = PackageManifestV1::new(
        "main",
        QualifiedName::parse(name).unwrap(),
        ExactVersion::parse("0.1.0").unwrap(),
        dependencies,
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .unwrap();
    PackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            source.as_bytes().to_vec(),
        )],
    )
    .unwrap()
}

#[test]
fn exact_dimension_packages_reopen_and_compile_without_nominalizing_dimensions() {
    let library = prepare_package_release_v1(sources("org.example.dimensions",
        "public dimension Speed = Length / s; dimension Length = m; public component Law(parameter value: m / s) { relation law { value = 0; } }", vec![]), &[]).unwrap();
    let library = PackageReleaseV1::from_json(&library.canonical_json().unwrap()).unwrap();
    let root = prepare_package_release_v1(sources("org.example.consumer",
        "import org.example.dimensions.main as units; dimension Velocity = units.Speed; model Main() { parameter x: Velocity = 0; instance same: units.Law(value = x); }",
        vec![PackageDependencyV1::new(library.package_identity().unwrap())]), std::slice::from_ref(&library)).unwrap();
    let root = PackageReleaseV1::from_json(&root.canonical_json().unwrap()).unwrap();
    let resolution =
        ResolutionRecordV1::from_exact_releases(&root, std::slice::from_ref(&library)).unwrap();
    let mut store = InMemoryPackageStore::default();
    store.insert(&root).unwrap();
    store.insert(&library).unwrap();
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "Main").unwrap();
    packaged
        .compilation()
        .validate_against(&resolution)
        .unwrap();
    assert_eq!(packaged.compilation().packages().len(), 2);
    // Repeated fresh compilation must consume the same exact locked source graph.
    let reopened = PackagedModelDocument::compile_locked(&store, &resolution, "Main").unwrap();
    assert_eq!(
        packaged.model().digest().unwrap(),
        reopened.model().digest().unwrap()
    );
}
