use super::*;

pub(super) fn encode_expression(
    encoder: &mut Encoder,
    expression: &Expr,
    budget: &mut Budget,
    depth: usize,
) -> Result<(), Diagnostic> {
    budget.account_expression(depth)?;
    match expression.kind() {
        ExprKind::Number(value) => {
            encoder.u16(1)?;
            encoder.f64(*value)
        }
        ExprKind::Quantity { value, unit } => {
            encoder.u16(9)?;
            encoder.field(1, |encoder| encoder.f64(*value))?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, unit, budget, next_depth(depth)?)
            })
        }
        ExprKind::Array(elements) => {
            encoder.u16(10)?;
            let child_depth = next_depth(depth)?;
            let mut encoded = Vec::with_capacity(elements.len());
            for element in elements {
                let mut element_encoder = Encoder::new(budget.limits.max_canonical_bytes);
                encode_expression(&mut element_encoder, element, budget, child_depth)?;
                let bytes = element_encoder.finish()?;
                budget.account_materialized_bytes(bytes.len())?;
                encoded.push(bytes);
            }
            encoder.field(1, |encoder| encoder.records(&encoded))
        }
        ExprKind::Index { value, index } => {
            encoder.u16(11)?;
            let child_depth = next_depth(depth)?;
            encoder.field(1, |encoder| {
                encode_expression(encoder, value, budget, child_depth)
            })?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, index, budget, child_depth)
            })
        }
        ExprKind::Name(name) => {
            encoder.u16(2)?;
            encoder.field(1, |encoder| encode_name(encoder, name, budget))
        }
        ExprKind::Path(path) => {
            encoder.u16(3)?;
            encoder.field(1, |encoder| encode_path(encoder, path, budget))
        }
        ExprKind::BoundaryPortSelection { port, selector } => {
            encoder.u16(7)?;
            encoder.field(1, |encoder| encode_path(encoder, port, budget))?;
            encoder.field(2, |encoder| {
                encode_boundary_port_selector(encoder, selector, budget)
            })
        }
        ExprKind::Unary { op, value } => {
            if matches!(op, UnaryOp::Neg)
                && matches!(value.kind(), ExprKind::Number(value) if *value == 0.0)
            {
                budget.account_expression(next_depth(depth)?)?;
                encoder.u16(1)?;
                return encoder.f64(0.0);
            }
            encoder.u16(4)?;
            encoder.field(1, |encoder| {
                encoder.u8(match op {
                    UnaryOp::Neg => 1,
                })
            })?;
            let child_depth = next_depth(depth)?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, value, budget, child_depth)
            })
        }
        ExprKind::Binary { op, left, right } => {
            encoder.u16(5)?;
            encoder.field(1, |encoder| {
                encoder.u8(match op {
                    BinaryOp::Add => 1,
                    BinaryOp::Sub => 2,
                    BinaryOp::Mul => 3,
                    BinaryOp::Div => 4,
                    BinaryOp::Pow => 5,
                })
            })?;
            let child_depth = next_depth(depth)?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, left, budget, child_depth)
            })?;
            encoder.field(3, |encoder| {
                encode_expression(encoder, right, budget, child_depth)
            })
        }
        ExprKind::Call { callee, arguments } if !callee.is_qualified() && arguments.len() == 1 => {
            // Byte-for-byte compatibility with source identity v1.
            encoder.u16(6)?;
            encoder.field(1, |encoder| encode_name(encoder, callee.as_str(), budget))?;
            let child_depth = next_depth(depth)?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, &arguments[0], budget, child_depth)
            })
        }
        ExprKind::Call { callee, arguments } => {
            encoder.u16(8)?;
            encoder.field(1, |encoder| encode_type_path(encoder, callee, budget))?;
            let child_depth = next_depth(depth)?;
            let mut encoded = Vec::with_capacity(arguments.len());
            for argument in arguments {
                let mut argument_encoder = Encoder::new(budget.limits.max_canonical_bytes);
                encode_expression(&mut argument_encoder, argument, budget, child_depth)?;
                encoded.push(argument_encoder.finish()?);
            }
            let materialized = encoded.iter().try_fold(0_usize, |total, value| {
                total.checked_add(value.len()).ok_or_else(|| {
                    source_identity_error("call argument encoding bytes overflow usize")
                })
            })?;
            budget.account_materialized_bytes(materialized)?;
            encoder.field(2, |encoder| encoder.records(&encoded))
        }
        _ => Err(source_identity_error(
            "expression syntax is newer than source identity v1",
        )),
    }
}

pub(super) fn encode_relation(
    encoder: &mut Encoder,
    declaration: &RelationDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    if declaration.equations().len() > budget.limits.max_residuals_per_relation {
        return Err(source_identity_error(format!(
            "Relation `{}` has {} residuals, exceeding the {} residual limit",
            declaration.name(),
            declaration.equations().len(),
            budget.limits.max_residuals_per_relation
        )));
    }
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| match declaration.activation() {
        ActivationSyntax::Continuous => encoder.u16(1),
        ActivationSyntax::Periodic(clock) => {
            encoder.u16(2)?;
            encoder.field(1, |encoder| encode_name(encoder, clock, budget))
        }
        _ => Err(source_identity_error(
            "Activation syntax is newer than source identity v1",
        )),
    })?;
    encoder.field(3, |encoder| {
        encode_optional_name(encoder, declaration.domain(), budget)
    })?;
    encoder.field(4, |encoder| {
        encoder.u32(as_u32(
            declaration.equations().len(),
            "Relation residual count",
        )?)?;
        for equation in declaration.equations() {
            encoder.field(1, |encoder| {
                encoder.field(1, |encoder| {
                    encode_expression(encoder, equation.left(), budget, 1)
                })?;
                encoder.field(2, |encoder| {
                    encode_expression(encoder, equation.right(), budget, 1)
                })
            })?;
        }
        Ok(())
    })
}

pub(super) fn encode_relation_family(
    encoder: &mut Encoder,
    declaration: &RelationFamilyDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_relation(encoder, declaration.relation(), budget)
    })?;
    encoder.field(2, |encoder| {
        encode_boundary_family_binder(encoder, declaration.binder(), budget)
    })
}
