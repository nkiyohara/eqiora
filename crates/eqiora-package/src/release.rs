use serde::{Deserialize, Serialize};

use crate::canonical;
use crate::{
    CanonicalModelDigest, ContractError, ExactVersion, ModelPackageIdentityV1,
    PackageCompilationDigest, PackageManifestV1, QualifiedName, ResolutionDigest,
    ResolutionRecordV1, ResolvedPackageGraph, SemanticContentV1, SourceBundleDigest,
    SourceBundleV1, SourceFileV1,
};

const RELEASE_SCHEMA: &str = "eqiora.package-release.v1";
const COMPILATION_SCHEMA: &str = "eqiora.package-compilation.v2";
const CANONICAL_JSON_ENCODING: &str = "eqiora.canonical-json.v1";
const MAX_COMPILATION_PACKAGES: usize = 65_536;
const V1: u32 = 1;
const V2: u32 = 2;

/// A complete typed package release as stored by exact source-bundle digest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageReleaseV1 {
    schema: String,
    semantic: SemanticContentV1,
    source: SourceBundleV1,
}

impl PackageReleaseV1 {
    pub fn new(
        manifest: PackageManifestV1,
        semantic: SemanticContentV1,
        files: Vec<SourceFileV1>,
    ) -> Result<Self, ContractError> {
        let package = semantic.package_identity(&manifest)?;
        let source = SourceBundleV1::new(package, manifest, files)?;
        Self {
            schema: RELEASE_SCHEMA.to_owned(),
            semantic,
            source,
        }
        .normalize()
    }

    fn normalize(mut self) -> Result<Self, ContractError> {
        if self.schema != RELEASE_SCHEMA {
            return Err(ContractError::new(format!(
                "unsupported package release schema `{}`",
                self.schema
            )));
        }
        self.semantic = self.semantic.normalize()?;
        self.source = self.source.normalize()?;
        let expected = self.semantic.package_identity(self.source.manifest())?;
        if self.source.package() != &expected {
            return Err(ContractError::new(
                "source bundle package identity does not match computed semantic identity",
            ));
        }
        Ok(self)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, ContractError> {
        canonical::from_slice::<Self>(bytes)?.normalize()
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>, ContractError> {
        canonical::to_bytes(self)
    }

    pub fn package_identity(&self) -> Result<ModelPackageIdentityV1, ContractError> {
        self.semantic.package_identity(self.source.manifest())
    }

    pub fn source_digest(&self) -> Result<SourceBundleDigest, ContractError> {
        Ok(self.source.identity()?.source_digest)
    }

    #[must_use]
    pub fn manifest(&self) -> &PackageManifestV1 {
        self.source.manifest()
    }

    #[must_use]
    pub fn semantic(&self) -> &SemanticContentV1 {
        &self.semantic
    }

    #[must_use]
    pub fn source(&self) -> &SourceBundleV1 {
        &self.source
    }
}

/// One exact package/source pair contributing to a package compilation.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilationPackageV1 {
    package: ModelPackageIdentityV1,
    source_digest: SourceBundleDigest,
}

impl CompilationPackageV1 {
    #[must_use]
    pub fn new(package: ModelPackageIdentityV1, source_digest: SourceBundleDigest) -> Self {
        Self {
            package,
            source_digest,
        }
    }

    #[must_use]
    pub fn package(&self) -> &ModelPackageIdentityV1 {
        &self.package
    }

    #[must_use]
    pub fn source_digest(&self) -> SourceBundleDigest {
        self.source_digest
    }
}

/// The exact compiler and canonicalization contracts used for a compilation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompilationToolchainV2 {
    compiler: QualifiedName,
    compiler_version: ExactVersion,
    semantic_canonicalization_version: u32,
    source_bundle_version: u32,
    resolution_version: u32,
}

impl CompilationToolchainV2 {
    #[must_use]
    pub fn new(compiler: QualifiedName, compiler_version: ExactVersion) -> Self {
        Self {
            compiler,
            compiler_version,
            semantic_canonicalization_version: V2,
            source_bundle_version: V1,
            resolution_version: V1,
        }
    }

    #[must_use]
    pub fn compiler(&self) -> &QualifiedName {
        &self.compiler
    }

    #[must_use]
    pub fn compiler_version(&self) -> &ExactVersion {
        &self.compiler_version
    }

    #[must_use]
    pub fn semantic_canonicalization_version(&self) -> u32 {
        self.semantic_canonicalization_version
    }

