//! Exact, offline model-package identity and resolution.
//!
//! This crate owns a typed package family. It deliberately does not define a
//! universal plugin payload, perform version selection, access a registry, or
//! invoke compiler semantics.

mod canonical;
mod digest;
#[cfg(feature = "filesystem")]
mod directory_authoring;
#[cfg(feature = "filesystem")]
mod directory_installation;
#[cfg(feature = "filesystem")]
mod directory_io;
mod execution_binding;
mod external_digest;
#[cfg(feature = "filesystem")]
mod filesystem;
mod identity;
mod manifest;
mod path;
mod release;
mod resolution;
mod run_binding;
mod semantic;
mod source;
mod store;

pub use digest::{
    PackageCompilationDigest, PackageExecutionBindingDigest, PackageRunBindingDigest,
    PackageSemanticDigest, ResolutionDigest, SourceBundleDigest,
};
#[cfg(feature = "filesystem")]
pub use directory_authoring::{
    AuthorPackageDirectory, AuthorPackageDirectoryError, AuthorPackageDirectoryResource,
};
#[cfg(feature = "filesystem")]
pub use directory_installation::{
    DirectoryPackageInstaller, PackageInstallDisposition, PackageInstallError,
    PackageInstallIoPhase, PackageInstallReceipt, PackageStageCleanup,
};
pub use execution_binding::{
    BoundExecutionRunSchemaV1, BoundRealizationSchemaV1, PackageExecutionBindingV1,
};
pub use external_digest::{CanonicalModelDigest, CanonicalRealizationDigest, CanonicalRunDigest};
#[cfg(feature = "filesystem")]
pub use filesystem::DirectoryPackageStore;
pub use identity::{ExactVersion, ModelPackageIdentityV1, QualifiedName};
pub use manifest::{AuthorManifestV1, BundleEntryV1, BundleRoleV1, DependencyRequirementV1};
pub use path::NormalizedRelativePath;
pub use release::{
    CompilationPackageV1, CompilationToolchainV2, PackageCompilationRecordV2, PackageReleaseV1,
};
pub use resolution::{
    ExactResolver, ResolutionEdgeV1, ResolutionError, ResolutionNodeV1, ResolutionRecordV1,
    ResolvedPackageGraph,
};
pub use run_binding::{BoundRunManifestSchemaV1, PackageRunBindingV1};
pub use semantic::{
    CanonicalDeclaration, DeclarationKindV1, SemanticContentV1, SemanticDeclarationV1, VisibilityV1,
};
pub use source::{AuthorPackageSourcesV1, SourceBundleIdentityV1, SourceBundleV1, SourceFileV1};
pub use store::{InMemoryPackageStore, PackageStore, StoreError};

/// Errors produced while constructing or decoding closed package contracts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractError {
    message: String,
}

impl ContractError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ContractError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ContractError {}
