//! Bounded proof of the reusable Component definition graph.
//!
//! Definition validation must not expand occurrence trees. A small acyclic
//! definition graph can describe an exponentially large future occurrence,
//! so this module retains instance-edge multiplicity while memoizing one
//! saturating footprint per definition. Cycle discovery is iterative and
//! independent of the occurrence-depth limit.

mod cycles;
use cycles::{cycle_diagnostic, strongly_connected_components};

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{ComponentItem, Item, PortSyntax, TextRange};

use crate::diagnostics::source_error;

use super::HierarchyLimits;
use super::preflight::{ComponentDefinition, DefinitionKey, Elaborator, ModelDefinition};

mod checked;
mod footprint;
/// Compiler-owned proof that Component references are acyclic and every
/// reusable definition has a bounded possible occurrence footprint.
///
/// Selected-root compilation reuses these summaries rather than recursively
/// recounting the definition DAG for each future occurrence.
/// The value remains private to hierarchy elaboration.
pub(crate) use checked::CheckedDefinitionGraph;
use footprint::{component_local_footprint, model_local_footprint};

/// Saturating footprint of one Component occurrence or one Model root.
///
/// Connector Domains are shared by nominal definition. Their exact reachable
/// set is retained separately from multiplicity-bearing ordinary declarations,
/// so Component factoring cannot change admission at a limit boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DefinitionSummary {
    component_levels: LimitedCount,
    instances: LimitedCount,
    ordinary_declarations: LimitedCount,
    declarations: LimitedCount,
    connections: LimitedCount,
    identity_nonconnector_entries: LimitedCount,
    provenance_nonconnector_entries: LimitedCount,
    staged_identities: LimitedCount,
    provenance_entries: LimitedCount,
    connector_domains: BTreeSet<DefinitionKey>,
}

impl DefinitionSummary {
    #[cfg(test)]
    pub(super) fn component_levels(&self) -> usize {
        self.component_levels.observed()
    }

    #[cfg(test)]
    pub(super) fn instances(&self) -> usize {
        self.instances.observed()
    }

    pub(super) fn declarations(&self) -> usize {
        self.declarations.observed()
    }

