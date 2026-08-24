//! **eqiora-compiler** — typed lowering from source AST to graph transaction.
//!
//! Parsing and recovery stay in `eqiora-lang`; storage stays in
//! `eqiora-graph`. This crate is the only bridge that resolves source names,
//! checks SI dimensions, constructs inspectable expression DAGs, and emits the
//! same typed transaction available through the handwritten Rust API.

#[doc(hidden)]
pub mod connection_sets;
mod diagnostics;
mod dimensions;
mod external;
mod hierarchy;
#[doc(hidden)]
pub mod identity;
mod lower;
pub mod projection;
#[doc(hidden)]
pub mod provenance;
mod pure_operator;
mod resolved;
#[doc(hidden)]
pub mod source_identity;

pub use external::{
    ExternalComponentBinding, ExternalGeometrySupportBinding, ExternalParameterBinding,
};
pub use hierarchy::compile_external_component;
pub use lower::{CompiledModel, ModelSymbols, compile, lower_draft, lower_model};
pub use resolved::{
    AnalyzedResolvedHierarchy, CanonicalDeclarationIdentity, CanonicalDeclarationKind,
    CanonicalDeclarationVisibility, CompilationNamespaceId, ResolvedAlias, ResolvedHierarchyInput,
    ResolvedSourceUnit, ValidatedResolvedHierarchy, analyze_resolved_hierarchy,
    preflight_resolved_hierarchy,
};
