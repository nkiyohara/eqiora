use std::collections::BTreeMap;

use eqiora_lang::{TextRange, VisibilitySyntax};

use super::{
    AnalyzedResolvedHierarchy, AnalyzedSourceUnit, CompilationNamespaceId,
    canonical_declaration_path,
};

impl AnalyzedResolvedHierarchy {
    /// Compiler-canonical declarations in `(namespace, path, kind)` order.
    #[must_use]
    pub fn canonical_declarations(&self) -> &[CanonicalDeclarationIdentity] {
        &self.canonical_declarations
    }
}

/// Top-level declaration families currently understood by package lowering.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum CanonicalDeclarationKind {
    PropertyContract,
    PropertyRelease,
    MaterialComposition,
    PureOperator,
    Connector,
    Component,
    Model,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CanonicalDeclarationVisibility {
    Private,
    Public,
}

impl From<VisibilitySyntax> for CanonicalDeclarationVisibility {
    fn from(value: VisibilitySyntax) -> Self {
        match value {
            VisibilitySyntax::Private => Self::Private,
            VisibilitySyntax::Public => Self::Public,
        }
    }
}

/// File-layout-independent canonical declaration emitted by the compiler.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalDeclarationIdentity {
    pub(super) namespace: CompilationNamespaceId,
    pub(super) path: String,
    pub(super) kind: CanonicalDeclarationKind,
    pub(super) visibility: CanonicalDeclarationVisibility,
    pub(super) canonical_form: String,
}

impl CanonicalDeclarationIdentity {
    #[must_use]
    pub const fn namespace(&self) -> &CompilationNamespaceId {
        &self.namespace
    }
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
    #[must_use]
    pub const fn kind(&self) -> CanonicalDeclarationKind {
        self.kind
    }
    #[must_use]
    pub const fn visibility(&self) -> CanonicalDeclarationVisibility {
        self.visibility
    }
    #[must_use]
    pub fn canonical_form(&self) -> &str {
        &self.canonical_form
    }
}

pub(super) fn collect_declaration_locations(
    units: &[AnalyzedSourceUnit],
    declarations: &[CanonicalDeclarationIdentity],
) -> Vec<(String, TextRange)> {
    let mut locations = BTreeMap::new();
    for unit in units {
        let namespace = unit.module.owner();
        let mut insert = |name: &str, kind, range| {
            locations.insert(
                (
                    namespace.clone(),
                    canonical_declaration_path(&unit.module, name),
                    kind,
                ),
                (unit.file.clone(), range),
            );
        };
        for (_, name, _, range) in unit.document.property_contract_syntax() {
            insert(name, CanonicalDeclarationKind::PropertyContract, range);
        }
        for (_, name, _, _, _, _, _, _, range) in unit.document.property_release_syntax() {
            insert(name, CanonicalDeclarationKind::PropertyRelease, range);
        }
        for (_, name, _, range) in unit.document.material_composition_syntax() {
            insert(name, CanonicalDeclarationKind::MaterialComposition, range);
        }
        for declaration in unit.document.connectors() {
            insert(
                declaration.name(),
                CanonicalDeclarationKind::Connector,
                declaration.range(),
            );
        }
        for declaration in unit.document.pure_operators() {
            insert(
                declaration.name(),
                CanonicalDeclarationKind::PureOperator,
                declaration.range(),
            );
        }
        for declaration in unit.document.components() {
            insert(
                declaration.name(),
                CanonicalDeclarationKind::Component,
                declaration.range(),
            );
        }
        for declaration in unit.document.models() {
            insert(
                declaration.name(),
                CanonicalDeclarationKind::Model,
                declaration.range(),
            );
        }
    }
    declarations
        .iter()
        .map(|declaration| {
            locations
                .remove(&(
                    declaration.namespace().clone(),
                    declaration.path().to_owned(),
                    declaration.kind(),
                ))
                .expect("every canonical declaration retains its source location")
        })
        .collect()
}
