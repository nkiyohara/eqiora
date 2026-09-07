//! Infer expression arena nodes from already inferred operands.

use super::*;

pub(super) enum NodeInference<I, E> {
    Typed(ExpressionType<I>),
    Unavailable,
    Symbol { symbol: SymbolRef, error: E },
    Type(TypeViolation<I>),
}

pub(super) fn infer_node<I: Clone + Eq, E>(
    expression: &ExprDag,
    node: &ExprNode,
    inferred: &[Option<ExpressionType<I>>],
    relation_support: Option<&SpatialSupport<I>>,
    symbol_type: &mut impl FnMut(SymbolRef) -> Result<ExpressionType<I>, E>,
) -> NodeInference<I, E> {
    let typed = match node {
        ExprNode::Constant(value) => Ok(ExpressionType::new(value.value_type().clone(), None)),
        ExprNode::Symbol(symbol) => {
            return match symbol_type(*symbol) {
                Ok(value) => NodeInference::Typed(value),
                Err(error) => NodeInference::Symbol {
                    symbol: *symbol,
                    error,
                },
            };
        }
        ExprNode::Array { elements } => {
            let Some(elements) = elements
                .iter()
                .map(|id| inferred_type(inferred, *id))
                .collect::<Option<Vec<_>>>()
            else {
                return NodeInference::Unavailable;
            };
            ExpressionType::array(&elements)
        }
        ExprNode::Index {
            value,
            index: position,
        } => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            value.index(*position)
        }
        ExprNode::Complex { real, imag } => {
            let Some((real, imag)) = inferred_binary(inferred, *real, *imag) else {
                return NodeInference::Unavailable;
            };
            real.complex(imag)
        }
        ExprNode::Neg(value) => {
            return inferred_type(inferred, *value)
                .map_or(NodeInference::Unavailable, NodeInference::Typed);
        }
        ExprNode::Add(left, right) | ExprNode::Sub(left, right) => {
            let Some((left, right)) = inferred_binary(inferred, *left, *right) else {
                return NodeInference::Unavailable;
            };
            additive(&left, &right)
        }
        ExprNode::Mul(left, right) => {
            let Some((left, right)) = inferred_binary(inferred, *left, *right) else {
                return NodeInference::Unavailable;
            };
            multiply(&left, &right)
        }
        ExprNode::Div(left, right) => {
            let Some((left, right)) = inferred_binary(inferred, *left, *right) else {
                return NodeInference::Unavailable;
            };
            divide(&left, &right)
        }
        ExprNode::PowI(base, exponent) => {
            let Some(base) = inferred_type(inferred, *base) else {
                return NodeInference::Unavailable;
            };
            power(&base, *exponent)
        }
        ExprNode::SpatialCoordinate(axis) => coordinate(*axis, relation_support),
        ExprNode::UnaryMath(function, value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            unary_math(*function, &value)
        }
        ExprNode::Gradient(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            gradient(&value)
        }
        ExprNode::Divergence(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            divergence(&value)
        }
        ExprNode::SymmetricPart(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            symmetric_part(&value)
        }
        ExprNode::IsotropicLift(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            isotropic_lift(&value)
        }
        ExprNode::Trace(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            trace(&value, relation_support)
        }
        ExprNode::NormalComponent(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            normal(&value, relation_support)
        }
        ExprNode::PureOperatorApplication(application) => {
            let mut arguments = Vec::with_capacity(application.arguments().len());
            for argument in application.arguments() {
                let Some(argument_type) = inferred_type(inferred, *argument) else {
                    return NodeInference::Unavailable;
                };
                arguments.push(argument_type);
            }
            let definition = expression
                .definition(application.definition())
                .expect("ExprDag keeps every pure application definition in its closed table");
            definition
                .instantiate(&arguments)
                .map(|application| application.result_type().clone())
                .map_err(TypeViolation::PureOperatorApplication)
        }
    };
    match typed {
        Ok(value) => NodeInference::Typed(value),
        Err(error) => NodeInference::Type(error),
    }
}

fn inferred_binary<I: Clone>(
    inferred: &[Option<ExpressionType<I>>],
    left: ExprId,
    right: ExprId,
) -> Option<(ExpressionType<I>, ExpressionType<I>)> {
    Some((
        inferred_type(inferred, left)?,
        inferred_type(inferred, right)?,
    ))
}

pub(super) fn inferred_type<I: Clone>(
    inferred: &[Option<ExpressionType<I>>],
    id: ExprId,
) -> Option<ExpressionType<I>> {
    usize::try_from(id.index())
        .ok()
        .and_then(|index| inferred.get(index))
        .cloned()
        .flatten()
}
