mod instruction;
mod lower;
use instruction::{Instruction, ValueId};
mod numerical_evaluation;
mod typed;
use numerical_evaluation::evaluate_instructions;

use std::collections::HashMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_schema::kernel::SymbolRef;

use crate::ScalarSymbolCoordinate;
use crate::{DifferentiationRole, LinearizedRelation, RelationCotangent, RelationTangent};

/// One dense IR-local input slot for component scalarization.
///
/// A slot is local plumbing, not Semantic Model meaning. Its ordinal is dense
/// within one scalar program and its source retains the exact real Field,
/// Parameter, derivative, or Port coordinate. The type deliberately cannot be
/// converted to or compared with [`SymbolRef`].
///
/// ```compile_fail
/// # use eqiora_ir::ScalarInputSlot;
/// # use eqiora_schema::kernel::SymbolRef;
/// # fn cannot_confuse(slot: ScalarInputSlot) {
/// let semantic_parameter: SymbolRef = slot;
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScalarInputSlot {
    ordinal: u32,
    source: ScalarSymbolCoordinate,
}

impl ScalarInputSlot {
    pub(crate) const fn new(ordinal: u32, source: ScalarSymbolCoordinate) -> Self {
        Self { ordinal, source }
    }

    /// Dense program-local ordinal.
    #[must_use]
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Exact Semantic symbol/component coordinate read by this local slot.
    #[must_use]
    pub const fn source(&self) -> &ScalarSymbolCoordinate {
        &self.source
    }
}

/// Scalar SSA program whose reads are typed IR-local slots.
///
/// This is the component-scalarization counterpart of [`ScalarOperatorIr`].
/// It shares the same ordered evaluator but has no semantic-symbol API and
/// cannot expose a fabricated Parameter identity.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarInputOperatorIr {
    slots: Vec<ScalarInputSlot>,
    instructions: Vec<Instruction>,
    roots: Vec<ValueId>,
}

impl ScalarInputOperatorIr {
    /// Dense local slots in exact read order.
    #[must_use]
    pub fn slots(&self) -> &[ScalarInputSlot] {
        &self.slots
    }

