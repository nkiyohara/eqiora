//! Bounded structural validation for all shared expression syntax.

use super::{
    AstConstructionError, Expr, ExprKind, checked_range, validate_boundary_port_selector,
    validate_finite, validate_identifier, validate_name_path,
};

pub(super) fn validate_expression(expression: &Expr) -> Result<(), AstConstructionError> {
    validate_expression_depth(expression, 1)
}

fn validate_expression_depth(expression: &Expr, depth: usize) -> Result<(), AstConstructionError> {
    if depth > 256 {
        return Err(AstConstructionError::new(
            "expression tree exceeds the 256-level limit",
        ));
    }
    checked_range(expression.range())?;
    match expression.kind() {
        ExprKind::Number(value) => validate_finite(*value, "expression literal"),
        ExprKind::Quantity { unit, .. } => validate_expression_depth(unit, depth + 1),
        ExprKind::Name(name) => validate_identifier(name, "expression name"),
        ExprKind::Path(path) => validate_name_path(path),
        ExprKind::BoundaryPortSelection { port, selector } => {
            validate_name_path(port)?;
            validate_boundary_port_selector(selector)
        }
        ExprKind::Unary { value, .. } => validate_expression_depth(value, depth + 1),
        ExprKind::Binary { left, right, .. } => {
            validate_expression_depth(left, depth + 1)?;
            validate_expression_depth(right, depth + 1)
        }
        ExprKind::Index { value, index } => {
            validate_expression_depth(value, depth + 1)?;
            validate_expression_depth(index, depth + 1)
        }
        ExprKind::Array(elements) => {
            if elements.is_empty() {
                return Err(AstConstructionError::new(
                    "array literal requires at least one element",
                ));
            }
            for element in elements {
                validate_expression_depth(element, depth + 1)?;
            }
            Ok(())
        }
        ExprKind::Call { callee, arguments } => {
            validate_name_path(callee)?;
            if arguments.is_empty() && callee.to_string() != "boundaries" {
                return Err(AstConstructionError::new(
                    "an expression operator call requires at least one argument",
                ));
            }
            for argument in arguments {
                validate_expression_depth(argument, depth + 1)?;
            }
            Ok(())
        }
    }
}
