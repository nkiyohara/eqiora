//! Method-neutral evaluation tape for scalar spatial data.

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, GraphPath, Id, RawId};
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef, UnaryMathFunction};
use eqiora_sem::KernelProgram;

/// Inspectable scalar expression over one physical coordinate space.
///
/// This is a lowered semantic object, not a Rust callback: distinct numerical
/// realizations can evaluate the same immutable tape without owning model
/// meaning. The tape contains only scalar arithmetic, indexed physical
/// coordinates, and dimensionally validated unary mathematics.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarSpatialExpression {
    coordinate_dimension: usize,
    instructions: Vec<Instruction>,
    root: usize,
    coordinate_dependent: bool,
    parameter_fields: Vec<Id<kinds::Parameter>>,
    parameter_values: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq)]
enum Instruction {
    Constant(f64),
    Parameter(usize),
    Coordinate(usize),
    Neg(usize),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
    Div(usize, usize),
    PowI(usize, i32),
    Sin(usize),
}

impl ScalarSpatialExpression {
    /// Exact physical coordinate dimension admitted by this tape.
    #[must_use]
    pub const fn coordinate_dimension(&self) -> usize {
        self.coordinate_dimension
    }

    /// Evaluate in coherent SI coordinates.
    ///
    /// # Errors
    /// Returns `EQ0702` for coordinate shape mismatch and `EQ0505` for a
    /// non-finite coordinate or intermediate value.
    pub fn evaluate(&self, coordinates: &[f64]) -> Result<f64, Diagnostic> {
        self.validate_coordinates(coordinates)?;
        let values = self.primal_values(coordinates)?;
        Ok(values[self.root])
    }

    fn primal_values(&self, coordinates: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        let mut values: Vec<f64> = Vec::with_capacity(self.instructions.len());
        for instruction in &self.instructions {
            let value = match *instruction {
                Instruction::Constant(value) => value,
                Instruction::Parameter(parameter) => self.parameter_values[parameter],
                Instruction::Coordinate(axis) => coordinates[axis],
                Instruction::Neg(value) => -values[value],
                Instruction::Add(left, right) => values[left] + values[right],
                Instruction::Sub(left, right) => values[left] - values[right],
                Instruction::Mul(left, right) => values[left] * values[right],
                Instruction::Div(left, right) => values[left] / values[right],
                Instruction::PowI(base, exponent) => f64::powi(values[base], exponent),
                Instruction::Sin(value) => f64::sin(values[value]),
            };
            if !value.is_finite() {
                return Err(nonfinite(
                    "scalar spatial expression produced a non-finite primal value",
                ));
            }
            values.push(value);
        }
        Ok(values)
    }

    /// Canonical Parameters retained by this tape, in dense tangent order.
    ///
    /// The order is deterministic first occurrence in the lowered expression.
    #[must_use]
    pub fn parameter_fields(&self) -> &[Id<kinds::Parameter>] {
        &self.parameter_fields
    }

    /// Complete Parameter values at this tape's evaluation point.
    #[must_use]
    pub fn parameter_values(&self) -> &[f64] {
        &self.parameter_values
    }