    pub(super) fn connections(&self) -> usize {
        self.connections.observed()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LimitedCount {
    value: usize,
    exceeded: bool,
}

impl LimitedCount {
    const fn exact(value: usize) -> Self {
        Self {
            value,
            exceeded: false,
        }
    }

    fn add(self, other: Self, limit: usize) -> Self {
        if self.exceeded || other.exceeded {
            return Self::beyond(limit);
        }
        match self.value.checked_add(other.value) {
            Some(value) if value <= limit => Self::exact(value),
            Some(_) | None => Self::beyond(limit),
        }
    }

    fn increment(self, limit: usize) -> Self {
        self.add(Self::exact(1), limit)
    }

    const fn max(self, other: Self) -> Self {
        if self.exceeded {
            self
        } else if other.exceeded || other.value > self.value {
            other
        } else {
            self
        }
    }

    const fn beyond(limit: usize) -> Self {
        Self {
            value: limit.saturating_add(1),
            exceeded: true,
        }
    }

    const fn observed(self) -> usize {
        self.value
    }

    const fn exceeds(self, limit: usize) -> bool {
        self.exceeded || self.value > limit
    }
}

#[derive(Clone, Copy, Debug)]
struct CountLimits {
    depth: usize,
    instances: usize,
    declarations: usize,
    connections: usize,
    staged_identities: usize,
    provenance_entries: usize,
    reachability_pairs: usize,
}

impl From<HierarchyLimits> for CountLimits {
    fn from(limits: HierarchyLimits) -> Self {
        Self {
            depth: limits.max_instance_depth,
            instances: limits.max_instances,
            declarations: limits.max_declarations,
            connections: limits.max_connections,
            staged_identities: limits.identity.max_staged_identities,
            provenance_entries: limits.provenance.max_entries,
            reachability_pairs: limits.max_definition_reachability_pairs,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct LocalFootprint {
    declarations: usize,
    connections: usize,
}

struct ReachabilityMemberships {
    used: usize,
    limit: usize,
}

impl ReachabilityMemberships {
    const fn new(limit: usize) -> Self {
        Self { used: 0, limit }
    }

    fn insert(
        &mut self,
        set: &mut BTreeSet<DefinitionKey>,
        connector: &DefinitionKey,
        file: &str,
        range: TextRange,
    ) -> Result<(), Vec<Diagnostic>> {
        if set.contains(connector) {
            return Ok(());
        }
        let required = self.used.checked_add(1).ok_or_else(|| {
            vec![source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "definition reachability-pair count overflows usize",
            )]
        })?;
        if required > self.limit {
            return Err(vec![source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                format!(
                    "exact Connector reachability summaries require {required} definition-membership pairs, exceeding the {} reachability-pair limit",
                    self.limit
                ),
            )]);
        }
        set.insert(connector.clone());
        self.used = required;
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct Edge<'a> {
    target: usize,
    occurrence: String,
    file: &'a str,
    range: TextRange,
}

#[derive(Clone)]
struct ComponentNode<'a> {
    key: DefinitionKey,
    file: &'a str,
    range: TextRange,
    local: LocalFootprint,
    local_connectors: BTreeSet<DefinitionKey>,
    edges: Vec<Edge<'a>>,
}

struct ModelNode<'a> {
    key: DefinitionKey,
    file: &'a str,
    range: TextRange,
    local: LocalFootprint,
    local_connectors: BTreeSet<DefinitionKey>,
    edges: Vec<Edge<'a>>,
}

struct DefinitionOrder {
    children_first: Vec<usize>,
    cyclic_components: Vec<Vec<usize>>,
}

/// Validate and summarize the complete resolved definition graph.
pub(super) fn validate(
    elaborator: &Elaborator<'_>,
) -> Result<CheckedDefinitionGraph, Vec<Diagnostic>> {
    let definition_count = elaborator
        .connectors()
        .len()
        .checked_add(elaborator.pure_operators().len())
        .and_then(|count| count.checked_add(elaborator.components().len()))
        .and_then(|count| count.checked_add(elaborator.models().len()))
        .ok_or_else(|| vec![definition_error("definition count overflows usize")])?;
    if definition_count > elaborator.limits.max_definitions {
        return Err(vec![definition_error(format!(
            "resolved hierarchy has {definition_count} definitions, exceeding the {} definition limit",
            elaborator.limits.max_definitions
        ))]);
    }

    let limits = CountLimits::from(elaborator.limits);
    let mut diagnostics = Vec::new();
    let (components, models) = build_graph(elaborator, &mut diagnostics)?;
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let order = strongly_connected_components(&components)?;
    for component in order.cyclic_components {
        diagnostics.push(cycle_diagnostic(&components, &component));
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut reachability = ReachabilityMemberships::new(limits.reachability_pairs);
    let mut summaries = vec![None; components.len()];
    let component_order = order
        .children_first
        .iter()
        .map(|index| components[*index].key.clone())
        .collect::<Vec<_>>();
    for index in order.children_first {
        let summary =
            summarize_component(&components[index], &summaries, limits, &mut reachability)?;
        append_limit_diagnostics(
            &mut diagnostics,
            &components[index].key,
            components[index].file,
            components[index].range,
            &summary,
            true,
            elaborator.limits,
        );
        summaries[index] = Some(summary);
    }

    let component_summaries = components
        .iter()
        .zip(summaries.iter())
        .map(|(component, summary)| {
            (
                component.key.clone(),
                summary
                    .clone()
                    .expect("every acyclic Component was summarized"),
            )
        })
        .collect();
    let mut model_summaries = BTreeMap::new();
    for model in models {
        let summary = summarize_model(&model, &summaries, limits, &mut reachability)?;
        append_limit_diagnostics(
            &mut diagnostics,
            &model.key,
            model.file,
            model.range,
            &summary,
            false,
            elaborator.limits,
        );
        model_summaries.insert(model.key, summary);
    }

    if diagnostics.is_empty() {
        Ok(CheckedDefinitionGraph::new(
            component_order,
            component_summaries,
            model_summaries,
        ))
    } else {
        Err(diagnostics)
    }
}

fn build_graph<'d>(
    elaborator: &Elaborator<'d>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<(Vec<ComponentNode<'d>>, Vec<ModelNode<'d>>), Vec<Diagnostic>> {
    let component_count = elaborator.components().len();
    let mut index = BTreeMap::new();
    let mut keys = Vec::new();
    keys.try_reserve_exact(component_count)
        .map_err(|_| vec![definition_error("cannot reserve component definition keys")])?;
    for (ordinal, (key, _)) in elaborator.components().enumerate() {
        index.insert(key.clone(), ordinal);
        keys.push(key.clone());
    }

    let mut edge_count = 0_usize;
    let mut components = Vec::new();
    components.try_reserve_exact(component_count).map_err(|_| {
        vec![definition_error(
            "cannot reserve component definition graph",
        )]
    })?;
    for (key, definition) in elaborator.components() {
        let mut edges = component_edges(elaborator, definition, &index, diagnostics);
        edge_count = checked_edge_total(
            edge_count,
            edges.len(),
            elaborator.limits,
            definition.file,
            definition.declaration.range(),
        )?;
        sort_edges(&mut edges, &keys);
        let (local, local_connectors) =
            component_local_footprint(elaborator, definition, diagnostics);
        components.push(ComponentNode {
            key: key.clone(),
            file: definition.file,
            range: definition.declaration.range(),
            local,
            local_connectors,
            edges,
        });
    }

    let mut models = Vec::new();
    models
        .try_reserve_exact(elaborator.models().len())
        .map_err(|_| vec![definition_error("cannot reserve Model definition graph")])?;
    for (key, definition) in elaborator.models() {
        let mut edges = model_edges(elaborator, definition, &index, diagnostics);
        edge_count = checked_edge_total(
            edge_count,
            edges.len(),
            elaborator.limits,
            definition.file,
            definition.declaration.range(),
        )?;
        sort_edges(&mut edges, &keys);
        models.push(ModelNode {
            key: key.clone(),
            file: definition.file,
            range: definition.declaration.range(),
            local: model_local_footprint(elaborator, definition, diagnostics),
            local_connectors: BTreeSet::new(),
            edges,
        });
    }
    Ok((components, models))
}

fn sort_edges(edges: &mut [Edge<'_>], keys: &[DefinitionKey]) {
    edges.sort_by(|left, right| {
        (&keys[left.target], left.occurrence.as_str())
            .cmp(&(&keys[right.target], right.occurrence.as_str()))
    });
}

fn checked_edge_total(
    current: usize,
    additional: usize,
    limits: HierarchyLimits,
    file: &str,
    range: TextRange,
) -> Result<usize, Vec<Diagnostic>> {
    let total = current.checked_add(additional).ok_or_else(|| {
        vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "component definition-edge count overflows usize",
        )]
    })?;
    if total > limits.max_definition_edges {
        return Err(vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            format!(
                "resolved hierarchy exceeds the {} definition-edge limit",
                limits.max_definition_edges
            ),
        )]);
    }
    Ok(total)
}

