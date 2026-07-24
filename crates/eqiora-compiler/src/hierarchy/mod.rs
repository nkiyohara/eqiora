//! Typed component elaboration into the existing flat Relation network.
//!
//! Hierarchy is compiler-owned source structure. This module resolves and
//! validates the complete instance tree, stages every semantic identity, and
//! only then invokes the ordinary flat lowerer once. No Component, Instance,
//! or ConnectorType node enters the Semantic Kernel.

use crate::resolved::AnalyzedResolvedHierarchy;
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{Document, TextRange};

use crate::connection_sets::ConnectionSetLimits;
use crate::diagnostics::source_error;
use crate::identity::ElaborationIdentityLimits;
use crate::lower::CompiledModel;
use crate::provenance::ProvenanceLimits;
use crate::source_identity::LocalSourceIdentity;

mod body_check;
mod check;
mod complete_exterior;
mod definition_graph;
mod expand;
mod exposure_cuts;
mod field_slots;
mod flat;
mod occurrence_connections;
mod parameters;
mod physical_closure;
mod preflight;
mod scope;
mod supports;

pub(crate) use definition_graph::CheckedDefinitionGraph;
use expand::RootExpansion;
use preflight::Elaborator;

/// Independent resource limits for one physical-exposure projection catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhysicalExposureProjectionLimits {
    /// Maximum eliminated exposure projections.
    pub max_projections: usize,
    /// Maximum retained Port memberships in one occurrence cut.
    pub max_members_per_cut: usize,
    /// Maximum retained Port memberships summed across every cut.
    pub max_memberships: usize,
    /// Maximum fragment-membership visits summed across cut derivation.
    pub max_traversal_memberships: usize,
}

impl Default for PhysicalExposureProjectionLimits {
    fn default() -> Self {
        Self {
            max_projections: 1_000_000,
            max_members_per_cut: 65_536,
            max_memberships: 4_000_000,
            max_traversal_memberships: 16_000_000,
        }
    }
}

/// Independent resource limits for occurrence-bound complete exteriors.
///
/// These limits constrain memberships, not the declarations produced by
/// boundary families. The latter remain governed by the ordinary hierarchy
/// declaration and connection budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompleteExteriorLimits {
    /// Maximum exact Boundary identities admitted by one explicit set.
    pub max_members_per_set: usize,
    /// Maximum explicit Boundary memberships resolved during one elaboration.
    pub max_total_memberships: usize,
}

impl Default for CompleteExteriorLimits {
    fn default() -> Self {
        Self {
            max_members_per_set: 65_536,
            max_total_memberships: 4_000_000,
        }
    }
}

/// Bounded policy for one typed hierarchy elaboration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HierarchyLimits {
    /// Maximum source bytes accepted by this elaboration entry point.
    pub max_source_bytes: usize,
    /// Maximum nested component instance depth, including the root Model.
    pub max_instance_depth: usize,
    /// Maximum component instances below one root Model.
    pub max_instances: usize,
    /// Maximum named Kernel declarations after flattening.
    pub max_declarations: usize,
    /// Maximum anonymous Connections after flattening.
    pub max_connections: usize,
    /// Independent topology-normalization budgets for physical connection
    /// fragments, endpoint memberships, maximal sets, and set width.
    pub connection_sets: ConnectionSetLimits,
    /// Independent budgets for eliminated physical exposure projections.
    pub physical_exposures: PhysicalExposureProjectionLimits,
    /// Independent budgets for occurrence-bound complete exteriors.
    pub complete_exteriors: CompleteExteriorLimits,
    /// Maximum source identifier bytes.
    pub max_identifier_bytes: usize,
    /// Maximum globally indexed Connector, pure-operator, Component, and Model definitions.
    pub max_definitions: usize,
    /// Maximum nested Component instance edges across definition bodies.
    pub max_definition_edges: usize,
    /// Maximum symbolic Parameter terms retained during definition checking.
    pub max_parameter_terms: usize,
    /// Maximum source diagnostics retained by definition validation.
    pub max_definition_diagnostics: usize,
    /// Maximum `(definition, reachable Connector)` memberships retained by
    /// exact reusable-definition summaries.
    pub max_definition_reachability_pairs: usize,
    /// Identity construction and projection limits.
    pub identity: ElaborationIdentityLimits,
    /// Source-provenance sidecar limits.
    pub provenance: ProvenanceLimits,
}

