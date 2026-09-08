//! Bound selected-root footprint before any occurrence graph allocation.
struct ChildFootprint<'a> {
    edges: Vec<Edge<'a>>,
    summaries: Vec<Option<DefinitionSummary>>,
    extra_connections: usize,
}

use super::super::{clocks, parameters};
use super::*;
use eqiora_lang::{ExprKind, InstanceDecl, NamedDefinitionDecl};
use parameters::SymbolicParameterMap;

type Values = Vec<(String, Option<eqiora_core::ValueLiteral>)>;

pub(in crate::hierarchy) fn selected_expansion_size(
    elaborator: &Elaborator<'_>,
    checked: &CheckedDefinitionGraph,
    model: &ModelDefinition<'_>,
) -> Result<super::super::preflight::ExpansionSize, Vec<Diagnostic>> {
    let values = model_values(model)?;
    selected_expansion_size_with_contexts(elaborator, checked, model, values, None)
}

pub(in crate::hierarchy) type ComponentContexts =
    BTreeMap<DefinitionKey, Vec<SymbolicParameterMap>>;

pub(in crate::hierarchy) fn component_contexts(
    elaborator: &Elaborator<'_>,
    checked: &CheckedDefinitionGraph,
) -> Result<ComponentContexts, Vec<Diagnostic>> {
    let mut contexts = ComponentContexts::new();
    for (_, model) in elaborator.models() {
        let values = model_values(model)?;
        selected_expansion_size_with_contexts(
            elaborator,
            checked,
            model,
            values,
            Some(&mut contexts),
        )?;
    }
    Ok(contexts)
}

fn model_values(model: &ModelDefinition<'_>) -> Result<SymbolicParameterMap, Vec<Diagnostic>> {
    let mut values =
        parameters::resolve_model_parameters_symbolically(model.file, model.declaration, |name| {
            clocks::model(model.file, model.declaration, name)
        })?;
    parameters::resolve_model_lets(model.file, model.declaration, &mut values, |name| {
        clocks::model(model.file, model.declaration, name)
    })?;
    Ok(values)
}

fn selected_expansion_size_with_contexts(
    elaborator: &Elaborator<'_>,
    checked: &CheckedDefinitionGraph,
    model: &ModelDefinition<'_>,
    values: SymbolicParameterMap,
    contexts: Option<&mut ComponentContexts>,
) -> Result<super::super::preflight::ExpansionSize, Vec<Diagnostic>> {
    let context_count = contexts
        .as_ref()
        .map_or(0, |contexts| contexts.values().map(Vec::len).sum());
    let mut preflight = Selected {
        elaborator,
        checked,
        cache: Vec::new(),
        indexed_subtrees: BTreeMap::new(),
        contexts,
        context_count,
    };
    let mut diagnostics = Vec::new();
    let symbolic = preflight.contexts.is_some()
        && unresolved_extents(
            model.file,
            model.owned_items().filter_map(|item| match item {
                Item::IndexSet(set) => Some(set),
                _ => None,
            }),
            &values,
        )?;
    let mut local = model_local_footprint(
        elaborator,
        model,
        &mut diagnostics,
        if symbolic { None } else { Some(&values) },
    );
    local.declarations = local
        .declarations
        .checked_add(
            elaborator
                .finite_spaces
                .values()
                .map(BTreeMap::len)
                .sum::<usize>(),
        )
        .ok_or_else(|| vec![definition_error("finite-space footprint overflows")])?;
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let sets = model
        .owned_items()
        .filter_map(|item| match item {
            Item::IndexSet(value) => Some(value),
            _ => None,
        })
        .collect::<Vec<_>>();
    let instances = model.owned_items().filter_map(|item| match item {
        Item::Instance(value) => Some(value),
        _ => None,
    });
    let ChildFootprint {
        edges,
        summaries,
        extra_connections,
    } = preflight.children(&model.namespace, model.file, instances, &sets, &values, 0)?;
    local.connections = local
        .connections
        .checked_add(extra_connections)
        .ok_or_else(|| vec![definition_error("connection footprint overflows")])?;
    let node = ModelNode {
        key: DefinitionKey {
            namespace: model.namespace.clone(),
            name: model.name().to_owned(),
        },
        file: model.file,
        range: model.range(),
        local,
        local_connectors: BTreeSet::new(),
        edges,
    };
    let summary = summarize_model(
        &node,
        &summaries,
        elaborator.limits.into(),
        &mut ReachabilityMemberships::new(elaborator.limits.max_definition_reachability_pairs),
    )?;
    append_limit_diagnostics(
        &mut diagnostics,
        &node.key,
        node.file,
        node.range,
        &summary,
        false,
        elaborator.limits,
    );
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(super::super::preflight::ExpansionSize {
        declarations: summary.declarations(),
        connections: summary.connections(),
    })
}