fn component_edges<'d>(
    elaborator: &Elaborator<'d>,
    definition: &ComponentDefinition<'d>,
    index: &BTreeMap<DefinitionKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Edge<'d>> {
    let edge_capacity = definition
        .declaration
        .items()
        .iter()
        .filter(|item| matches!(item, ComponentItem::Instance(_)))
        .count();
    let mut edges = Vec::new();
    if edges.try_reserve_exact(edge_capacity).is_err() {
        diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            definition.file,
            definition.declaration.range(),
            "cannot reserve Component definition edges",
        ));
        return edges;
    }
    for item in definition.declaration.items() {
        let ComponentItem::Instance(instance) = item else {
            continue;
        };
        match elaborator.resolve_component(
            &definition.namespace,
            instance.definition(),
            definition.file,
            instance.range(),
        ) {
            Ok(child) => {
                let key = DefinitionKey {
                    namespace: child.namespace,
                    name: child.declaration.name().to_owned(),
                };
                match index.get(&key).copied() {
                    Some(target) => edges.push(Edge {
                        target,
                        occurrence: instance.name().to_owned(),
                        file: definition.file,
                        range: instance.range(),
                    }),
                    None => diagnostics.push(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        definition.file,
                        instance.range(),
                        format!(
                            "resolved component `{}` is absent from the definition index",
                            key.display()
                        ),
                    )),
                }
            }
            Err(error) => diagnostics.push(error),
        }
    }
    edges
}

