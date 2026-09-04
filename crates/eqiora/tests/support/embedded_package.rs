#![allow(dead_code)] // Each integration-test crate consumes a subset of the shared fixtures.

use eqiora::package::{
    BundleEntryV1, BundleRoleV1, ExactVersion, NormalizedRelativePath, PackageManifestV1,
    PackageSourcesV1, QualifiedName, SourceFileV1,
};

/// Reconstruct an exact checked-in package inventory without granting tests
/// ambient filesystem authority at runtime.
pub(crate) fn sources(
    manifest_json: &[u8],
    files: &[(&str, BundleRoleV1, &[u8])],
) -> PackageSourcesV1 {
    let manifest = PackageManifestV1::from_json(manifest_json).expect("embedded package manifest");
    let files = files
        .iter()
        .map(|(path, role, contents)| {
            SourceFileV1::new(
                NormalizedRelativePath::parse(*path).expect("embedded package path"),
                *role,
                contents.to_vec(),
            )
        })
        .collect();
    PackageSourcesV1::new(manifest, files).expect("closed embedded package inventory")
}

pub(crate) fn generated_sources(
    name: &str,
    version: &str,
    files: &[(&str, BundleRoleV1, &[u8])],
) -> PackageSourcesV1 {
    let entry = files
        .iter()
        .find(|(_, role, _)| *role == BundleRoleV1::ModelSource)
        .expect("embedded model source")
        .0
        .strip_prefix("src/")
        .unwrap()
        .strip_suffix(".eqi")
        .unwrap()
        .replace('/', ".");
    let bundle = files
        .iter()
        .map(|(path, role, _)| {
            BundleEntryV1::new(
                NormalizedRelativePath::parse(*path).expect("embedded package path"),
                *role,
            )
        })
        .collect();
    let manifest = PackageManifestV1::new(
        &entry,
        QualifiedName::parse(name).expect("embedded package name"),
        ExactVersion::parse(version).expect("embedded package version"),
        Vec::new(),
        bundle,
    )
    .expect("generated package manifest");
    let source_files = files
        .iter()
        .map(|(path, role, bytes)| {
            SourceFileV1::new(
                NormalizedRelativePath::parse(*path).expect("embedded package path"),
                *role,
                bytes.to_vec(),
            )
        })
        .collect();
    PackageSourcesV1::new(manifest, source_files).expect("embedded package sources")
}

/// Return one exact public package from compile-time embedded repository bytes.
pub(crate) fn public_sources(package: &str) -> PackageSourcesV1 {
    match package {
        "Eqiora.Mechanics.Interfaces" => sources(
            include_bytes!("../../../../packages/Eqiora.Mechanics.Interfaces/package.json"),
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/Eqiora.Mechanics.Interfaces/README.md"),
                ),
                (
                    "src/interfaces.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/Eqiora.Mechanics.Interfaces/src/interfaces.eqi"
                    ),
                ),
            ],
        ),
        "Eqiora.Mechanics.BoundaryLoads" => sources(
            include_bytes!("../../../../packages/Eqiora.Mechanics.BoundaryLoads/package.json"),
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/Eqiora.Mechanics.BoundaryLoads/README.md"),
                ),
                (
                    "src/boundary_loads.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/Eqiora.Mechanics.BoundaryLoads/src/boundary_loads.eqi"
                    ),
                ),
            ],
        ),
        "Eqiora.Fluid.Incompressible" => sources(
            include_bytes!("../../../../packages/Eqiora.Fluid.Incompressible/package.json"),
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/Eqiora.Fluid.Incompressible/README.md"),
                ),
                (
                    "src/incompressible.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi"
                    ),
                ),
            ],
        ),
        "Eqiora.Fluid.InertialStokes" => sources(
            include_bytes!("../../../../packages/Eqiora.Fluid.InertialStokes/package.json"),
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/Eqiora.Fluid.InertialStokes/README.md"),
                ),
                (
                    "src/inertial_stokes.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/Eqiora.Fluid.InertialStokes/src/inertial_stokes.eqi"
                    ),
                ),
            ],
        ),
        "Eqiora.Solid.LinearElasticity" => sources(
            include_bytes!("../../../../packages/Eqiora.Solid.LinearElasticity/package.json"),
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/Eqiora.Solid.LinearElasticity/README.md"),
                ),
                (
                    "src/linear_elasticity.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/Eqiora.Solid.LinearElasticity/src/linear_elasticity.eqi"
                    ),
                ),
            ],
        ),
        other => panic!("unsupported embedded public package `{other}`"),
    }
}

/// Return one exact immutable package release from compile-time embedded bytes.
pub(crate) fn release_sources(package: &str, version: &str) -> PackageSourcesV1 {
    match (package, version) {
        ("Eqiora.Fluid", "0.1.0") => generated_sources(
            package,
            version,
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/releases/Eqiora.Fluid/0.1.0/README.md"),
                ),
                (
                    "src/fluid.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Fluid/0.1.0/src/fluid.eqi"
                    ),
                ),
            ],
        ),
        ("Eqiora.Fluid", "0.2.0") => generated_sources(
            package,
            version,
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/releases/Eqiora.Fluid/0.2.0/README.md"),
                ),
                (
                    "src/fluid.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Fluid/0.2.0/src/fluid.eqi"
                    ),
                ),
            ],
        ),
        ("Eqiora.Fluid", "0.3.0") => generated_sources(
            package,
            version,
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/releases/Eqiora.Fluid/0.3.0/README.md"),
                ),
                (
                    "src/fluid.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Fluid/0.3.0/src/fluid.eqi"
                    ),
                ),
            ],
        ),
        ("Eqiora.Solid", "0.2.0") => generated_sources(
            package,
            version,
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/releases/Eqiora.Solid/0.2.0/README.md"),
                ),
                (
                    "src/solid.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Solid/0.2.0/src/solid.eqi"
                    ),
                ),
            ],
        ),
        ("Eqiora.Solid", "0.3.0") => generated_sources(
            package,
            version,
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!("../../../../packages/releases/Eqiora.Solid/0.3.0/README.md"),
                ),
                (
                    "src/solid.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Solid/0.3.0/src/solid.eqi"
                    ),
                ),
            ],
        ),
        ("Eqiora.Mechanics.Interfaces", "0.2.0") => generated_sources(
            package,
            version,
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Mechanics.Interfaces/0.2.0/README.md"
                    ),
                ),
                (
                    "src/interfaces.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Mechanics.Interfaces/0.2.0/src/interfaces.eqi"
                    ),
                ),
            ],
        ),
        ("Eqiora.Fluid.Incompressible", "0.3.0") => generated_sources(
            package,
            version,
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Fluid.Incompressible/0.3.0/README.md"
                    ),
                ),
                (
                    "src/incompressible.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Fluid.Incompressible/0.3.0/src/incompressible.eqi"
                    ),
                ),
            ],
        ),
        ("Eqiora.Solid.LinearElasticity", "0.5.0") => generated_sources(
            package,
            version,
            &[
                (
                    "README.md",
                    BundleRoleV1::Documentation,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Solid.LinearElasticity/0.5.0/README.md"
                    ),
                ),
                (
                    "src/linear_elasticity.eqi",
                    BundleRoleV1::ModelSource,
                    include_bytes!(
                        "../../../../packages/releases/Eqiora.Solid.LinearElasticity/0.5.0/src/linear_elasticity.eqi"
                    ),
                ),
            ],
        ),
        _ => panic!("unsupported embedded package release `{package}` `{version}`"),
    }
}
