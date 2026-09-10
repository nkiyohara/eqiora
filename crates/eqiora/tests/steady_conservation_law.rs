//! Exact package composition preserves physical Law meaning independently of residuals.
use eqiora::api::ModelDocument;
use eqiora::package::{
    BundleEntryV1, BundleRoleV1, ExactVersion, InMemoryPackageStore, NormalizedRelativePath,
    PackageManifestV1, PackageSourcesV1, PackagedModelDocument, QualifiedName, ResolutionRecordV1,
    SourceFileV1, prepare_package_release_v1,
};

const SOURCE: &str = r#"
public component Balance(
  support body: volume(ambient_dimension=1),
  variable value: K on body,
  parameter conductivity: kg*m/s^3/K,
  parameter production: kg/m/s^3
) {
  law heat on body { flux -conductivity*grad(value); source production; }
}
model heated_interval() {
  domain body=box(0,1);
  variable value: K on body;
  parameter conductivity: kg*m/s^3/K=2;
  parameter production: kg/m/s^3=4;
  instance balance: Balance(body=body,value=value,conductivity=conductivity,production=production);
}
"#;

#[test]
fn exact_package_replay_keeps_law_distinct_from_its_balance_equation() {
    let direct = ModelDocument::compile("law.eqi", SOURCE).unwrap();
    let path = NormalizedRelativePath::parse("src/models/law.eqi").unwrap();
    let manifest = PackageManifestV1::new(
        "models.law",
        QualifiedName::parse("org.eqiora.test.SteadyLaw").unwrap(),
        ExactVersion::parse("1.0.0").unwrap(),
        vec![],
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .unwrap();
    let sources = PackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            SOURCE.as_bytes().to_vec(),
        )],
    )
    .unwrap();
    let release = prepare_package_release_v1(sources, &[]).unwrap();
    let mut store = InMemoryPackageStore::default();
    store.insert(&release).unwrap();
    let resolution = ResolutionRecordV1::from_exact_releases(&release, &[]).unwrap();
    let packaged =
        PackagedModelDocument::compile_locked(&store, &resolution, "heated_interval").unwrap();
    assert!(packaged.model().structurally_equivalent(&direct).unwrap());
    // Same mathematical operands, but an ordinary equation does not own physical terms.
    let equation = SOURCE.replace(
        "law heat on body { flux -conductivity*grad(value); source production; }",
        "relation heat on body { div(-conductivity*grad(value)) = production; }",
    );
    let ordinary = ModelDocument::compile("equation.eqi", &equation).unwrap();
    assert!(!direct.structurally_equivalent(&ordinary).unwrap());
}
