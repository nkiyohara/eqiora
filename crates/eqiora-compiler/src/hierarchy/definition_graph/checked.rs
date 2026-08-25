//! Retained summaries from one completely validated definition graph.

use std::collections::BTreeMap;

use super::{DefinitionKey, DefinitionSummary};

impl DefinitionSummary {
    pub(in crate::hierarchy) fn staged_identities(&self) -> usize {
        self.staged_identities.observed()
    }

    pub(in crate::hierarchy) fn provenance_entries(&self) -> usize {
        self.provenance_entries.observed()
    }
}

/// Compiler-owned proof that every Component reference is acyclic and every
/// reusable definition has a bounded possible occurrence footprint.
#[derive(Clone, Debug)]
pub(crate) struct CheckedDefinitionGraph {
    component_order: Vec<DefinitionKey>,
    component_summaries: BTreeMap<DefinitionKey, DefinitionSummary>,
    pub(super) model_summaries: BTreeMap<DefinitionKey, DefinitionSummary>,
}

impl CheckedDefinitionGraph {
    pub(super) fn new(
        component_order: Vec<DefinitionKey>,
        component_summaries: BTreeMap<DefinitionKey, DefinitionSummary>,
        model_summaries: BTreeMap<DefinitionKey, DefinitionSummary>,
    ) -> Self {
        Self {
            component_order,
            component_summaries,
            model_summaries,
        }
    }

    pub(in crate::hierarchy) fn component_order(&self) -> &[DefinitionKey] {
        &self.component_order
    }

    pub(in crate::hierarchy) fn model_summary(
        &self,
        key: &DefinitionKey,
    ) -> Option<&DefinitionSummary> {
        self.model_summaries.get(key)
    }

    pub(in crate::hierarchy) fn component_summary(
        &self,
        key: &DefinitionKey,
    ) -> Option<&DefinitionSummary> {
        self.component_summaries.get(key)
    }
}