impl Default for HierarchyLimits {
    fn default() -> Self {
        let identity = ElaborationIdentityLimits::default();
        let connection_sets = ConnectionSetLimits {
            max_members_per_fragment: identity.max_anonymous_connection_members,
            max_members_per_set: identity.max_anonymous_connection_members,
            ..ConnectionSetLimits::default()
        };
        Self {
            max_source_bytes: 16 * 1_024 * 1_024,
            max_instance_depth: 64,
            max_instances: 1_000_000,
            max_declarations: 4_000_000,
            max_connections: 1_000_000,
            connection_sets,
            physical_exposures: PhysicalExposureProjectionLimits::default(),
            complete_exteriors: CompleteExteriorLimits::default(),
            max_identifier_bytes: 1_024,
            max_definitions: 1_000_000,
            max_definition_edges: 1_000_000,
            max_parameter_terms: 4_000_000,
            max_definition_diagnostics: 4_096,
            max_definition_reachability_pairs: 4_000_000,
            identity,
            provenance: ProvenanceLimits::default(),
        }
    }
}

pub(crate) fn compile_hierarchy(
    file: &str,
    source_bytes: usize,
    document: &Document,
) -> Result<Vec<CompiledModel>, Vec<Diagnostic>> {
    compile_hierarchy_with_limits(file, source_bytes, document, HierarchyLimits::default())
}

fn compile_hierarchy_with_limits(
    file: &str,
    source_bytes: usize,
    document: &Document,
    limits: HierarchyLimits,
) -> Result<Vec<CompiledModel>, Vec<Diagnostic>> {
    if source_bytes > limits.max_source_bytes {
        return Err(vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            TextRange::new(0, u32::try_from(source_bytes).unwrap_or(u32::MAX)),
            format!(
                "source requires {source_bytes} bytes, exceeding the {} byte hierarchy limit",
                limits.max_source_bytes
            ),
        )]);
    }
    let source_identity =
        LocalSourceIdentity::from_document(document).map_err(|error| vec![error])?;
    let elaborator = Elaborator::new(file, source_bytes, document, source_identity, limits)?;
    let checked = check::validate(&elaborator)?;
    let mut compiled = Vec::new();
    let mut diagnostics = Vec::new();
    for model in document.models() {
        let model = elaborator.local_model(model);
        let result = checked_model_expansion_size(&checked, &model)
            .and_then(|size| {
                RootExpansion::new(&elaborator, model, size).map_err(|error| vec![error])
            })
            .and_then(RootExpansion::expand)
            .and_then(|blueprint| blueprint.compile(limits));
        match result {
            Ok(model) => compiled.push(model),
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if diagnostics.is_empty() {
        Ok(compiled)
    } else {
        Err(diagnostics)
    }
}

pub(crate) fn compile_resolved_hierarchy(
    analysis: &AnalyzedResolvedHierarchy,
    checked: &CheckedDefinitionGraph,
    model: &str,
    limits: HierarchyLimits,
) -> Result<CompiledModel, Vec<Diagnostic>> {
    let elaborator = Elaborator::new_resolved(analysis, limits)?;
    let root = elaborator.root_model(model).ok_or_else(|| {
        vec![hierarchy_error(format!(
            "root namespace `{}` has no package-local Model `{model}`",
            analysis.root
        ))]
    })?;
    let size = checked_model_expansion_size(checked, &root)?;
    RootExpansion::new(&elaborator, root, size)
        .map_err(|error| vec![error])?
        .expand()?
        .compile(limits)
}

fn checked_model_expansion_size(
    checked: &CheckedDefinitionGraph,
    model: &preflight::ModelDefinition<'_>,
) -> Result<preflight::ExpansionSize, Vec<Diagnostic>> {
    let key = preflight::DefinitionKey {
        namespace: model.namespace.clone(),
        name: model.declaration.name().to_owned(),
    };
    let summary = checked.model_summary(&key).ok_or_else(|| {
        vec![hierarchy_error(format!(
            "validated definition graph has no summary for root Model `{}`",
            key.display()
        ))]
    })?;
    Ok(preflight::ExpansionSize {
        declarations: summary.declarations(),
        connections: summary.connections(),
    })
}

pub(crate) fn validate_resolved_hierarchy(
    analysis: &AnalyzedResolvedHierarchy,
    limits: HierarchyLimits,
) -> Result<(), Vec<Diagnostic>> {
    Elaborator::new_resolved(analysis, limits).map(|_| ())
}

pub(crate) fn validate_resolved_definitions(
    analysis: &AnalyzedResolvedHierarchy,
    limits: HierarchyLimits,
) -> Result<CheckedDefinitionGraph, Vec<Diagnostic>> {
    let elaborator = Elaborator::new_resolved(analysis, limits)?;
    check::validate(&elaborator)
}

fn hierarchy_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_LOWERING_ERROR, message)
}

#[cfg(test)]
mod tests;
