use std::path::{Path, PathBuf};

use eqiora::compatibility::ExactModelCodec;
use eqiora::kernel::BoundarySide;
use eqiora::package::{
    AuthorManifestV1, AuthorPackageDirectory, AuthorPackageSourcesV1, BundleEntryV1, BundleRoleV1,
    DependencyRequirementV1, ExactVersion, InMemoryPackageStore, NormalizedRelativePath,
    PackagePreparationError, PackageReleaseV1, PackagedModelDocument, QualifiedName,
    ResolutionRecordV1, SourceFileV1, prepare_package_release_v1,
};
use eqiora_numerics::{AleFsiCartesianModel3d, lower_ale_fsi_cartesian_3d};

const PACKAGED_ROOT: &str =
    include_str!("../../../verify/fsi/fixed-topology-ale-monolithic-3d/models/packaged.eqi");
const DIRECT_SOURCE: &str =
    include_str!("../../../verify/fsi/fixed-topology-ale-monolithic-3d/models/direct.eqi");
const MECHANICS_SEMANTIC_DIGEST: &str =
    "5bff8e5ac0adee425bbaff949ffedc29c455966b074352c7149816d4b63b50d7";
const MECHANICS_SOURCE_DIGEST: &str =
    "6dce490b9b5d4407edd281c0a0c3b5bd1d9d6774d7400fb27d06fe3e750a24b7";
const FLUID_SEMANTIC_DIGEST: &str =
    "411f97ec6e4d8f99b001b4c5dddcfbd7cfdefb8df93e14474f3679949ad8792d";
const FLUID_SOURCE_DIGEST: &str =
    "568215efa7195a3cb2033cf441c52527b604e3ef7167897cb24330a76a460463";
const SOLID_SEMANTIC_DIGEST: &str =
    "cc0febf7ee9c2ec8a5cca24904dcd9df92e75c89b7597b4fffb4915b8bd5ab6a";
const SOLID_SOURCE_DIGEST: &str =
    "4ba59f5d0bac61a683b8552f16685b0adeba2686afb3498741dcdfb252a7a63d";

fn release_root(package: &str, version: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/releases")
        .join(package)
        .join(version)
}

fn current_release_root(package: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages")
        .join(package)
}

fn prepare_release(root: &Path, dependencies: &[PackageReleaseV1]) -> PackageReleaseV1 {
    let sources = AuthorPackageDirectory::open_ambient(root)
        .unwrap_or_else(|error| panic!("open package release {}: {error}", root.display()))
        .read_sources()
        .unwrap_or_else(|error| panic!("read package release {}: {error}", root.display()));
    prepare_package_release_v1(sources, dependencies)
        .unwrap_or_else(|error| panic!("prepare package release {}: {error:?}", root.display()))
}

fn assert_release_identity(
    release: &PackageReleaseV1,
    version: &str,
    semantic_digest: &str,
    source_digest: &str,
) {
    let identity = release.package_identity().expect("package identity");
    assert_eq!(identity.version.as_str(), version);
    assert_eq!(identity.semantic_digest.to_hex(), semantic_digest);
    assert_eq!(
        release.source_digest().expect("source digest").to_hex(),
        source_digest
    );
}

#[test]
fn prepares_immutable_three_dimensional_mechanics_release() {
    let release = prepare_release(&release_root("Eqiora.Mechanics.Interfaces", "0.2.0"), &[]);
    let identity = release.package_identity().expect("mechanics identity");
    assert_eq!(identity.semantic_digest.to_hex(), MECHANICS_SEMANTIC_DIGEST);
    assert_eq!(identity.name.as_str(), "Eqiora.Mechanics.Interfaces");
    assert_eq!(identity.version.as_str(), "0.2.0");
    assert_eq!(
        release.source_digest().expect("source digest").to_hex(),
        MECHANICS_SOURCE_DIGEST
    );
}

#[test]
fn prepares_method_neutral_three_dimensional_fluid_and_solid_releases() {
    let mechanics = prepare_release(&release_root("Eqiora.Mechanics.Interfaces", "0.2.0"), &[]);
    for (package, version, semantic_digest, source_digest) in [
        (
            "Eqiora.Fluid.Incompressible",
            "0.3.0",
            FLUID_SEMANTIC_DIGEST,
            FLUID_SOURCE_DIGEST,
        ),
        (
            "Eqiora.Solid.LinearElasticity",
            "0.5.0",
            SOLID_SEMANTIC_DIGEST,
            SOLID_SOURCE_DIGEST,
        ),
    ] {
        let release = prepare_release(
            &release_root(package, version),
            std::slice::from_ref(&mechanics),
        );
        let identity = release.package_identity().expect("package identity");
        assert_eq!(identity.name.as_str(), package);
        assert_eq!(identity.version.as_str(), version);
        assert_eq!(identity.semantic_digest.to_hex(), semantic_digest);
        assert_eq!(
            release.source_digest().expect("source digest").to_hex(),
            source_digest
        );
    }
}

