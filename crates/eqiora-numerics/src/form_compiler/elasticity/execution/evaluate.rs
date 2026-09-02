//! Primal and differential evaluation of a validated local-form tape.

use eqiora_core::Diagnostic;

use super::{INPUT_COUNT, LocalFormInput2d, LocalFormInstruction, LocalFormProgram2d, tape_error};

pub(super) fn closed_inputs() -> &'static [LocalFormInput2d; INPUT_COUNT] {
    &[
        LocalFormInput2d::Material(0),
        LocalFormInput2d::Material(1),
        LocalFormInput2d::StateJet {
            component: 0,
            axis: 0,
        },
        LocalFormInput2d::StateJet {
            component: 0,
            axis: 1,
        },
        LocalFormInput2d::StateJet {
            component: 1,
            axis: 0,
        },
        LocalFormInput2d::StateJet {
            component: 1,
            axis: 1,
        },
        LocalFormInput2d::ShapeGradient(0),
        LocalFormInput2d::ShapeGradient(1),
        LocalFormInput2d::ShapeValue,
        LocalFormInput2d::BodyForce(0),
        LocalFormInput2d::BodyForce(1),
        LocalFormInput2d::QuadratureScale,
    ]
}

pub(super) fn instruction_value(
    instruction: LocalFormInstruction,
    inputs: &[f64],
    values: &[f64],
) -> f64 {
    match instruction {
        LocalFormInstruction::Read(input) => inputs[usize::from(input)],
        LocalFormInstruction::ConstantBits(bits) => f64::from_bits(bits),
        LocalFormInstruction::Neg(value) => -values[usize::from(value)],
        LocalFormInstruction::Add(left, right) => {
            values[usize::from(left)] + values[usize::from(right)]
        }
        LocalFormInstruction::Mul(left, right) => {
            values[usize::from(left)] * values[usize::from(right)]
        }
    }
}

pub(super) fn instruction_tangent(
    instruction: LocalFormInstruction,
    input_tangent: &[f64],
    values: &[f64],
    tangents: &[f64],
) -> f64 {
    match instruction {
        LocalFormInstruction::Read(input) => input_tangent[usize::from(input)],
        LocalFormInstruction::ConstantBits(_) => 0.0,
        LocalFormInstruction::Neg(value) => -tangents[usize::from(value)],
        LocalFormInstruction::Add(left, right) => {
            tangents[usize::from(left)] + tangents[usize::from(right)]
        }
        LocalFormInstruction::Mul(left, right) => {
            tangents[usize::from(left)] * values[usize::from(right)]
                + values[usize::from(left)] * tangents[usize::from(right)]
        }
    }
}

pub(super) fn reverse(
    program: &LocalFormProgram2d,
    values: &[f64],
    root_cotangent: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let mut adjoints = vec![0.0; program.instructions.len()];
    for (root, cotangent) in program.roots.iter().zip(root_cotangent) {
        adjoints[usize::from(*root)] += cotangent;
    }
    let mut inputs = vec![0.0; INPUT_COUNT];
    for index in (0..program.instructions.len()).rev() {
        let adjoint = adjoints[index];
        match program.instructions[index] {
            LocalFormInstruction::Read(input) => inputs[usize::from(input)] += adjoint,
            LocalFormInstruction::ConstantBits(_) => {}
            LocalFormInstruction::Neg(value) => adjoints[usize::from(value)] -= adjoint,
            LocalFormInstruction::Add(left, right) => {
                adjoints[usize::from(left)] += adjoint;
                adjoints[usize::from(right)] += adjoint;
            }
            LocalFormInstruction::Mul(left, right) => {
                adjoints[usize::from(left)] += adjoint * values[usize::from(right)];
                adjoints[usize::from(right)] += adjoint * values[usize::from(left)];
            }
        }
        if !adjoints[index].is_finite() || inputs.iter().any(|value| !value.is_finite()) {
            return Err(tape_error("local-form reverse intermediate is non-finite"));
        }
    }
    Ok(inputs)
}