fn model_edges<'d>(
    elaborator: &Elaborator<'d>,
    definition: &ModelDefinition<'d>,
    index: &BTreeMap<DefinitionKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<Edge<'d>> {
    let edge_capacity = definition
        .declaration
        .items()
        .iter()
        .filter(|item| matches!(item, Item::Instance(_)))
        .count();
    let mut edges = Vec::new();
    if edges.try_reserve_exact(edge_capacity).is_err() {
        diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            definition.file,
            definition.declaration.range(),
            "cannot reserve Model definition edges",
        ));
        return edges;
    }
    for item in definition.declaration.items() {
        let Item::Instance(instance) = item else {
            continue;
        };
        match elaborator.resolve_component(
            &definition.namespace,
            instance.definition(),
            definition.file,
            instance.range(),
        ) {
            Ok(child) => {
                let key = DefinitionKey {
                    namespace: child.namespace,
                    name: child.declaration.name().to_owned(),
                };
                match index.get(&key).copied() {
                    Some(target) => edges.push(Edge {
                        target,
                        occurrence: instance.name().to_owned(),
                        file: definition.file,
                        range: instance.range(),
                    }),
                    None => diagnostics.push(source_error(
                        codes::LANGUAGE_LOWERING_ERROR,
                        definition.file,
                        instance.range(),
                        format!(
                            "resolved component `{}` is absent from the definition index",
                            key.display()
                        ),
                    )),
                }
            }
            Err(error) => diagnostics.push(error),
        }
    }
    edges
}

fn summarize_component(
    node: &ComponentNode<'_>,
    summaries: &[Option<DefinitionSummary>],
    limits: CountLimits,
    reachability: &mut ReachabilityMemberships,
) -> Result<DefinitionSummary, Vec<Diagnostic>> {
    let mut depth = LimitedCount::exact(1);
    let mut instances = LimitedCount::exact(1);
    let mut ordinary_declarations = LimitedCount::exact(node.local.declarations);
    let mut connections = LimitedCount::exact(node.local.connections);
    let local_nonconnector = node
        .local
        .declarations
        .checked_add(node.local.connections)
        .ok_or_else(|| {
            vec![source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                node.file,
                node.range,
                "local semantic entry count overflows usize",
            )]
        })?;
    let mut identity_nonconnector_entries = LimitedCount::exact(local_nonconnector);
    let mut provenance_nonconnector_entries = LimitedCount::exact(local_nonconnector);
    let mut connector_domains = BTreeSet::new();
    for connector in &node.local_connectors {
        reachability.insert(&mut connector_domains, connector, node.file, node.range)?;
    }
    for edge in &node.edges {
        let child = summaries[edge.target]
            .as_ref()
            .expect("finish order summarizes children first");
        depth = depth.max(child.component_levels.increment(limits.depth));
        instances = instances.add(child.instances, limits.instances);
        ordinary_declarations =
            ordinary_declarations.add(child.ordinary_declarations, limits.declarations);
        connections = connections.add(child.connections, limits.connections);
        identity_nonconnector_entries = identity_nonconnector_entries.add(
            child.identity_nonconnector_entries,
            limits.staged_identities,
        );
        provenance_nonconnector_entries = provenance_nonconnector_entries.add(
            child.provenance_nonconnector_entries,
            limits.provenance_entries,
        );
        for connector in &child.connector_domains {
            reachability.insert(&mut connector_domains, connector, node.file, node.range)?;
        }
    }
    let connector_count = LimitedCount::exact(connector_domains.len());
    let declarations = ordinary_declarations.add(connector_count, limits.declarations);
    let staged_identities = identity_nonconnector_entries
        .add(connector_count, limits.staged_identities)
        .increment(limits.staged_identities);
    let provenance_entries = provenance_nonconnector_entries
        .add(connector_count, limits.provenance_entries)
        .increment(limits.provenance_entries);
    Ok(DefinitionSummary {
        component_levels: depth,
        instances,
        ordinary_declarations,
        declarations,
        connections,
        identity_nonconnector_entries,
        provenance_nonconnector_entries,
        staged_identities,
        provenance_entries,
        connector_domains,
    })
}