#[test]
fn published_two_dimensional_release_identities_remain_unchanged() {
    let mechanics = prepare_release(&current_release_root("Eqiora.Mechanics.Interfaces"), &[]);
    assert_release_identity(
        &mechanics,
        "0.1.0",
        "f8c5b9000415d3288a68377d507d16b3524bf17a3aa0a54aee9b003d187534f4",
        "407744105ebeb9577944169cae56a44eec30565050588dc2407461d7cf43725d",
    );
    let fluid = prepare_release(
        &current_release_root("Eqiora.Fluid.Incompressible"),
        std::slice::from_ref(&mechanics),
    );
    assert_release_identity(
        &fluid,
        "0.2.0",
        "39a8eadba1f1c0028d23b42f506b6899320f46e4ef7ba7b45dec3e0524d2c01b",
        "69ac5967d961c2ae4aa558ee020020093329f0050397d54893e465a3ff22eaba",
    );
    let solid = prepare_release(
        &current_release_root("Eqiora.Solid.LinearElasticity"),
        std::slice::from_ref(&mechanics),
    );
    assert_release_identity(
        &solid,
        "0.4.0",
        "35fd309d5fc8287f526482a1843bf936d52a71046b5fadd68bf8d9b3aecbcfc3",
        "9776d28253484ee8898554ddf9a0fa5fe2f590de66922e61a04494fe6cf1c043",
    );
}

#[test]
fn exact_package_graph_lowers_to_three_dimensional_ale_fsi_roles() {
    let mechanics = prepare_release(&release_root("Eqiora.Mechanics.Interfaces", "0.2.0"), &[]);
    let fluid = prepare_release(
        &release_root("Eqiora.Fluid.Incompressible", "0.3.0"),
        std::slice::from_ref(&mechanics),
    );
    let solid = prepare_release(
        &release_root("Eqiora.Solid.LinearElasticity", "0.5.0"),
        std::slice::from_ref(&mechanics),
    );
    let root = prepare_root(&mechanics, &fluid, &solid);
    let resolution = ResolutionRecordV1::from_exact_releases(
        &root,
        &[mechanics.clone(), fluid.clone(), solid.clone()],
    )
    .expect("derive exact offline resolution");
    let mut store = InMemoryPackageStore::default();
    for release in [&mechanics, &fluid, &solid, &root] {
        store.insert(release).expect("install exact release");
    }
    let document =
        PackagedModelDocument::compile_locked(&store, &resolution, "Main", ExactModelCodec::V5)
            .expect("compile exact 3D FSI package graph offline");
    let model = lower_ale_fsi_cartesian_3d(document.model().program())
        .expect("package-expanded canonical 3D ALE FSI roles lower");
    let direct = ExactModelCodec::V5
        .compile("direct-ale-fsi-3d.eqi", DIRECT_SOURCE)
        .expect("direct 3D ALE FSI Model compiles");
    let direct_model = lower_ale_fsi_cartesian_3d(direct.program())
        .expect("direct canonical 3D ALE FSI roles lower");
    assert_same_canonical_roles(&direct_model, &model);

    assert_eq!(model.fluid().bounds(), &[[0.0, 1.0]; 3]);
    assert_eq!(
        model.solid().bounds(),
        &[[1.0, 2.0], [0.0, 1.0], [0.0, 1.0]]
    );
    assert_eq!(model.fluid().mass_density(), 1.0);
    assert_eq!(model.fluid().dynamic_viscosity(), 0.2);
    assert_eq!(model.solid().mass_density(), 1.0);
    assert_eq!(model.solid().shear_modulus(), 2.0);
    assert_eq!(model.solid().first_lame_parameter(), 1.0);
    assert_eq!(model.interface().axis(), 0);
    assert_eq!(model.interface().fluid().side(), BoundarySide::Upper);
    assert_eq!(model.interface().solid().side(), BoundarySide::Lower);
    assert_eq!(
        model
            .fluid()
            .conservative_body_force(&[0.25, 0.5, 0.75])
            .expect("fluid force"),
        [0.0; 3]
    );
    assert_eq!(
        model
            .solid()
            .conservative_body_force(&[1.25, 0.5, 0.75])
            .expect("solid force"),
        [0.0; 3]
    );
}