    #[must_use]
    pub fn source_bundle_version(&self) -> u32 {
        self.source_bundle_version
    }

    #[must_use]
    pub fn resolution_version(&self) -> u32 {
        self.resolution_version
    }

    fn validate_v2(&self) -> Result<(), ContractError> {
        if self.semantic_canonicalization_version != V2
            || self.source_bundle_version != V1
            || self.resolution_version != V1
        {
            return Err(ContractError::new(
                "package compilation v2 requires semantic canonicalization version 2 and source-bundle and resolution version 1",
            ));
        }
        Ok(())
    }
}

/// Exact provenance identity for one canonical model compilation.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageCompilationRecordV2 {
    schema: String,
    encoding: String,
    model_sha256: CanonicalModelDigest,
    root: ModelPackageIdentityV1,
    resolution_digest: ResolutionDigest,
    packages: Vec<CompilationPackageV1>,
    toolchain: CompilationToolchainV2,
}

impl PackageCompilationRecordV2 {
    pub fn new(
        model_sha256: CanonicalModelDigest,
        resolved: &ResolvedPackageGraph,
        toolchain: CompilationToolchainV2,
    ) -> Result<Self, ContractError> {
        Self {
            schema: COMPILATION_SCHEMA.to_owned(),
            encoding: CANONICAL_JSON_ENCODING.to_owned(),
            model_sha256,
            root: resolved.root().clone(),
            resolution_digest: resolved.resolution_digest(),
            packages: resolved.compilation_packages().to_vec(),
            toolchain,
        }
        .normalize()
    }