fn summarize_model(
    node: &ModelNode<'_>,
    summaries: &[Option<DefinitionSummary>],
    limits: CountLimits,
    reachability: &mut ReachabilityMemberships,
) -> Result<DefinitionSummary, Vec<Diagnostic>> {
    let mut depth = LimitedCount::exact(1);
    let mut instances = LimitedCount::exact(0);
    let mut ordinary_declarations = LimitedCount::exact(node.local.declarations);
    let mut connections = LimitedCount::exact(node.local.connections);
    let local_nonconnector = node
        .local
        .declarations
        .checked_add(node.local.connections)
        .ok_or_else(|| {
            vec![source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                node.file,
                node.range,
                "local semantic entry count overflows usize",
            )]
        })?;
    let mut identity_nonconnector_entries = LimitedCount::exact(local_nonconnector);
    let mut provenance_nonconnector_entries = LimitedCount::exact(local_nonconnector);
    let mut connector_domains = BTreeSet::new();
    for connector in &node.local_connectors {
        reachability.insert(&mut connector_domains, connector, node.file, node.range)?;
    }
    for edge in &node.edges {
        let child = summaries[edge.target]
            .as_ref()
            .expect("all Component summaries exist");
        depth = depth.max(child.component_levels.increment(limits.depth));
        instances = instances.add(child.instances, limits.instances);
        ordinary_declarations =
            ordinary_declarations.add(child.ordinary_declarations, limits.declarations);
        connections = connections.add(child.connections, limits.connections);
        identity_nonconnector_entries = identity_nonconnector_entries.add(
            child.identity_nonconnector_entries,
            limits.staged_identities,
        );
        provenance_nonconnector_entries = provenance_nonconnector_entries.add(
            child.provenance_nonconnector_entries,
            limits.provenance_entries,
        );
        for connector in &child.connector_domains {
            reachability.insert(&mut connector_domains, connector, node.file, node.range)?;
        }
    }
    let connector_count = LimitedCount::exact(connector_domains.len());
    let declarations = ordinary_declarations.add(connector_count, limits.declarations);
    let staged_identities = identity_nonconnector_entries
        .add(connector_count, limits.staged_identities)
        .increment(limits.staged_identities);
    let provenance_entries = provenance_nonconnector_entries
        .add(connector_count, limits.provenance_entries)
        .increment(limits.provenance_entries);
    Ok(DefinitionSummary {
        component_levels: depth,
        instances,
        ordinary_declarations,
        declarations,
        connections,
        identity_nonconnector_entries,
        provenance_nonconnector_entries,
        staged_identities,
        provenance_entries,
        connector_domains,
    })
}

#[allow(clippy::too_many_arguments)]
fn append_limit_diagnostics(
    diagnostics: &mut Vec<Diagnostic>,
    key: &DefinitionKey,
    file: &str,
    range: TextRange,
    summary: &DefinitionSummary,
    future_wrapper: bool,
    limits: HierarchyLimits,
) {
    let depth = if future_wrapper {
        summary
            .component_levels
            .increment(limits.max_instance_depth)
    } else {
        summary.component_levels
    };
    let subject = if future_wrapper {
        format!("component `{}`", key.display())
    } else {
        format!("Model `{}`", key.display())
    };
    if depth.exceeds(limits.max_instance_depth) {
        let resource = if future_wrapper {
            "future Model-relative instance depth"
        } else {
            "Model-relative instance depth"
        };
        diagnostics.push(limit_error(
            file,
            range,
            &subject,
            resource,
            depth.observed(),
            limits.max_instance_depth,
        ));
    }
    if summary.instances.exceeds(limits.max_instances) {
        diagnostics.push(limit_error(
            file,
            range,
            &subject,
            "Component instances",
            summary.instances.observed(),
            limits.max_instances,
        ));
    }
    if summary.declarations.exceeds(limits.max_declarations) {
        diagnostics.push(limit_error(
            file,
            range,
            &subject,
            "declarations",
            summary.declarations.observed(),
            limits.max_declarations,
        ));
    }
    if summary.connections.exceeds(limits.max_connections) {
        diagnostics.push(limit_error(
            file,
            range,
            &subject,
            "Connections",
            summary.connections.observed(),
            limits.max_connections,
        ));
    }
    if summary
        .staged_identities
        .exceeds(limits.identity.max_staged_identities)
    {
        diagnostics.push(limit_error(
            file,
            range,
            &subject,
            "staged identities",
            summary.staged_identities.observed(),
            limits.identity.max_staged_identities,
        ));
    }
    if summary
        .provenance_entries
        .exceeds(limits.provenance.max_entries)
    {
        diagnostics.push(limit_error(
            file,
            range,
            &subject,
            "provenance entries",
            summary.provenance_entries.observed(),
            limits.provenance.max_entries,
        ));
    }
}

fn limit_error(
    file: &str,
    range: TextRange,
    subject: &str,
    resource: &str,
    observed: usize,
    limit: usize,
) -> Diagnostic {
    source_error(
        codes::LANGUAGE_LOWERING_ERROR,
        file,
        range,
        format!("{subject} requires {observed} {resource}, exceeding the {limit} limit"),
    )
}

fn definition_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_LOWERING_ERROR, message)
}

