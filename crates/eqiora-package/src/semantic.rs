use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::canonical;
use crate::{
    AuthorManifestV1, ContractError, ModelPackageIdentityV1, PackageSemanticDigest, QualifiedName,
};

const SCHEMA: &str = "eqiora.semantic-content.v1";
const MAX_DECLARATIONS: usize = 1_000_000;
const MAX_DECLARATION_BYTES: usize = 16 * 1024 * 1024;
const MAX_TOTAL_DECLARATION_BYTES: usize = 64 * 1024 * 1024;

/// The closed declaration families understood by the package identity wire.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclarationKindV1 {
    Module,
    Constant,
    // Alpha-v1 additive vocabulary: older readers reject these unknown kinds
    // rather than guessing, while exact semantic/toolchain identity prevents
    // fallback to a parallel schema.
    PropertyContract,
    PropertyRelease,
    PureOperator,
    Connector,
    Component,
    Model,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisibilityV1 {
    Public,
    Private,
}

/// Canonical typed declaration text emitted by the owning compiler version.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct CanonicalDeclaration(String);

impl CanonicalDeclaration {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_DECLARATION_BYTES || value.contains('\r') {
            return Err(ContractError::new(
                "canonical declaration must be non-empty LF text within the v1 byte limit",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for CanonicalDeclaration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CanonicalDeclaration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticDeclarationV1 {
    path: QualifiedName,
    kind: DeclarationKindV1,
    visibility: VisibilityV1,
    canonical_form: CanonicalDeclaration,
}

impl SemanticDeclarationV1 {
    #[must_use]
    pub fn new(
        path: QualifiedName,
        kind: DeclarationKindV1,
        visibility: VisibilityV1,
        canonical_form: CanonicalDeclaration,
    ) -> Self {
        Self {
            path,
            kind,
            visibility,
            canonical_form,
        }
    }

    #[must_use]
    pub fn path(&self) -> &QualifiedName {
        &self.path
    }

    #[must_use]
    pub fn kind(&self) -> DeclarationKindV1 {
        self.kind
    }

    #[must_use]
    pub fn visibility(&self) -> VisibilityV1 {
        self.visibility
    }

    #[must_use]
    pub fn canonical_form(&self) -> &CanonicalDeclaration {
        &self.canonical_form
    }
}

/// File-layout-independent canonical semantic records.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticContentV1 {
    schema: String,
    declarations: Vec<SemanticDeclarationV1>,
}

impl SemanticContentV1 {
    pub fn new(declarations: Vec<SemanticDeclarationV1>) -> Result<Self, ContractError> {
        Self {
            schema: SCHEMA.to_owned(),
            declarations,
        }
        .normalize()
    }

    pub(crate) fn normalize(mut self) -> Result<Self, ContractError> {
        if self.schema != SCHEMA {
            return Err(ContractError::new(format!(
                "unsupported semantic content schema `{}`",
                self.schema
            )));
        }
        if self.declarations.is_empty() || self.declarations.len() > MAX_DECLARATIONS {
            return Err(ContractError::new(
                "semantic content must contain a bounded, non-empty declaration set",
            ));
        }
        let total_bytes = self
            .declarations
            .iter()
            .try_fold(0_usize, |total, declaration| {
                total
                    .checked_add(declaration.canonical_form.as_str().len())
                    .ok_or_else(|| ContractError::new("semantic declaration byte count overflow"))
            })?;
        if total_bytes > MAX_TOTAL_DECLARATION_BYTES {
            return Err(ContractError::new(
                "semantic content exceeds total declaration byte limit",
            ));
        }
        self.declarations.sort();
        for pair in self.declarations.windows(2) {
            if pair[0].path == pair[1].path {
                return Err(ContractError::new(format!(
                    "duplicate semantic declaration `{}`",
                    pair[0].path
                )));
            }
        }
        Ok(self)
    }

    pub fn from_json(bytes: &[u8]) -> Result<Self, ContractError> {
        canonical::from_slice::<Self>(bytes)?.normalize()
    }

    pub fn canonical_json(&self) -> Result<Vec<u8>, ContractError> {
        canonical::to_bytes(self)
    }

    pub fn package_identity(
        &self,
        manifest: &AuthorManifestV1,
    ) -> Result<ModelPackageIdentityV1, ContractError> {
        #[derive(Serialize)]
        struct SemanticManifest<'a> {
            schema: &'static str,
            name: &'a QualifiedName,
            version: &'a crate::ExactVersion,
            dependencies: BTreeSet<&'a ModelPackageIdentityV1>,
        }
        #[derive(Serialize)]
        struct SemanticPreimage<'a> {
            canonicalization: &'static str,
            manifest: SemanticManifest<'a>,
            content: &'a SemanticContentV1,
        }
        let bytes = canonical::to_bytes(&SemanticPreimage {
            canonicalization: "eqiora.package-semantic-canonical.v1",
            manifest: SemanticManifest {
                schema: "eqiora.semantic-manifest.v1",
                name: manifest.name(),
                version: manifest.version(),
                dependencies: manifest
                    .dependencies()
                    .iter()
                    .map(crate::DependencyRequirementV1::target)
                    .collect(),
            },
            content: self,
        })?;
        Ok(ModelPackageIdentityV1::new(
            manifest.name().clone(),
            manifest.version().clone(),
            PackageSemanticDigest::compute(&bytes),
        ))
    }

