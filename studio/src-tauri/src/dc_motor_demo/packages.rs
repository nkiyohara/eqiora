//! Exact checked-in package closure for the DC-drive presentation.

use eqiora::package::{
    BundleRoleV1, InMemoryPackageStore, NormalizedRelativePath, PackageManifestV1,
    PackageReleaseV1, PackageSourcesV1, PackagedModelDocument, ResolutionRecordV1, SourceFileV1,
    prepare_package_release_v1,
};

const ELECTRICAL_SOURCE: &str =
    include_str!("../../../../packages/Eqiora.Electrical.Basic/src/basic.eqi");
const ELECTRICAL_README: &[u8] =
    include_bytes!("../../../../packages/Eqiora.Electrical.Basic/README.md");
const ELECTRICAL_MANIFEST: &[u8] =
    include_bytes!("../../../../packages/Eqiora.Electrical.Basic/package.json");
const DRIVE_SOURCE: &str =
    include_str!("../../../../packages/Eqiora.Electromechanical.DcDrive/src/drive.eqi");
const DRIVE_README: &[u8] =
    include_bytes!("../../../../packages/Eqiora.Electromechanical.DcDrive/README.md");
const DRIVE_MANIFEST: &[u8] =
    include_bytes!("../../../../packages/Eqiora.Electromechanical.DcDrive/package.json");
const ROOT_SOURCE: &str =
    include_str!("../../../../packages/org.example.dc-motor-control/src/main.eqi");
const ROOT_README: &[u8] =
    include_bytes!("../../../../packages/org.example.dc-motor-control/README.md");
const ROOT_MANIFEST: &[u8] =
    include_bytes!("../../../../packages/org.example.dc-motor-control/package.json");

pub(super) struct PreparedPackages {
    pub(super) document: PackagedModelDocument,
    pub(super) resolution: ResolutionRecordV1,
}

impl PreparedPackages {
    pub(super) fn compile() -> Result<Self, String> {
        let electrical = release(
            ELECTRICAL_MANIFEST,
            "src/basic.eqi",
            ELECTRICAL_SOURCE,
            ELECTRICAL_README,
            &[],
        )?;
        let drive = release(
            DRIVE_MANIFEST,
            "src/drive.eqi",
            DRIVE_SOURCE,
            DRIVE_README,
            std::slice::from_ref(&electrical),
        )?;
        let root = release(
            ROOT_MANIFEST,
            "src/main.eqi",
            ROOT_SOURCE,
            ROOT_README,
            &[electrical.clone(), drive.clone()],
        )?;
        let resolution =
            ResolutionRecordV1::from_exact_releases(&root, &[electrical.clone(), drive.clone()])
                .map_err(|error| error.to_string())?;
        let mut store = InMemoryPackageStore::default();
        for release in [&electrical, &drive, &root] {
            store.insert(release).map_err(|error| error.to_string())?;
        }
        let document = PackagedModelDocument::compile_locked(&store, &resolution, "Main")
            .map_err(|error| error.to_string())?;
        Ok(Self {
            document,
            resolution,
        })
    }
}

fn release(
    manifest: &[u8],
    model_path: &str,
    model_source: &str,
    readme: &[u8],
    dependencies: &[PackageReleaseV1],
) -> Result<PackageReleaseV1, String> {
    let manifest = PackageManifestV1::from_json(manifest).map_err(|error| error.to_string())?;
    let sources = PackageSourcesV1::new(
        manifest,
        vec![
            source_file("README.md", BundleRoleV1::Documentation, readme)?,
            source_file(
                model_path,
                BundleRoleV1::ModelSource,
                model_source.as_bytes(),
            )?,
        ],
    )
    .map_err(|error| error.to_string())?;
    prepare_package_release_v1(sources, dependencies).map_err(|error| error.to_string())
}

fn source_file(path: &str, role: BundleRoleV1, bytes: &[u8]) -> Result<SourceFileV1, String> {
    Ok(SourceFileV1::new(
        NormalizedRelativePath::parse(path).map_err(|error| error.to_string())?,
        role,
        bytes.to_vec(),
    ))
}
