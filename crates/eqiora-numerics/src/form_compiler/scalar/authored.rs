use eqiora_compiler::{AuthoredFormExpressionV1, AuthoredFormulationProjection};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef, UnaryMathFunction};
use eqiora_sem::KernelProgram;

use super::{DerivedScalarGalerkinForm, typed_relation};

pub(crate) fn admit(
    projection: &AuthoredFormulationProjection,
    program: &KernelProgram,
    derived: &DerivedScalarGalerkinForm,
) -> Result<(), Diagnostic> {
    let expected_relation = derived.volume_relation.ulid().to_string();
    let expected_domain = derived.domain.ulid().to_string();
    let expected_trial = derived.field.ulid().to_string();
    if projection.relation_ulid() != expected_relation {
        return Err(rejection_with(
            projection,
            "Relation differs from the admitted strong Law",
        ));
    }
    if projection.domain_ulid() != expected_domain {
        return Err(rejection_with(
            projection,
            "integration support differs from the admitted Domain",
        ));
    }
    if projection.trial_ulid() != expected_trial {
        return Err(rejection_with(
            projection,
            "trial/test Field differs from the admitted unknown",
        ));
    }

    let typed = typed_relation(program, derived.volume_relation)?;
    let dag = typed.expression();
    let ExprNode::Divergence(flux) = node(dag, derived.volume_nodes.divergence)? else {
        return Err(rejection_with(
            projection,
            "admitted divergence certificate is stale",
        ));
    };
    let test = AuthoredFormExpressionV1::Test {
        field_ulid: expected_trial,
    };
    let left = AuthoredFormExpressionV1::Integrate {
        domain_ulid: expected_domain.clone(),
        integrand: Box::new(AuthoredFormExpressionV1::Dot {
            left: Box::new(AuthoredFormExpressionV1::Gradient {
                value: Box::new(test.clone()),
            }),
            right: Box::new(from_dag(dag, *flux)?),
        }),
    };
    let right = AuthoredFormExpressionV1::Integrate {
        domain_ulid: expected_domain,
        integrand: Box::new(AuthoredFormExpressionV1::Mul {
            left: Box::new(test),
            right: Box::new(from_dag(dag, derived.volume_nodes.source)?),
        }),
    };
    if !equivalent(projection.left(), &left) {
        return Err(rejection_with(
            projection,
            "left bilinear term, coefficient, sign, or contraction differs from the admitted primal form",
        ));
    }
    if !equivalent(projection.right(), &right) {
        return Err(rejection_with(
            projection,
            "right source term, sign, or test pairing differs from the admitted primal form",
        ));
    }
    Ok(())
}

fn from_dag(dag: &ExprDag, id: ExprId) -> Result<AuthoredFormExpressionV1, Diagnostic> {
    let convert = |id| from_dag(dag, id).map(Box::new);
    Ok(match node(dag, id)? {
        ExprNode::Constant(value) => AuthoredFormExpressionV1::Number {
            value: value.value(),
        },
        ExprNode::Symbol(SymbolRef::Field(id)) => AuthoredFormExpressionV1::Field {
            ulid: id.ulid().to_string(),
        },
        ExprNode::Symbol(SymbolRef::Parameter(id)) => AuthoredFormExpressionV1::Parameter {
            ulid: id.ulid().to_string(),
        },
        ExprNode::SpatialCoordinate(axis) => AuthoredFormExpressionV1::Coordinate { axis: *axis },
        ExprNode::Neg(value) => AuthoredFormExpressionV1::Neg {
            value: convert(*value)?,
        },
        ExprNode::Add(left, right) => AuthoredFormExpressionV1::Add {
            left: convert(*left)?,
            right: convert(*right)?,
        },
        ExprNode::Sub(left, right) => AuthoredFormExpressionV1::Sub {
            left: convert(*left)?,
            right: convert(*right)?,
        },
        ExprNode::Mul(left, right) => AuthoredFormExpressionV1::Mul {
            left: convert(*left)?,
            right: convert(*right)?,
        },
        ExprNode::Div(left, right) => AuthoredFormExpressionV1::Div {
            left: convert(*left)?,
            right: convert(*right)?,
        },
        ExprNode::PowI(base, exponent) => AuthoredFormExpressionV1::Pow {
            base: convert(*base)?,
            exponent: *exponent,
        },
        ExprNode::Gradient(value) => AuthoredFormExpressionV1::Gradient {
            value: convert(*value)?,
        },
        ExprNode::UnaryMath(UnaryMathFunction::Sin, value) => AuthoredFormExpressionV1::Sin {
            value: convert(*value)?,
        },
        _ => {
            return Err(rejection(
                "admitted strong expression exceeds the authored scalar-primal inventory",
            ));
        }
    })
}