    /// Number of ordered scalar instructions.
    #[must_use]
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Evaluate roots from values in [`Self::slots`] order.
    ///
    /// # Errors
    /// Returns `EQ0702` for wrong cardinality and `EQ0505` for non-finite
    /// arithmetic, exactly as [`ScalarOperatorIr::evaluate`].
    pub fn evaluate(&self, inputs: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        if inputs.len() != self.slots.len() {
            return Err(Diagnostic::error(
                codes::OPERATOR_INPUT_MISMATCH,
                format!(
                    "scalar input-slot IR expects {} inputs, received {}",
                    self.slots.len(),
                    inputs.len()
                ),
            )
            .with_graph_path(GraphPath::new(["operator-ir", "inputs"])));
        }
        let values = evaluate_instructions(&self.instructions, inputs)?;
        collect_roots(&self.roots, &self.instructions, &values)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ScalarInputValueId(ValueId);

pub(crate) struct ScalarInputIrBuilder {
    slots: Vec<ScalarInputSlot>,
    slot_indices: HashMap<ScalarInputSlot, SymbolSlot>,
    instructions: Vec<Instruction>,
}

impl ScalarInputIrBuilder {
    pub(crate) fn new() -> Self {
        Self {
            slots: Vec::new(),
            slot_indices: HashMap::new(),
            instructions: Vec::new(),
        }
    }

    pub(crate) fn constant(
        &mut self,
        value: eqiora_core::DynQuantity,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        if !value.value().is_finite() {
            return Err(ir_builder_error("input-slot constant must be finite"));
        }
        self.push(Instruction::Constant(value))
    }

    pub(crate) fn input(
        &mut self,
        slot: ScalarInputSlot,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        let next = u32::try_from(self.slots.len()).map_err(|_| ir_size_error())?;
        let index = if let Some(index) = self.slot_indices.get(&slot).copied() {
            index
        } else {
            if slot.ordinal != next {
                return Err(ir_builder_error(
                    "input-slot ordinals must be dense in first-read order",
                ));
            }
            let index = SymbolSlot(next);
            self.slots.push(slot.clone());
            self.slot_indices.insert(slot, index);
            index
        };
        self.push(Instruction::Read(index))
    }

    pub(crate) fn neg(
        &mut self,
        value: ScalarInputValueId,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        self.unary(value, Instruction::Neg)
    }

    pub(crate) fn add(
        &mut self,
        left: ScalarInputValueId,
        right: ScalarInputValueId,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        self.binary(left, right, Instruction::Add)
    }

    pub(crate) fn sub(
        &mut self,
        left: ScalarInputValueId,
        right: ScalarInputValueId,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        self.binary(left, right, Instruction::Sub)
    }

    pub(crate) fn mul(
        &mut self,
        left: ScalarInputValueId,
        right: ScalarInputValueId,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        self.binary(left, right, Instruction::Mul)
    }

    pub(crate) fn div(
        &mut self,
        left: ScalarInputValueId,
        right: ScalarInputValueId,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        self.binary(left, right, Instruction::Div)
    }

    pub(crate) fn powi(
        &mut self,
        base: ScalarInputValueId,
        exponent: i32,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        self.require_prior(base)?;
        self.push(Instruction::PowI(base.0, exponent))
    }

    pub(crate) fn finish(
        self,
        roots: impl IntoIterator<Item = ScalarInputValueId>,
    ) -> Result<ScalarInputOperatorIr, Diagnostic> {
        let roots = roots.into_iter().map(|root| root.0).collect::<Vec<_>>();
        if roots.is_empty()
            || roots.iter().any(|root| {
                usize::try_from(root.0).map_or(true, |index| index >= self.instructions.len())
            })
        {
            return Err(ir_builder_error(
                "input-slot scalar program requires valid nonempty roots",
            ));
        }
        Ok(ScalarInputOperatorIr {
            slots: self.slots,
            instructions: self.instructions,
            roots,
        })
    }

    fn unary(
        &mut self,
        value: ScalarInputValueId,
        instruction: impl FnOnce(ValueId) -> Instruction,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        self.require_prior(value)?;
        self.push(instruction(value.0))
    }

    fn binary(
        &mut self,
        left: ScalarInputValueId,
        right: ScalarInputValueId,
        instruction: impl FnOnce(ValueId, ValueId) -> Instruction,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        self.require_prior(left)?;
        self.require_prior(right)?;
        self.push(instruction(left.0, right.0))
    }

    fn require_prior(&self, value: ScalarInputValueId) -> Result<(), Diagnostic> {
        if usize::try_from(value.0.0).map_or(true, |index| index >= self.instructions.len()) {
            Err(ir_builder_error(
                "input-slot scalar instruction references a non-prior value",
            ))
        } else {
            Ok(())
        }
    }

    fn push(&mut self, instruction: Instruction) -> Result<ScalarInputValueId, Diagnostic> {
        let index = u32::try_from(self.instructions.len()).map_err(|_| ir_size_error())?;
        self.instructions.push(instruction);
        Ok(ScalarInputValueId(ValueId(index)))
    }
}

/// Compact scalar SSA Operator IR lowered from one residual DAG.
#[derive(Debug, Clone, PartialEq)]
pub struct ScalarOperatorIr {
    source_values: Vec<ValueId>,
    typed_constants: Vec<eqiora_core::ValueLiteral>,
    symbols: Vec<SymbolRef>,
    instructions: Vec<Instruction>,
    roots: Vec<ValueId>,
}

impl ScalarOperatorIr {
    /// Dense symbol order expected by [`Self::evaluate`].
    #[must_use]
    pub fn symbols(&self) -> &[SymbolRef] {
        &self.symbols
    }

    /// Number of scalar SSA instructions.
    #[must_use]
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Number of residual roots produced by this program.
    #[must_use]
    pub fn residual_count(&self) -> usize {
        self.roots.len()
    }

    /// Prove and extract a residual Jacobian that is constant with respect to
    /// every other expression symbol.
    ///
    /// This is a structural proof over the SSA program, not a numerical probe.
    /// Expressions with state-dependent coefficients or nonlinear dependence
    /// on any selected variable fail closed. The returned rows follow residual
    /// root order and columns follow `variables` order.
    ///
    /// # Errors
    /// Returns the first structural reason that a constant Jacobian cannot be
    /// proven, or reports a repeated selected variable.
    pub fn constant_symbol_jacobian(
        &self,
        variables: &[SymbolRef],
    ) -> Result<ConstantSymbolJacobian, SymbolicLinearityFailure> {
        let columns = selected_columns(variables)?;
        let summaries = self.affine_summaries(&columns, None)?;

        let mut coefficients = Vec::with_capacity(self.roots.len() * variables.len());
        for root in &self.roots {
            coefficients.extend_from_slice(
                &summaries[summary_index(*root, self.instructions.len())?].coefficients,
            );
        }
        Ok(ConstantSymbolJacobian {
            rows: self.roots.len(),
            columns: variables.len(),
            coefficients,
        })
    }

    /// Bind every non-selected symbol and prove an exact affine residual form.
    ///
    /// The returned form satisfies `R(w) = A w + c`: rows retain residual-root
    /// order, columns retain `selected_symbols` order, and coefficient/offset
    /// arithmetic follows the original SSA instruction order. This admission
    /// is structural; it performs no numerical probing or finite differences.
    /// Extra bindings are permitted, but every supplied binding is validated.
    ///
    /// # Errors
    /// Fails closed for repeated selected symbols, duplicate or non-finite
    /// bindings, binding a selected symbol, an unbound symbol read by the IR,
    /// nonlinear selected-symbol dependence, non-finite affine arithmetic, or
    /// an invalid SSA value reference.
    pub fn bind_affine(
        &self,
        selected_symbols: &[SymbolRef],
        bindings: &[(SymbolRef, f64)],
    ) -> Result<BoundAffineScalarIr, BoundAffineFailure> {
        let columns = selected_columns(selected_symbols).map_err(BoundAffineFailure::from)?;
        let mut constants = HashMap::with_capacity(bindings.len());
        for &(symbol, value) in bindings {
            if columns.contains_key(&symbol) {
                return Err(BoundAffineFailure::SelectedSymbolBound(symbol));
            }
            if constants.insert(symbol, value).is_some() {
                return Err(BoundAffineFailure::DuplicateBinding(symbol));
            }
        }
        for &(symbol, value) in bindings {
            if !value.is_finite() {
                return Err(BoundAffineFailure::NonFiniteBinding(symbol));
            }
        }
        for &symbol in &self.symbols {
            if !columns.contains_key(&symbol) && !constants.contains_key(&symbol) {
                return Err(BoundAffineFailure::UnboundSymbol(symbol));
            }
        }

        let summaries = self
            .affine_summaries(&columns, Some(&constants))
            .map_err(BoundAffineFailure::from)?;
        let mut coefficients = Vec::with_capacity(self.roots.len() * selected_symbols.len());
        let mut offsets = Vec::with_capacity(self.roots.len());
        for root in &self.roots {
            let summary = &summaries[summary_index(*root, self.instructions.len())
                .map_err(BoundAffineFailure::from)?];
            coefficients.extend_from_slice(&summary.coefficients);
            offsets.push(summary.constant.ok_or(BoundAffineFailure::InvalidProgram {
                instruction: self.instructions.len(),
            })?);
        }
        Ok(BoundAffineScalarIr {
            selected_symbols: selected_symbols.to_vec(),
            residuals: self.roots.len(),
            coefficients,
            offsets,
        })
    }

    /// Evaluate residual roots with values matching [`Self::symbols`].
    ///
    /// # Errors
    /// Returns `EQ0702` for the wrong input count and `EQ0505` for a non-finite
    /// intermediate result.
    pub fn evaluate(&self, inputs: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        if inputs.len() != self.symbols.len() {
            return Err(Diagnostic::error(
                codes::OPERATOR_INPUT_MISMATCH,
                format!(
                    "scalar Operator IR expects {} symbol inputs, received {}",
                    self.symbols.len(),
                    inputs.len()
                ),
            )
            .with_graph_path(GraphPath::new(["operator-ir", "inputs"])));
        }
        let values = self.evaluate_values(inputs)?;
        self.collect_roots(&values)
    }

    /// Bind one finite point and explicit input roles into an immutable
    /// differentiable residual relation.
    ///
    /// # Errors
    /// Returns `EQ0704` when point/role cardinality differs from the dense
    /// symbol order or the point contains a non-finite value.
    pub fn linearize(
        &self,
        inputs: &[f64],
        roles: &[DifferentiationRole],
    ) -> Result<ScalarLinearization<'_>, Diagnostic> {
        if inputs.len() != self.symbols.len() || roles.len() != self.symbols.len() {
            return Err(invalid_linearization(format!(
                "scalar linearization expects {} point values and roles, received {} values and {} roles",
                self.symbols.len(),
                inputs.len(),
                roles.len()
            )));
        }
        require_finite(inputs, "linearization point")?;
        let mut unknown_dimension = 0usize;
        let mut parameter_dimension = 0usize;
        let bindings = roles
            .iter()
            .map(|role| match role {
                DifferentiationRole::Unknown => {
                    let coordinate = unknown_dimension;
                    unknown_dimension += 1;
                    InputBinding::Unknown(coordinate)
                }
                DifferentiationRole::Parameter => {
                    let coordinate = parameter_dimension;
                    parameter_dimension += 1;
                    InputBinding::Parameter(coordinate)
                }
                DifferentiationRole::Frozen => InputBinding::Frozen,
            })
            .collect();
        Ok(ScalarLinearization {
            ir: self,
            inputs: inputs.to_vec(),
            bindings,
            unknown_dimension,
            parameter_dimension,
        })
    }

    fn evaluate_values(&self, inputs: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        evaluate_instructions(&self.instructions, inputs)
    }

    fn collect_roots(&self, values: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        collect_roots(&self.roots, &self.instructions, values)
    }

    fn affine_summaries(
        &self,
        columns: &HashMap<SymbolRef, usize>,
        constants: Option<&HashMap<SymbolRef, f64>>,
    ) -> Result<Vec<AffineSummary>, SymbolicLinearityFailure> {
        let dimension = columns.len();
        let mut summaries: Vec<AffineSummary> = Vec::with_capacity(self.instructions.len());
        for (index, instruction) in self.instructions.iter().copied().enumerate() {
            let summary = match instruction {
                Instruction::Compare(_, _, _)
                | Instruction::Not(_)
                | Instruction::And(_, _)
                | Instruction::Or(_, _)
                | Instruction::TypedConstant(_)
                | Instruction::Quotient(_, _)
                | Instruction::Remainder(_, _)
                | Instruction::ToReal(_)
                | Instruction::ToInteger(_)
                | Instruction::Ordinal(_) => {
                    return Err(SymbolicLinearityFailure::InvalidProgram { instruction: index });
                }
                Instruction::Constant(value) => AffineSummary::constant(value.value(), dimension),
                Instruction::Read(slot) => {
                    let symbol = self
                        .symbols
                        .get(usize::try_from(slot.0).map_err(|_| {
                            SymbolicLinearityFailure::InvalidProgram { instruction: index }
                        })?)
                        .copied()
                        .ok_or(SymbolicLinearityFailure::InvalidProgram { instruction: index })?;
                    if let Some(column) = columns.get(&symbol).copied() {
                        AffineSummary::variable(column, dimension)
                    } else if let Some(value) = constants.and_then(|values| values.get(&symbol)) {
                        AffineSummary::constant(*value, dimension)
                    } else {
                        AffineSummary::independent(dimension)
                    }
                }
                Instruction::Neg(value) => {
                    summaries[summary_index(value, index)?].scaled(-1.0, index)?
                }
                Instruction::Add(left, right) => AffineSummary::sum(
                    &summaries[summary_index(left, index)?],
                    &summaries[summary_index(right, index)?],
                    1.0,
                    index,
                )?,
                Instruction::Sub(left, right) => AffineSummary::sum(
                    &summaries[summary_index(left, index)?],
                    &summaries[summary_index(right, index)?],
                    -1.0,
                    index,
                )?,
                Instruction::Mul(left, right) => AffineSummary::product(
                    &summaries[summary_index(left, index)?],
                    &summaries[summary_index(right, index)?],
                    index,
                )?,
                Instruction::Div(left, right) => AffineSummary::quotient(
                    &summaries[summary_index(left, index)?],
                    &summaries[summary_index(right, index)?],
                    index,
                )?,
                Instruction::PowI(base, exponent) => {
                    summaries[summary_index(base, index)?].integer_power(exponent, index)?
                }
            };
            summaries.push(summary);
        }
        Ok(summaries)
    }
}

/// Immutable dense affine form `R(w) = A w + c` admitted from scalar SSA.
#[derive(Debug, Clone, PartialEq)]
pub struct BoundAffineScalarIr {
    selected_symbols: Vec<SymbolRef>,
    residuals: usize,
    coefficients: Vec<f64>,
    offsets: Vec<f64>,
}

impl BoundAffineScalarIr {
    /// Selected-symbol order defining the columns of `A`.
    #[must_use]
    pub fn selected_symbols(&self) -> &[SymbolRef] {
        &self.selected_symbols
    }

