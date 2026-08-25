//! Typed component elaboration into the existing flat Relation network.
//!
//! Hierarchy is compiler-owned source structure. This module resolves and
//! validates the complete instance tree, stages every semantic identity, and
//! only then invokes the ordinary flat lowerer once. No Component, Instance,
//! or ConnectorType node enters the Semantic Kernel.

use crate::resolved::AnalyzedResolvedHierarchy;
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{Document, ExprKind, Item, NamePath, SourceAstFactory, TextRange, parse};

use crate::connection_sets::ConnectionSetLimits;
use crate::diagnostics::source_error;
use crate::identity::ElaborationIdentityLimits;
use crate::lower::CompiledModel;
use crate::provenance::ProvenanceLimits;
use crate::source_identity::LocalSourceIdentity;
use crate::{ExternalComponentBinding, ExternalGeometrySupportBinding};

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

/// Compile one local Component definition as an ephemeral root occurrence
/// carrying caller-provided external Geometry support identities.
///
/// The external root is compiler-owned structure: no Cartesian stand-in,
/// formatted source, transaction rewrite, or second lowerer is constructed.
/// This step validates the binding's shape, not its digest or entity-set names
/// against a concrete Geometry artifact; geometry-aware semantic admission
/// performs that validation later.
///
/// # Errors
/// Returns accumulated source, binding, hierarchy, or typed-lowering
/// diagnostics. No partial transaction is returned.
pub fn compile_external_component(
    file: &str,
    source: &str,
    binding: &ExternalComponentBinding,
) -> Result<CompiledModel, Vec<Diagnostic>> {
    let limits = HierarchyLimits::default();
    if source.len() > limits.max_source_bytes {
        return Err(vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            TextRange::new(0, u32::try_from(source.len()).unwrap_or(u32::MAX)),
            format!(
                "source requires {} bytes, exceeding the {} byte hierarchy limit",
                source.len(),
                limits.max_source_bytes
            ),
        )]);
    }
    let document = parse(file, source).into_document()?;
    if !document.models().is_empty() {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            TextRange::default(),
            "external Component binding requires a definitions-only source without a root Model",
        )]);
    }
    validate_external_support_inventory(file, binding.supports())?;

    let source_identity =
        LocalSourceIdentity::from_document(&document).map_err(|error| vec![error])?;
    let elaborator = Elaborator::new(file, source.len(), &document, source_identity, limits)?;
    let checked = check::validate(&elaborator)?;
    let range = TextRange::default();
    let component_path = NamePath::from_segments([binding.component()], range)
        .map_err(|error| vec![hierarchy_error(error.message())])?;
    let component = elaborator
        .resolve_component(
            &preflight::DefinitionNamespace::Local,
            &component_path,
            file,
            range,
        )
        .map_err(|error| vec![error])?;
    let key = preflight::DefinitionKey {
        namespace: preflight::DefinitionNamespace::Local,
        name: component.name().to_owned(),
    };
    let summary = checked.component_summary(&key).ok_or_else(|| {
        vec![hierarchy_error(format!(
            "validated definition graph has no summary for Component `{}`",
            component.name()
        ))]
    })?;
    let declarations = summary
        .declarations()
        .checked_add(binding.supports().len())
        .ok_or_else(|| {
            vec![hierarchy_error(
                "external declaration count overflows usize",
            )]
        })?;
    if declarations > limits.max_declarations {
        return Err(vec![hierarchy_error(format!(
            "external Component occurrence has {declarations} declarations, exceeding the {} declaration limit",
            limits.max_declarations
        ))]);
    }

    let parameter_bindings = binding
        .parameters()
        .iter()
        .map(|parameter| {
            let value = SourceAstFactory::expression(ExprKind::Number(parameter.value()), range)?;
            SourceAstFactory::parameter_binding(parameter.parameter(), value, range)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| vec![hierarchy_error(error.message())])?;
    let support_bindings = binding
        .supports()
        .iter()
        .map(|support| SourceAstFactory::support_binding(support.slot(), support.slot(), range))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| vec![hierarchy_error(error.message())])?;
    let instance = SourceAstFactory::instance_with_support_bindings(
        "definition",
        component_path,
        parameter_bindings,
        support_bindings,
        range,
    )
    .map_err(|error| vec![hierarchy_error(error.message())])?;
    let root = SourceAstFactory::model(binding.model(), vec![Item::Instance(instance)], range)
        .map_err(|error| vec![hierarchy_error(error.message())])?;
    let model = preflight::ModelDefinition {
        namespace: preflight::DefinitionNamespace::Local,
        file,
        declaration: &root,
    };
    RootExpansion::new(
        &elaborator,
        model,
        preflight::ExpansionSize {
            declarations,
            connections: summary.connections(),
        },
    )
    .map_err(|error| vec![error])?
    .expand_external(component, binding.supports())?
    .compile(limits)
}

fn validate_external_support_inventory(
    file: &str,
    supports: &[ExternalGeometrySupportBinding],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut slots = std::collections::BTreeSet::new();
    let mut entity_sets = std::collections::BTreeSet::new();
    let geometry = supports
        .first()
        .map(ExternalGeometrySupportBinding::geometry);
    for support in supports {
        if !slots.insert(support.slot()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                TextRange::default(),
                format!(
                    "duplicate external support binding for slot `{}`",
                    support.slot()
                ),
            ));
        }
        if !entity_sets.insert(support.entity_set()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                TextRange::default(),
                format!(
                    "external Geometry entity set `{}` is bound to more than one support slot",
                    support.entity_set()
                ),
            ));
        }
        if Some(support.geometry()) != geometry {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                TextRange::default(),
                "external support bindings must name one exact Geometry identity",
            ));
        }
        if let ExternalGeometrySupportBinding::Region {
            ambient_dimension, ..
        } = support
            && *ambient_dimension == 0
        {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                TextRange::default(),
                format!(
                    "external region support `{}` has zero ambient dimension",
                    support.slot()
                ),
            ));
        }
    }
    for support in supports {
        let ExternalGeometrySupportBinding::Boundary { parent_slot, .. } = support else {
            continue;
        };
        if !supports.iter().any(|candidate| {
            matches!(candidate, ExternalGeometrySupportBinding::Region { slot, .. } if slot == parent_slot)
        }) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                TextRange::default(),
                format!(
                    "external boundary support `{}` has no region parent binding `{parent_slot}`",
                    support.slot()
                ),
            ));
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
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
