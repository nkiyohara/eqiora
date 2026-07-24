#![allow(dead_code)] // Each integration-test crate consumes a subset of the shared fixtures.

use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleRoleV1, NormalizedRelativePath, SourceFileV1,
};

/// Reconstruct an exact checked-in package inventory without granting tests
/// ambient filesystem authority at runtime.
pub(crate) fn sources(
    manifest_json: &[u8],
    files: &[(&str, BundleRoleV1, &[u8])],
) -> AuthorPackageSourcesV1 {
    let manifest = AuthorManifestV1::from_json(manifest_json).expect("embedded package manifest");
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
    AuthorPackageSourcesV1::new(manifest, files).expect("closed embedded package inventory")
}

/// Return one exact public package from compile-time embedded repository bytes.
pub(crate) fn public_sources(package: &str) -> AuthorPackageSourcesV1 {
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
pub(crate) fn release_sources(package: &str, version: &str) -> AuthorPackageSourcesV1 {
    match (package, version) {
        ("Eqiora.Mechanics.Interfaces", "0.2.0") => sources(
            include_bytes!(
                "../../../../packages/releases/Eqiora.Mechanics.Interfaces/0.2.0/package.json"
            ),
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
        ("Eqiora.Fluid.Incompressible", "0.3.0") => sources(
            include_bytes!(
                "../../../../packages/releases/Eqiora.Fluid.Incompressible/0.3.0/package.json"
            ),
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
        ("Eqiora.Solid.LinearElasticity", "0.5.0") => sources(
            include_bytes!(
                "../../../../packages/releases/Eqiora.Solid.LinearElasticity/0.5.0/package.json"
            ),
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
