//! Compiler-owned translation from exact source syntax to Kernel definitions.
//!
//! This is the single semantic conversion path used by local compilation,
//! resolved-package analysis, and source identity.  Keeping it here prevents
//! source adapters from inventing a second interpretation of the bounded
//! pure calculus.

use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{
    PureOperatorBinaryOp, PureOperatorDecl, PureOperatorExpr, PureOperatorExprKind,
    PureValueClassSyntax, TextRange,
};
use eqiora_schema::kernel::pure_operator::{
    CalculusBuilder, CalculusNode, CalculusNodeId, ExactRational, PureOperatorDefinition,
    PureOperatorError, PureValueClass, ResultAxis,
};

use crate::diagnostics::source_error;

/// Whether a call path denotes the closed source builtin vocabulary rather
/// than a package-resolved pure definition.
pub(crate) fn is_builtin_operator(path: &eqiora_lang::NamePath) -> bool {
    !path.is_qualified()
        && matches!(
            path.as_str(),
            "across"
                | "through"
                | "flux"
                | "trace"
                | "coordinate"
                | "sin"
                | "grad"
                | "div"
                | "symmetric_part"
                | "isotropic_lift"
                | "normal"
                | "derivative"
                | "pre"
                | "next"
        )
}

/// Compile one closed source declaration into its name-free Kernel meaning.
pub(crate) fn compile_definition(
    file: &str,
    declaration: &PureOperatorDecl,
) -> Result<PureOperatorDefinition, Diagnostic> {
    let mut formal_names = BTreeMap::new();
    let mut formal_rules = Vec::with_capacity(declaration.formals().len());
    for (slot, formal) in declaration.formals().iter().enumerate() {
        let slot = u16::try_from(slot).map_err(|_| {
            pure_error(
                file,
                formal.range(),
                "pure operator has too many ordered formals",
            )
        })?;
        if formal_names.insert(formal.name(), slot).is_some() {
            return Err(pure_error(
                file,
                formal.range(),
                format!("duplicate pure-operator formal `{}`", formal.name()),
            ));
        }
        formal_rules.push(value_class(file, formal.range(), formal.value_class())?);
    }
    let result_rule = value_class(file, declaration.range(), declaration.result())?;
    let mut builder = CalculusBuilder::new(formal_rules, result_rule)
        .map_err(|error| kernel_error(file, declaration.range(), error))?;
    let root = compile_expression(file, declaration.body(), &formal_names, &mut builder)?;
    builder
        .finish(root)
        .map_err(|error| kernel_error(file, declaration.body().range(), error))
}

fn value_class(
    file: &str,
    range: TextRange,
    syntax: &PureValueClassSyntax,
) -> Result<PureValueClass, Diagnostic> {
    match syntax {
        PureValueClassSyntax::Scalar => Ok(PureValueClass::invariant_scalar()),
        PureValueClassSyntax::Spatial { rank } => u16::try_from(rank.value())
            .map_err(|_| pure_error(file, rank.range(), "spatial rank exceeds u16"))
            .and_then(|rank| {
                PureValueClass::spatial_tensor(rank)
                    .map_err(|error| kernel_error(file, range, error))
            }),
        _ => Err(pure_error(
            file,
            range,
            "pure value-class syntax is newer than this compiler",
        )),
    }
}