    #[must_use]
    pub fn declarations(&self) -> &[SemanticDeclarationV1] {
        &self.declarations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BundleEntryV1, BundleRoleV1, DependencyRequirementV1, ExactVersion, NormalizedRelativePath,
        PackageSemanticDigest,
    };

    fn source_inventory() -> Vec<BundleEntryV1> {
        vec![BundleEntryV1::new(
            NormalizedRelativePath::parse("src/package.eqi").expect("path"),
            BundleRoleV1::ModelSource,
        )]
    }

    fn declaration(path: &str, form: &str) -> SemanticDeclarationV1 {
        SemanticDeclarationV1::new(
            QualifiedName::parse(path).expect("path"),
            DeclarationKindV1::Component,
            VisibilityV1::Public,
            CanonicalDeclaration::new(form).expect("form"),
        )
    }

    #[test]
    fn semantic_identity_ignores_insertion_order_but_not_content() {
        let manifest = AuthorManifestV1::new(
            QualifiedName::parse("org.example.Basic").expect("name"),
            ExactVersion::parse("1.0.0").expect("version"),
            vec![],
            source_inventory(),
        )
        .expect("manifest");
        let first = SemanticContentV1::new(vec![
            declaration("B", "component B {}"),
            declaration("A", "component A {}"),
        ])
        .expect("content");
        let second = SemanticContentV1::new(vec![
            declaration("A", "component A {}"),
            declaration("B", "component B {}"),
        ])
        .expect("content");
        let changed = SemanticContentV1::new(vec![
            declaration("A", "component A { parameter x; }"),
            declaration("B", "component B {}"),
        ])
        .expect("content");
        assert_eq!(first.canonical_json(), second.canonical_json());
        assert_eq!(
            first.package_identity(&manifest),
            second.package_identity(&manifest)
        );
        assert_ne!(
            first.package_identity(&manifest),
            changed.package_identity(&manifest)
        );
    }

    #[test]
    fn semantic_identity_uses_exact_dependency_identity_not_local_alias() {
        let target = ModelPackageIdentityV1::new(
            QualifiedName::parse("org.example.Leaf").expect("name"),
            ExactVersion::parse("1.0.0").expect("version"),
            PackageSemanticDigest::parse(&"12".repeat(32)).expect("digest"),
        );
        let manifest = |alias: &str, target: ModelPackageIdentityV1| {
            AuthorManifestV1::new(
                QualifiedName::parse("org.example.Root").expect("name"),
                ExactVersion::parse("1.0.0").expect("version"),
                vec![
                    DependencyRequirementV1::new(
                        QualifiedName::parse(alias).expect("alias"),
                        target,
                    )
                    .expect("dependency"),
                ],
                source_inventory(),
            )
            .expect("manifest")
        };
        let content =
            SemanticContentV1::new(vec![declaration("Main", "model Main {}")]).expect("content");
        let first = manifest("leaf", target.clone());
        let renamed = manifest("electrical", target.clone());
        assert_eq!(
            content.package_identity(&first),
            content.package_identity(&renamed)
        );

        let changed_target = ModelPackageIdentityV1::new(
            target.name,
            ExactVersion::parse("1.0.1").expect("version"),
            target.semantic_digest,
        );
        assert_ne!(
            content.package_identity(&first),
            content.package_identity(&manifest("leaf", changed_target))
        );
    }
}
