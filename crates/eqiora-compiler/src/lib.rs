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
mod external_compile;
mod formulation;
mod hierarchy;
#[doc(hidden)]
pub mod identity;
mod lower;
pub mod projection;
mod property;
#[doc(hidden)]
pub mod provenance;
mod pure_operator;
mod resolved;
mod source_compile;
#[doc(hidden)]
pub mod source_identity;

pub use formulation::{
    AuthoredFormExpressionV1, AuthoredFormulationProjection, CompiledAuthoredFormulation,
};
pub use lower::{CompiledModel, ModelSymbols, lower_draft, lower_model};
pub use resolved::{
    AnalyzedResolvedHierarchy, CanonicalDeclarationIdentity, CanonicalDeclarationKind,
    CanonicalDeclarationVisibility, CompilationNamespaceId, ResolvedAlias, ResolvedHierarchyInput,
    ResolvedSourceUnit, ValidatedResolvedHierarchy, analyze_resolved_hierarchy,
    preflight_resolved_hierarchy,
};
pub use source_compile::compile;
