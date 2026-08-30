use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef, UnaryMathFunction};
use eqiora_sem::KernelProgram;
use serde::{Deserialize, Serialize};

use super::{DerivedScalarGalerkinForm, typed_relation};

const SCHEMA: &str = "eqiora.authored-scalar-primal-form/v1";
const MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcceptedAuthoredScalarPrimalForm {
    source_identity: String,
    bytes: Box<[u8]>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireForm {
    schema: String,
    source_identity: String,
    relation_ulid: String,
    domain_ulid: String,
    trial_ulid: String,
    left: WireExpression,
    right: WireExpression,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
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

impl AcceptedAuthoredScalarPrimalForm {
    pub(crate) fn admit(
        bytes: &[u8],
        program: &KernelProgram,
        derived: &DerivedScalarGalerkinForm,
    ) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_BYTES {
            return Err(rejection(
                "projection exceeds the scalar-primal decoder limit",
            ));
        }
        let wire: WireForm = serde_json::from_slice(bytes)
            .map_err(|_| rejection("projection is not the closed canonical scalar-primal wire"))?;
        if wire.schema != SCHEMA || serde_json::to_vec(&wire).ok().as_deref() != Some(bytes) {
            return Err(rejection(
                "projection schema or canonical encoding is invalid",
            ));
        }
        if wire.source_identity.len() != 64
            || !wire
                .source_identity
                .bytes()
                .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        {
            return Err(rejection(
                "source identity is not one canonical SHA-256 digest",
            ));
        }
        let expected_relation = derived.volume_relation.ulid().to_string();
        let expected_domain = derived.domain.ulid().to_string();
        let expected_trial = derived.field.ulid().to_string();
        if wire.relation_ulid != expected_relation {
            return Err(rejection_with(
                &wire,
                "Relation differs from the admitted strong Law",
            ));
        }
        if wire.domain_ulid != expected_domain {
            return Err(rejection_with(
                &wire,
                "integration support differs from the admitted Domain",
            ));
        }
        if wire.trial_ulid != expected_trial {
            return Err(rejection_with(
                &wire,
                "trial/test Field differs from the admitted unknown",
            ));
        }

        let typed = typed_relation(program, derived.volume_relation)?;
        let dag = typed.expression();
        let ExprNode::Divergence(flux) = node(dag, derived.volume_nodes.divergence)? else {
            return Err(rejection_with(
                &wire,
                "admitted divergence certificate is stale",
            ));
        };
        let test = WireExpression::Test {
            field_ulid: expected_trial,
        };
        let left = WireExpression::Integrate {
            domain_ulid: expected_domain.clone(),
            integrand: Box::new(WireExpression::Dot {
                left: Box::new(WireExpression::Gradient {
                    value: Box::new(test.clone()),
                }),
                right: Box::new(from_dag(dag, *flux)?),
            }),
        };
        let right = WireExpression::Integrate {
            domain_ulid: expected_domain,
            integrand: Box::new(WireExpression::Mul {
                left: Box::new(test),
                right: Box::new(from_dag(dag, derived.volume_nodes.source)?),
            }),
        };
        if !equivalent(&wire.left, &left) {
            return Err(rejection_with(
                &wire,
                "left bilinear term, coefficient, sign, or contraction differs from the admitted primal form",
            ));
        }
        if !equivalent(&wire.right, &right) {
            return Err(rejection_with(
                &wire,
                "right source term, sign, or test pairing differs from the admitted primal form",
            ));
        }
        Ok(Self {
            source_identity: wire.source_identity,
            bytes: bytes.into(),
        })
    }

    pub(crate) fn source_identity(&self) -> &str {
        &self.source_identity
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

fn from_dag(dag: &ExprDag, id: ExprId) -> Result<WireExpression, Diagnostic> {
    let convert = |id| from_dag(dag, id).map(Box::new);
    Ok(match node(dag, id)? {
        ExprNode::Constant(value) => WireExpression::Number {
            value: value.value(),
        },
        ExprNode::Symbol(SymbolRef::Field(id)) => WireExpression::Field {
            ulid: id.ulid().to_string(),
        },
        ExprNode::Symbol(SymbolRef::Parameter(id)) => WireExpression::Parameter {
            ulid: id.ulid().to_string(),
        },
        ExprNode::SpatialCoordinate(axis) => WireExpression::Coordinate { axis: *axis },
        ExprNode::Neg(value) => WireExpression::Neg {
            value: convert(*value)?,
        },
        ExprNode::Add(left, right) => WireExpression::Add {
            left: convert(*left)?,
            right: convert(*right)?,
        },
        ExprNode::Sub(left, right) => WireExpression::Sub {
            left: convert(*left)?,
            right: convert(*right)?,
        },
        ExprNode::Mul(left, right) => WireExpression::Mul {
            left: convert(*left)?,
            right: convert(*right)?,
        },
        ExprNode::Div(left, right) => WireExpression::Div {
            left: convert(*left)?,
            right: convert(*right)?,
        },
        ExprNode::PowI(base, exponent) => WireExpression::Pow {
            base: convert(*base)?,
            exponent: *exponent,
        },
        ExprNode::Gradient(value) => WireExpression::Gradient {
            value: convert(*value)?,
        },
        ExprNode::UnaryMath(UnaryMathFunction::Sin, value) => WireExpression::Sin {
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

fn equivalent(left: &WireExpression, right: &WireExpression) -> bool {
    match (left, right) {
        (WireExpression::Add { .. }, WireExpression::Add { .. }) => {
            multiset(left, true) == multiset(right, true)
        }
        (WireExpression::Mul { .. }, WireExpression::Mul { .. }) => {
            multiset(left, false) == multiset(right, false)
        }
        (WireExpression::Number { value: a }, WireExpression::Number { value: b }) => {
            a.to_bits() == b.to_bits()
        }
        (WireExpression::Field { ulid: a }, WireExpression::Field { ulid: b })
        | (WireExpression::Parameter { ulid: a }, WireExpression::Parameter { ulid: b }) => a == b,
        (WireExpression::Coordinate { axis: a }, WireExpression::Coordinate { axis: b }) => a == b,
        (WireExpression::Test { field_ulid: a }, WireExpression::Test { field_ulid: b }) => a == b,
        (WireExpression::Neg { value: a }, WireExpression::Neg { value: b })
        | (WireExpression::Gradient { value: a }, WireExpression::Gradient { value: b })
        | (WireExpression::Sin { value: a }, WireExpression::Sin { value: b }) => equivalent(a, b),
        (
            WireExpression::Sub {
                left: al,
                right: ar,
            },
            WireExpression::Sub {
                left: bl,
                right: br,
            },
        )
        | (
            WireExpression::Div {
                left: al,
                right: ar,
            },
            WireExpression::Div {
                left: bl,
                right: br,
            },
        )
        | (
            WireExpression::Dot {
                left: al,
                right: ar,
            },
            WireExpression::Dot {
                left: bl,
                right: br,
            },
        ) => equivalent(al, bl) && equivalent(ar, br),
        (
            WireExpression::Pow {
                base: a,
                exponent: ae,
            },
            WireExpression::Pow {
                base: b,
                exponent: be,
            },
        ) => ae == be && equivalent(a, b),
        (
            WireExpression::Integrate {
                domain_ulid: ad,
                integrand: a,
            },
            WireExpression::Integrate {
                domain_ulid: bd,
                integrand: b,
            },
        ) => ad == bd && equivalent(a, b),
        _ => false,
    }
}

fn multiset(value: &WireExpression, addition: bool) -> Vec<String> {
    fn collect<'a>(
        value: &'a WireExpression,
        addition: bool,
        values: &mut Vec<&'a WireExpression>,
    ) {
        match (addition, value) {
            (true, WireExpression::Add { left, right })
            | (false, WireExpression::Mul { left, right }) => {
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

fn rejection_with(wire: &WireForm, message: &str) -> Diagnostic {
    rejection(&format!(
        "{message} (source identity {})",
        wire.source_identity
    ))
}