    /// Number of residual rows.
    #[must_use]
    pub const fn residual_count(&self) -> usize {
        self.residuals
    }

    /// Number of selected-symbol columns.
    #[must_use]
    pub fn selected_symbol_count(&self) -> usize {
        self.selected_symbols.len()
    }

    /// Complete row-major coefficient storage for `A`.
    #[must_use]
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }

    /// One coefficient row in selected-symbol order.
    #[must_use]
    pub fn coefficient_row(&self, row: usize) -> Option<&[f64]> {
        if row >= self.residuals {
            return None;
        }
        let start = row.checked_mul(self.selected_symbols.len())?;
        self.coefficients
            .get(start..start.checked_add(self.selected_symbols.len())?)
    }

    /// Constant offsets `c` in residual-root order.
    #[must_use]
    pub fn offsets(&self) -> &[f64] {
        &self.offsets
    }
}

/// Reason a bound affine scalar form could not be admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundAffineFailure {
    /// The selected-symbol list contains the same symbol more than once.
    RepeatedSelectedSymbol(SymbolRef),
    /// More than one binding was supplied for the same symbol.
    DuplicateBinding(SymbolRef),
    /// A selected symbol was also supplied as a fixed binding.
    SelectedSymbolBound(SymbolRef),
    /// A fixed binding is NaN or infinite.
    NonFiniteBinding(SymbolRef),
    /// A non-selected symbol read by the SSA program has no binding.
    UnboundSymbol(SymbolRef),
    /// Dependence on a selected symbol is nonlinear.
    Nonlinear { instruction: usize },
    /// Exact affine arithmetic produced NaN or infinity.
    NonFiniteArithmetic { instruction: usize },
    /// The SSA program contains an invalid value reference.
    InvalidProgram { instruction: usize },
}

