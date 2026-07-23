use std::collections::HashMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef};

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
        self.push(Instruction::Constant(value.value()))
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
    symbols: Vec<SymbolRef>,
    instructions: Vec<Instruction>,
    roots: Vec<ValueId>,
}

impl ScalarOperatorIr {
    /// Lower a canonical expression DAG into dense symbol slots and scalar
    /// instructions without changing operation order.
    ///
    /// # Errors
    /// Returns `EQ0701` if an operand/root index is inconsistent with the DAG
    /// contract.
    pub fn lower(expression: &ExprDag) -> Result<Self, Diagnostic> {
        let mut symbols = Vec::new();
        let mut symbol_slots = HashMap::new();
        let mut instructions = Vec::with_capacity(expression.nodes().len());
        for (index, node) in expression.nodes().iter().enumerate() {
            let instruction = match node {
                ExprNode::Constant(value) => Instruction::Constant(value.value()),
                ExprNode::Symbol(symbol) => {
                    let next_slot = u32::try_from(symbols.len()).map_err(|_| ir_size_error())?;
                    let slot = *symbol_slots.entry(*symbol).or_insert_with(|| {
                        symbols.push(*symbol);
                        SymbolSlot(next_slot)
                    });
                    Instruction::Read(slot)
                }
                ExprNode::Neg(value) => Instruction::Neg(value_id(*value, index)?),
                ExprNode::Add(left, right) => {
                    Instruction::Add(value_id(*left, index)?, value_id(*right, index)?)
                }
                ExprNode::Sub(left, right) => {
                    Instruction::Sub(value_id(*left, index)?, value_id(*right, index)?)
                }
                ExprNode::Mul(left, right) => {
                    Instruction::Mul(value_id(*left, index)?, value_id(*right, index)?)
                }
                ExprNode::Div(left, right) => {
                    Instruction::Div(value_id(*left, index)?, value_id(*right, index)?)
                }
                ExprNode::PowI(base, exponent) => {
                    Instruction::PowI(value_id(*base, index)?, *exponent)
                }
                _ => {
                    return Err(Diagnostic::error(
                        codes::INVALID_OPERATOR_IR,
                        "expression node is newer than scalar Operator IR",
                    )
                    .with_graph_path(ir_path(index)));
                }
            };
            instructions.push(instruction);
        }
        let roots = expression
            .roots()
            .iter()
            .map(|root| value_id(*root, instructions.len()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            symbols,
            instructions,
            roots,
        })
    }

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
                Instruction::Constant(value) => AffineSummary::constant(value, dimension),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ValueId(u32);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Instruction {
    Constant(f64),
    Read(SymbolSlot),
    Neg(ValueId),
    Add(ValueId, ValueId),
    Sub(ValueId, ValueId),
    Mul(ValueId, ValueId),
    Div(ValueId, ValueId),
    PowI(ValueId, i32),
}

fn value_id(id: ExprId, upper_bound: usize) -> Result<ValueId, Diagnostic> {
    let index = usize::try_from(id.index()).map_err(|_| invalid_value(id, upper_bound))?;
    if index >= upper_bound {
        Err(invalid_value(id, upper_bound))
    } else {
        Ok(ValueId(id.index()))
    }
}

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

fn evaluate_instructions(
    instructions: &[Instruction],
    inputs: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let mut values = Vec::with_capacity(instructions.len());
    for (index, instruction) in instructions.iter().enumerate() {
        let value =
            match *instruction {
                Instruction::Constant(value) => value,
                Instruction::Read(slot) => inputs
                    .get(slot_index(slot, index)?)
                    .copied()
                    .ok_or_else(|| {
                        Diagnostic::error(
                            codes::INVALID_OPERATOR_IR,
                            "scalar input slot is outside the supplied input inventory",
                        )
                        .with_graph_path(ir_path(index))
                    })?,
                Instruction::Neg(value) => -read(&values, value, index)?,
                Instruction::Add(left, right) => {
                    read(&values, left, index)? + read(&values, right, index)?
                }
                Instruction::Sub(left, right) => {
                    read(&values, left, index)? - read(&values, right, index)?
                }
                Instruction::Mul(left, right) => {
                    read(&values, left, index)? * read(&values, right, index)?
                }
                Instruction::Div(left, right) => {
                    read(&values, left, index)? / read(&values, right, index)?
                }
                Instruction::PowI(base, exponent) => read(&values, base, index)?.powi(exponent),
            };
        if !value.is_finite() {
            return Err(Diagnostic::error(
                codes::NONFINITE_EVALUATION,
                format!("scalar Operator IR instruction {index} evaluated to {value}"),
            )
            .with_graph_path(ir_path(index)));
        }
        values.push(value);
    }
    Ok(values)
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

fn invalid_value(id: ExprId, upper_bound: usize) -> Diagnostic {
    Diagnostic::error(
        codes::INVALID_OPERATOR_IR,
        format!(
            "expression value {} is not below the instruction bound {upper_bound}",
            id.index()
        ),
    )
    .with_graph_path(ir_path(upper_bound))
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
mod tests {
    use eqiora_core::{DimExponents, DynQuantity, Id, entity::kinds};
    use eqiora_schema::kernel::ExprDagBuilder;

    use super::*;

    #[test]
    fn lowering_deduplicates_symbols_and_preserves_residual_value() {
        let field = Id::<kinds::Field>::new();
        let mut expression = ExprDagBuilder::new();
        let value = expression.symbol(SymbolRef::Field(field)).expect("field");
        let two = expression
            .constant(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
            .expect("constant");
        let square = expression.mul(value, value).expect("square");
        let residual = expression.sub(square, two).expect("residual");
        let dag = expression.finish([residual]).expect("DAG");

        let ir = ScalarOperatorIr::lower(&dag).expect("lowered");
        assert_eq!(ir.symbols(), &[SymbolRef::Field(field)]);
        assert_eq!(ir.evaluate(&[3.0]).expect("evaluate"), vec![7.0]);
    }

    #[test]
    fn evaluation_rejects_wrong_or_nonfinite_inputs() {
        let field = Id::<kinds::Field>::new();
        let mut expression = ExprDagBuilder::new();
        let root = expression.symbol(SymbolRef::Field(field)).expect("field");
        let ir =
            ScalarOperatorIr::lower(&expression.finish([root]).expect("DAG")).expect("lowered");

        assert_eq!(
            ir.evaluate(&[]).expect_err("missing input").code(),
            codes::OPERATOR_INPUT_MISMATCH
        );
        assert_eq!(
            ir.evaluate(&[f64::INFINITY])
                .expect_err("nonfinite input")
                .code(),
            codes::NONFINITE_EVALUATION
        );
    }

    #[test]
    fn scalar_ssa_jvp_and_vjp_are_paired_on_a_nonsymmetric_relation() {
        let first = Id::<kinds::Field>::new();
        let second = Id::<kinds::Field>::new();
        let first_parameter = Id::<kinds::Parameter>::new();
        let second_parameter = Id::<kinds::Parameter>::new();
        let mut expression = ExprDagBuilder::new();
        let w0 = expression.symbol(SymbolRef::Field(first)).unwrap();
        let w1 = expression.symbol(SymbolRef::Field(second)).unwrap();
        let p0 = expression
            .symbol(SymbolRef::Parameter(first_parameter))
            .unwrap();
        let p1 = expression
            .symbol(SymbolRef::Parameter(second_parameter))
            .unwrap();
        let two = expression
            .constant(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
            .unwrap();
        let three = expression
            .constant(DynQuantity::new(3.0, DimExponents::DIMENSIONLESS))
            .unwrap();
        let square = expression.mul(w0, w0).unwrap();
        let first_sum = expression.add(square, w1).unwrap();
        let first_residual = expression.sub(first_sum, p0).unwrap();
        let twice_w0 = expression.mul(two, w0).unwrap();
        let thrice_w1 = expression.mul(three, w1).unwrap();
        let second_sum = expression.add(twice_w0, thrice_w1).unwrap();
        let second_residual = expression.sub(second_sum, p1).unwrap();
        let ir = ScalarOperatorIr::lower(
            &expression
                .finish([first_residual, second_residual])
                .unwrap(),
        )
        .unwrap();
        let point = [2.0, 1.0, 5.0, 7.0];
        let roles = [
            DifferentiationRole::Unknown,
            DifferentiationRole::Unknown,
            DifferentiationRole::Parameter,
            DifferentiationRole::Parameter,
        ];
        let linearization = ir.linearize(&point, &roles).unwrap();

        let mut primal = [f64::NAN; 2];
        linearization.primal(&mut primal).unwrap();
        assert_eq!(primal, [0.0, 0.0]);

        let unknown_tangent = [0.25, -0.5];
        let parameter_tangent = [0.75, -1.0];
        let mut jvp = [0.0; 2];
        linearization
            .jvp(
                RelationTangent::Both {
                    unknown: &unknown_tangent,
                    parameter: &parameter_tangent,
                },
                &mut jvp,
            )
            .unwrap();
        assert_eq!(jvp, [-0.25, 0.0]);

        let residual_cotangent = [1.5, -0.25];
        let mut unknown_cotangent = [0.0; 2];
        let mut parameter_cotangent = [0.0; 2];
        linearization
            .vjp(
                &residual_cotangent,
                RelationCotangent::Both {
                    unknown: &mut unknown_cotangent,
                    parameter: &mut parameter_cotangent,
                },
            )
            .unwrap();
        assert_eq!(unknown_cotangent, [5.5, 0.75]);
        assert_eq!(parameter_cotangent, [-1.5, 0.25]);

        let left = residual_cotangent
            .iter()
            .zip(jvp)
            .map(|(left, right)| left * right)
            .sum::<f64>();
        let right = unknown_tangent
            .iter()
            .zip(unknown_cotangent)
            .chain(parameter_tangent.iter().zip(parameter_cotangent))
            .map(|(left, right)| left * right)
            .sum::<f64>();
        assert!((left - right).abs() < 1.0e-14);

        let full_tangent = [0.25, -0.5, 0.75, -1.0];
        let step = 1.0e-6;
        let plus: [f64; 4] = std::array::from_fn(|index| point[index] + step * full_tangent[index]);
        let minus: [f64; 4] =
            std::array::from_fn(|index| point[index] - step * full_tangent[index]);
        let plus = ir.evaluate(&plus).unwrap();
        let minus = ir.evaluate(&minus).unwrap();
        for ((plus, minus), jvp) in plus.iter().zip(minus).zip(jvp) {
            let difference = (plus - minus) / (2.0 * step);
            assert!((difference - jvp).abs() < 2.0e-9);
        }
    }

    #[test]
    fn linearization_fails_closed_on_shapes_and_handles_zero_power_at_zero() {
        let field = Id::<kinds::Field>::new();
        let mut expression = ExprDagBuilder::new();
        let value = expression.symbol(SymbolRef::Field(field)).unwrap();
        let root = expression.powi(value, 0).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([root]).unwrap()).unwrap();
        let linearization = ir
            .linearize(&[0.0], &[DifferentiationRole::Unknown])
            .unwrap();
        let mut tangent = [f64::NAN];
        linearization
            .jvp(RelationTangent::Unknown(&[2.0]), &mut tangent)
            .unwrap();
        assert_eq!(tangent, [0.0]);
        assert_eq!(
            linearization
                .jvp(RelationTangent::Unknown(&[]), &mut tangent)
                .expect_err("wrong unknown tangent")
                .code(),
            codes::INVALID_LINEARIZATION
        );
        assert_eq!(
            ir.linearize(&[0.0], &[]).expect_err("missing role").code(),
            codes::INVALID_LINEARIZATION
        );

        let mut expression = ExprDagBuilder::new();
        let value = expression.symbol(SymbolRef::Field(field)).unwrap();
        let root = expression.powi(value, i32::MIN).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([root]).unwrap()).unwrap();
        let linearization = ir
            .linearize(&[1.0], &[DifferentiationRole::Unknown])
            .unwrap();
        linearization
            .jvp(RelationTangent::Unknown(&[1.0]), &mut tangent)
            .unwrap();
        assert_eq!(tangent, [f64::from(i32::MIN)]);
    }

    #[test]
    fn constant_symbol_jacobian_proves_an_exact_derivative_identity() {
        let first = Id::<kinds::Field>::new();
        let second = Id::<kinds::Field>::new();
        let parameter = Id::<kinds::Parameter>::new();
        let mut expression = ExprDagBuilder::new();
        let d_first = expression.symbol(SymbolRef::Derivative(first)).unwrap();
        let d_second = expression.symbol(SymbolRef::Derivative(second)).unwrap();
        let first_value = expression.symbol(SymbolRef::Field(first)).unwrap();
        let parameter_value = expression.symbol(SymbolRef::Parameter(parameter)).unwrap();
        let first_residual = expression.sub(d_first, first_value).unwrap();
        let twice_second = expression.add(d_second, d_second).unwrap();
        let scaled_second = expression.sub(twice_second, parameter_value).unwrap();
        let ir =
            ScalarOperatorIr::lower(&expression.finish([first_residual, scaled_second]).unwrap())
                .unwrap();

        let jacobian = ir
            .constant_symbol_jacobian(&[
                SymbolRef::Derivative(first),
                SymbolRef::Derivative(second),
            ])
            .unwrap();
        assert_eq!(jacobian.row_count(), 2);
        assert_eq!(jacobian.column_count(), 2);
        assert_eq!(jacobian.row(0), Some([1.0, 0.0].as_slice()));
        assert_eq!(jacobian.row(1), Some([0.0, 2.0].as_slice()));
        assert_eq!(jacobian.row(2), None);
    }

    #[test]
    fn constant_symbol_jacobian_rejects_variable_and_nonlinear_coefficients() {
        let field = Id::<kinds::Field>::new();
        let derivative = SymbolRef::Derivative(field);

        let mut expression = ExprDagBuilder::new();
        let rate = expression.symbol(derivative).unwrap();
        let state = expression.symbol(SymbolRef::Field(field)).unwrap();
        let root = expression.mul(rate, state).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([root]).unwrap()).unwrap();
        assert!(matches!(
            ir.constant_symbol_jacobian(&[derivative]),
            Err(SymbolicLinearityFailure::VariableCoefficient { .. })
        ));

        let mut expression = ExprDagBuilder::new();
        let rate = expression.symbol(derivative).unwrap();
        let root = expression.mul(rate, rate).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([root]).unwrap()).unwrap();
        assert!(matches!(
            ir.constant_symbol_jacobian(&[derivative]),
            Err(SymbolicLinearityFailure::Nonlinear { .. })
        ));
        assert_eq!(
            ir.constant_symbol_jacobian(&[derivative, derivative]),
            Err(SymbolicLinearityFailure::RepeatedVariable(derivative))
        );
    }

    #[test]
    fn bound_affine_admits_parameter_coefficients_and_retains_declared_orders() {
        let first = SymbolRef::Field(Id::<kinds::Field>::new());
        let second = SymbolRef::Field(Id::<kinds::Field>::new());
        let parameter = SymbolRef::Parameter(Id::<kinds::Parameter>::new());
        let mut expression = ExprDagBuilder::new();
        let first_value = expression.symbol(first).unwrap();
        let second_value = expression.symbol(second).unwrap();
        let parameter_value = expression.symbol(parameter).unwrap();
        let time = expression.symbol(SymbolRef::Time).unwrap();
        let scaled_first = expression.mul(parameter_value, first_value).unwrap();
        let first_sum = expression.add(scaled_first, second_value).unwrap();
        let first_residual = expression.sub(first_sum, time).unwrap();
        let divided_first = expression.div(first_value, parameter_value).unwrap();
        let second_residual = expression.add(divided_first, time).unwrap();
        let ir = ScalarOperatorIr::lower(
            &expression
                .finish([first_residual, second_residual])
                .unwrap(),
        )
        .unwrap();

        let selected = [second, first];
        let affine = ir
            .bind_affine(&selected, &[(parameter, 3.0), (SymbolRef::Time, 5.0)])
            .unwrap();

        assert_eq!(affine.selected_symbols(), selected.as_slice());
        assert_eq!(affine.residual_count(), 2);
        assert_eq!(affine.selected_symbol_count(), 2);
        assert_eq!(affine.coefficients(), &[1.0, 3.0, 0.0, 1.0 / 3.0]);
        assert_eq!(affine.coefficient_row(0), Some([1.0, 3.0].as_slice()));
        assert_eq!(affine.coefficient_row(1), Some([0.0, 1.0 / 3.0].as_slice()));
        assert_eq!(affine.coefficient_row(2), None);
        assert_eq!(affine.offsets(), &[-5.0, 5.0]);
    }

    #[test]
    fn bound_affine_validates_the_complete_binding_boundary() {
        let unknown = SymbolRef::Field(Id::<kinds::Field>::new());
        let parameter = SymbolRef::Parameter(Id::<kinds::Parameter>::new());
        let mut expression = ExprDagBuilder::new();
        let unknown_value = expression.symbol(unknown).unwrap();
        let parameter_value = expression.symbol(parameter).unwrap();
        let residual = expression.add(unknown_value, parameter_value).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([residual]).unwrap()).unwrap();

        assert_eq!(
            ir.bind_affine(&[unknown, unknown], &[(parameter, 2.0)]),
            Err(BoundAffineFailure::RepeatedSelectedSymbol(unknown))
        );
        assert_eq!(
            ir.bind_affine(&[unknown], &[(parameter, 2.0), (parameter, 3.0)]),
            Err(BoundAffineFailure::DuplicateBinding(parameter))
        );
        assert_eq!(
            ir.bind_affine(&[unknown], &[(unknown, 1.0), (parameter, 2.0)]),
            Err(BoundAffineFailure::SelectedSymbolBound(unknown))
        );
        assert_eq!(
            ir.bind_affine(&[unknown], &[(parameter, f64::NAN)]),
            Err(BoundAffineFailure::NonFiniteBinding(parameter))
        );
        assert_eq!(
            ir.bind_affine(&[unknown], &[]),
            Err(BoundAffineFailure::UnboundSymbol(parameter))
        );
    }

