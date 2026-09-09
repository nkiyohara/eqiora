//! Bounded structural validation for all shared expression syntax.

use super::{
    AstConstructionError, Expr, ExprKind, checked_range, validate_boundary_port_selector,
    validate_identifier, validate_name_path,
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
        ExprKind::Number(_) | ExprKind::Boolean(_) => Ok(()),
        ExprKind::Member { value, member } => {
            if !matches!(
                value.kind(),
                ExprKind::Index { .. } | ExprKind::Member { .. }
            ) {
                return Err(AstConstructionError::new(
                    "member access requires an indexed component occurrence",
                ));
            }
            validate_identifier(member, "indexed occurrence member")?;
            validate_expression_depth(value, depth + 1)
        }
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
        ExprKind::Slice {
            value,
            lower,
            upper,
        } => {
            validate_expression_depth(value, depth + 1)?;
            validate_expression_depth(lower, depth + 1)?;
            validate_expression_depth(upper, depth + 1)
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
        ExprKind::Case { value, arms } => {
            validate_expression_depth(value, depth + 1)?;
            super::enumeration::validate_arms(arms)?;
            for arm in arms {
                validate_expression_depth(arm.value(), depth + 1)?;
            }
            Ok(())
        }
        ExprKind::Select {
            condition,
            then_value,
            else_value,
        } => {
            validate_expression_depth(condition, depth + 1)?;
            validate_expression_depth(then_value, depth + 1)?;
            validate_expression_depth(else_value, depth + 1)
        }
        ExprKind::Reduction { binder, value, .. } => {
            super::validate_identifier(binder.member(), "reduction member")?;
            validate_name_path(binder.set())?;
            super::checked_range(binder.range())?;
            validate_expression_depth(value, depth + 1)
        }
        ExprKind::Call { callee, arguments } => {
            validate_name_path(callee)?;
            if matches!(callee.as_str(), "sum" | "product") {
                return Err(AstConstructionError::new(
                    "sum/product require a structured reduction binder",
                ));
            }
            if arguments.expressions().len() == 0 && callee.as_str() != "boundaries" {
                return Err(AstConstructionError::new(
                    "an expression operator call requires at least one argument",
                ));
            }
            if let Some(bindings) = arguments.named() {
                let mut names = std::collections::HashSet::new();
                for binding in bindings {
                    validate_identifier(binding.name(), "argument name")?;
                    checked_range(binding.range())?;
                    if !names.insert(binding.name()) {
                        return Err(AstConstructionError::new("duplicate named argument"));
                    }
                }
            }
            if callee.as_str() == "tensor_value"
                && !arguments.named().is_some_and(|bindings| {
                    bindings.len() == 2
                        && bindings
                            .iter()
                            .any(|binding| binding.name() == "components")
                        && bindings.iter().any(|binding| {
                            binding.name() == "frame"
                                && matches!(
                                    binding.value().kind(),
                                    ExprKind::Name(_) | ExprKind::Path(_)
                                )
                        })
                })
            {
                return Err(AstConstructionError::new(
                    "tensor_value requires a frame name and components",
                ));
            }
            for argument in arguments.expressions() {
                validate_expression_depth(argument, depth + 1)?;
            }
            Ok(())
        }
    }
}

pub(super) fn validate_endpoint(expression: &Expr) -> Result<(), AstConstructionError> {
    validate_expression(expression)?;
    match expression.kind() {
        ExprKind::Name(_) | ExprKind::Path(_) | ExprKind::Member { .. } => Ok(()),
        _ => Err(AstConstructionError::new(
            "Connection endpoint requires an exact declared Port selection",
        )),
    }
}

impl super::SourceAstFactory {
    /// Construct a bounded scalar reduction with an explicit lexical binder.
    ///
    /// # Errors
    /// Rejects malformed binder names/ranges or excessive expression depth.
    /// The compiler checks index-set identity, cardinality, and scalar domain.
    pub fn reduction(
        operation: crate::ReductionOp,
        binder: crate::FamilyBinderSyntax,
        value: Expr,
        range: crate::TextRange,
    ) -> Result<Expr, AstConstructionError> {
        Self::expression(
            ExprKind::Reduction {
                operation,
                binder,
                value: Box::new(value),
            },
            range,
        )
    }
}
