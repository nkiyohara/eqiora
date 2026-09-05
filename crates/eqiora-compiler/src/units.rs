//! Closed input-unit catalog and the numerical conversion boundary.

use eqiora_core::{DimExponents, DynQuantity};
use eqiora_lang::{BinaryOp, Expr, ExprKind};

use crate::dimensions::rational_literal;

pub(crate) fn parameter_value(
    file: &str,
    declaration: &eqiora_lang::ParameterDecl,
) -> Result<f64, eqiora_core::Diagnostic> {
    let dimension = crate::dimensions::lower_dimension(file, declaration.dimension())?;
    let result = match declaration.value().kind() {
        ExprKind::Number(value) => normalize_value(*value, 1.0),
        ExprKind::Quantity { value, unit } => quantity(*value, unit).and_then(|quantity| {
            if quantity.dim() == dimension {
                Ok(quantity.value())
            } else {
                Err("parameter input unit does not match its declared dimension")
            }
        }),
        _ => Err("parameter value must be a numeric or quantity literal"),
    };
    result.map_err(|message| {
        crate::diagnostics::source_error(
            eqiora_core::diagnostic::codes::LANGUAGE_TYPE_ERROR,
            file,
            declaration.value().range(),
            message,
        )
    })
}

pub(crate) fn coherent_dimension(name: &str) -> Option<DimExponents> {
    let exponents = match name {
        "kg" => [1, 0, 0, 0, 0, 0, 0],
        "m" => [0, 1, 0, 0, 0, 0, 0],
        "s" => [0, 0, 1, 0, 0, 0, 0],
        "A" => [0, 0, 0, 1, 0, 0, 0],
        "K" => [0, 0, 0, 0, 1, 0, 0],
        "mol" => [0, 0, 0, 0, 0, 1, 0],
        "cd" => [0, 0, 0, 0, 0, 0, 1],
        "Hz" => [0, 0, -1, 0, 0, 0, 0],
        "N" => [1, 1, -2, 0, 0, 0, 0],
        "Pa" => [1, -1, -2, 0, 0, 0, 0],
        "J" => [1, 2, -2, 0, 0, 0, 0],
        "W" => [1, 2, -3, 0, 0, 0, 0],
        "C" => [0, 0, 1, 1, 0, 0, 0],
        "V" => [1, 2, -3, -1, 0, 0, 0],
        "Ohm" => [1, 2, -3, -2, 0, 0, 0],
        "S" => [-1, -2, 3, 2, 0, 0, 0],
        "F" => [-1, -2, 4, 2, 0, 0, 0],
        "H" => [1, 2, -2, -2, 0, 0, 0],
        "Wb" => [1, 2, -2, -1, 0, 0, 0],
        "T" => [1, 0, -2, -1, 0, 0, 0],
        _ => return None,
    };
    DimExponents::from_integers(exponents)
}

struct Unit {
    dimension: DimExponents,
    decimal_power: i32,
}

fn bare_unit(name: &str) -> Option<Unit> {
    if name == "g" {
        return Some(Unit {
            dimension: coherent_dimension("kg")?,
            decimal_power: -3,
        });
    }
    coherent_dimension(name).map(|dimension| Unit {
        dimension,
        decimal_power: 0,
    })
}

fn named_unit(name: &str) -> Option<Unit> {
    if let Some(unit) = bare_unit(name) {
        return Some(unit);
    }
    for (prefix, power) in [
        ("n", -9),
        ("u", -6),
        ("m", -3),
        ("k", 3),
        ("M", 6),
        ("G", 9),
    ] {
        if let Some(base) = name.strip_prefix(prefix).filter(|base| *base != "kg")
            && let Some(mut unit) = bare_unit(base)
        {
            unit.decimal_power += power;
            return Some(unit);
        }
    }
    None
}

fn lower_unit(expression: &Expr, depth: usize) -> Result<Unit, &'static str> {
    if depth > 256 {
        return Err("input-unit expression exceeds depth 256");
    }
    match expression.kind() {
        ExprKind::Number(value) if *value == 1.0 => Ok(Unit {
            dimension: DimExponents::DIMENSIONLESS,
            decimal_power: 0,
        }),
        ExprKind::Name(name) => named_unit(name).ok_or("unknown input-unit symbol"),
        ExprKind::Binary {
            op: BinaryOp::Pow,
            left,
            right,
        } => {
            let base = lower_unit(left, depth + 1)?;
            let (n, d) = rational_literal(right)
                .ok_or("input-unit exponent must be an exact bounded rational")?;
            let dimension = base
                .dimension
                .pow(n, d)
                .ok_or("input-unit dimension exceeds exponent bounds")?;
            let power = i64::from(base.decimal_power) * i64::from(n);
            if power % i64::from(d) != 0 {
                return Err("input-unit scale root is not an exact rational");
            }
            let decimal_power = i32::try_from(power / i64::from(d))
                .map_err(|_| "input-unit scale exceeds exponent bounds")?;
            Ok(Unit {
                dimension,
                decimal_power,
            })
        }
        ExprKind::Binary { op, left, right } if matches!(op, BinaryOp::Mul | BinaryOp::Div) => {
            let left = lower_unit(left, depth + 1)?;
            let right = lower_unit(right, depth + 1)?;
            let (dimension, power) = if *op == BinaryOp::Mul {
                (
                    left.dimension.mul(right.dimension),
                    left.decimal_power.checked_add(right.decimal_power),
                )
            } else {
                (
                    left.dimension.div(right.dimension),
                    left.decimal_power.checked_sub(right.decimal_power),
                )
            };
            Ok(Unit {
                dimension: dimension.ok_or("input-unit dimension exceeds exponent bounds")?,
                decimal_power: power.ok_or("input-unit scale exceeds exponent bounds")?,
            })
        }
        _ => Err("invalid input-unit expression"),
    }
}

