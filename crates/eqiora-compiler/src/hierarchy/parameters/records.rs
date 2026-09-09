//! Exact nominal context alongside the ordinary scalar Parameter dependency graph.
use super::*;
use crate::hierarchy::preflight::{ComponentDefinition, Elaborator, ModelDefinition};
use crate::record::BoundRecord;
use eqiora_core::{Id, entity::kinds};
use eqiora_lang::{Item, SignatureItem, ValueTypeSyntax, ValueTypeSyntaxKind};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Default)]
pub(in crate::hierarchy) struct RecordContext {
    pub(in crate::hierarchy) visible: BTreeMap<String, BoundRecord>,
    pub(in crate::hierarchy) parameters: BTreeMap<String, Id<kinds::Record>>,
}

impl RecordContext {
    pub(in crate::hierarchy) fn component(
        elaborator: &Elaborator<'_>,
        definition: &ComponentDefinition<'_>,
    ) -> Self {
        let mut result = Self {
            visible: elaborator
                .visible_records(&definition.namespace)
                .into_iter()
                .map(|(name, value)| (name, value.clone()))
                .collect(),
            parameters: BTreeMap::new(),
        };
        for declaration in parameter_declarations(definition.declaration).values() {
            result.insert(declaration.name(), declaration.value_type());
        }
        result
    }

    pub(in crate::hierarchy) fn model(
        elaborator: &Elaborator<'_>,
        definition: &ModelDefinition<'_>,
    ) -> Self {
        let mut result = Self {
            visible: elaborator
                .visible_records(&definition.namespace)
                .into_iter()
                .map(|(name, value)| (name, value.clone()))
                .collect(),
            parameters: BTreeMap::new(),
        };
        for item in definition.signature() {
            if let SignatureItem::Parameter(value) = item {
                result.insert(value.name(), value.value_type());
            }
        }
        for item in definition.declaration.items() {
            if let Item::Parameter(value) = item {
                result.insert(value.name(), value.value_type());
            }
        }
        result
    }

    pub(in crate::hierarchy) fn insert(&mut self, name: &str, syntax: &ValueTypeSyntax) {
        if let Some(record) = self.record_for_type(syntax) {
            self.parameters
                .insert(name.to_owned(), record.definition.id());
        }
    }

    pub(in crate::hierarchy) fn record_for_type(
        &self,
        syntax: &ValueTypeSyntax,
    ) -> Option<&BoundRecord> {
        let ValueTypeSyntaxKind::Named(path) = syntax.kind() else {
            return None;
        };
        self.visible.get(path.as_str())
    }

    pub(in crate::hierarchy) fn root(&self, name: &str) -> Option<&BoundRecord> {
        let id = self.parameters.get(name)?;
        self.visible
            .values()
            .find(|record| record.definition.id() == *id)
    }

    pub(in crate::hierarchy) fn records(&self) -> impl Iterator<Item = (&str, &BoundRecord)> {
        self.parameters
            .keys()
            .filter_map(|name| self.root(name).map(|record| (name.as_str(), record)))
    }
}

pub(super) fn expand(
    file: &str,
    declarations: BTreeMap<String, ComponentParameterDecl>,
    records: &RecordContext,
) -> Result<BTreeMap<String, ComponentParameterDecl>, Vec<Diagnostic>> {
    let mut result = BTreeMap::new();
    for (name, declaration) in declarations {
        let Some(record) = records.record_for_type(declaration.value_type()) else {
            result.insert(name, declaration);
            continue;
        };
        let defaults = declaration
            .default()
            .map(|expression| {
                crate::record::parameters::member_initializers(
                    file,
                    record,
                    expression,
                    |path| {
                        records
                            .visible
                            .get(path.as_str())
                            .map(|record| record.definition.id())
                    },
                    |path| records.parameters.get(path.as_str()).copied(),
                )
            })
            .transpose()
            .map_err(|error| vec![error])?;
        result.extend(
            crate::record::parameters::parameter_leaves(
                file,
                &name,
                record,
                declaration.visibility(),
                defaults.as_deref(),
                declaration.range(),
            )
            .map_err(|error| vec![error])?,
        );
    }
    Ok(result)
}
