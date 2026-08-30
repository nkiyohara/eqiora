//! Shared source-level SI-dimension checking.
//!
//! Flat and hierarchical source paths consume these exact operations before
//! canonical lowering. Keeping them here prevents either path from becoming
//! the accidental owner of physical-dimension semantics.

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents};
use eqiora_lang::{BinaryOp, Expr, ExprKind, TextRange, UnaryOp};

use crate::diagnostics::source_error;

pub(crate) fn lower_dimension(file: &str, expression: &Expr) -> Result<DimExponents, Diagnostic> {
    match expression.kind() {
        ExprKind::Number(value) if *value == 1.0 => Ok(DimExponents::DIMENSIONLESS),
        ExprKind::Name(name) => coherent_dimension(name).ok_or_else(|| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                expression.range(),
                format!("unknown coherent-SI dimension symbol `{name}`"),
            )
        }),
        ExprKind::Binary { op, left, right } if matches!(op, BinaryOp::Mul | BinaryOp::Div) => {
            let left = lower_dimension(file, left)?;
            let right = lower_dimension(file, right)?;
            let operation = if *op == BinaryOp::Mul {
                i8::checked_add
            } else {
                i8::checked_sub
            };
            checked_dimensions(left, right, operation)
                .ok_or_else(|| dimension_overflow(file, expression.range()))
        }
        ExprKind::Binary {
            op: BinaryOp::Pow,
            left,
            right,
        } => {
            let dimension = lower_dimension(file, left)?;
            let exponent = integer_literal(right).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    right.range(),
                    "dimension power must be an i32 integer literal",
                )
            })?;
            checked_scale_dimension(dimension, exponent)
                .ok_or_else(|| dimension_overflow(file, expression.range()))
        }
        _ => Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            expression.range(),
            "dimension must use `1`, SI base symbols, `*`, `/`, and integer powers",
        )),
    }
}

pub(crate) fn checked_dimensions(
    left: DimExponents,
    right: DimExponents,
    operation: fn(i8, i8) -> Option<i8>,
) -> Option<DimExponents> {
    Some(DimExponents {
        mass: operation(left.mass, right.mass)?,
        length: operation(left.length, right.length)?,
        time: operation(left.time, right.time)?,
        current: operation(left.current, right.current)?,
        temperature: operation(left.temperature, right.temperature)?,
        amount: operation(left.amount, right.amount)?,
        luminous_intensity: operation(left.luminous_intensity, right.luminous_intensity)?,
    })
}

pub(crate) fn checked_scale_dimension(
    dimension: DimExponents,
    exponent: i32,
) -> Option<DimExponents> {
    fn scale(value: i8, exponent: i32) -> Option<i8> {
        i32::from(value)
            .checked_mul(exponent)
            .and_then(|value| i8::try_from(value).ok())
    }
    Some(DimExponents {
        mass: scale(dimension.mass, exponent)?,
        length: scale(dimension.length, exponent)?,
        time: scale(dimension.time, exponent)?,
        current: scale(dimension.current, exponent)?,
        temperature: scale(dimension.temperature, exponent)?,
        amount: scale(dimension.amount, exponent)?,
        luminous_intensity: scale(dimension.luminous_intensity, exponent)?,
    })
}

pub(crate) fn integer_literal(expression: &Expr) -> Option<i32> {
    let value = match expression.kind() {
        ExprKind::Number(value) => *value,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => match value.kind() {
            ExprKind::Number(value) => -*value,
            _ => return None,
        },
        _ => return None,
    };
    (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

pub(crate) const fn time_dimension() -> DimExponents {
    DimExponents {
        time: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

pub(crate) const fn length_dimension() -> DimExponents {
    DimExponents {
        length: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

fn coherent_dimension(name: &str) -> Option<DimExponents> {
    let mut dimension = DimExponents::DIMENSIONLESS;
    match name {
        "kg" => dimension.mass = 1,
        "m" => dimension.length = 1,
        "s" => dimension.time = 1,
        "A" => dimension.current = 1,
        "K" => dimension.temperature = 1,
        "mol" => dimension.amount = 1,
        "cd" => dimension.luminous_intensity = 1,
        "Hz" => dimension.time = -1,
        "N" => {
            dimension.mass = 1;
            dimension.length = 1;
            dimension.time = -2;
        }
        "Pa" => {
            dimension.mass = 1;
            dimension.length = -1;
            dimension.time = -2;
        }
        "J" => {
            dimension.mass = 1;
            dimension.length = 2;
            dimension.time = -2;
        }
        "W" => {
            dimension.mass = 1;
            dimension.length = 2;
            dimension.time = -3;
        }
        _ => return None,
    }
    Some(dimension)
}

pub(crate) fn dimension_overflow(file: &str, range: TextRange) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        range,
        "physical-dimension exponent arithmetic overflows i8",
    )
}

#[cfg(test)]
mod tests {
    use eqiora_lang::parse;

    use super::lower_dimension;

    fn parameter_dimension(source: &str) -> eqiora_core::DimExponents {
        let source = format!("model M {{ parameter value: {source} = 1; }}");
        let document = parse("dimension.eqi", &source)
            .into_document()
            .expect("dimension source parses");
        let eqiora_lang::Item::Parameter(parameter) = &document.models()[0].items()[0] else {
            panic!("one Parameter declaration");
        };
        lower_dimension("dimension.eqi", parameter.dimension()).expect("dimension lowers")
    }

    #[test]
    fn coherent_aliases_equal_their_base_si_expansions() {
        for (alias, expanded) in [
            ("Hz", "1 / s"),
            ("N", "kg * m / s ^ 2"),
            ("Pa", "kg / (m * s ^ 2)"),
            ("J", "kg * m ^ 2 / s ^ 2"),
            ("W", "kg * m ^ 2 / s ^ 3"),
        ] {
            assert_eq!(parameter_dimension(alias), parameter_dimension(expanded));
        }
    }

    #[test]
    fn coherent_aliases_compile_across_model_declarations() {
        let source = r#"
model Catalog {
  parameter length: m = 2;
  parameter force: N = 3;
  parameter duration: s = 1;
  let energy: J = force * length;
  field power: W = 0;
  field pressure: Pa = 0;
  port frequency: signal input Hz;
  relation balance continuous {
    power = energy / duration;
    pressure = 0;
  }
}
"#;
        let mut compiled = crate::compile("catalog.eqi", source)
            .expect("coherent aliases compile through the shared dimension checker");
        let compiled = compiled.pop().expect("one Model");
        assert!(compiled.symbols().get("energy").is_none());
        assert!(compiled.symbols().get("power").is_some());
        assert!(compiled.symbols().get("pressure").is_some());
        assert!(compiled.symbols().get("frequency").is_some());
    }
}
