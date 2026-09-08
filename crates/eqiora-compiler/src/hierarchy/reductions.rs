//! Bounded source expansion for ordered finite scalar reductions.
use eqiora_core::{Diagnostic, diagnostic::codes};
use eqiora_lang::{DecimalLiteral, Expr, ExprKind, FamilyBinderSyntax, NamePath, SourceAstFactory};

use crate::{diagnostics::source_error, source_identity::LocalSourceIdentityLimits};

fn error(file: &str, expression: &Expr, message: impl Into<String>) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        expression.range(),
        message,
    )
}

pub(super) fn preflight(
    file: &str,
    expression: &Expr,
    resolve_extent: &mut dyn FnMut(&str) -> Option<u32>,
    max_terms: usize,
) -> Result<(), Diagnostic> {
    expanded_nodes(file, expression, resolve_extent, max_terms).map(|_| ())
}

pub(super) fn expanded_nodes(
    file: &str,
    expression: &Expr,
    resolve_extent: &mut dyn FnMut(&str) -> Option<u32>,
    max_terms: usize,
) -> Result<usize, Diagnostic> {
    let limits = LocalSourceIdentityLimits::default();
    let (nodes, _) = measure(
        file,
        expression,
        resolve_extent,
        max_terms.min(limits.max_expression_nodes),
        1,
        &mut Vec::new(),
    )?;
    Ok(nodes)
}

fn measure<'a>(
    file: &str,
    expression: &'a Expr,
    resolve_extent: &mut dyn FnMut(&str) -> Option<u32>,
    budget: usize,
    source_depth: usize,
    binders: &mut Vec<&'a str>,
) -> Result<(usize, usize), Diagnostic> {
    let depth_limit = LocalSourceIdentityLimits::default().max_expression_depth;
    let exceeded = || {
        error(
            file,
            expression,
            "reduction expansion exceeds the expression work or depth limit",
        )
    };
    if source_depth > depth_limit {
        return Err(exceeded());
    }
    let result = if let ExprKind::Reduction { binder, value, .. } = expression.kind() {
        if binders.contains(&binder.member()) {
            return Err(error(
                file,
                expression,
                "nested reduction cannot shadow an active member binder",
            ));
        }
        let extent = resolve_extent(binder.set().as_str())
            .filter(|n| *n > 0)
            .ok_or_else(|| {
                error(
                    file,
                    expression,
                    "reduction requires an existing nonempty bounded IndexSet",
                )
            })? as usize;
        binders.push(binder.member());
        let (nodes, depth) = measure(
            file,
            value,
            resolve_extent,
            budget,
            source_depth + 1,
            binders,
        )?;
        binders.pop();
        (
            nodes
                .checked_mul(extent)
                .and_then(|n| n.checked_add(extent - 1))
                .ok_or_else(exceeded)?,
            depth.checked_add(extent - 1).ok_or_else(exceeded)?,
        )
    } else {
        let mut nodes = 1usize;
        let mut depth = 1usize;
        visit_children(expression, &mut |child| {
            let (child_nodes, child_depth) = measure(
                file,
                child,
                resolve_extent,
                budget,
                source_depth + 1,
                binders,
            )?;
            nodes = nodes.checked_add(child_nodes).ok_or_else(exceeded)?;
            depth = depth.max(child_depth + 1);
            if nodes > budget {
                return Err(exceeded());
            }
            Ok(())
        })?;
        (nodes, depth)
    };
    if result.0 > budget || result.1 > depth_limit {
        return Err(exceeded());
    }
    Ok(result)
}

pub(super) fn contains(expression: &Expr) -> bool {
    let mut pending = vec![expression];
    while let Some(value) = pending.pop() {
        if matches!(value.kind(), ExprKind::Reduction { .. }) {
            return true;
        }
        visit_children(value, &mut |child| {
            pending.push(child);
            Ok(())
        })
        .expect("collecting children cannot fail");
    }
    false
}

