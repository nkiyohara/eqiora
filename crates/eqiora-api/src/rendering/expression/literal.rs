use super::{Context, Math, MathReference, failure};
use eqiora_core::{Diagnostic, ValueLiteral};
use eqiora_schema::kernel::KernelNode;

impl Context<'_> {
    pub(super) fn literal(&mut self, value: &ValueLiteral) -> Result<Math, Diagnostic> {
        let value_type = value.value_type();
        if let Some(boolean) = value.as_bool() {
            return Ok(Math::Function(
                if boolean { "true" } else { "false" }.into(),
                vec![],
            ));
        }
        if let (Some(id), Some(tag)) = (value_type.enum_definition(), value.enum_tag()) {
            let Some(KernelNode::Enum(definition)) = self.document.program().node(id.into()) else {
                return Err(failure("enum literal has no exact admitted declaration"));
            };
            let member = definition
                .members()
                .get(tag as usize)
                .ok_or_else(|| failure("enum literal has no exact admitted member"))?;
            if member.len() > super::super::MAX_OUTPUT / 2 {
                return Err(failure("enum member exceeds bounded presentation size"));
            }
            let label = format!("enum {id} member {member}");
            self.reference(MathReference {
                graph_id: Some(id.into()),
                role: None,
                declarations: vec![],
                operator: None,
            });
            return Ok(Math::Function(label, vec![]));
        }
        let count = value.component_count();
        if value_type.shape().rank() > super::super::MAX_NODES {
            return Err(failure("literal type exceeds bounded presentation size"));
        }
        // Complex components have two literal children; charge that cost before
        // iterating compact shaped zeros or allocating any presentation storage.
        let cost = count.saturating_mul(3);
        if cost > self.remaining {
            return Err(failure("literal exceeds bounded presentation size"));
        }
        self.remaining -= cost;
        let mut components = if let Some(values) = value.integer_components() {
            values
                .map(|value| Math::Number(value.to_string()))
                .collect::<Vec<_>>()
        } else if let Some(values) = value.components() {
            values
                .map(|(real, imaginary)| {
                    if imaginary == 0.0 {
                        Math::Number(real.to_string())
                    } else {
                        Math::Function(
                            "complex".into(),
                            vec![
                                Math::Number(real.to_string()),
                                Math::Number(imaginary.to_string()),
                            ],
                        )
                    }
                })
                .collect::<Vec<_>>()
        } else {
            return Err(failure("unsupported admitted literal payload"));
        };
        let result = if value_type.shape().is_scalar() {
            components.remove(0)
        } else {
            Math::Array(components)
        };
        if value_type.shape().is_scalar()
            && value_type.index_set().is_none()
            && value_type.finite_space().is_none()
            && value_type.dimension() == eqiora_core::DimExponents::DIMENSIONLESS
        {
            Ok(result)
        } else {
            for id in [
                value_type.index_set().map(Into::into),
                value_type.finite_space().map(Into::into),
            ]
            .into_iter()
            .flatten()
            {
                self.reference(MathReference {
                    graph_id: Some(id),
                    role: None,
                    declarations: vec![],
                    operator: None,
                });
            }
            Ok(Math::Typed(Box::new(result), value_type.clone()))
        }
    }
}