    /// Return the same immutable tape at one complete enclosing Parameter point.
    ///
    /// `parameter_fields` may contain Parameters used by sibling expressions,
    /// but it must contain every Parameter used by this tape exactly once.
    /// The returned expression owns its point; this expression remains
    /// unchanged.
    ///
    /// # Errors
    /// Returns a shape/identity diagnostic for mismatched, duplicate, or
    /// incomplete coordinates, and a non-finite diagnostic for invalid values.
    pub(crate) fn bind_parameter_point(
        &self,
        parameter_fields: &[Id<kinds::Parameter>],
        parameter_values: &[f64],
    ) -> Result<Self, Diagnostic> {
        if parameter_fields.len() != parameter_values.len() {
            return Err(input_mismatch(format!(
                "Parameter point contains {} identities and {} values",
                parameter_fields.len(),
                parameter_values.len()
            )));
        }
        if parameter_fields
            .iter()
            .enumerate()
            .any(|(index, field)| parameter_fields[..index].contains(field))
        {
            return Err(input_mismatch("Parameter point identities must be unique"));
        }
        if parameter_values.iter().any(|value| !value.is_finite()) {
            return Err(nonfinite("Parameter point contains a non-finite value"));
        }

        let bound_values = self
            .parameter_fields
            .iter()
            .map(|field| {
                parameter_fields
                    .iter()
                    .position(|candidate| candidate == field)
                    .map(|index| parameter_values[index])
                    .ok_or_else(|| {
                        input_mismatch(
                            "Parameter point is missing an identity required by the spatial expression",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut bound = self.clone();
        bound.parameter_values = bound_values;
        Ok(bound)
    }

    /// Evaluate the primal value and one Parameter JVP in a single tape pass.
    ///
    /// `parameter_tangent` uses [`Self::parameter_fields`] order. Coordinates
    /// are held fixed: spatial differentiation is a separate operator action.
    ///
    /// # Errors
    /// Returns `EQ0702` for a coordinate/tangent shape mismatch and `EQ0505`
    /// for non-finite input or an invalid/non-finite intermediate value.
    pub fn evaluate_parameter_jvp(
        &self,
        coordinates: &[f64],
        parameter_tangent: &[f64],
    ) -> Result<(f64, f64), Diagnostic> {
        self.evaluate_jvp(
            coordinates,
            &vec![0.0; self.coordinate_dimension],
            parameter_tangent,
        )
    }

    /// Evaluate the primal value and a combined coordinate/Parameter JVP.
    ///
    /// Coordinate tangents are physical mesh-motion velocities. Parameter
    /// tangents retain [`Self::parameter_fields`] order. Supplying both forms
    /// one total derivative of the same immutable expression tape.
    ///
    /// # Errors
    /// Returns `EQ0702` for a coordinate/tangent shape mismatch and `EQ0505`
    /// for non-finite input or an invalid/non-finite intermediate value.
    pub fn evaluate_jvp(
        &self,
        coordinates: &[f64],
        coordinate_tangent: &[f64],
        parameter_tangent: &[f64],
    ) -> Result<(f64, f64), Diagnostic> {
        self.validate_coordinates(coordinates)?;
        if coordinate_tangent.len() != self.coordinate_dimension
            || parameter_tangent.len() != self.parameter_fields.len()
        {
            return Err(input_mismatch(format!(
                "spatial expression expects {}/{} coordinate/Parameter tangents, received {}/{}",
                self.coordinate_dimension,
                self.parameter_fields.len(),
                coordinate_tangent.len(),
                parameter_tangent.len()
            )));
        }
        if coordinate_tangent
            .iter()
            .chain(parameter_tangent)
            .any(|value| !value.is_finite())
        {
            return Err(nonfinite(
                "spatial coordinate or Parameter tangent is non-finite",
            ));
        }
        let mut values: Vec<f64> = Vec::with_capacity(self.instructions.len());
        let mut tangents: Vec<f64> = Vec::with_capacity(self.instructions.len());
        for instruction in &self.instructions {
            let (value, tangent) = match *instruction {
                Instruction::Constant(value) => (value, 0.0),
                Instruction::Parameter(parameter) => (
                    self.parameter_values[parameter],
                    parameter_tangent[parameter],
                ),
                Instruction::Coordinate(axis) => (coordinates[axis], coordinate_tangent[axis]),
                Instruction::Neg(value) => (-values[value], -tangents[value]),
                Instruction::Add(left, right) => (
                    values[left] + values[right],
                    tangents[left] + tangents[right],
                ),
                Instruction::Sub(left, right) => (
                    values[left] - values[right],
                    tangents[left] - tangents[right],
                ),
                Instruction::Mul(left, right) => (
                    values[left] * values[right],
                    tangents[left] * values[right] + values[left] * tangents[right],
                ),
                Instruction::Div(left, right) => (
                    values[left] / values[right],
                    (tangents[left] * values[right] - values[left] * tangents[right])
                        / values[right].powi(2),
                ),
                Instruction::PowI(base, exponent) => {
                    let value = f64::powi(values[base], exponent);
                    let tangent = if exponent == 0 {
                        0.0
                    } else {
                        f64::from(exponent) * f64::powi(values[base], exponent - 1) * tangents[base]
                    };
                    (value, tangent)
                }
                Instruction::Sin(value) => (
                    f64::sin(values[value]),
                    f64::cos(values[value]) * tangents[value],
                ),
            };
            if !value.is_finite() || !tangent.is_finite() {
                return Err(nonfinite(
                    "scalar spatial expression produced a non-finite primal or tangent value",
                ));
            }
            values.push(value);
            tangents.push(tangent);
        }
        Ok((values[self.root], tangents[self.root]))
    }

    /// Evaluate the primal value and one Parameter VJP in a reverse tape pass.
    ///
    /// The returned cotangent uses [`Self::parameter_fields`] order.
    /// Coordinates are held fixed: use [`Self::evaluate_vjp`] when coordinate
    /// cotangents are also required.
    ///
    /// # Errors
    /// Returns `EQ0702` for coordinate shape mismatch and `EQ0505` for a
    /// non-finite coordinate, cotangent, or intermediate value.
    pub fn evaluate_parameter_vjp(
        &self,
        coordinates: &[f64],
        output_cotangent: f64,
    ) -> Result<(f64, Vec<f64>), Diagnostic> {
        let (primal, _, parameter_cotangent) = self.evaluate_vjp(coordinates, output_cotangent)?;
        Ok((primal, parameter_cotangent))
    }

    /// Evaluate the primal value and coordinate/Parameter VJPs.
    ///
    /// The first returned cotangent uses physical coordinate-axis order; the
    /// second uses [`Self::parameter_fields`] order. Both are pullbacks of the
    /// same immutable expression tape at its bound Parameter values.
    ///
    /// # Errors
    /// Returns `EQ0702` for coordinate shape mismatch and `EQ0505` for a
    /// non-finite coordinate, cotangent, or intermediate value.
    pub fn evaluate_vjp(
        &self,
        coordinates: &[f64],
        output_cotangent: f64,
    ) -> Result<(f64, Vec<f64>, Vec<f64>), Diagnostic> {
        self.validate_coordinates(coordinates)?;
        if !output_cotangent.is_finite() {
            return Err(nonfinite(
                "spatial expression output cotangent is non-finite",
            ));
        }

        let values = self.primal_values(coordinates)?;
        let mut adjoints = vec![0.0; self.instructions.len()];
        adjoints[self.root] = output_cotangent;
        let mut coordinate_cotangent = vec![0.0; self.coordinate_dimension];
        let mut parameter_cotangent = vec![0.0; self.parameter_fields.len()];

        for index in (0..self.instructions.len()).rev() {
            let adjoint = adjoints[index];
            match self.instructions[index] {
                Instruction::Constant(_) => {}
                Instruction::Parameter(parameter) => {
                    parameter_cotangent[parameter] += adjoint;
                }
                Instruction::Coordinate(axis) => {
                    coordinate_cotangent[axis] += adjoint;
                }
                Instruction::Neg(value) => adjoints[value] -= adjoint,
                Instruction::Add(left, right) => {
                    adjoints[left] += adjoint;
                    adjoints[right] += adjoint;
                }
                Instruction::Sub(left, right) => {
                    adjoints[left] += adjoint;
                    adjoints[right] -= adjoint;
                }
                Instruction::Mul(left, right) => {
                    adjoints[left] += adjoint * values[right];
                    adjoints[right] += adjoint * values[left];
                }
                Instruction::Div(left, right) => {
                    adjoints[left] += adjoint / values[right];
                    adjoints[right] -= adjoint * values[left] / values[right].powi(2);
                }
                Instruction::PowI(base, exponent) if exponent != 0 => {
                    adjoints[base] +=
                        adjoint * f64::from(exponent) * f64::powi(values[base], exponent - 1);
                }
                Instruction::PowI(_, _) => {}
                Instruction::Sin(value) => {
                    adjoints[value] += adjoint * f64::cos(values[value]);
                }
            }
        }

        if adjoints
            .iter()
            .chain(&coordinate_cotangent)
            .chain(&parameter_cotangent)
            .any(|value| !value.is_finite())
        {
            return Err(nonfinite(
                "scalar spatial expression produced a non-finite cotangent",
            ));
        }

        Ok((values[self.root], coordinate_cotangent, parameter_cotangent))
    }

    fn validate_coordinates(&self, coordinates: &[f64]) -> Result<(), Diagnostic> {
        if coordinates.len() != self.coordinate_dimension {
            return Err(input_mismatch(format!(
                "spatial expression expects {} coordinates, received {}",
                self.coordinate_dimension,
                coordinates.len()
            )));
        }
        if coordinates.iter().any(|coordinate| !coordinate.is_finite()) {
            return Err(nonfinite("spatial coordinate is non-finite"));
        }
        Ok(())
    }

    /// Whether evaluation depends on the physical coordinate.
    #[must_use]
    pub const fn is_coordinate_dependent(&self) -> bool {
        self.coordinate_dependent
    }

    /// Evaluate a spatially constant tape once.
    #[must_use]
    pub fn constant_value(&self) -> Option<f64> {
        (!self.coordinate_dependent)
            .then(|| self.evaluate(&vec![0.0; self.coordinate_dimension]).ok())
            .flatten()
    }

    /// Whether two coefficients retain the same exact lowered relation.
    ///
    /// Equality includes the expression tape, canonical Parameter identities,
    /// and their revision-local values. It intentionally does not infer
    /// equality from coincident numerical values; algebraic equivalence is a
    /// separate compiler concern.
    #[must_use]
    pub(crate) fn is_same_coefficient_as(&self, other: &Self) -> bool {
        self == other
    }

    /// Exact affine coordinate gradient when the lowered tape is affine.
    ///
    /// Parameter values are already point-bound constants. Returning
    /// `None` is a structural rejection, not a sample-based linearity guess.
    pub(crate) fn affine_gradient(&self) -> Option<Vec<f64>> {
        let zero = || vec![0.0; self.coordinate_dimension];
        let mut forms: Vec<(f64, Vec<f64>)> = Vec::with_capacity(self.instructions.len());
        for instruction in &self.instructions {
            let form = match *instruction {
                Instruction::Constant(value) => (value, zero()),
                Instruction::Parameter(parameter) => (self.parameter_values[parameter], zero()),
                Instruction::Coordinate(axis) => {
                    let mut gradient = zero();
                    gradient[axis] = 1.0;
                    (0.0, gradient)
                }
                Instruction::Neg(value) => scale_affine(&forms[value], -1.0),
                Instruction::Add(left, right) => add_affine(&forms[left], &forms[right], 1.0),
                Instruction::Sub(left, right) => add_affine(&forms[left], &forms[right], -1.0),
                Instruction::Mul(left, right) => {
                    let left_spatial = affine_is_spatial(&forms[left]);
                    let right_spatial = affine_is_spatial(&forms[right]);
                    match (left_spatial, right_spatial) {
                        (true, true) => return None,
                        (true, false) => scale_affine(&forms[left], forms[right].0),
                        (false, true) => scale_affine(&forms[right], forms[left].0),
                        (false, false) => (forms[left].0 * forms[right].0, zero()),
                    }
                }
                Instruction::Div(left, right)
                    if !affine_is_spatial(&forms[right]) && forms[right].0 != 0.0 =>
                {
                    scale_affine(&forms[left], forms[right].0.recip())
                }
                Instruction::PowI(_, 0) => (1.0, zero()),
                Instruction::PowI(base, 1) => forms[base].clone(),
                Instruction::PowI(base, exponent) if !affine_is_spatial(&forms[base]) => {
                    (forms[base].0.powi(exponent), zero())
                }
                Instruction::Sin(value) if !affine_is_spatial(&forms[value]) => {
                    (forms[value].0.sin(), zero())
                }
                _ => return None,
            };
            if !form.0.is_finite() || form.1.iter().any(|value| !value.is_finite()) {
                return None;
            }
            forms.push(form);
        }
        forms.get(self.root).map(|(_, gradient)| gradient.clone())
    }

    pub(crate) fn constant(coordinate_dimension: usize, value: f64) -> Self {
        Self {
            coordinate_dimension,
            instructions: vec![Instruction::Constant(value)],
            root: 0,
            coordinate_dependent: false,
            parameter_fields: Vec::new(),
            parameter_values: Vec::new(),
        }
    }

    pub(crate) fn multiply(self, right: Self) -> Self {
        assert_eq!(self.coordinate_dimension, right.coordinate_dimension);
        let mut parameter_fields = self.parameter_fields.clone();
        let mut parameter_values = self.parameter_values.clone();
        let right_parameter_remap = right
            .parameter_fields
            .iter()
            .zip(&right.parameter_values)
            .map(|(field, value)| {
                parameter_fields
                    .iter()
                    .position(|existing| existing == field)
                    .unwrap_or_else(|| {
                        let index = parameter_fields.len();
                        parameter_fields.push(*field);
                        parameter_values.push(*value);
                        index
                    })
            })
            .collect::<Vec<_>>();
        let right_offset = self.instructions.len();
        let mut instructions = self.instructions;
        instructions.extend(right.instructions.into_iter().map(|instruction| {
            remap_instruction(instruction, right_offset, &right_parameter_remap)
        }));
        let root = instructions.len();
        instructions.push(Instruction::Mul(self.root, right_offset + right.root));
        Self {
            coordinate_dimension: self.coordinate_dimension,
            instructions,
            root,
            coordinate_dependent: self.coordinate_dependent || right.coordinate_dependent,
            parameter_fields,
            parameter_values,
        }
    }
}

fn affine_is_spatial((_, gradient): &(f64, Vec<f64>)) -> bool {
    gradient.iter().any(|value| *value != 0.0)
}

fn scale_affine((constant, gradient): &(f64, Vec<f64>), scale: f64) -> (f64, Vec<f64>) {
    (
        scale * constant,
        gradient.iter().map(|value| scale * value).collect(),
    )
}

fn add_affine(
    (left_constant, left_gradient): &(f64, Vec<f64>),
    (right_constant, right_gradient): &(f64, Vec<f64>),
    right_scale: f64,
) -> (f64, Vec<f64>) {
    (
        left_constant + right_scale * right_constant,
        left_gradient
            .iter()
            .zip(right_gradient)
            .map(|(left, right)| left + right_scale * right)
            .collect(),
    )
}

pub(crate) fn lower(
    program: &KernelProgram,
    expression: &ExprDag,
    root: ExprId,
    owner: RawId,
    coordinate_dimension: usize,
) -> Result<ScalarSpatialExpression, Diagnostic> {
    if coordinate_dimension == 0 {
        return Err(invalid(
            owner,
            "scalar spatial lowering requires a positive coordinate dimension",
        ));
    }
    let required = required_nodes(expression, root, owner)?;
    let mut remap = vec![None; expression.nodes().len()];
    let mut instructions = Vec::new();
    let mut coordinate_dependent = false;
    let mut parameter_fields = Vec::new();
    let mut parameter_values = Vec::new();

    for (index, node) in expression.nodes().iter().enumerate() {
        if !required[index] {
            continue;
        }
        let instruction = match node {
            ExprNode::Constant(value) => Instruction::Constant(value.value()),
            ExprNode::Symbol(SymbolRef::Parameter(parameter)) => {
                let value = program
                    .value(parameter.erase())
                    .map(|value| value.value())
                    .ok_or_else(|| invalid(owner, "Parameter has no revision-local value"))?;
                let parameter = parameter_fields
                    .iter()
                    .position(|existing| existing == parameter)
                    .unwrap_or_else(|| {
                        let index = parameter_fields.len();
                        parameter_fields.push(*parameter);
                        parameter_values.push(value);
                        index
                    });
                Instruction::Parameter(parameter)
            }
            ExprNode::SpatialCoordinate(axis) if *axis < coordinate_dimension => {
                coordinate_dependent = true;
                Instruction::Coordinate(*axis)
            }
            ExprNode::SpatialCoordinate(axis) => {
                return Err(invalid(
                    owner,
                    format!(
                        "coordinate axis {axis} is outside spatial dimension {coordinate_dimension}"
                    ),
                ));
            }
            ExprNode::Neg(value) => Instruction::Neg(remapped(&remap, *value, owner)?),
            ExprNode::Add(left, right) => Instruction::Add(
                remapped(&remap, *left, owner)?,
                remapped(&remap, *right, owner)?,
            ),
            ExprNode::Sub(left, right) => Instruction::Sub(
                remapped(&remap, *left, owner)?,
                remapped(&remap, *right, owner)?,
            ),
            ExprNode::Mul(left, right) => Instruction::Mul(
                remapped(&remap, *left, owner)?,
                remapped(&remap, *right, owner)?,
            ),
            ExprNode::Div(left, right) => Instruction::Div(
                remapped(&remap, *left, owner)?,
                remapped(&remap, *right, owner)?,
            ),
            ExprNode::PowI(base, exponent) => {
                Instruction::PowI(remapped(&remap, *base, owner)?, *exponent)
            }
            ExprNode::UnaryMath(UnaryMathFunction::Sin, value) => {
                Instruction::Sin(remapped(&remap, *value, owner)?)
            }
            _ => {
                return Err(invalid(
                    owner,
                    "source must use constants, Parameters, in-domain coordinates, scalar arithmetic, and supported unary mathematics",
                ));
            }
        };
        let lowered = instructions.len();
        instructions.push(instruction);
        remap[index] = Some(lowered);
    }

    Ok(ScalarSpatialExpression {
        coordinate_dimension,
        instructions,
        root: remapped(&remap, root, owner)?,
        coordinate_dependent,
        parameter_fields,
        parameter_values,
    })
}

fn remap_instruction(
    instruction: Instruction,
    node_offset: usize,
    parameter_remap: &[usize],
) -> Instruction {
    match instruction {
        Instruction::Constant(value) => Instruction::Constant(value),
        Instruction::Parameter(parameter) => Instruction::Parameter(parameter_remap[parameter]),
        Instruction::Coordinate(axis) => Instruction::Coordinate(axis),
        Instruction::Neg(value) => Instruction::Neg(node_offset + value),
        Instruction::Add(left, right) => Instruction::Add(node_offset + left, node_offset + right),
        Instruction::Sub(left, right) => Instruction::Sub(node_offset + left, node_offset + right),
        Instruction::Mul(left, right) => Instruction::Mul(node_offset + left, node_offset + right),
        Instruction::Div(left, right) => Instruction::Div(node_offset + left, node_offset + right),
        Instruction::PowI(base, exponent) => Instruction::PowI(node_offset + base, exponent),
        Instruction::Sin(value) => Instruction::Sin(node_offset + value),
    }
}

fn required_nodes(
    expression: &ExprDag,
    root: ExprId,
    owner: RawId,
) -> Result<Vec<bool>, Diagnostic> {
    let mut required = vec![false; expression.nodes().len()];
    let mut pending = vec![root];
    while let Some(value) = pending.pop() {
        let index = usize::try_from(value.index())
            .map_err(|_| invalid(owner, "source expression index exceeds usize"))?;
        let Some(slot) = required.get_mut(index) else {
            return Err(invalid(
                owner,
                "source expression references a missing node",
            ));
        };
        if *slot {
            continue;
        }
        *slot = true;
        let node = expression
            .node(value)
            .ok_or_else(|| invalid(owner, "source expression node is missing"))?;
        match node {
            ExprNode::Neg(value) | ExprNode::PowI(value, _) | ExprNode::UnaryMath(_, value) => {
                pending.push(*value);
            }
            ExprNode::Add(left, right)
            | ExprNode::Sub(left, right)
            | ExprNode::Mul(left, right)
            | ExprNode::Div(left, right) => {
                pending.push(*left);
                pending.push(*right);
            }
            _ => {}
        }
    }
    Ok(required)
}

fn remapped(remap: &[Option<usize>], value: ExprId, owner: RawId) -> Result<usize, Diagnostic> {
    usize::try_from(value.index())
        .ok()
        .and_then(|index| remap.get(index).copied().flatten())
        .ok_or_else(|| invalid(owner, "source expression is not topologically ordered"))
}

fn invalid(owner: RawId, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_SPATIAL_LOWERING, message).with_graph_path(GraphPath::new([
        owner.kind().graph().name().to_owned(),
        format!("{:?}", owner.kind()),
        owner.to_string(),
    ]))
}

fn nonfinite(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NONFINITE_EVALUATION, message)
}

fn input_mismatch(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::OPERATOR_INPUT_MISMATCH, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_compiler::compile;
    use eqiora_graph::{GraphStore, InMemoryGraphStore};
    use eqiora_schema::kernel::KernelNode;

    #[test]
    fn evaluates_distinct_axes_and_requires_exact_coordinate_shape() {
        let expression = ScalarSpatialExpression {
            coordinate_dimension: 2,
            instructions: vec![
                Instruction::Coordinate(0),
                Instruction::Coordinate(1),
                Instruction::Mul(1, 1),
                Instruction::Add(0, 2),
            ],
            root: 3,
            coordinate_dependent: true,
            parameter_fields: Vec::new(),
            parameter_values: Vec::new(),
        };

        assert_eq!(expression.coordinate_dimension(), 2);
        assert_eq!(expression.evaluate(&[2.0, 3.0]).unwrap(), 11.0);
        assert_eq!(
            expression.evaluate(&[2.0]).unwrap_err().code(),
            codes::OPERATOR_INPUT_MISMATCH
        );
        assert_eq!(
            expression.evaluate(&[2.0, f64::NAN]).unwrap_err().code(),
            codes::NONFINITE_EVALUATION
        );
    }

    #[test]
    fn constant_tape_retains_coordinate_dimension() {
        let expression = ScalarSpatialExpression::constant(3, 4.5);
        assert_eq!(expression.coordinate_dimension(), 3);
        assert_eq!(expression.constant_value(), Some(4.5));
        assert_eq!(expression.evaluate(&[1.0, 2.0, 3.0]).unwrap(), 4.5);
    }

    #[test]
    fn lowers_both_axes_from_one_canonical_plane_relation() {
        let source = r#"
model plane_source {
  domain plane = box(0, 2, 0, 3);
  representation space = continuum;
  field u on plane as space: m = 0;
  relation identity continuous on plane {
    u - (coordinate(0) + coordinate(1)) = 0;
  }
}
"#;
        let mut compiled = compile("plane-source.eqi", source).unwrap();
        let (transaction, model, _) = compiled.remove(0).into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
        let relation = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Relation(relation) => Some(relation),
                _ => None,
            })
            .unwrap();
        let root = relation.residuals().roots()[0];
        let source_root = match relation.residuals().node(root) {
            Some(ExprNode::Sub(_, source)) => *source,
            _ => panic!("fixture has one field-minus-source residual"),
        };
        let lowered = lower(
            &program,
            relation.residuals(),
            source_root,
            relation.id().erase(),
            2,
        )
        .unwrap();

        assert_eq!(lowered.coordinate_dimension(), 2);
        assert_eq!(lowered.evaluate(&[2.0, 3.0]).unwrap(), 5.0);
    }

    #[test]
    fn retains_parameter_identity_and_evaluates_analytic_jvp() {
        let source = r#"
model parameterized_source {
  domain interval = box(0, 2);
  representation space = continuum;
  field u on interval as space: m ^ 2 = 0;
  parameter amplitude: m = 3;
  relation identity continuous on interval {
    u - amplitude ^ 2 * math.sin(coordinate(0) / amplitude) = 0;
  }
}
"#;
        let mut compiled = compile("parameterized-source.eqi", source).unwrap();
        let amplitude = compiled[0]
            .symbols()
            .get("amplitude")
            .unwrap()
            .downcast::<kinds::Parameter>()
            .unwrap();
        let (transaction, model, _) = compiled.remove(0).into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
        let relation = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Relation(relation) => Some(relation),
                _ => None,
            })
            .unwrap();
        let root = relation.residuals().roots()[0];
        let source_root = match relation.residuals().node(root) {
            Some(ExprNode::Sub(_, source)) => *source,
            _ => panic!("fixture has one field-minus-source residual"),
        };
        let lowered = lower(
            &program,
            relation.residuals(),
            source_root,
            relation.id().erase(),
            1,
        )
        .unwrap();