impl From<SymbolicLinearityFailure> for BoundAffineFailure {
    fn from(value: SymbolicLinearityFailure) -> Self {
        match value {
            SymbolicLinearityFailure::RepeatedVariable(symbol) => {
                Self::RepeatedSelectedSymbol(symbol)
            }
            SymbolicLinearityFailure::VariableCoefficient { instruction }
            | SymbolicLinearityFailure::Nonlinear { instruction } => {
                Self::Nonlinear { instruction }
            }
            SymbolicLinearityFailure::NonFiniteCoefficient { instruction } => {
                Self::NonFiniteArithmetic { instruction }
            }
            SymbolicLinearityFailure::InvalidProgram { instruction } => {
                Self::InvalidProgram { instruction }
            }
        }
    }
}

/// Dense row-major Jacobian proven constant by scalar SSA structure.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstantSymbolJacobian {
    rows: usize,
    columns: usize,
    coefficients: Vec<f64>,
}

impl ConstantSymbolJacobian {
    /// Number of residual rows.
    #[must_use]
    pub const fn row_count(&self) -> usize {
        self.rows
    }

    /// Number of selected-symbol columns.
    #[must_use]
    pub const fn column_count(&self) -> usize {
        self.columns
    }

    /// Complete row-major coefficient storage.
    #[must_use]
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }

    /// One row in selected-symbol order.
    #[must_use]
    pub fn row(&self, row: usize) -> Option<&[f64]> {
        let start = row.checked_mul(self.columns)?;
        self.coefficients
            .get(start..start.checked_add(self.columns)?)
    }
}