struct Selected<'a, 'd, 'c> {
    elaborator: &'a Elaborator<'d>,
    checked: &'a CheckedDefinitionGraph,
    cache: Vec<(DefinitionKey, Values, DefinitionSummary)>,
    indexed_subtrees: BTreeMap<DefinitionKey, (bool, bool)>,
    contexts: Option<&'c mut ComponentContexts>,
    context_count: usize,
}
impl Selected<'_, '_, '_> {
    fn structural_profile(
        &mut self,
        component: &ComponentDefinition<'_>,
    ) -> Result<(bool, bool), Vec<Diagnostic>> {
        let key = DefinitionKey {
            namespace: component.namespace.clone(),
            name: component.name().to_owned(),
        };
        if let Some(found) = self.indexed_subtrees.get(&key) {
            return Ok(*found);
        }
        let mut found = (false, false);
        for item in component.owned_items() {
            match item {
                ComponentItem::IndexSet(set) => {
                    found.0 = true;
                    let closed = match set.value().kind() {
                        ExprKind::Call { callee, arguments } if callee.as_str() == "range" => {
                            match arguments.as_slice() {
                                [extent] => matches!(
                                    parameters::structural_extent(
                                        component.file,
                                        extent,
                                        &SymbolicParameterMap::new(),
                                    ),
                                    Ok(Some(_))
                                ),
                                _ => false,
                            }
                        }
                        _ => false,
                    };
                    found.1 |= !closed;
                }
                ComponentItem::Instance(instance) => {
                    let child = self
                        .elaborator
                        .resolve_component(
                            &component.namespace,
                            instance.definition(),
                            component.file,
                            instance.range(),
                        )
                        .map_err(|e| vec![e])?;
                    let child_profile = self.structural_profile(&child)?;
                    found.0 |= instance.family().is_some() || child_profile.0;
                    found.1 |= instance.family().is_some() || child_profile.1;
                }
                _ => {}
            }
        }
        self.indexed_subtrees.insert(key, found);
        Ok(found)
    }
    fn children<'i>(
        &mut self,
        namespace: &super::super::preflight::DefinitionNamespace,
        file: &'i str,
        instances: impl IntoIterator<Item = &'i InstanceDecl>,
        sets: &[&NamedDefinitionDecl],
        values: &SymbolicParameterMap,
        depth: usize,
    ) -> Result<ChildFootprint<'i>, Vec<Diagnostic>> {
        let mut edges = Vec::new();
        let mut summaries = Vec::new();
        let mut extra_connections = 0usize;
        for instance in instances {
            let child = self
                .elaborator
                .resolve_component(namespace, instance.definition(), file, instance.range())
                .map_err(|e| vec![e])?;
            let key = DefinitionKey {
                namespace: child.namespace.clone(),
                name: child.name().to_owned(),
            };
            let (multiplicity, summary) = if let Some(family) = instance.family() {
                let set = sets
                    .iter()
                    .find(|set| set.name() == family.set().as_str())
                    .ok_or_else(|| {
                        vec![source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            family.range(),
                            "indexed instance requires an enclosing IndexSet",
                        )]
                    })?;
                let ExprKind::Call { callee, arguments } = set.value().kind() else {
                    return Err(vec![definition_error("index set requires range(extent)")]);
                };
                let [extent] = arguments.as_slice() else {
                    return Err(vec![definition_error("range requires one extent")]);
                };
                if callee.as_str() != "range" {
                    return Err(vec![definition_error("index set requires range(extent)")]);
                }
                let resolved_extent =
                    parameters::structural_extent(file, extent, values).map_err(|e| vec![e])?;
                if resolved_extent.is_none() && self.contexts.is_some() {
                    // An unbound family has no concrete member context to collect.
                    // Reuse only its checked generic footprint; selected materialization
                    // below still requires the exact extent.
                    edges.push(Edge {
                        target: summaries.len(),
                        multiplicity: 1,
                        occurrence: instance.name().to_owned(),
                        file,
                        range: instance.range(),
                    });
                    summaries.push(Some(
                        self.checked
                            .component_summary(&key)
                            .expect("validated definition graph")
                            .clone(),
                    ));
                    continue;
                }
                let extent = resolved_extent
                    .ok_or_else(|| {
                        vec![source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            file,
                            family.range(),
                            "selected indexed extent remains unresolved",
                        )]
                    })?
                    .0 as usize;
                if self.structural_profile(&child)?.1 {
                    return Err(vec![source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        family.range(),
                        "parameter-dependent child IndexSets and nested indexed families are outside this bounded expansion profile",
                    )]);
                }
                (
                    extent,
                    self.checked
                        .component_summary(&key)
                        .expect("validated definition graph")
                        .clone(),
                )
            } else if !self.structural_profile(&child)?.0 {
                (
                    1,
                    self.checked
                        .component_summary(&key)
                        .expect("validated definition graph")
                        .clone(),
                )
            } else {
                let child_values = parameters::resolve_instance_parameters_symbolically(
                    child.file,
                    file,
                    child.declaration,
                    instance,
                    values,
                    &mut |name| clocks::component(child.file, child.declaration, name),
                    &mut |_| None,
                )?;
                (1, self.component(&child, child_values, depth + 1)?)
            };
            // Reject multiplication before reserving or iterating any member.
            if summary
                .instances
                .multiply(multiplicity, self.elaborator.limits.max_instances)
                .exceeds(self.elaborator.limits.max_instances)
            {
                return Err(vec![source_error(
                    codes::LANGUAGE_LOWERING_ERROR,
                    file,
                    instance.range(),
                    "indexed footprint exceeds the Component instances limit",
                )]);
            }
            if multiplicity > 1 {
                let mut diagnostics = Vec::new();
                let inputs = footprint::input_binding_count(
                    self.elaborator,
                    namespace,
                    file,
                    instance,
                    &mut diagnostics,
                );
                if !diagnostics.is_empty() {
                    return Err(diagnostics);
                }
                extra_connections = inputs
                    .checked_mul(multiplicity - 1)
                    .and_then(|n| extra_connections.checked_add(n))
                    .filter(|n| *n <= self.elaborator.limits.max_connections)
                    .ok_or_else(|| {
                        vec![definition_error(
                            "indexed Input connection footprint exceeds limit",
                        )]
                    })?;
            }
            edges.push(Edge {
                target: summaries.len(),
                multiplicity,
                occurrence: instance.name().to_owned(),
                file,
                range: instance.range(),
            });
            summaries.push(Some(summary));
        }
        Ok(ChildFootprint {
            edges,
            summaries,
            extra_connections,
        })
    }
    fn component(
        &mut self,
        component: &ComponentDefinition<'_>,
        mut values: SymbolicParameterMap,
        depth: usize,
    ) -> Result<DefinitionSummary, Vec<Diagnostic>> {
        if depth > self.elaborator.limits.max_instance_depth {
            return Err(vec![definition_error(
                "selected instance depth exceeds limit",
            )]);
        }
        let key = DefinitionKey {
            namespace: component.namespace.clone(),
            name: component.name().to_owned(),
        };
        let cache_values = values
            .iter()
            .map(|(name, value)| (name.clone(), value.value.clone()))
            .collect::<Values>();
        if let Some((_, _, summary)) = self
            .cache
            .iter()
            .find(|(definition, inputs, _)| definition == &key && inputs == &cache_values)
        {
            return Ok(summary.clone());
        }
        parameters::resolve_component_lets(
            component.file,
            component.declaration,
            &mut values,
            |name| clocks::component(component.file, component.declaration, name),
        )?;
        if self.contexts.is_some()
            && unresolved_extents(
                component.file,
                component.owned_items().filter_map(|item| match item {
                    ComponentItem::IndexSet(set) => Some(set),
                    _ => None,
                }),
                &values,
            )?
        {
            // Keep the unresolved definition on the ordinary symbolic checking path.
            return Ok(self
                .checked
                .component_summary(&key)
                .expect("validated definition graph")
                .clone());
        }
        if let Some(contexts) = self.contexts.as_mut() {
            if self.context_count >= self.elaborator.limits.max_definition_reachability_pairs {
                return Err(vec![definition_error(
                    "concrete Component contexts exceed the definition reachability limit",
                )]);
            }
            self.context_count += 1;
            contexts
                .entry(key.clone())
                .or_default()
                .push(values.clone());
        }
        let mut diagnostics = Vec::new();
        let (mut local, local_connectors) =
            component_local_footprint(self.elaborator, component, &mut diagnostics, Some(&values));
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        let sets = component
            .owned_items()
            .filter_map(|item| match item {
                ComponentItem::IndexSet(value) => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        let instances = component.owned_items().filter_map(|item| match item {
            ComponentItem::Instance(value) => Some(value),
            _ => None,
        });
        let ChildFootprint {
            edges,
            summaries,
            extra_connections,
        } = self.children(
            &component.namespace,
            component.file,
            instances,
            &sets,
            &values,
            depth,
        )?;
        local.connections = local
            .connections
            .checked_add(extra_connections)
            .ok_or_else(|| vec![definition_error("connection footprint overflows")])?;
        let node = ComponentNode {
            key: key.clone(),
            file: component.file,
            range: component.range(),
            local,
            local_connectors,
            edges,
        };
        let summary = summarize_component(
            &node,
            &summaries,
            self.elaborator.limits.into(),
            &mut ReachabilityMemberships::new(
                self.elaborator.limits.max_definition_reachability_pairs,
            ),
        )?;
        append_limit_diagnostics(
            &mut diagnostics,
            &key,
            node.file,
            node.range,
            &summary,
            false,
            self.elaborator.limits,
        );
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        self.cache.push((key, cache_values, summary.clone()));
        Ok(summary)
    }
}

fn unresolved_extents<'a>(
    file: &str,
    sets: impl IntoIterator<Item = &'a NamedDefinitionDecl>,
    values: &SymbolicParameterMap,
) -> Result<bool, Vec<Diagnostic>> {
    for set in sets {
        let ExprKind::Call { arguments, .. } = set.value().kind() else {
            return Err(vec![definition_error("index set requires range(extent)")]);
        };
        let [extent] = arguments.as_slice() else {
            return Err(vec![definition_error("range requires one extent")]);
        };
        if parameters::structural_extent(file, extent, values)
            .map_err(|error| vec![error])?
            .is_none()
        {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn size(
        source: &str,
        limits: HierarchyLimits,
    ) -> Result<super::super::super::preflight::ExpansionSize, Vec<Diagnostic>> {
        let document = eqiora_lang::parse("indexed-budget.eqi", source)
            .into_compilation_document()
            .unwrap();
        let identity =
            crate::source_identity::LocalSourceIdentity::from_document(&document).unwrap();
        let elaborator = Elaborator::new(
            "indexed-budget.eqi",
            source.len(),
            &document,
            identity,
            limits,
        )?;
        let checked = validate(&elaborator)?;
        let model = elaborator.find_entry_model("M").unwrap().unwrap();
        selected_expansion_size(&elaborator, &checked, &model)
    }
    #[test]
    fn actual_extent_multiplies_the_existing_child_footprint() {
        let source = "component Cell(parameter value:1) {} model M(parameter n:integer=3) { indexset Stages=range(n); instance cell[i in Stages]:Cell(value=to_real(ordinal(i))); }";
        let admitted = size(source, HierarchyLimits::default()).unwrap();
        assert_eq!(admitted.declarations, 5); // n + IndexSet + three Cell Parameters.
        assert_eq!(admitted.connections, 0);
        for limits in [
            HierarchyLimits {
                max_instances: 2,
                ..Default::default()
            },
            HierarchyLimits {
                max_declarations: 4,
                ..Default::default()
            },
            HierarchyLimits {
                identity: crate::identity::ElaborationIdentityLimits {
                    max_staged_identities: 5,
                    ..Default::default()
                },
                ..Default::default()
            },
            HierarchyLimits {
                provenance: crate::provenance::ProvenanceLimits {
                    max_entries: 5,
                    ..Default::default()
                },
                ..Default::default()
            },
        ] {
            assert!(size(source, limits).is_err());
        }
    }
    #[test]
    fn sibling_products_are_added_before_occurrence_allocation() {
        let source = "component Cell(parameter value:integer) {} model M() { indexset S=range(3); instance a[i in S]:Cell(value=ordinal(i)); instance b[j in S]:Cell(value=ordinal(j)); }";
        let errors = size(
            source,
            HierarchyLimits {
                max_instances: 4,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("Component instances")),
            "{errors:?}"
        );
    }
    #[test]
    fn closed_child_index_sets_have_a_fixed_per_member_footprint() {
        let source = "component Cell(parameter value:integer) { indexset Local=range(2); } model M() { indexset S=range(3); instance cell[i in S]:Cell(value=ordinal(i)); }";
        let admitted = size(source, HierarchyLimits::default()).unwrap();
        // Root IndexSet plus three copies of a Parameter and a local IndexSet.
        assert_eq!(admitted.declarations, 7);
        assert!(
            size(
                source,
                HierarchyLimits {
                    max_declarations: 6,
                    ..Default::default()
                }
            )
            .is_err()
        );
        let dependent = "component Cell(parameter value:integer) { indexset Local=range(value); } model M() { indexset S=range(3); instance cell[i in S]:Cell(value=ordinal(i)+1); }";
        let errors = size(dependent, HierarchyLimits::default()).unwrap_err();
        assert!(
            errors.iter().any(|error| error
                .message()
                .contains("parameter-dependent child IndexSets")),
            "{errors:?}"
        );
    }
    #[test]
    fn nested_structural_profile_rejects_without_sampling_an_ordinal() {
        let source = "component Cell(parameter value:integer) {} component Row(parameter n:integer) { indexset Columns=range(n); instance cell[j in Columns]:Cell(value=ordinal(j)); } model M() { indexset Rows=range(2); instance row[i in Rows]:Row(n=ordinal(i)+1); }";
        let errors = size(source, HierarchyLimits::default()).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| error.message().contains("nested indexed families")),
            "{errors:?}"
        );
    }
}
