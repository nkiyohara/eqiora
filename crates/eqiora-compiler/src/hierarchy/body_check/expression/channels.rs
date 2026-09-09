//! Shared scalar-domain and support checks for ordered channel operations.
use super::*;

impl ExpressionChecker<'_, '_, '_> {
    pub(super) fn channels(
        &mut self,
        expression: &Expr,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        match expression.kind() {
            ExprKind::Array(elements) => {
                let mut types = elements
                    .iter()
                    .map(|element| self.check(element))
                    .collect::<Result<Vec<_>, _>>()?;
                if types.iter().any(|value| {
                    value.value_type.scalar_domain() == eqiora_core::ScalarDomain::Integer
                }) {
                    for (element, element_type) in elements.iter().zip(&mut types) {
                        if element_type.value_type.scalar_domain()
                            != eqiora_core::ScalarDomain::Integer
                        {
                            *element_type = self.check_numeric_context(
                                element,
                                eqiora_core::ScalarDomain::Integer,
                            )?;
                        }
                    }
                }
                let inferred = ExpressionType::array(&types)
                    .map_err(|error| type_error(self.scope.file, expression, error))?;
                crate::typed_values::check_type(&inferred.value_type).map_err(|message| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        expression.range(),
                        message,
                    )
                })?;
                Ok(inferred)
            }
            ExprKind::Index { value, index } => {
                let index = crate::hierarchy::parameters::static_index(
                    self.scope.file,
                    index,
                    &self.scope.static_values,
                )?;
                ExpressionType::index(self.check(value)?, index)
                    .map_err(|error| type_error(self.scope.file, expression, error))
            }
            ExprKind::Slice {
                value,
                lower,
                upper,
            } => {
                let (start, end) = crate::hierarchy::parameters::static_slice(
                    self.scope.file,
                    lower,
                    upper,
                    &self.scope.static_values,
                )?;
                let value = self.check(value)?;
                ExpressionType::index(value.clone(), end - 1)
                    .map_err(|error| type_error(self.scope.file, expression, error))?;
                let element = ExpressionType::index(value, start)
                    .map_err(|error| type_error(self.scope.file, expression, error))?;
                let value_type = element.value_type.array(end - start).map_err(|error| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        expression.range(),
                        error.to_string(),
                    )
                })?;
                Ok(ExpressionType::new(value_type, element.support))
            }
            _ => unreachable!("channel expression dispatch"),
        }
    }
}