    #[test]
    fn bound_affine_rejects_nonlinear_dependence_and_nonfinite_arithmetic() {
        let unknown = SymbolRef::Field(Id::<kinds::Field>::new());
        let parameter = SymbolRef::Parameter(Id::<kinds::Parameter>::new());

        let mut expression = ExprDagBuilder::new();
        let value = expression.symbol(unknown).unwrap();
        let square = expression.mul(value, value).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([square]).unwrap()).unwrap();
        assert!(matches!(
            ir.bind_affine(&[unknown], &[]),
            Err(BoundAffineFailure::Nonlinear { .. })
        ));

        let mut expression = ExprDagBuilder::new();
        let value = expression.symbol(unknown).unwrap();
        let one = expression
            .constant(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
            .unwrap();
        let reciprocal = expression.div(one, value).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([reciprocal]).unwrap()).unwrap();
        assert!(matches!(
            ir.bind_affine(&[unknown], &[]),
            Err(BoundAffineFailure::Nonlinear { .. })
        ));

        let mut expression = ExprDagBuilder::new();
        let value = expression.symbol(unknown).unwrap();
        let square = expression.powi(value, 2).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([square]).unwrap()).unwrap();
        assert!(matches!(
            ir.bind_affine(&[unknown], &[]),
            Err(BoundAffineFailure::Nonlinear { .. })
        ));

        let mut expression = ExprDagBuilder::new();
        let value = expression.symbol(unknown).unwrap();
        let divisor = expression.symbol(parameter).unwrap();
        let quotient = expression.div(value, divisor).unwrap();
        let ir = ScalarOperatorIr::lower(&expression.finish([quotient]).unwrap()).unwrap();
        assert!(matches!(
            ir.bind_affine(&[unknown], &[(parameter, 0.0)]),
            Err(BoundAffineFailure::NonFiniteArithmetic { .. })
        ));
    }
}
