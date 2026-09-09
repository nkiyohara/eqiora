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
        "public dimension Speed = Length / s; dimension Length = m; public operator half(input value: Speed): Speed = 0.5 * value; public component Law(parameter value: m / s) { relation law { value = 0; } }", vec![]), &[]).unwrap();
    let library = PackageReleaseV1::from_json(&library.canonical_json().unwrap()).unwrap();
    let root = prepare_package_release_v1(sources("org.example.consumer",
        "import org.example.dimensions.main as units; dimension Velocity = units.Speed; model Main() { parameter x: Velocity = 0; instance same: units.Law(value = x); variable velocity: Velocity; relation scale { velocity = units.half(value = 4[m/s]); } }",
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

#[test]
fn dimensioned_property_contract_and_release_reach_an_independent_locked_consumer() {
    let provider = prepare_package_release_v1(
        sources(
            "org.example.properties",
            "public dimension Speed = m/s; public property contract SpeedValue(): Speed { derivatives value_only; } public property release ReferenceSpeed implements SpeedValue { value = 2; source_unit: Speed = 1; validity = unconditional; citation = org.example.measurement; license = spdx.CC0_1_0; }",
            vec![],
        ),
        &[],
    )
    .unwrap();
    let root = prepare_package_release_v1(
        sources(
            "org.example.consumer",
            "import org.example.properties.main as props; public component Law(property speed: props.SpeedValue) { variable velocity: props.Speed; relation law { velocity = speed; } } model Main() { instance law: Law(speed = props.ReferenceSpeed); }",
            vec![PackageDependencyV1::new(provider.package_identity().unwrap())],
        ),
        std::slice::from_ref(&provider),
    )
    .unwrap();
    let resolution =
        ResolutionRecordV1::from_exact_releases(&root, std::slice::from_ref(&provider)).unwrap();
    let mut store = InMemoryPackageStore::default();
    for release in [root, provider] {
        store
            .insert(&PackageReleaseV1::from_json(&release.canonical_json().unwrap()).unwrap())
            .unwrap();
    }
    let compiled = PackagedModelDocument::compile_locked(&store, &resolution, "Main").unwrap();
    let bindings = compiled.property_bindings().collect::<Vec<_>>();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].5.component(0), Some((2.0, 0.0)));
    assert_eq!(
        bindings[0].5.value_type().dimension(),
        eqiora::DimExponents::from_integers([0, 1, -1, 0, 0, 0, 0]).unwrap()
    );
    compiled
        .compilation()
        .validate_against(&resolution)
        .unwrap();
}
