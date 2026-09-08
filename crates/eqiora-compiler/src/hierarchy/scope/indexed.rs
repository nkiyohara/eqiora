//! Nominal index-set lookup and immutable per-occurrence binder values.
use super::*;
use crate::hierarchy::hierarchy_error;
use crate::hierarchy::parameters::{ParameterLineage, SymbolicParameterValue};
use eqiora_schema::kernel::IndexSetDef;

impl Scope {
    pub(in crate::hierarchy) fn index_set(&self, name: &str) -> Option<&IndexSetDef> {
        self.index_sets.get(name)
    }

    pub(in crate::hierarchy) fn insert_index_set(
        &mut self,
        name: String,
        definition: IndexSetDef,
    ) -> Result<(), Diagnostic> {
        if self.index_sets.insert(name, definition).is_some() {
            return Err(hierarchy_error(
                "index set name is declared more than once in this occurrence",
            ));
        }
        Ok(())
    }

    pub(in crate::hierarchy) fn with_index_member(
        &self,
        name: &str,
        definition: &IndexSetDef,
        ordinal: u32,
    ) -> Result<Self, Diagnostic> {
        let value =
            eqiora_core::ValueLiteral::from_integer(definition.value_type(), i64::from(ordinal))
                .map_err(|error| hierarchy_error(error.to_string()))?;
        let mut scope = self.clone();
        scope
            .insert_let(
                name.to_owned(),
                SymbolicParameterValue {
                    value_type: value.value_type().clone(),
                    expression: Some(LoweringExpression::literal(
                        value.clone(),
                        TextRange::new(0, 0),
                    )),
                    value: Some(value),
                    lineage: Some(ParameterLineage::Constant),
                },
            )
            .map_err(hierarchy_error)?;
        Ok(scope)
    }
}