/// Reason a constant selected-symbol Jacobian could not be proven.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolicLinearityFailure {
    /// The selected variable list contains the same symbol more than once.
    RepeatedVariable(SymbolRef),
    /// A selected variable is multiplied or divided by another model symbol.
    VariableCoefficient { instruction: usize },
    /// Dependence on a selected variable is nonlinear.
    Nonlinear { instruction: usize },
    /// Exact coefficient arithmetic produced a non-finite value.
    NonFiniteCoefficient { instruction: usize },
    /// The SSA program contains an invalid value reference.
    InvalidProgram { instruction: usize },
}

#[derive(Debug, Clone, PartialEq)]
struct AffineSummary {
    constant: Option<f64>,
    coefficients: Vec<f64>,
}

impl AffineSummary {
    fn constant(value: f64, dimension: usize) -> Self {
        Self {
            constant: Some(value),
            coefficients: vec![0.0; dimension],
        }
    }

    fn independent(dimension: usize) -> Self {
        Self {
            constant: None,
            coefficients: vec![0.0; dimension],
        }
    }

    fn variable(column: usize, dimension: usize) -> Self {
        let mut coefficients = vec![0.0; dimension];
        coefficients[column] = 1.0;
        Self {
            constant: Some(0.0),
            coefficients,
        }
    }

    fn depends_on_selected(&self) -> bool {
        self.coefficients
            .iter()
            .any(|coefficient| *coefficient != 0.0)
    }

    fn scaled(&self, scale: f64, instruction: usize) -> Result<Self, SymbolicLinearityFailure> {
        let constant = self.constant.map(|value| value * scale);
        let coefficients = self
            .coefficients
            .iter()
            .map(|coefficient| coefficient * scale)
            .collect::<Vec<_>>();
        Self::finite(constant, coefficients, instruction)
    }

    fn sum(
        left: &Self,
        right: &Self,
        right_scale: f64,
        instruction: usize,
    ) -> Result<Self, SymbolicLinearityFailure> {
        let constant = left
            .constant
            .zip(right.constant)
            .map(|(left, right)| left + right_scale * right);
        let coefficients = left
            .coefficients
            .iter()
            .zip(&right.coefficients)
            .map(|(left, right)| left + right_scale * right)
            .collect::<Vec<_>>();
        Self::finite(constant, coefficients, instruction)
    }

    fn product(
        left: &Self,
        right: &Self,
        instruction: usize,
    ) -> Result<Self, SymbolicLinearityFailure> {
        match (left.depends_on_selected(), right.depends_on_selected()) {
            (true, true) => Err(SymbolicLinearityFailure::Nonlinear { instruction }),
            (true, false) => right.constant.map_or_else(
                || Err(SymbolicLinearityFailure::VariableCoefficient { instruction }),
                |scale| left.scaled(scale, instruction),
            ),
            (false, true) => left.constant.map_or_else(
                || Err(SymbolicLinearityFailure::VariableCoefficient { instruction }),
                |scale| right.scaled(scale, instruction),
            ),
            (false, false) => {
                let constant = left
                    .constant
                    .zip(right.constant)
                    .map(|(left, right)| left * right);
                Self::finite(constant, vec![0.0; left.coefficients.len()], instruction)
            }
        }
    }

    fn quotient(
        numerator: &Self,
        denominator: &Self,
        instruction: usize,
    ) -> Result<Self, SymbolicLinearityFailure> {
        if denominator.depends_on_selected() {
            return Err(SymbolicLinearityFailure::Nonlinear { instruction });
        }
        if numerator.depends_on_selected() {
            return denominator.constant.map_or_else(
                || Err(SymbolicLinearityFailure::VariableCoefficient { instruction }),
                |denominator| numerator.scaled(1.0 / denominator, instruction),
            );
        }
        let constant = numerator
            .constant
            .zip(denominator.constant)
            .map(|(numerator, denominator)| numerator / denominator);
        Self::finite(
            constant,
            vec![0.0; numerator.coefficients.len()],
            instruction,
        )
    }