fn compile_expression(
    file: &str,
    expression: &PureOperatorExpr,
    formals: &BTreeMap<&str, u16>,
    builder: &mut CalculusBuilder,
) -> Result<CalculusNodeId, Diagnostic> {
    let node = match expression.kind() {
        PureOperatorExprKind::Rational {
            numerator,
            denominator,
        } => {
            let numerator = i64::try_from(numerator.value()).map_err(|_| {
                pure_error(
                    file,
                    numerator.range(),
                    "exact rational numerator exceeds i64",
                )
            })?;
            let denominator = i64::try_from(denominator.value()).map_err(|_| {
                pure_error(
                    file,
                    denominator.range(),
                    "exact rational denominator exceeds i64",
                )
            })?;
            CalculusNode::Rational(
                ExactRational::new(numerator, denominator)
                    .map_err(|error| kernel_error(file, expression.range(), error))?,
            )
        }
        PureOperatorExprKind::Component {
            formal,
            formal_range,
            result_axes,
        } => {
            let slot = formals.get(formal.as_str()).copied().ok_or_else(|| {
                pure_error(
                    file,
                    *formal_range,
                    format!("unknown pure-operator formal `{formal}`"),
                )
            })?;
            let axes = result_axes
                .iter()
                .map(|axis| {
                    u16::try_from(axis.value())
                        .map(ResultAxis::new)
                        .map_err(|_| pure_error(file, axis.range(), "result axis exceeds u16"))
                })
                .collect::<Result<Box<[_]>, _>>()?;
            CalculusNode::FormalComponent { formal: slot, axes }
        }
        PureOperatorExprKind::Delta {
            left_axis,
            right_axis,
        } => CalculusNode::KroneckerDelta(
            ResultAxis::new(
                u16::try_from(left_axis.value())
                    .map_err(|_| pure_error(file, left_axis.range(), "result axis exceeds u16"))?,
            ),
            ResultAxis::new(
                u16::try_from(right_axis.value())
                    .map_err(|_| pure_error(file, right_axis.range(), "result axis exceeds u16"))?,
            ),
        ),
        PureOperatorExprKind::Neg(value) => {
            CalculusNode::Neg(compile_expression(file, value, formals, builder)?)
        }
        PureOperatorExprKind::Binary { op, left, right } => {
            let left = compile_expression(file, left, formals, builder)?;
            let right = compile_expression(file, right, formals, builder)?;
            match op {
                PureOperatorBinaryOp::Add => CalculusNode::Add(left, right),
                PureOperatorBinaryOp::Sub => {
                    let right = builder
                        .push(CalculusNode::Neg(right))
                        .map_err(|error| kernel_error(file, expression.range(), error))?;
                    CalculusNode::Add(left, right)
                }
                PureOperatorBinaryOp::Mul => CalculusNode::Mul(left, right),
            }
        }
        _ => {
            return Err(pure_error(
                file,
                expression.range(),
                "pure-operator expression syntax is newer than this compiler",
            ));
        }
    };
    builder
        .push(node)
        .map_err(|error| kernel_error(file, expression.range(), error))
}

fn kernel_error(file: &str, range: TextRange, error: PureOperatorError) -> Diagnostic {
    pure_error(file, range, format!("invalid pure operator: {error}"))
}

fn pure_error(file: &str, range: TextRange, message: impl Into<String>) -> Diagnostic {
    source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message)
}

#[cfg(test)]
mod tests {
    use eqiora_lang::parse;

    use super::*;

    #[test]
    fn source_names_do_not_enter_definition_identity() {
        let left = parse(
            "left.eqi",
            "public pure operator dyadic(a: spatial[1], b: spatial[1]) -> spatial[2] = component(a, 0) * component(b, 1);\nmodel M {}\n",
        )
        .into_document()
        .expect("source");
        let right = parse(
            "right.eqi",
            "public pure operator outer(x: spatial[1], y: spatial[1]) -> spatial[2] = component(x, 0) * component(y, 1);\nmodel M {}\n",
        )
        .into_document()
        .expect("source");
        let left = compile_definition("left.eqi", &left.pure_operators()[0]).expect("definition");
        let right =
            compile_definition("right.eqi", &right.pure_operators()[0]).expect("definition");
        assert_eq!(left.digest(), right.digest());
    }

    #[test]
    fn unresolved_formals_fail_at_the_central_definition_boundary() {
        let document = parse(
            "invalid.eqi",
            "pure operator broken(value: scalar) -> scalar = component(missing);\n",
        )
        .into_document()
        .expect("closed syntax still parses");
        let diagnostic = compile_definition("invalid.eqi", &document.pure_operators()[0])
            .expect_err("free source names cannot enter canonical calculus");
        assert!(
            diagnostic
                .message()
                .contains("unknown pure-operator formal `missing`")
        );
        assert!(diagnostic.source_span().is_some());
    }
}
