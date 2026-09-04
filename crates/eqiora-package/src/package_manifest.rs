use serde::{Deserialize, Serialize};

use crate::canonical;
use crate::{
    ContractError, ExactVersion, ModelPackageIdentityV1, NormalizedRelativePath, QualifiedName,
};

const SCHEMA: &str = "eqiora.package-manifest.v1";
const MAX_DEPENDENCIES: usize = 4096;
const MAX_BUNDLE_ENTRIES: usize = 65_536;

/// One exact direct dependency in a generated package manifest.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageDependencyV1 {
    target: ModelPackageIdentityV1,
}

impl PackageDependencyV1 {
    #[must_use]
    pub const fn new(target: ModelPackageIdentityV1) -> Self {
        Self { target }
    }

    #[must_use]
    pub fn target(&self) -> &ModelPackageIdentityV1 {
        &self.target
    }
}

/// The role of a file in an exact source bundle.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BundleRoleV1 {
    ModelSource,
    Documentation,
}

/// One generated source-bundle inventory entry.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BundleEntryV1 {
    path: NormalizedRelativePath,
    role: BundleRoleV1,
}

impl BundleEntryV1 {
    #[must_use]
    pub fn new(path: NormalizedRelativePath, role: BundleRoleV1) -> Self {
        Self { path, role }
    }

    #[must_use]
    pub fn path(&self) -> &NormalizedRelativePath {
        &self.path
    }

    #[must_use]
    pub fn role(&self) -> BundleRoleV1 {
        self.role
    }
}

/// Closed generated package-artifact metadata. Computed digests are absent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageManifestV1 {
    schema: String,
    entry: QualifiedName,
    name: QualifiedName,
    version: ExactVersion,
    dependencies: Vec<PackageDependencyV1>,
    bundle: Vec<BundleEntryV1>,
}

impl PackageManifestV1 {
    pub fn new(
        entry: &str,
        name: QualifiedName,
        version: ExactVersion,
        dependencies: Vec<PackageDependencyV1>,
        bundle: Vec<BundleEntryV1>,
    ) -> Result<Self, ContractError> {
        Self {
            schema: SCHEMA.to_owned(),
            entry: QualifiedName::parse(entry)?,
            name,
            version,
            dependencies,
            bundle,
        }
        .normalize()
    }