fn node(dag: &ExprDag, id: ExprId) -> Result<&ExprNode, Diagnostic> {
    dag.nodes()
        .get(id.index() as usize)
        .ok_or_else(|| rejection("admitted expression certificate references a missing node"))
}

fn equivalent(left: &AuthoredFormExpressionV1, right: &AuthoredFormExpressionV1) -> bool {
    use AuthoredFormExpressionV1 as Expression;

    match (left, right) {
        (Expression::Add { .. }, Expression::Add { .. }) => {
            multiset(left, true) == multiset(right, true)
        }
        (Expression::Mul { .. }, Expression::Mul { .. }) => {
            multiset(left, false) == multiset(right, false)
        }
        (Expression::Number { value: a }, Expression::Number { value: b }) => {
            a.to_bits() == b.to_bits()
        }
        (Expression::Field { ulid: a }, Expression::Field { ulid: b })
        | (Expression::Parameter { ulid: a }, Expression::Parameter { ulid: b }) => a == b,
        (Expression::Coordinate { axis: a }, Expression::Coordinate { axis: b }) => a == b,
        (Expression::Test { field_ulid: a }, Expression::Test { field_ulid: b }) => a == b,
        (Expression::Neg { value: a }, Expression::Neg { value: b })
        | (Expression::Gradient { value: a }, Expression::Gradient { value: b })
        | (Expression::Sin { value: a }, Expression::Sin { value: b }) => equivalent(a, b),
        (
            Expression::Sub {
                left: al,
                right: ar,
            },
            Expression::Sub {
                left: bl,
                right: br,
            },
        )
        | (
            Expression::Div {
                left: al,
                right: ar,
            },
            Expression::Div {
                left: bl,
                right: br,
            },
        )
        | (
            Expression::Dot {
                left: al,
                right: ar,
            },
            Expression::Dot {
                left: bl,
                right: br,
            },
        ) => equivalent(al, bl) && equivalent(ar, br),
        (
            Expression::Pow {
                base: a,
                exponent: ae,
            },
            Expression::Pow {
                base: b,
                exponent: be,
            },
        ) => ae == be && equivalent(a, b),
        (
            Expression::Integrate {
                domain_ulid: ad,
                integrand: a,
            },
            Expression::Integrate {
                domain_ulid: bd,
                integrand: b,
            },
        ) => ad == bd && equivalent(a, b),
        _ => false,
    }
}

fn multiset(value: &AuthoredFormExpressionV1, addition: bool) -> Vec<String> {
    fn collect<'a>(
        value: &'a AuthoredFormExpressionV1,
        addition: bool,
        values: &mut Vec<&'a AuthoredFormExpressionV1>,
    ) {
        match (addition, value) {
            (true, AuthoredFormExpressionV1::Add { left, right })
            | (false, AuthoredFormExpressionV1::Mul { left, right }) => {
                collect(left, addition, values);
                collect(right, addition, values);
            }
            _ => values.push(value),
        }
    }
    let mut values = Vec::new();
    collect(value, addition, &mut values);
    let mut values = values
        .into_iter()
        .map(|value| serde_json::to_string(value).expect("wire expression serializes"))
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn rejection(message: &str) -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_DISCRETIZATION,
        format!("authored scalar-primal Formulation rejected: {message}"),
    )
}

fn rejection_with(projection: &AuthoredFormulationProjection, message: &str) -> Diagnostic {
    rejection(&format!(
        "{message} (source identity {})",
        projection.source_identity()
    ))
}