fn visit_children<'a>(
    expression: &'a Expr,
    visit: &mut impl FnMut(&'a Expr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    match expression.kind() {
        ExprKind::Select {
            condition,
            then_value,
            else_value,
        } => {
            visit(condition)?;
            visit(then_value)?;
            visit(else_value)?;
        }
        ExprKind::Quantity { unit, .. } => visit(unit)?,
        ExprKind::Unary { value, .. }
        | ExprKind::Member { value, .. }
        | ExprKind::Reduction { value, .. } => visit(value)?,
        ExprKind::Binary { left, right, .. } => {
            visit(left)?;
            visit(right)?;
        }
        ExprKind::Index { value, index } => {
            visit(value)?;
            visit(index)?;
        }
        ExprKind::Call { arguments, .. } => {
            for argument in arguments.expressions() {
                visit(argument)?;
            }
        }
        ExprKind::Array(elements) => {
            for element in elements {
                visit(element)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn instantiate(
    file: &str,
    value: &Expr,
    binder: &FamilyBinderSyntax,
    ordinal: u32,
) -> Result<Expr, Diagnostic> {
    substitute(file, value, binder.member(), ordinal, 1)
}

fn substitute(
    file: &str,
    expression: &Expr,
    member: &str,
    ordinal: u32,
    depth: usize,
) -> Result<Expr, Diagnostic> {
    if depth > LocalSourceIdentityLimits::default().max_expression_depth {
        return Err(error(
            file,
            expression,
            "reduction member expression exceeds the depth limit",
        ));
    }
    let construct = |kind| {
        SourceAstFactory::expression(kind, expression.range())
            .map_err(|failure| error(file, expression, failure.to_string()))
    };
    if let ExprKind::Call { callee, arguments } = expression.kind()
        && callee.as_str() == "ordinal"
        && matches!(arguments.positional(), Some([argument]) if matches!(argument.kind(), ExprKind::Name(name) if name == member))
    {
        let number = construct(ExprKind::Number(
            DecimalLiteral::parse(&ordinal.to_string())
                .map_err(|failure| error(file, expression, failure.to_string()))?,
        ))?;
        return construct(ExprKind::Call {
            callee: NamePath::from_segments(["to_integer"], expression.range())
                .map_err(|failure| error(file, expression, failure.to_string()))?,
            arguments: eqiora_lang::CallArguments::Positional(vec![number]),
        });
    }
    let mut child = |value: &Expr| substitute(file, value, member, ordinal, depth + 1);
    let kind = match expression.kind() {
        ExprKind::Name(name) if name == member => {
            return Err(error(
                file,
                expression,
                "reduction members require explicit ordinal(member) projection",
            ));
        }
        ExprKind::Reduction {
            operation,
            binder,
            value,
        } => {
            if binder.member() == member {
                return Err(error(
                    file,
                    expression,
                    "nested reduction cannot shadow an active member binder",
                ));
            }
            ExprKind::Reduction {
                operation: *operation,
                binder: binder.clone(),
                value: Box::new(child(value)?),
            }
        }
        ExprKind::Select {
            condition,
            then_value,
            else_value,
        } => ExprKind::Select {
            condition: Box::new(child(condition)?),
            then_value: Box::new(child(then_value)?),
            else_value: Box::new(child(else_value)?),
        },
        ExprKind::Quantity { value, unit } => ExprKind::Quantity {
            value: value.clone(),
            unit: Box::new(child(unit)?),
        },
        ExprKind::Unary { op, value } => ExprKind::Unary {
            op: *op,
            value: Box::new(child(value)?),
        },
        ExprKind::Binary { op, left, right } => ExprKind::Binary {
            op: *op,
            left: Box::new(child(left)?),
            right: Box::new(child(right)?),
        },
        ExprKind::Array(elements) => {
            ExprKind::Array(elements.iter().map(&mut child).collect::<Result<_, _>>()?)
        }
        ExprKind::Index { value, index } => ExprKind::Index {
            value: Box::new(child(value)?),
            index: Box::new(child(index)?),
        },
        ExprKind::Member { value, member } => ExprKind::Member {
            value: Box::new(child(value)?),
            member: member.clone(),
        },
        ExprKind::Call { callee, arguments } => ExprKind::Call {
            callee: callee.clone(),
            arguments: match arguments {
                eqiora_lang::CallArguments::Positional(values) => {
                    eqiora_lang::CallArguments::Positional(
                        values.iter().map(&mut child).collect::<Result<_, _>>()?,
                    )
                }
                eqiora_lang::CallArguments::Named(values) => eqiora_lang::CallArguments::Named(
                    values
                        .iter()
                        .map(|binding| {
                            SourceAstFactory::named_binding(
                                binding.name(),
                                child(binding.value())?,
                                binding.range(),
                            )
                            .map_err(|failure| error(file, expression, failure.to_string()))
                        })
                        .collect::<Result<_, _>>()?,
                ),
            },
        },
        _ => return Ok(expression.clone()),
    };
    let mut result = construct(kind)?;
    if let Some(value_type) = expression.resolved_nominal() {
        let ExprKind::Call {
            arguments: eqiora_lang::CallArguments::Positional(arguments),
            ..
        } = expression.kind()
        else {
            return Err(error(
                file,
                expression,
                "nominal metadata requires a constructor call",
            ));
        };
        let declaration = match arguments.first().map(Expr::kind) {
            Some(ExprKind::Path(path)) => path.clone(),
            Some(ExprKind::Name(name)) => {
                NamePath::from_segments([name.as_str()], arguments[0].range())
                    .map_err(|failure| error(file, expression, failure.to_string()))?
            }
            _ => {
                return Err(error(
                    file,
                    expression,
                    "nominal constructor requires its exact declaration name",
                ));
            }
        };
        SourceAstFactory::bind_nominal_expression(&mut result, &declaration, value_type.clone())
            .map_err(|failure| error(file, expression, failure.to_string()))?;
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_lang::Item;

    fn expression(text: &str) -> Expr {
        let source = format!("model M() {{ let value = {text}; }}");
        let document = eqiora_lang::parse("reduce.eqi", &source)
            .into_document()
            .unwrap();
        let Item::Let(value) = &document.models()[0].items()[0] else {
            panic!("alias")
        };
        value.value().clone()
    }

    #[test]
    fn expansion_counts_nested_work_and_left_fold_depth_before_copying() {
        let nested = expression("sum(product(1, over = (j in B)), over = (i in A))");
        // Inner three leaves + two product nodes; outer two copies + one sum.
        let mut extent = |name: &str| match name {
            "A" => Some(2),
            "B" => Some(3),
            _ => None,
        };
        assert!(preflight("test", &nested, &mut extent, 11).is_ok());
        assert!(preflight("test", &nested, &mut extent, 10).is_err());
        let flat = expression("sum(1, over = (i in A))");
        assert!(preflight("test", &flat, &mut |_| Some(256), 1000).is_ok());
        assert!(preflight("test", &flat, &mut |_| Some(257), 1000).is_err());
        assert!(preflight("test", &flat, &mut |_| Some(0), 1000).is_err());
        assert!(preflight("test", &flat, &mut |_| None, 1000).is_err());
        assert!(preflight("test", &nested, &mut |_| Some(u32::MAX), usize::MAX).is_err());
    }

    #[test]
    fn substitution_is_capture_aware_and_keeps_integer_projection() {
        let outer =
            expression("sum(product(ordinal(i) + ordinal(j), over = (j in B)), over = (i in A))");
        let ExprKind::Reduction { binder, value, .. } = outer.kind() else {
            panic!("reduction")
        };
        let result = instantiate("test", value, binder, u32::MAX).unwrap();
        let ExprKind::Reduction {
            binder: inner,
            value,
            ..
        } = result.kind()
        else {
            panic!("nested")
        };
        assert_eq!(inner.member(), "j");
        let ExprKind::Binary { left, right, .. } = value.kind() else {
            panic!("sum")
        };
        let ExprKind::Call {
            callee,
            arguments: eqiora_lang::CallArguments::Positional(arguments),
        } = left.kind()
        else {
            panic!("conversion")
        };
        assert_eq!(callee.as_str(), "to_integer");
        let ExprKind::Number(number) = arguments[0].kind() else {
            panic!("number")
        };
        assert_eq!(number.to_i64().unwrap(), i64::from(u32::MAX));
        let ExprKind::Call { callee, .. } = right.kind() else {
            panic!("inner ordinal")
        };
        assert_eq!(callee.as_str(), "ordinal");
        assert!(instantiate("test", &expression("i"), binder, 0).is_err());
        let collision = expression("sum(ordinal(i), over = (i in A))");
        assert!(instantiate("test", &collision, binder, 0).is_err());
        assert!(
            preflight(
                "test",
                &expression("sum(sum(1, over = (i in B)), over = (i in A))"),
                &mut |_| Some(1),
                100
            )
            .is_err()
        );
    }
    #[test]
    fn indexed_selection_retains_exact_nominal_constructor_metadata() {
        let reduction = expression("sum(ordinal(i), over = (i in Stages))");
        let ExprKind::Reduction { binder, .. } = reduction.kind() else {
            panic!("reduction")
        };
        let mut constructor = expression("index(Stages, ordinal(i))");
        let exact_type = eqiora_core::ValueType::index(eqiora_core::Id::new(), 4).unwrap();
        let declaration = NamePath::from_segments(["Stages"], constructor.range()).unwrap();
        SourceAstFactory::bind_nominal_expression(
            &mut constructor,
            &declaration,
            exact_type.clone(),
        )
        .unwrap();
        let result = instantiate("test", &constructor, binder, 3).unwrap();
        assert_eq!(result.resolved_nominal(), Some(&exact_type));
        assert_eq!(result.range(), constructor.range());
        let ExprKind::Call {
            arguments: eqiora_lang::CallArguments::Positional(arguments),
            ..
        } = result.kind()
        else {
            panic!("index")
        };
        assert_eq!(
            arguments[0],
            match constructor.kind() {
                ExprKind::Call {
                    arguments: eqiora_lang::CallArguments::Positional(arguments),
                    ..
                } => arguments[0].clone(),
                _ => unreachable!(),
            }
        );
    }
}
