use crate::lower::LoweringExpression;

use super::Scope;
use crate::hierarchy::parameters::{
    ParameterLineage, ResolvedParameter, SymbolicParameterMap, SymbolicParameterValue,
};

#[derive(Debug, Clone)]
pub(super) enum ScopedValue {
    Static(ResolvedParameter),
    Runtime(LoweringExpression),
}

impl Scope {
    pub(in crate::hierarchy) fn insert_parameter(
        &mut self,
        name: String,
        parameter: ResolvedParameter,
    ) -> Option<ResolvedParameter> {
        self.values
            .insert(name, ScopedValue::Static(parameter))
            .and_then(|value| match value {
                ScopedValue::Static(value) => Some(value),
                ScopedValue::Runtime(_) => None,
            })
    }

    pub(in crate::hierarchy) fn parameter(&self, name: &str) -> Option<&ResolvedParameter> {
        match self.values.get(name) {
            Some(ScopedValue::Static(value)) => Some(value),
            _ => None,
        }
    }

    pub(in crate::hierarchy) fn symbolic_parameters(&self) -> SymbolicParameterMap {
        self.values
            .iter()
            .filter_map(|(name, value)| {
                let ScopedValue::Static(value) = value else {
                    return None;
                };
                Some((
                    name.clone(),
                    SymbolicParameterValue {
                        value: Some(value.value.literal()),
                        value_type: value.value.value_type().clone(),
                        expression: Some(value.expression.clone()),
                        lineage: Some(value.lineage.clone()),
                    },
                ))
            })
            .collect()
    }

    pub(in crate::hierarchy) fn value_expression(&self, name: &str) -> Option<LoweringExpression> {
        match self.values.get(name)? {
            ScopedValue::Static(_) => Some(self.parameter_expression(name)),
            ScopedValue::Runtime(expression) => Some(expression.clone()),
        }
    }

    pub(in crate::hierarchy) fn insert_runtime_let(
        &mut self,
        name: String,
        expression: LoweringExpression,
    ) -> Result<(), &'static str> {
        if self
            .values
            .insert(name, ScopedValue::Runtime(expression))
            .is_some()
        {
            Err("runtime let alias collides with a scoped value")
        } else {
            Ok(())
        }
    }

    pub(in crate::hierarchy) fn insert_let(
        &mut self,
        name: String,
        value: SymbolicParameterValue,
    ) -> Result<(), &'static str> {
        let (Some(scalar), Some(expression)) = (value.value, value.expression) else {
            return Err("static let alias did not resolve to a closed expression");
        };
        let resolved = ResolvedParameter {
            value: eqiora_core::ValueLiteral::new(value.value_type, scalar)
                .map_err(|_| "static let alias has an invalid typed literal")?,
            expression,
            lineage: ParameterLineage::Derived,
        };
        if self
            .values
            .insert(name, ScopedValue::Static(resolved))
            .is_some()
        {
            Err("static let alias collides with a compile-time value")
        } else {
            Ok(())
        }
    }
}