fn assert_same_canonical_roles(direct: &AleFsiCartesianModel3d, packaged: &AleFsiCartesianModel3d) {
    assert_eq!(direct.fluid().bounds(), packaged.fluid().bounds());
    assert_eq!(direct.solid().bounds(), packaged.solid().bounds());
    assert_eq!(
        direct.fluid().mass_density().to_bits(),
        packaged.fluid().mass_density().to_bits()
    );
    assert_eq!(
        direct.fluid().dynamic_viscosity().to_bits(),
        packaged.fluid().dynamic_viscosity().to_bits()
    );
    assert_eq!(
        direct.solid().mass_density().to_bits(),
        packaged.solid().mass_density().to_bits()
    );
    assert_eq!(
        direct.solid().shear_modulus().to_bits(),
        packaged.solid().shear_modulus().to_bits()
    );
    assert_eq!(
        direct.solid().first_lame_parameter().to_bits(),
        packaged.solid().first_lame_parameter().to_bits()
    );
    assert_eq!(direct.interface().axis(), packaged.interface().axis());
    assert_eq!(
        direct.interface().fluid().side(),
        packaged.interface().fluid().side()
    );
    assert_eq!(
        direct.interface().solid().side(),
        packaged.interface().solid().side()
    );
    assert_eq!(
        direct
            .fluid()
            .conservative_body_force(&[0.25, 0.5, 0.75])
            .expect("direct fluid force"),
        packaged
            .fluid()
            .conservative_body_force(&[0.25, 0.5, 0.75])
            .expect("packaged fluid force")
    );
    assert_eq!(
        direct
            .solid()
            .conservative_body_force(&[1.25, 0.5, 0.75])
            .expect("direct solid force"),
        packaged
            .solid()
            .conservative_body_force(&[1.25, 0.5, 0.75])
            .expect("packaged solid force")
    );
}

#[test]
fn new_releases_reject_the_old_mechanics_identity_before_elaboration() {
    let old_mechanics = prepare_release(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/Eqiora.Mechanics.Interfaces"),
        &[],
    );
    let error = prepare_release_result(
        &release_root("Eqiora.Fluid.Incompressible", "0.3.0"),
        std::slice::from_ref(&old_mechanics),
    )
    .expect_err("exact dependency digest mismatch must fail closed");
    assert!(matches!(
        error,
        PackagePreparationError::MissingDependency { .. }
    ));
}

fn prepare_root(
    mechanics: &PackageReleaseV1,
    fluid: &PackageReleaseV1,
    solid: &PackageReleaseV1,
) -> PackageReleaseV1 {
    let dependencies = [
        ("mechanics", mechanics),
        ("fluid_laws", fluid),
        ("solid_laws", solid),
    ]
    .into_iter()
    .map(|(alias, release)| {
        DependencyRequirementV1::new(
            QualifiedName::parse(alias).expect("dependency alias"),
            release.package_identity().expect("dependency identity"),
        )
        .expect("exact dependency")
    })
    .collect();
    let readme = NormalizedRelativePath::parse("README.md").expect("README path");
    let source = NormalizedRelativePath::parse("src/main.eqi").expect("source path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.eqiora.verify.ale_fsi_3d_packages").expect("root name"),
        ExactVersion::parse("0.1.0").expect("root version"),
        dependencies,
        vec![
            BundleEntryV1::new(readme.clone(), BundleRoleV1::Documentation),
            BundleEntryV1::new(source.clone(), BundleRoleV1::ModelSource),
        ],
    )
    .expect("root manifest");
    let sources = AuthorPackageSourcesV1::new(
        manifest,
        vec![
            SourceFileV1::new(
                readme,
                BundleRoleV1::Documentation,
                b"Exact-package authoring fixture for canonical tetrahedral ALE FSI.\n".to_vec(),
            ),
            SourceFileV1::new(
                source,
                BundleRoleV1::ModelSource,
                PACKAGED_ROOT.as_bytes().to_vec(),
            ),
        ],
    )
    .expect("closed root source inventory");
    prepare_package_release_v1(sources, &[mechanics.clone(), fluid.clone(), solid.clone()])
        .expect("prepare exact 3D FSI root")
}

fn prepare_release_result(
    root: &Path,
    dependencies: &[PackageReleaseV1],
) -> Result<PackageReleaseV1, eqiora::package::PackagePreparationError> {
    let sources = AuthorPackageDirectory::open_ambient(root)
        .unwrap_or_else(|error| panic!("open package release {}: {error}", root.display()))
        .read_sources()
        .unwrap_or_else(|error| panic!("read package release {}: {error}", root.display()));
    prepare_package_release_v1(sources, dependencies)
}
