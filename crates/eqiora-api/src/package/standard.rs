//! Explicit transport of the exact semantic packages shipped with the distribution.

use eqiora_package::{
    BundleRoleV1, NormalizedRelativePath, PackageManifestV1, PackageReleaseV1, PackageSourcesV1,
    SourceFileV1,
};

use super::{PackagePreparationError, prepare_package_release_v1};

pub(super) fn closure(
    name: &str,
    version: &str,
) -> Result<Vec<PackageReleaseV1>, PackagePreparationError> {
    let mechanics = sources(
        include_bytes!("../../../../packages/Eqiora.Mechanics.Interfaces/package.json"),
        include_bytes!("../../../../packages/Eqiora.Mechanics.Interfaces/README.md"),
        "src/interfaces.eqi",
        include_bytes!("../../../../packages/Eqiora.Mechanics.Interfaces/src/interfaces.eqi"),
    )?;
    let mechanics = prepare_package_release_v1(mechanics, &[])?;
    let mut releases = vec![mechanics];
    let source = match name {
        "Eqiora.Mechanics.Interfaces" => None,
        "Eqiora.Fluid.Incompressible" => Some(sources(
            include_bytes!("../../../../packages/Eqiora.Fluid.Incompressible/package.json"),
            include_bytes!("../../../../packages/Eqiora.Fluid.Incompressible/README.md"),
            "src/incompressible.eqi",
            include_bytes!(
                "../../../../packages/Eqiora.Fluid.Incompressible/src/incompressible.eqi"
            ),
        )?),
        "Eqiora.Solid.LinearElasticity" => Some(sources(
            include_bytes!("../../../../packages/Eqiora.Solid.LinearElasticity/package.json"),
            include_bytes!("../../../../packages/Eqiora.Solid.LinearElasticity/README.md"),
            "src/linear_elasticity.eqi",
            include_bytes!(
                "../../../../packages/Eqiora.Solid.LinearElasticity/src/linear_elasticity.eqi"
            ),
        )?),
        _ => {
            return Err(PackagePreparationError::LocalDirectoryGraph(format!(
                "distribution has no bundled package `{name}@{version}`"
            )));
        }
    };
    if let Some(source) = source {
        releases.push(prepare_package_release_v1(source, &releases)?);
    }
    let identity = releases
        .last()
        .expect("nonempty bundled closure")
        .package_identity()?;
    if identity.name.as_str() != name || identity.version.as_str() != version {
        return Err(PackagePreparationError::LocalDirectoryGraph(format!(
            "bundled package request `{name}@{version}` differs from exact shipped release `{}@{}`",
            identity.name, identity.version
        )));
    }
    Ok(releases)
}

fn sources(
    manifest: &[u8],
    readme: &[u8],
    source_path: &str,
    source: &[u8],
) -> Result<PackageSourcesV1, PackagePreparationError> {
    Ok(PackageSourcesV1::new(
        PackageManifestV1::from_json(manifest)?,
        vec![
            SourceFileV1::new(
                NormalizedRelativePath::parse("README.md")?,
                BundleRoleV1::Documentation,
                readme.to_vec(),
            ),
            SourceFileV1::new(
                NormalizedRelativePath::parse(source_path)?,
                BundleRoleV1::ModelSource,
                source.to_vec(),
            ),
        ],
    )?)
}