#[cfg(test)]
mod tests {
    use eqiora_lang::parse;

    use crate::source_identity::LocalSourceIdentity;

    use super::*;

    fn validate_source(
        source: &str,
        limits: HierarchyLimits,
    ) -> Result<CheckedDefinitionGraph, Vec<Diagnostic>> {
        let document = parse("definition-graph.eqi", source)
            .into_compilation_document()
            .expect("definition graph fixture parses");
        let source_identity =
            LocalSourceIdentity::from_document(&document).expect("fixture has canonical identity");
        let elaborator = Elaborator::new(
            "definition-graph.eqi",
            source.len(),
            &document,
            source_identity,
            limits,
        )
        .expect("fixture scopes resolve");
        validate(&elaborator)
    }

    fn model_summary<'a>(graph: &'a CheckedDefinitionGraph, name: &str) -> &'a DefinitionSummary {
        graph
            .model_summaries
            .iter()
            .find_map(|(key, summary)| (key.display() == name).then_some(summary))
            .expect("Model summary exists")
    }

    #[test]
    fn future_model_depth_boundary_is_exact() {
        let source = "component C2() {} component C1() { instance c2: C2; } component C0() { instance c1: C1; } model Main { instance root: C0; }";
        let limits = HierarchyLimits {
            max_instance_depth: 4,
            ..HierarchyLimits::default()
        };
        let graph = validate_source(source, limits).expect("Model plus three Components fits");
        assert_eq!(model_summary(&graph, "Main").component_levels(), 4);

        let over = "component C3() {} component C2() { instance c3: C3; } component C1() { instance c2: C2; } component C0() { instance c1: C1; } model Main { instance root: C0; }";
        let diagnostics = validate_source(over, limits).expect_err("fifth level fails");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("Model-relative instance depth")
                && diagnostic.source_span().is_some()
        }));
    }

    #[test]
    fn cycle_detection_is_independent_of_depth_cutoff() {
        let source = "component C0() { instance c1: C1; } component C1() { instance c2: C2; } component C2() { instance c3: C3; } component C3() { instance c4: C4; } component C4() { instance c0: C0; } model Main {}";
        let limits = HierarchyLimits {
            max_instance_depth: 4,
            ..HierarchyLimits::default()
        };
        let diagnostics = validate_source(source, limits).expect_err("cycle fails");
        assert_eq!(
            diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.message().contains("recursive component"))
                .count(),
            1
        );
        assert!(diagnostics[0].source_span().is_some());
    }

    #[test]
    fn repeated_definition_edges_retain_occurrence_multiplicity() {
        let source = "component Leaf() {} component Branch() { instance a: Leaf; instance b: Leaf; } component Root() { instance x: Branch; instance y: Branch; } model Main { instance root: Root; }";
        let graph = validate_source(source, HierarchyLimits::default()).expect("DAG is bounded");
        let root = model_summary(&graph, "Main");
        assert_eq!(root.instances(), 7);
        assert_eq!(root.component_levels(), 4);
    }

    #[test]
    fn exponential_occurrence_fails_from_memoized_definition_summary() {
        let source = "component C10() {} component C9() { instance a:C10; instance b:C10; } component C8() { instance a:C9; instance b:C9; } component C7() { instance a:C8; instance b:C8; } component C6() { instance a:C7; instance b:C7; } component C5() { instance a:C6; instance b:C6; } component C4() { instance a:C5; instance b:C5; } component C3() { instance a:C4; instance b:C4; } component C2() { instance a:C3; instance b:C3; } component C1() { instance a:C2; instance b:C2; } component C0() { instance a:C1; instance b:C1; } model Main {}";
        let limits = HierarchyLimits {
            max_instances: 1_000,
            ..HierarchyLimits::default()
        };
        let diagnostics = validate_source(source, limits).expect_err("2^11-1 exceeds bound");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message().contains("Component instances")
                && diagnostic.source_span().is_some()
        }));
    }

    #[test]
    fn model_edges_share_the_definition_edge_budget() {
        let source = "component Leaf() {} model Main { instance a:Leaf; instance b:Leaf; }";
        let limits = HierarchyLimits {
            max_definition_edges: 1,
            ..HierarchyLimits::default()
        };
        let diagnostics = validate_source(source, limits).expect_err("second Model edge exceeds");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.message().contains("definition-edge limit")
                && diagnostic.source_span().is_some()
        }));
    }
}