        let coordinate = 0.4_f64;
        let (value, tangent) = lowered
            .evaluate_parameter_jvp(&[coordinate], &[2.0])
            .unwrap();
        let (_, combined_tangent) = lowered
            .evaluate_jvp(&[coordinate], &[0.25], &[2.0])
            .unwrap();
        let output_cotangent = 1.75;
        let (vjp_value, coordinate_cotangent, parameter_cotangent) = lowered
            .evaluate_vjp(&[coordinate], output_cotangent)
            .unwrap();
        let expected_value = 9.0 * (coordinate / 3.0).sin();
        let expected_tangent =
            2.0 * (6.0 * (coordinate / 3.0).sin() - coordinate * (coordinate / 3.0).cos());
        let expected_coordinate_derivative = 3.0 * (coordinate / 3.0).cos();
        let expected_parameter_derivative =
            6.0 * (coordinate / 3.0).sin() - coordinate * (coordinate / 3.0).cos();
        assert_eq!(lowered.parameter_fields(), &[amplitude]);
        assert_eq!(lowered.parameter_values(), &[3.0]);
        assert!((value - expected_value).abs() < 1.0e-14);
        assert!((vjp_value - expected_value).abs() < 1.0e-14);
        assert!((tangent - expected_tangent).abs() < 1.0e-14);
        assert!(
            (combined_tangent - (expected_tangent + 0.25 * expected_coordinate_derivative)).abs()
                < 1.0e-14
        );
        assert!(
            (coordinate_cotangent[0] - output_cotangent * expected_coordinate_derivative).abs()
                < 1.0e-14
        );
        assert!(
            (parameter_cotangent[0] - output_cotangent * expected_parameter_derivative).abs()
                < 1.0e-14
        );
        let jvp_pairing = output_cotangent * combined_tangent;
        let vjp_pairing = 0.25 * coordinate_cotangent[0] + 2.0 * parameter_cotangent[0];
        assert!((jvp_pairing - vjp_pairing).abs() < 1.0e-14);
    }

    #[test]
    fn product_deduplicates_parameter_coordinates() {
        let parameter = Id::<kinds::Parameter>::new();
        let left = ScalarSpatialExpression {
            coordinate_dimension: 1,
            instructions: vec![Instruction::Parameter(0)],
            root: 0,
            coordinate_dependent: false,
            parameter_fields: vec![parameter],
            parameter_values: vec![2.0],
        };
        let right = ScalarSpatialExpression {
            coordinate_dimension: 1,
            instructions: vec![
                Instruction::Parameter(0),
                Instruction::Coordinate(0),
                Instruction::Add(0, 1),
            ],
            root: 2,
            coordinate_dependent: true,
            parameter_fields: vec![parameter],
            parameter_values: vec![2.0],
        };
        let product = left.multiply(right);

        assert_eq!(product.parameter_fields(), &[parameter]);
        assert_eq!(
            product.evaluate_parameter_jvp(&[3.0], &[1.0]).unwrap(),
            (10.0, 7.0)
        );
    }

    #[test]
    fn coefficient_equality_is_identity_aware_not_value_inferred() {
        let first = Id::<kinds::Parameter>::new();
        let second = Id::<kinds::Parameter>::new();
        let coefficient = |parameter| ScalarSpatialExpression {
            coordinate_dimension: 2,
            instructions: vec![Instruction::Parameter(0)],
            root: 0,
            coordinate_dependent: false,
            parameter_fields: vec![parameter],
            parameter_values: vec![3.0],
        };

        let volume = coefficient(first);
        assert!(volume.is_same_coefficient_as(&volume.clone()));
        assert!(
            !volume.is_same_coefficient_as(&coefficient(second)),
            "equal revision-local values do not merge independent Parameters"
        );
        assert!(
            ScalarSpatialExpression::constant(2, 3.0)
                .is_same_coefficient_as(&ScalarSpatialExpression::constant(2, 3.0))
        );
    }
}