pub(crate) fn quantity(value: f64, expression: &Expr) -> Result<DynQuantity, &'static str> {
    let unit = lower_unit(expression, 0)?;
    // All catalog scales are powers of ten. Compose them exactly above;
    // round the final scale to binary64 only at this numerical boundary.
    let scale = format!("1e{}", unit.decimal_power)
        .parse::<f64>()
        .map_err(|_| "input-unit scale cannot be represented")?;
    Ok(DynQuantity::new(
        normalize_value(value, scale)?,
        unit.dimension,
    ))
}

pub(crate) fn normalize_value(value: f64, scale: f64) -> Result<f64, &'static str> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err("input-unit scale is outside the finite binary64 range");
    }
    let normalized = value * scale;
    if !normalized.is_finite() {
        return Err("normalized quantity must be finite");
    }
    Ok(if normalized == 0.0 { 0.0 } else { normalized })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_lang::{Item, parse};

    fn unit(source: &str) -> Expr {
        let source = format!("model M {{ let value = 1 [{source}]; }}");
        let document = parse("unit.eqi", &source).into_document().unwrap();
        let Item::Let(binding) = &document.models()[0].items()[0] else {
            panic!("let")
        };
        let ExprKind::Quantity { unit, .. } = binding.value().kind() else {
            panic!("quantity")
        };
        unit.as_ref().clone()
    }

    #[test]
    fn catalog_scales_normalize_once_and_roots_remain_exact() {
        for (value, symbol, expected, dimension) in [
            (10.0, "ms", 0.01, "s"),
            (1.0, "kOhm", 1000.0, "Ohm"),
            (210.0, "GPa", 210_000_000_000.0, "Pa"),
            (1.0, "uF", 0.000001, "F"),
            (1.0, "mg", 0.000001, "kg"),
            (1.0, "(mm ^ 2) ^ (1 / 2)", 0.001, "m"),
        ] {
            let converted = quantity(value, &unit(symbol)).unwrap();
            assert_eq!(converted.value(), expected, "{symbol}");
            assert_eq!(converted.dim(), coherent_dimension(dimension).unwrap());
        }
        assert_eq!(quantity(1.0, &unit("km ^ (23 / 3)")).unwrap().value(), 1e23);
        let inverse_root_time = quantity(1.0, &unit("Hz ^ (-1 / 2)")).unwrap();
        assert_eq!(inverse_root_time.dim().exponents()[2], (1, 2));
    }

    #[test]
    fn source_quantities_reach_parameters_defaults_bindings_and_relations() {
        use eqiora_graph::Op;
        use eqiora_schema::kernel::KernelNode;

        let source = r#"
dimension Duration = s;
component Delay {
  public parameter duration: Duration = 10 [ms];
  relation balance continuous { duration - 0.01 [s] = 0; }
}
model Quantities {
  parameter duration: Duration = -10[ms];
  let ms: m = 3;
  let positive: Duration = 10 [ms];
  field elapsed: s = 0;
  relation balance continuous { elapsed - positive = 0; }
  instance defaulted: Delay();
  instance bound: Delay(duration = 10 [ms]);
}
"#;
        let compiled = crate::compile("quantities.eqi", source).unwrap();
        let values: Vec<_> = compiled[0]
            .transaction()
            .ops()
            .iter()
            .filter_map(|op| match op {
                Op::DefineKernelNode {
                    node: KernelNode::Parameter(parameter),
                } => Some(parameter.value()),
                _ => None,
            })
            .collect();
        assert!(values.iter().any(|value| value.value() == -0.01));
        for value in values {
            assert_eq!(value.dim(), coherent_dimension("s").unwrap());
            assert_eq!(value.value().abs(), 0.01);
        }
        for wrong in [
            source.replace("-10[ms]", "-10[m]"),
            source.replace("= 10 [ms];", "= 10 [m];"),
            source.replace("duration = 10 [ms]", "duration = 10 [m]"),
            source.replace("0.01 [s]", "0.01 [m]"),
            source.replace("10 [ms]", "10 [Duration]"),
        ] {
            assert!(crate::compile("wrong.eqi", &wrong).is_err(), "{wrong}");
        }
    }

    #[test]
    fn unit_errors_do_not_guess_symbols_or_approximate_scale_roots() {
        for symbol in [
            "mkg",
            "kkOhm",
            "KOhm",
            "Speed",
            "ms ^ (1 / 2)",
            "km ^ 2147483647",
        ] {
            assert!(quantity(1.0, &unit(symbol)).is_err(), "{symbol}");
        }
        assert!(quantity(f64::MAX, &unit("km")).is_err());
        assert!(
            parse("invalid-unit.eqi", "model M { let value = 1 [µF]; }")
                .into_document()
                .is_err()
        );
        assert!(quantity(1.0, &unit("nm ^ 100")).is_err());
        assert_eq!(quantity(-0.0, &unit("ms")).unwrap().value().to_bits(), 0);
    }
}
