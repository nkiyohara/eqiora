//! Preserve scientific release identity until an exact expression occurrence is built.
use super::*;
use eqiora_schema::kernel::{PropertyMeaning, PropertyRelease};

impl LoweringExpression {
    pub(crate) fn property(
        release: Arc<PropertyRelease>,
        arguments: Vec<Self>,
        range: TextRange,
    ) -> Self {
        Self {
            node: Arc::new(LoweringExpressionNode::Property { release, arguments }),
            range,
            structural_parameters: None,
        }
    }
}

pub(super) fn infer<I: Clone + Eq>(
    release: &PropertyRelease,
    arguments: &[ExpressionType<I>],
) -> Result<ExpressionType<I>, &'static str> {
    match release.meaning() {
        PropertyMeaning::Constant(value) => {
            if !arguments.is_empty() {
                return Err("constant property release takes no arguments");
            }
            Ok(ExpressionType::new(value.value_type().clone(), None))
        }
        meaning @ (PropertyMeaning::Analytic(_) | PropertyMeaning::Table(_)) => meaning
            .definition()
            .expect("nonconstant property definition")
            .instantiate(arguments)
            .map(|value| value.result_type().clone())
            .map_err(
                |_| "property application violates its exact input types or common volume support",
            ),
    }
}

impl ExpressionLowerer<'_> {
    pub(super) fn lower_property(
        &mut self,
        expression: &LoweringExpression,
        release: &Arc<PropertyRelease>,
        arguments: &[LoweringExpression],
    ) -> Result<TypedExpression, Diagnostic> {
        let arguments = arguments
            .iter()
            .map(|argument| self.lower(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let value_type = release.meaning().value_type().ok_or_else(|| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                "property release has no exact result type",
            )
        })?;
        let id = self
            .builder
            .property(
                (**release).clone(),
                arguments.iter().map(|argument| argument.id),
            )
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))?;
        Ok(TypedExpression {
            id,
            dimension: value_type.dimension(),
        })
    }
}
