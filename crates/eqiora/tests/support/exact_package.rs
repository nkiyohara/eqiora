use eqiora::compiler::{
    AnalyzedResolvedHierarchy, CanonicalDeclarationKind, CanonicalDeclarationVisibility,
    CompilationNamespaceId,
};
use eqiora::package::{
    AuthorManifestV1, CanonicalDeclaration, DeclarationKindV1, ExactVersion,
    ModelPackageIdentityV1, PackageReleaseV1, PackageSemanticDigest, QualifiedName,
    SemanticContentV1, SemanticDeclarationV1, SourceFileV1, VisibilityV1,
};

pub(crate) fn canonical_manifest(encoded: &[u8]) -> AuthorManifestV1 {
    let manifest = AuthorManifestV1::from_json(encoded).expect("package manifest");
    assert_eq!(
        manifest.canonical_json().expect("canonical manifest"),
        encoded.strip_suffix(b"\n").unwrap_or(encoded)
    );
    manifest
}

pub(crate) fn namespace(identity: &ModelPackageIdentityV1) -> CompilationNamespaceId {
    CompilationNamespaceId::new([
        identity.name.as_str(),
        identity.version.as_str(),
        &identity.semantic_digest.to_hex(),
    ])
    .expect("package namespace")
}

pub(crate) fn provisional_namespace(name: &str, version: &str) -> CompilationNamespaceId {
    namespace(&ModelPackageIdentityV1::new(
        QualifiedName::parse(name).expect("name"),
        ExactVersion::parse(version).expect("version"),
        PackageSemanticDigest::parse(&"00".repeat(32)).expect("provisional digest"),
    ))
}

fn semantic_content(
    analyzed: &AnalyzedResolvedHierarchy,
    selected: &CompilationNamespaceId,
) -> SemanticContentV1 {
    let declarations = analyzed
        .canonical_declarations()
        .iter()
        .filter(|declaration| declaration.namespace() == selected)
        .map(|declaration| {
            let kind = match declaration.kind() {
                CanonicalDeclarationKind::PureOperator => DeclarationKindV1::PureOperator,
                CanonicalDeclarationKind::Connector => DeclarationKindV1::Connector,
                CanonicalDeclarationKind::Component => DeclarationKindV1::Component,
                CanonicalDeclarationKind::Model => DeclarationKindV1::Model,
                other => panic!("unsupported package declaration kind {other:?}"),
            };
            SemanticDeclarationV1::new(
                QualifiedName::parse(declaration.path()).expect("declaration path"),
                kind,
                match declaration.visibility() {
                    CanonicalDeclarationVisibility::Private => VisibilityV1::Private,
                    CanonicalDeclarationVisibility::Public => VisibilityV1::Public,
                },
                CanonicalDeclaration::new(declaration.canonical_form())
                    .expect("canonical declaration"),
            )
        })
        .collect();
    SemanticContentV1::new(declarations).expect("semantic content")
}

pub(crate) fn package_release(
    manifest: AuthorManifestV1,
    analyzed: &AnalyzedResolvedHierarchy,
    selected: &CompilationNamespaceId,
    source_files: Vec<SourceFileV1>,
) -> PackageReleaseV1 {
    PackageReleaseV1::new(manifest, semantic_content(analyzed, selected), source_files)
        .expect("package release")
}
