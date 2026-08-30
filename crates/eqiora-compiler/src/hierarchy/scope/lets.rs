use super::Scope;
use crate::hierarchy::parameters::{
    ConstantValue, ParameterLineage, ResolvedParameter, SymbolicParameterMap,
    SymbolicParameterValue,
};

impl Scope {
    pub(in crate::hierarchy) fn insert_parameter(
        &mut self,
        name: String,
        parameter: ResolvedParameter,
    ) -> Option<ResolvedParameter> {
        self.parameters.insert(name, parameter)
    }

    pub(in crate::hierarchy) fn parameter(&self, name: &str) -> Option<&ResolvedParameter> {
        self.parameters.get(name)
    }

    pub(in crate::hierarchy) fn symbolic_parameters(&self) -> SymbolicParameterMap {
        self.parameters
            .iter()
            .map(|(name, value)| {
                (
                    name.clone(),
                    SymbolicParameterValue {
                        value: Some(value.value.value),
                        dimension: value.value.dimension,
                        expression: Some(value.expression.clone()),
                        lineage: Some(value.lineage.clone()),
                    },
                )
            })
            .collect()
    }

    pub(in crate::hierarchy) fn insert_let(
        &mut self,
        name: String,
        value: SymbolicParameterValue,
    ) -> Result<(), &'static str> {
        let (Some(scalar), Some(expression)) = (value.value, value.expression) else {
            return Err("model let alias did not resolve to a closed expression");
        };
        let resolved = ResolvedParameter {
            value: ConstantValue {
                value: scalar,
                dimension: value.dimension,
            },
            expression,
            lineage: ParameterLineage::Derived,
        };
        if self.parameters.insert(name, resolved).is_some() {
            Err("model let alias collides with a compile-time value")
        } else {
            Ok(())
        }
    }
}