    pub(crate) fn normalize(mut self) -> Result<Self, ContractError> {
        if self.schema != SCHEMA {
            return Err(ContractError::new(format!(
                "unsupported package manifest schema `{}`",
                self.schema
            )));
        }
        if self.dependencies.len() > MAX_DEPENDENCIES {
            return Err(ContractError::new(
                "package manifest exceeds dependency limit",
            ));
        }
        if self.bundle.len() > MAX_BUNDLE_ENTRIES {
            return Err(ContractError::new(
                "package manifest exceeds bundle-entry limit",
            ));
        }
        if !self
            .bundle
            .iter()
            .any(|entry| entry.role == BundleRoleV1::ModelSource)
        {
            return Err(ContractError::new(
                "model package manifest must inventory at least one model source",
            ));
        }
        self.dependencies.sort();
        for pair in self.dependencies.windows(2) {
            if pair[0].target.name == pair[1].target.name {
                return Err(ContractError::new(format!(
                    "duplicate direct dependency `{}`",
                    pair[0].target.name
                )));
            }
        }
        self.bundle.sort();
        for pair in self.bundle.windows(2) {
            if pair[0].path == pair[1].path {
                return Err(ContractError::new(format!(
                    "duplicate normalized bundle path `{}`",
                    pair[0].path
                )));
            }
        }
        let mut portable_paths = self
            .bundle
            .iter()
            .map(|entry| (entry.path.ascii_case_key(), &entry.path))
            .collect::<Vec<_>>();
        portable_paths.sort_by(|left, right| left.0.cmp(&right.0));
        for pair in portable_paths.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(ContractError::new(format!(
                    "bundle paths `{}` and `{}` collide under portable ASCII case folding",
                    pair[0].1, pair[1].1
                )));
            }
        }
        Ok(self)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, ContractError> {
        canonical::from_slice::<Self>(bytes)?.normalize()
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>, ContractError> {
        canonical::checked_round_trip(self)
    }

    #[must_use]
    pub fn name(&self) -> &QualifiedName {
        &self.name
    }

    /// Relative module selected as the package entry.
    #[must_use]
    pub fn entry(&self) -> &QualifiedName {
        &self.entry
    }

    #[must_use]
    pub fn version(&self) -> &ExactVersion {
        &self.version
    }

    #[must_use]
    pub fn dependencies(&self) -> &[PackageDependencyV1] {
        &self.dependencies
    }

    #[must_use]
    pub fn bundle(&self) -> &[BundleEntryV1] {
        &self.bundle
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PackageSemanticDigest;

    fn identity(name: &str) -> ModelPackageIdentityV1 {
        ModelPackageIdentityV1::new(
            QualifiedName::parse(name).expect("name"),
            ExactVersion::parse("1.0.0").expect("version"),
            PackageSemanticDigest::parse(&"01".repeat(32)).expect("digest"),
        )
    }

    #[test]
    fn manifest_order_does_not_change_canonical_bytes() {
        let a = PackageDependencyV1::new(identity("org.example.A"));
        let b = PackageDependencyV1::new(identity("org.example.B"));
        let source = BundleEntryV1::new(
            NormalizedRelativePath::parse("src/root.eqi").expect("path"),
            BundleRoleV1::ModelSource,
        );
        let docs = BundleEntryV1::new(
            NormalizedRelativePath::parse("README.md").expect("path"),
            BundleRoleV1::Documentation,
        );
        let make = |dependencies, bundle| {
            PackageManifestV1::new(
                "main",
                QualifiedName::parse("org.example.Root").expect("name"),
                ExactVersion::parse("1.0.0+build.7").expect("version"),
                dependencies,
                bundle,
            )
            .expect("manifest")
        };
        let first = make(
            vec![b.clone(), a.clone()],
            vec![source.clone(), docs.clone()],
        );
        let second = make(vec![a, b], vec![docs, source]);
        assert_eq!(first.canonical_json(), second.canonical_json());
        assert_eq!(
            PackageManifestV1::from_json(&first.canonical_json().expect("JSON")),
            Ok(first)
        );
    }

    #[test]
    fn manifest_rejects_unknown_fields_and_duplicate_normalized_paths() {
        let invalid = br#"{"schema":"eqiora.package-manifest.v1","name":"a","version":"1.0.0","dependencies":[],"bundle":[],"payload":{}}"#;
        assert!(PackageManifestV1::from_json(invalid).is_err());
        let entry = BundleEntryV1::new(
            NormalizedRelativePath::parse("a.eqi").expect("path"),
            BundleRoleV1::ModelSource,
        );
        assert!(
            PackageManifestV1::new(
                "main",
                QualifiedName::parse("a").expect("name"),
                ExactVersion::parse("1.0.0").expect("version"),
                vec![],
                vec![entry.clone(), entry],
            )
            .is_err()
        );
        assert!(
            PackageManifestV1::new(
                "main",
                QualifiedName::parse("a").expect("name"),
                ExactVersion::parse("1.0.0").expect("version"),
                vec![],
                vec![BundleEntryV1::new(
                    NormalizedRelativePath::parse("README.md").expect("path"),
                    BundleRoleV1::Documentation,
                )],
            )
            .is_err()
        );

        let upper = BundleEntryV1::new(
            NormalizedRelativePath::parse("src/Main.eqi").expect("upper path"),
            BundleRoleV1::ModelSource,
        );
        let lower = BundleEntryV1::new(
            NormalizedRelativePath::parse("src/main.eqi").expect("lower path"),
            BundleRoleV1::ModelSource,
        );
        assert!(
            PackageManifestV1::new(
                "main",
                QualifiedName::parse("a").expect("name"),
                ExactVersion::parse("1.0.0").expect("version"),
                vec![],
                vec![upper, lower],
            )
            .is_err()
        );
    }
}