    fn normalize(mut self) -> Result<Self, ContractError> {
        if self.schema != COMPILATION_SCHEMA || self.encoding != CANONICAL_JSON_ENCODING {
            return Err(ContractError::new(
                "unsupported package compilation schema or encoding",
            ));
        }
        if self.packages.is_empty() || self.packages.len() > MAX_COMPILATION_PACKAGES {
            return Err(ContractError::new(
                "package compilation must contain a bounded, non-empty package inventory",
            ));
        }
        self.toolchain.validate_v2()?;
        self.packages.sort();
        for pair in self.packages.windows(2) {
            if pair[0].package == pair[1].package {
                return Err(ContractError::new(format!(
                    "duplicate compilation package `{}`",
                    pair[0].package.name
                )));
            }
            if pair[0].package.name == pair[1].package.name
                && pair[0].package.version == pair[1].package.version
            {
                return Err(ContractError::new(format!(
                    "ambiguous compilation package `{}@{}` has multiple semantic digests",
                    pair[0].package.name, pair[0].package.version
                )));
            }
        }
        if !self
            .packages
            .iter()
            .any(|package| package.package == self.root)
        {
            return Err(ContractError::new(
                "package compilation inventory does not contain its root identity",
            ));
        }
        Ok(self)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, ContractError> {
        canonical::from_slice::<Self>(bytes)?.normalize()
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>, ContractError> {
        canonical::to_bytes(self)
    }

    pub fn digest(&self) -> Result<PackageCompilationDigest, ContractError> {
        Ok(PackageCompilationDigest::compute(&self.canonical_json()?))
    }

    /// Verifies that a decoded provenance record names exactly the locked
    /// graph whose digest and inventory it carries.
    pub fn validate_against(&self, resolution: &ResolutionRecordV1) -> Result<(), ContractError> {
        let expected_packages: Vec<_> = resolution
            .nodes()
            .iter()
            .map(|node| CompilationPackageV1::new(node.identity().clone(), node.source_digest()))
            .collect();
        if self.root != *resolution.root()
            || self.resolution_digest != resolution.digest()?
            || self.packages != expected_packages
        {
            return Err(ContractError::new(
                "package compilation provenance does not match the exact resolution record",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn model_digest(&self) -> CanonicalModelDigest {
        self.model_sha256
    }

    #[must_use]
    pub fn root(&self) -> &ModelPackageIdentityV1 {
        &self.root
    }

    #[must_use]
    pub fn resolution_digest(&self) -> ResolutionDigest {
        self.resolution_digest
    }

    #[must_use]
    pub fn packages(&self) -> &[CompilationPackageV1] {
        &self.packages
    }

    #[must_use]
    pub fn toolchain(&self) -> &CompilationToolchainV2 {
        &self.toolchain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BundleEntryV1, BundleRoleV1, CanonicalDeclaration, DeclarationKindV1, ExactResolver,
        InMemoryPackageStore, NormalizedRelativePath, ResolutionNodeV1, ResolutionRecordV1,
        SemanticDeclarationV1, VisibilityV1,
    };

    #[test]
    fn formatting_changes_only_source_identity() {
        let path = NormalizedRelativePath::parse("src/main.eqi").expect("path");
        let manifest = PackageManifestV1::new(
            "main",
            QualifiedName::parse("org.example.Main").expect("name"),
            ExactVersion::parse("1.0.0").expect("version"),
            vec![],
            vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
        )
        .expect("manifest");
        let semantic = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
            QualifiedName::parse("Main").expect("declaration"),
            DeclarationKindV1::Model,
            VisibilityV1::Public,
            CanonicalDeclaration::new("model Main {}\n").expect("canonical"),
        )])
        .expect("semantic");
        let make = |bytes: &[u8]| {
            PackageReleaseV1::new(
                manifest.clone(),
                semantic.clone(),
                vec![SourceFileV1::new(
                    path.clone(),
                    BundleRoleV1::ModelSource,
                    bytes.to_vec(),
                )],
            )
            .expect("release")
        };
        let compact = make(b"model Main {}\n");
        let formatted = make(b"model   Main { }\n");
        assert_eq!(compact.package_identity(), formatted.package_identity());
        assert_ne!(compact.source_digest(), formatted.source_digest());
        assert_eq!(
            PackageReleaseV1::from_json(&compact.canonical_json().expect("JSON")),
            Ok(compact)
        );
    }

    #[test]
    fn relocation_and_documentation_change_only_source_identity() {
        let source_a = NormalizedRelativePath::parse("src/main.eqi").expect("path");
        let source_b = NormalizedRelativePath::parse("models/main.eqi").expect("path");
        let docs = NormalizedRelativePath::parse("guide/README.md").expect("path");
        let make_manifest = |bundle| {
            PackageManifestV1::new(
                "main",
                QualifiedName::parse("org.example.Main").expect("name"),
                ExactVersion::parse("1.0.0").expect("version"),
                vec![],
                bundle,
            )
            .expect("manifest")
        };
        let first_manifest = make_manifest(vec![BundleEntryV1::new(
            source_a.clone(),
            BundleRoleV1::ModelSource,
        )]);
        let second_manifest = make_manifest(vec![
            BundleEntryV1::new(source_b.clone(), BundleRoleV1::ModelSource),
            BundleEntryV1::new(docs.clone(), BundleRoleV1::Documentation),
        ]);
        let semantic = SemanticContentV1::new(vec![SemanticDeclarationV1::new(
            QualifiedName::parse("Main").expect("declaration"),
            DeclarationKindV1::Model,
            VisibilityV1::Public,
            CanonicalDeclaration::new("model Main {}\n").expect("canonical"),
        )])
        .expect("semantic");
        let first = PackageReleaseV1::new(
            first_manifest,
            semantic.clone(),
            vec![SourceFileV1::new(
                source_a,
                BundleRoleV1::ModelSource,
                b"model Main {}\n".to_vec(),
            )],
        )
        .expect("release");
        let second = PackageReleaseV1::new(
            second_manifest,
            semantic,
            vec![
                SourceFileV1::new(
                    source_b,
                    BundleRoleV1::ModelSource,
                    b"model Main {}\n".to_vec(),
                ),
                SourceFileV1::new(
                    docs,
                    BundleRoleV1::Documentation,
                    b"new documentation\n".to_vec(),
                ),
            ],
        )
        .expect("release");
        assert_eq!(first.package_identity(), second.package_identity());
        assert_ne!(first.source_digest(), second.source_digest());
    }

    #[test]
    fn compilation_record_is_closed_ordered_and_model_bound() {
        let path = NormalizedRelativePath::parse("src/main.eqi").expect("path");
        let release = PackageReleaseV1::new(
            PackageManifestV1::new(
                "main",
                QualifiedName::parse("org.example.Main").expect("name"),
                ExactVersion::parse("1.0.0+cpu").expect("version"),
                vec![],
                vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
            )
            .expect("manifest"),
            SemanticContentV1::new(vec![SemanticDeclarationV1::new(
                QualifiedName::parse("Main").expect("declaration"),
                DeclarationKindV1::Model,
                VisibilityV1::Private,
                CanonicalDeclaration::new("model Main {}\n").expect("canonical"),
            )])
            .expect("semantic"),
            vec![SourceFileV1::new(
                path,
                BundleRoleV1::ModelSource,
                b"model Main {}\n".to_vec(),
            )],
        )
        .expect("release");
        let package = release.package_identity().expect("identity");
        let source_digest = release.source_digest().expect("source digest");
        let resolution = ResolutionRecordV1::new(
            package.clone(),
            vec![ResolutionNodeV1::new(package.clone(), source_digest)],
            vec![],
        )
        .expect("resolution");
        let mut store = InMemoryPackageStore::default();
        store.insert(&release).expect("store insert");
        let resolved = ExactResolver
            .resolve(&resolution, &store)
            .expect("resolved graph");
        let make = |model: &str| {
            PackageCompilationRecordV2::new(
                CanonicalModelDigest::parse(model).expect("model digest"),
                &resolved,
                CompilationToolchainV2::new(
                    QualifiedName::parse("Eqiora.Compiler").expect("compiler"),
                    ExactVersion::parse("0.1.0").expect("compiler version"),
                ),
            )
            .expect("record")
        };
        let first = make(&"78".repeat(32));
        let changed = make(&"9a".repeat(32));
        assert_ne!(first.digest(), changed.digest());
        assert_eq!(
            PackageCompilationRecordV2::from_json(&first.canonical_json().expect("JSON")),
            Ok(first.clone())
        );
        let current_wire: serde_json::Value =
            serde_json::from_slice(&first.canonical_json().expect("JSON")).expect("value");
        assert_eq!(current_wire["schema"], COMPILATION_SCHEMA);
        assert_eq!(
            current_wire["toolchain"]["semantic_canonicalization_version"],
            V2
        );
        assert_eq!(first.packages()[0].package(), &package);
        first
            .validate_against(&resolution)
            .expect("matching resolution");
        let mut mismatched = first.clone();
        mismatched.resolution_digest =
            ResolutionDigest::parse(&"56".repeat(32)).expect("resolution");
        assert!(mismatched.validate_against(&resolution).is_err());

        let historical_v1 = br#"{"schema":"eqiora.package-compilation.v1","encoding":"eqiora.canonical-json.v1","model_sha256":"7878787878787878787878787878787878787878787878787878787878787878","root":{"name":"org.example.Main","version":"1.0.0+cpu","semantic_digest":"1212121212121212121212121212121212121212121212121212121212121212"},"resolution_digest":"5656565656565656565656565656565656565656565656565656565656565656","packages":[],"toolchain":{"compiler":"Eqiora.Compiler","compiler_version":"0.1.0","semantic_canonicalization_version":1,"source_bundle_version":1,"resolution_version":1}}"#;
        assert!(PackageCompilationRecordV2::from_json(historical_v1).is_err());

        let unknown = br#"{"schema":"eqiora.package-compilation.v2","encoding":"eqiora.canonical-json.v1","model_sha256":"7878787878787878787878787878787878787878787878787878787878787878","root":{"name":"org.example.Main","version":"1.0.0+cpu","semantic_digest":"1212121212121212121212121212121212121212121212121212121212121212"},"resolution_digest":"5656565656565656565656565656565656565656565656565656565656565656","packages":[],"toolchain":{"compiler":"Eqiora.Compiler","compiler_version":"0.1.0","semantic_canonicalization_version":2,"source_bundle_version":1,"resolution_version":1},"payload":null}"#;
        assert!(PackageCompilationRecordV2::from_json(unknown).is_err());

        let mut unsupported_toolchain = CompilationToolchainV2::new(
            QualifiedName::parse("Eqiora.Compiler").expect("compiler"),
            ExactVersion::parse("0.1.0").expect("compiler version"),
        );
        unsupported_toolchain.semantic_canonicalization_version = 1;
        let unsupported = PackageCompilationRecordV2 {
            toolchain: unsupported_toolchain,
            ..first.clone()
        };
        assert!(unsupported.normalize().is_err());

        let ambiguous = ModelPackageIdentityV1::new(
            package.name.clone(),
            package.version.clone(),
            crate::PackageSemanticDigest::parse(&"ab".repeat(32)).expect("semantic digest"),
        );
        let ambiguous = PackageCompilationRecordV2 {
            packages: vec![
                CompilationPackageV1::new(package, source_digest),
                CompilationPackageV1::new(ambiguous, source_digest),
            ],
            ..first
        };
        assert!(ambiguous.normalize().is_err());
    }
}
