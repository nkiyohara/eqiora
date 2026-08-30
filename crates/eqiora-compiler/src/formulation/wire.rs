use eqiora_core::RawId;
use eqiora_lang::BinaryOp;
use serde::Serialize;

use super::{AuthoredFormExpression, AuthoredFormExpressionKind, AuthoredScalarPrimalForm};

const SCHEMA: &str = "eqiora.authored-scalar-primal-form/v1";

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct WireForm {
    schema: &'static str,
    source_identity: String,
    relation_ulid: String,
    domain_ulid: String,
    trial_ulid: String,
    left: WireExpression,
    right: WireExpression,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum WireExpression {
    Number {
        value: f64,
    },
    Field {
        ulid: String,
    },
    Parameter {
        ulid: String,
    },
    Coordinate {
        axis: usize,
    },
    Test {
        field_ulid: String,
    },
    Neg {
        value: Box<Self>,
    },
    Add {
        left: Box<Self>,
        right: Box<Self>,
    },
    Sub {
        left: Box<Self>,
        right: Box<Self>,
    },
    Mul {
        left: Box<Self>,
        right: Box<Self>,
    },
    Div {
        left: Box<Self>,
        right: Box<Self>,
    },
    Pow {
        base: Box<Self>,
        exponent: i32,
    },
    Gradient {
        value: Box<Self>,
    },
    Sin {
        value: Box<Self>,
    },
    Dot {
        left: Box<Self>,
        right: Box<Self>,
    },
    Integrate {
        domain_ulid: String,
        integrand: Box<Self>,
    },
}

pub(super) fn encode(form: &AuthoredScalarPrimalForm) -> Vec<u8> {
    let wire = WireForm {
        schema: SCHEMA,
        source_identity: form.source_identity().to_string(),
        relation_ulid: ulid(form.relation().erase()),
        domain_ulid: ulid(form.domain().erase()),
        trial_ulid: ulid(form.trial().erase()),
        left: expression(&form.left),
        right: expression(&form.right),
    };
    serde_json::to_vec(&wire).expect("typed authored Formulation is canonical JSON")
}

fn expression(value: &AuthoredFormExpression) -> WireExpression {
    match &value.kind {
        AuthoredFormExpressionKind::Number(value) => WireExpression::Number { value: *value },
        AuthoredFormExpressionKind::Field(id) => WireExpression::Field {
            ulid: ulid(id.erase()),
        },
        AuthoredFormExpressionKind::Parameter(id) => WireExpression::Parameter {
            ulid: ulid(id.erase()),
        },
        AuthoredFormExpressionKind::Coordinate(axis) => WireExpression::Coordinate { axis: *axis },
        AuthoredFormExpressionKind::Test(id) => WireExpression::Test {
            field_ulid: ulid(id.erase()),
        },
        AuthoredFormExpressionKind::Neg(value) => WireExpression::Neg {
            value: Box::new(expression(value)),
        },
        AuthoredFormExpressionKind::Binary {
            operator,
            left,
            right,
        } => {
            let left = Box::new(expression(left));
            let right = Box::new(expression(right));
            match operator {
                BinaryOp::Add => WireExpression::Add { left, right },
                BinaryOp::Sub => WireExpression::Sub { left, right },
                BinaryOp::Mul => WireExpression::Mul { left, right },
                BinaryOp::Div => WireExpression::Div { left, right },
                BinaryOp::Pow => unreachable!("power is represented by the typed Pow node"),
            }
        }
        AuthoredFormExpressionKind::Pow(base, exponent) => WireExpression::Pow {
            base: Box::new(expression(base)),
            exponent: *exponent,
        },
        AuthoredFormExpressionKind::Gradient(value) => WireExpression::Gradient {
            value: Box::new(expression(value)),
        },
        AuthoredFormExpressionKind::Sin(value) => WireExpression::Sin {
            value: Box::new(expression(value)),
        },
        AuthoredFormExpressionKind::Dot(left, right) => WireExpression::Dot {
            left: Box::new(expression(left)),
            right: Box::new(expression(right)),
        },
        AuthoredFormExpressionKind::Integrate { domain, integrand } => WireExpression::Integrate {
            domain_ulid: ulid(domain.erase()),
            integrand: Box::new(expression(integrand)),
        },
    }
}

fn ulid(id: RawId) -> String {
    id.ulid().to_string()
}
