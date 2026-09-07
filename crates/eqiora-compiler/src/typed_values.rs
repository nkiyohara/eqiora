//! Component data operations after common mathematical type checking.

use eqiora_core::{ValueLiteral, ValueType};
use eqiora_lang::BinaryOp;

pub(crate) fn retype(value: &ValueLiteral, target: ValueType) -> Result<ValueLiteral, String> {
    if value.is_zero() {
        return ValueLiteral::from_real(target, 0.0).map_err(|error| error.to_string());
    }
    ValueLiteral::new(target, value.components()).map_err(|error| error.to_string())
}

pub(crate) fn negate(value: &ValueLiteral) -> Result<ValueLiteral, String> {
    ValueLiteral::new(
        value.value_type().clone(),
        value.components().map(|(r, i)| (-r, -i)),
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn binary(
    operator: BinaryOp,
    left: &ValueLiteral,
    right: &ValueLiteral,
    target: ValueType,
    exponent: Option<i32>,
) -> Result<ValueLiteral, String> {
    let count = target.shape().component_count().expect("checked type");
    let mut output = Vec::with_capacity(count);
    for index in 0..count {
        let a = left
            .component(if left.value_type().shape().is_scalar() {
                0
            } else {
                index
            })
            .ok_or("left component shape does not match checked arithmetic")?;
        let b = right
            .component(if right.value_type().shape().is_scalar() {
                0
            } else {
                index
            })
            .ok_or("right component shape does not match checked arithmetic")?;
        let component = match operator {
            BinaryOp::Add => (a.0 + b.0, a.1 + b.1),
            BinaryOp::Sub => (a.0 - b.0, a.1 - b.1),
            BinaryOp::Mul => multiply(a, b),
            BinaryOp::Div => divide(a, b)?,
            BinaryOp::Pow => power(
                a,
                exponent.ok_or("power requires a checked static exponent")?,
            )?,
        };
        output.push(component);
    }
    ValueLiteral::new(target, output).map_err(|error| error.to_string())
}

fn multiply(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
    (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
}

fn divide(a: (f64, f64), b: (f64, f64)) -> Result<(f64, f64), String> {
    if b == (0.0, 0.0) {
        return Err("division by zero in static value".into());
    }
    // Ratio form avoids squaring the denominator's components.
    if b.0.abs() >= b.1.abs() {
        let ratio = b.1 / b.0;
        let denominator = b.0 + b.1 * ratio;
        Ok((
            (a.0 + a.1 * ratio) / denominator,
            (a.1 - a.0 * ratio) / denominator,
        ))
    } else {
        let ratio = b.0 / b.1;
        let denominator = b.1 + b.0 * ratio;
        Ok((
            (a.0 * ratio + a.1) / denominator,
            (a.1 * ratio - a.0) / denominator,
        ))
    }
}

fn power(mut value: (f64, f64), exponent: i32) -> Result<(f64, f64), String> {
    if value.1 == 0.0 {
        return Ok((value.0.powi(exponent), 0.0));
    }
    if exponent < 0 {
        value = divide((1.0, 0.0), value)?;
    }
    let mut result = (1.0, 0.0);
    let mut remaining = exponent.unsigned_abs();
    while remaining != 0 {
        if remaining & 1 != 0 {
            result = multiply(result, value);
        }
        remaining >>= 1;
        if remaining != 0 {
            value = multiply(value, value);
        }
    }
    Ok(result)
}

pub(crate) fn check_type(value_type: &ValueType) -> Result<(), String> {
    eqiora_lang::ValueTypeSyntax::from_checked(value_type)
        .map(|_| ())
        .map_err(|error| error.to_string())
}