    fn integer_power(
        &self,
        exponent: i32,
        instruction: usize,
    ) -> Result<Self, SymbolicLinearityFailure> {
        match exponent {
            0 => Ok(Self::constant(1.0, self.coefficients.len())),
            1 => Ok(self.clone()),
            _ if self.depends_on_selected() => {
                Err(SymbolicLinearityFailure::Nonlinear { instruction })
            }
            _ => {
                let constant = self.constant.map(|value| value.powi(exponent));
                Self::finite(constant, vec![0.0; self.coefficients.len()], instruction)
            }
        }
    }

    fn finite(
        constant: Option<f64>,
        coefficients: Vec<f64>,
        instruction: usize,
    ) -> Result<Self, SymbolicLinearityFailure> {
        if constant.is_some_and(|value| !value.is_finite())
            || coefficients.iter().any(|value| !value.is_finite())
        {
            Err(SymbolicLinearityFailure::NonFiniteCoefficient { instruction })
        } else {
            Ok(Self {
                constant,
                coefficients,
            })
        }
    }
}

fn summary_index(id: ValueId, upper_bound: usize) -> Result<usize, SymbolicLinearityFailure> {
    usize::try_from(id.0)
        .ok()
        .filter(|index| *index < upper_bound)
        .ok_or(SymbolicLinearityFailure::InvalidProgram {
            instruction: upper_bound,
        })
}

fn selected_columns(
    selected_symbols: &[SymbolRef],
) -> Result<HashMap<SymbolRef, usize>, SymbolicLinearityFailure> {
    let mut columns = HashMap::with_capacity(selected_symbols.len());
    for (column, symbol) in selected_symbols.iter().copied().enumerate() {
        if columns.insert(symbol, column).is_some() {
            return Err(SymbolicLinearityFailure::RepeatedVariable(symbol));
        }
    }
    Ok(columns)
}

/// `f64` scalar SSA relation fixed at one explicit linearization point.
#[derive(Debug)]
pub struct ScalarLinearization<'a> {
    ir: &'a ScalarOperatorIr,
    inputs: Vec<f64>,
    bindings: Vec<InputBinding>,
    unknown_dimension: usize,
    parameter_dimension: usize,
}

