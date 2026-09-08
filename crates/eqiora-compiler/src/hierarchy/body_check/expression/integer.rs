use super::*;
use eqiora_core::{ScalarDomain, ValueType};

impl ExpressionChecker<'_, '_, '_> {
    pub(super) fn check_pair(
        &mut self,
        left: &Expr,
        right: &Expr,
    ) -> Result<(ExpressionType<String>, ExpressionType<String>), Diagnostic> {
        let mut left_type = self.check(left)?;
        let mut right_type = self.check(right)?;
        if left_type.value_type.scalar_domain() == ScalarDomain::Integer && numeric_tree(right) {
            right_type = self.check_numeric_context(right, ScalarDomain::Integer)?;
        } else if right_type.value_type.scalar_domain() == ScalarDomain::Integer
            && numeric_tree(left)
        {
            left_type = self.check_numeric_context(left, ScalarDomain::Integer)?;
        }
        Ok((left_type, right_type))
    }
    pub(super) fn check_numeric_context(
        &mut self,
        value: &Expr,
        domain: ScalarDomain,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        if domain != ScalarDomain::Integer || !numeric_tree(value) {
            return self.check(value);
        }
        match value.kind() {
            ExprKind::Case { value: selector, arms } => return self.check_case(value,selector,arms,Some(domain)),
            ExprKind::Select {condition,then_value,else_value} => {
                return self.check(condition)?.select(self.check_numeric_context(then_value,domain)?,self.check_numeric_context(else_value,domain)?).map_err(|error|type_error(self.scope.file,value,error));
            }
            ExprKind::Array(elements) => {
                let elements = elements
                    .iter()
                    .map(|element| self.check_numeric_context(element, domain))
                    .collect::<Result<Vec<_>, _>>()?;
                return ExpressionType::array(&elements)
                    .map_err(|error| type_error(self.scope.file, value, error));
            }
            ExprKind::Number(number) => {
                self.check_integer_number(value, number)?;
            }
            ExprKind::Unary {
                op: UnaryOp::Neg,
                value: operand,
            } => {
                if let ExprKind::Number(number) = operand.kind() {
                    let text = number.canonical_text();
                    let signed = if let Some(text) = text.strip_prefix('-') {
                        text.to_owned()
                    } else {
                        format!("-{text}")
                    };
                    let number = eqiora_lang::DecimalLiteral::parse(&signed).map_err(|e| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.scope.file,
                            value.range(),
                            e.to_string(),
                        )
                    })?;
                    self.check_integer_number(value, &number)?;
                } else {
                    return self.check_numeric_context(operand, domain);
                }
            }
            ExprKind::Binary { op, left, right } => {
                let left = self.check_numeric_context(left, domain)?;
                let right = self.check_numeric_context(right, domain)?;
                let checked = match op {
                    BinaryOp::Add => left.sum(right),
                    BinaryOp::Sub => typing::additive(&left, &right),
                    BinaryOp::Mul => typing::multiply(&left, &right),
                    _ => Err(TypeViolation::ScalarDomainMismatch),
                };
                return checked.map_err(|e| type_error(self.scope.file, value, e));
            }
            _ => unreachable!("numeric tree checked"),
        }
        Ok(ExpressionType::new(
            ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS).expect("admitted numeric scalar type"),
            None,
        ))
    }
    fn check_integer_number(
        &self,
        expression: &Expr,
        number: &eqiora_lang::DecimalLiteral,
    ) -> Result<(), Diagnostic> {
        number.to_i64().map(|_| ()).map_err(|e| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                e.to_string(),
            )
        })
    }
}
pub(super) fn numeric_tree(value: &Expr) -> bool {
    match value.kind() {
        ExprKind::Number(_) => true,
        ExprKind::Case {arms,..} => arms.iter().all(|arm|numeric_tree(arm.value())),
        ExprKind::Select {then_value,else_value,..} => numeric_tree(then_value) && numeric_tree(else_value),
        ExprKind::Array(elements) => elements.iter().all(numeric_tree),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => numeric_tree(value),
        ExprKind::Binary {
            op: BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Pow,
            left,
            right,
        } => numeric_tree(left) && numeric_tree(right),
        _ => false,
    }
}