impl LinearizedRelation<f64> for ScalarLinearization<'_> {
    fn unknown_dimension(&self) -> usize {
        self.unknown_dimension
    }

    fn parameter_dimension(&self) -> usize {
        self.parameter_dimension
    }

    fn residual_dimension(&self) -> usize {
        self.ir.roots.len()
    }

    fn primal(&self, residual: &mut [f64]) -> Result<(), Diagnostic> {
        require_length(residual, self.residual_dimension(), "primal residual")?;
        let values = self.ir.evaluate_values(&self.inputs)?;
        write_roots(self.ir, &values, residual)
    }

    fn jvp(
        &self,
        tangent: RelationTangent<'_, f64>,
        residual_tangent: &mut [f64],
    ) -> Result<(), Diagnostic> {
        let (unknown_tangent, parameter_tangent) = match tangent {
            RelationTangent::Unknown(unknown) => (Some(unknown), None),
            RelationTangent::Parameter(parameter) => (None, Some(parameter)),
            RelationTangent::Both { unknown, parameter } => (Some(unknown), Some(parameter)),
        };
        if let Some(unknown) = unknown_tangent {
            require_length(unknown, self.unknown_dimension, "unknown tangent")?;
            require_finite(unknown, "unknown tangent")?;
        }
        if let Some(parameter) = parameter_tangent {
            require_length(parameter, self.parameter_dimension, "parameter tangent")?;
            require_finite(parameter, "parameter tangent")?;
        }
        require_length(
            residual_tangent,
            self.residual_dimension(),
            "residual tangent",
        )?;

        let values = self.ir.evaluate_values(&self.inputs)?;
        let mut tangents = Vec::with_capacity(self.ir.instructions.len());
        for (index, instruction) in self.ir.instructions.iter().enumerate() {
            let tangent = match *instruction {
                Instruction::Compare(_, _, _)
                | Instruction::Not(_)
                | Instruction::And(_, _)
                | Instruction::Or(_, _)
                | Instruction::TypedConstant(_)
                | Instruction::Quotient(_, _)
                | Instruction::Remainder(_, _)
                | Instruction::ToReal(_)
                | Instruction::ToInteger(_)
                | Instruction::Ordinal(_) => {
                    return Err(ir_builder_error(
                        "discrete operations cannot be differentiated",
                    ));
                }
                Instruction::Constant(_) => 0.0,
                Instruction::Read(slot) => match self.bindings[slot_index(slot, index)?] {
                    InputBinding::Unknown(coordinate) => {
                        unknown_tangent.map_or(0.0, |values| values[coordinate])
                    }
                    InputBinding::Parameter(coordinate) => {
                        parameter_tangent.map_or(0.0, |values| values[coordinate])
                    }
                    InputBinding::Frozen => 0.0,
                },
                Instruction::Neg(value) => -read(&tangents, value, index)?,
                Instruction::Add(left, right) => {
                    read(&tangents, left, index)? + read(&tangents, right, index)?
                }
                Instruction::Sub(left, right) => {
                    read(&tangents, left, index)? - read(&tangents, right, index)?
                }
                Instruction::Mul(left, right) => {
                    read(&tangents, left, index)? * read(&values, right, index)?
                        + read(&values, left, index)? * read(&tangents, right, index)?
                }
                Instruction::Div(left, right) => {
                    let denominator = read(&values, right, index)?;
                    (read(&tangents, left, index)? * denominator
                        - read(&values, left, index)? * read(&tangents, right, index)?)
                        / denominator.powi(2)
                }
                Instruction::PowI(base, exponent) => {
                    powi_derivative(read(&values, base, index)?, exponent)
                        * read(&tangents, base, index)?
                }
            };
            require_finite_value(tangent, "JVP", index)?;
            tangents.push(tangent);
        }
        write_roots(self.ir, &tangents, residual_tangent)
    }

    fn vjp(
        &self,
        residual_cotangent: &[f64],
        cotangent: RelationCotangent<'_, f64>,
    ) -> Result<(), Diagnostic> {
        let (mut unknown_cotangent, mut parameter_cotangent) = match cotangent {
            RelationCotangent::Unknown(unknown) => (Some(unknown), None),
            RelationCotangent::Parameter(parameter) => (None, Some(parameter)),
            RelationCotangent::Both { unknown, parameter } => (Some(unknown), Some(parameter)),
        };
        require_length(
            residual_cotangent,
            self.residual_dimension(),
            "residual cotangent",
        )?;
        if let Some(unknown) = unknown_cotangent.as_deref_mut() {
            require_length(unknown, self.unknown_dimension, "unknown cotangent")?;
            unknown.fill(0.0);
        }
        if let Some(parameter) = parameter_cotangent.as_deref_mut() {
            require_length(parameter, self.parameter_dimension, "parameter cotangent")?;
            parameter.fill(0.0);
        }
        require_finite(residual_cotangent, "residual cotangent")?;

        let values = self.ir.evaluate_values(&self.inputs)?;
        let mut adjoints = vec![0.0; self.ir.instructions.len()];
        for (root, seed) in self.ir.roots.iter().zip(residual_cotangent) {
            accumulate(&mut adjoints, *root, *seed, self.ir.instructions.len())?;
        }

        for (index, instruction) in self.ir.instructions.iter().enumerate().rev() {
            let cotangent = adjoints[index];
            match *instruction {
                Instruction::Compare(_, _, _)
                | Instruction::Not(_)
                | Instruction::And(_, _)
                | Instruction::Or(_, _)
                | Instruction::TypedConstant(_)
                | Instruction::Quotient(_, _)
                | Instruction::Remainder(_, _)
                | Instruction::ToReal(_)
                | Instruction::ToInteger(_)
                | Instruction::Ordinal(_) => {
                    return Err(ir_builder_error(
                        "discrete operations cannot be differentiated",
                    ));
                }
                Instruction::Constant(_) => {}
                Instruction::Read(slot) => match self.bindings[slot_index(slot, index)?] {
                    InputBinding::Unknown(coordinate) => {
                        if let Some(values) = unknown_cotangent.as_deref_mut() {
                            accumulate_coordinate(values, coordinate, cotangent, "unknown VJP")?;
                        }
                    }
                    InputBinding::Parameter(coordinate) => {
                        if let Some(values) = parameter_cotangent.as_deref_mut() {
                            accumulate_coordinate(values, coordinate, cotangent, "parameter VJP")?;
                        }
                    }
                    InputBinding::Frozen => {}
                },
                Instruction::Neg(value) => {
                    accumulate(&mut adjoints, value, -cotangent, index)?;
                }
                Instruction::Add(left, right) => {
                    accumulate(&mut adjoints, left, cotangent, index)?;
                    accumulate(&mut adjoints, right, cotangent, index)?;
                }
                Instruction::Sub(left, right) => {
                    accumulate(&mut adjoints, left, cotangent, index)?;
                    accumulate(&mut adjoints, right, -cotangent, index)?;
                }
                Instruction::Mul(left, right) => {
                    accumulate(
                        &mut adjoints,
                        left,
                        cotangent * read(&values, right, index)?,
                        index,
                    )?;
                    accumulate(
                        &mut adjoints,
                        right,
                        cotangent * read(&values, left, index)?,
                        index,
                    )?;
                }
                Instruction::Div(left, right) => {
                    let denominator = read(&values, right, index)?;
                    accumulate(&mut adjoints, left, cotangent / denominator, index)?;
                    accumulate(
                        &mut adjoints,
                        right,
                        -cotangent * read(&values, left, index)? / denominator.powi(2),
                        index,
                    )?;
                }
                Instruction::PowI(base, exponent) => {
                    if exponent != 0 {
                        accumulate(
                            &mut adjoints,
                            base,
                            cotangent * powi_derivative(read(&values, base, index)?, exponent),
                            index,
                        )?;
                    }
                }
            }
        }
        if let Some(unknown) = unknown_cotangent.as_deref() {
            require_finite(unknown, "unknown VJP")?;
        }
        if let Some(parameter) = parameter_cotangent.as_deref() {
            require_finite(parameter, "parameter VJP")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputBinding {
    Unknown(usize),
    Parameter(usize),
    Frozen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SymbolSlot(u32);

fn read(values: &[f64], id: ValueId, instruction: usize) -> Result<f64, Diagnostic> {
    usize::try_from(id.0)
        .ok()
        .and_then(|index| values.get(index))
        .copied()
        .ok_or_else(|| {
            Diagnostic::error(
                codes::INVALID_OPERATOR_IR,
                format!(
                    "SSA value {} is unavailable at instruction {instruction}",
                    id.0
                ),
            )
            .with_graph_path(ir_path(instruction))
        })
}

fn collect_roots(
    roots: &[ValueId],
    instructions: &[Instruction],
    values: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    roots
        .iter()
        .map(|root| read(values, *root, instructions.len()))
        .collect()
}

fn slot_index(slot: SymbolSlot, instruction: usize) -> Result<usize, Diagnostic> {
    usize::try_from(slot.0).map_err(|_| {
        Diagnostic::error(codes::INVALID_OPERATOR_IR, "symbol slot exceeds usize")
            .with_graph_path(ir_path(instruction))
    })
}

fn write_roots(
    ir: &ScalarOperatorIr,
    values: &[f64],
    output: &mut [f64],
) -> Result<(), Diagnostic> {
    for (output, root) in output.iter_mut().zip(&ir.roots) {
        *output = read(values, *root, ir.instructions.len())?;
    }
    Ok(())
}

fn accumulate(
    values: &mut [f64],
    id: ValueId,
    contribution: f64,
    instruction: usize,
) -> Result<(), Diagnostic> {
    let index = usize::try_from(id.0).map_err(|_| invalid_value_index(id, instruction))?;
    let value = values
        .get_mut(index)
        .ok_or_else(|| invalid_value_index(id, instruction))?;
    let next = *value + contribution;
    require_finite_value(next, "VJP", instruction)?;
    *value = next;
    Ok(())
}

fn accumulate_coordinate(
    values: &mut [f64],
    coordinate: usize,
    contribution: f64,
    name: &str,
) -> Result<(), Diagnostic> {
    let next = values[coordinate] + contribution;
    if !next.is_finite() {
        return Err(invalid_linearization(format!(
            "{name} coordinate {coordinate} evaluated to {next}"
        )));
    }
    values[coordinate] = next;
    Ok(())
}

fn require_length(values: &[f64], expected: usize, name: &str) -> Result<(), Diagnostic> {
    if values.len() == expected {
        Ok(())
    } else {
        Err(invalid_linearization(format!(
            "{name} expects {expected} values, received {}",
            values.len()
        )))
    }
}

fn require_finite(values: &[f64], name: &str) -> Result<(), Diagnostic> {
    if values.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(invalid_linearization(format!(
            "{name} must contain only finite values"
        )))
    }
}

fn require_finite_value(value: f64, name: &str, instruction: usize) -> Result<(), Diagnostic> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(Diagnostic::error(
            codes::NONFINITE_EVALUATION,
            format!("scalar Operator IR {name} instruction {instruction} evaluated to {value}"),
        )
        .with_graph_path(ir_path(instruction)))
    }
}

fn powi_derivative(base: f64, exponent: i32) -> f64 {
    match exponent {
        0 => 0.0,
        i32::MIN => f64::from(exponent) * base.powi(exponent) / base,
        _ => f64::from(exponent) * base.powi(exponent - 1),
    }
}

fn invalid_value_index(id: ValueId, instruction: usize) -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_OPERATOR_IR,
        format!(
            "SSA value {} is unavailable at reverse instruction {instruction}",
            id.0
        ),
    )
    .with_graph_path(ir_path(instruction))
}

fn invalid_linearization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_LINEARIZATION, message)
        .with_graph_path(GraphPath::new(["operator-ir", "linearization"]))
}

fn ir_size_error() -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_OPERATOR_IR,
        "scalar Operator IR exceeds the u32 slot limit",
    )
}

fn ir_builder_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_OPERATOR_IR, message)
        .with_graph_path(GraphPath::new(["operator-ir", "input-slots"]))
}

fn ir_path(index: usize) -> GraphPath {
    GraphPath::new(["operator-ir".to_owned(), index.to_string()])
}

#[cfg(test)]
mod tests;
