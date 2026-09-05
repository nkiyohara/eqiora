use std::collections::HashMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DimExponents, DynQuantity, GraphPath, ValueShape};
use eqiora_schema::kernel::typing::{ExpressionType, TypedResidual};
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef};

use crate::scalar::{
    ScalarInputIrBuilder, ScalarInputOperatorIr, ScalarInputSlot, ScalarInputValueId,
};
use crate::{
    OperatorExpansionExt, PureOperatorDefinition, ScalarCalculusNode, StandardPureOperator,
};

/// One scalar coordinate of a shaped Semantic Model symbol.
///
/// The empty component index denotes a scalar. Non-scalar indices are
/// lexicographic row-major coordinates with the last axis varying fastest.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ScalarSymbolCoordinate {
    symbol: SymbolRef,
    component_index: Box<[u32]>,
}

impl ScalarSymbolCoordinate {
    /// Semantic symbol before Operator lowering.
    #[must_use]
    pub const fn symbol(&self) -> SymbolRef {
        self.symbol
    }

    /// Exact row-major component multi-index; empty for a scalar.
    #[must_use]
    pub const fn component_index(&self) -> &[u32] {
        &self.component_index
    }
}

/// One deterministic scalar residual row lowered from a shaped root.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentScalarRow {
    root_index: usize,
    component_index: Box<[u32]>,
    symbols: Vec<ScalarSymbolCoordinate>,
    ir: ScalarInputOperatorIr,
}

impl ComponentScalarRow {
    /// Original Relation root index.
    #[must_use]
    pub const fn root_index(&self) -> usize {
        self.root_index
    }

    /// Row-major component multi-index within the original shaped root.
    #[must_use]
    pub const fn component_index(&self) -> &[u32] {
        &self.component_index
    }

    /// Dense shaped-symbol coordinates expected by [`Self::evaluate`].
    #[must_use]
    pub fn symbols(&self) -> &[ScalarSymbolCoordinate] {
        &self.symbols
    }

    /// Typed IR-local reads corresponding one-for-one with [`Self::symbols`].
    ///
    /// These dense slots are evaluator plumbing. Their source coordinates
    /// retain real Semantic identities, but the slots are not Parameters or
    /// any other Semantic symbol kind.
    #[must_use]
    pub fn input_slots(&self) -> &[ScalarInputSlot] {
        self.ir.slots()
    }

    /// Evaluate this scalar row using dense inputs matching [`Self::symbols`].
    ///
    /// # Errors
    /// Returns the ordinary scalar Operator IR diagnostics for invalid input
    /// cardinality or non-finite arithmetic.
    pub fn evaluate(&self, inputs: &[f64]) -> Result<f64, Diagnostic> {
        let values = self.ir.evaluate(inputs)?;
        values
            .first()
            .copied()
            .ok_or_else(|| invalid_component_ir("component scalar row produced no residual value"))
    }
}

/// Operator-lowering proof that shaped residuals mean componentwise zero.
///
/// Semantic expression nodes remain shaped. This lowered form alone expands
/// roots into deterministic scalar rows; it does not add component-selection
/// nodes to canonical meaning.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentScalarization {
    rows: Vec<ComponentScalarRow>,
}

impl ComponentScalarization {
    /// Scalarize one fully typed pointwise residual.
    ///
    /// Rows are ordered by Relation root, then row-major component index. A
    /// scalar root contributes one row with an empty index. Spatial operators
    /// fail closed because their scalarization belongs to a discretized
    /// Operator lowering, not this pointwise contract.
    ///
    /// # Errors
    /// Returns `EQ0701` for shape cardinality mismatch, unrepresentable row
    /// count, inconsistent repeated-symbol shape, a non-pointwise expression,
    /// or an invalid exact component coordinate.
    pub fn lower<I: Clone + Eq>(residual: &TypedResidual<I>) -> Result<Self, Diagnostic> {
        let expression = residual.expression();
        let mut rows = Vec::new();
        for (root_index, root) in expression.roots().iter().copied().enumerate() {
            let root_node_index = node_index(root, expression.nodes().len())?;
            let root_shape = &residual.node_types()[root_node_index].shape;
            let component_count = root_shape.component_count().ok_or_else(|| {
                invalid_component_ir("component scalarization row count exceeds local usize")
            })?;

            for flat_index in 0..component_count {
                let component_index = row_major_index(root_shape, flat_index)?;
                let (ir, symbols) = component_single_root(
                    expression,
                    residual.node_types(),
                    root,
                    &component_index,
                )?;
                let aligned = ir.slots().iter().zip(&symbols).enumerate().all(
                    |(index, (slot, coordinate))| {
                        usize::try_from(slot.ordinal()) == Ok(index) && slot.source() == coordinate
                    },
                );
                if ir.slots().len() != symbols.len() || !aligned {
                    return Err(invalid_component_ir(
                        "component input coordinates do not match typed local slots",
                    ));
                }
                rows.push(ComponentScalarRow {
                    root_index,
                    component_index,
                    symbols,
                    ir,
                });
            }
        }
        Ok(Self { rows })
    }

    /// Deterministic scalar residual rows.
    #[must_use]
    pub fn rows(&self) -> &[ComponentScalarRow] {
        &self.rows
    }

    /// Evaluate every row through one shaped-coordinate resolver.
    ///
    /// # Errors
    /// Returns `EQ0702` when a coordinate is missing, or the underlying scalar
    /// Operator IR diagnostic for non-finite arithmetic.
    pub fn evaluate(
        &self,
        mut resolve: impl FnMut(&ScalarSymbolCoordinate) -> Option<f64>,
    ) -> Result<Vec<f64>, Diagnostic> {
        self.rows
            .iter()
            .map(|row| {
                let inputs = row
                    .symbols()
                    .iter()
                    .map(|coordinate| {
                        resolve(coordinate).ok_or_else(|| {
                            Diagnostic::error(
                                codes::OPERATOR_INPUT_MISMATCH,
                                format!(
                                    "missing component input for {:?}{:?}",
                                    coordinate.symbol(),
                                    coordinate.component_index()
                                ),
                            )
                            .with_graph_path(GraphPath::new(["operator-ir", "component-inputs"]))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                row.evaluate(&inputs)
            })
            .collect()
    }
}

fn component_single_root<I: Clone + Eq>(
    expression: &ExprDag,
    node_types: &[ExpressionType<I>],
    root: ExprId,
    component_index: &[u32],
) -> Result<(ScalarInputOperatorIr, Vec<ScalarSymbolCoordinate>), Diagnostic> {
    let mut lowering = ComponentDagLowering {
        expression,
        node_types,
        builder: ScalarInputIrBuilder::new(),
        remapped: HashMap::new(),
        inputs: Vec::new(),
        input_nodes: HashMap::new(),
        symbol_shapes: HashMap::new(),
    };
    let root = lowering.lower(root, component_index)?;
    Ok((lowering.builder.finish([root])?, lowering.inputs))
}

struct ComponentDagLowering<'a, I> {
    expression: &'a ExprDag,
    node_types: &'a [ExpressionType<I>],
    builder: ScalarInputIrBuilder,
    remapped: HashMap<(usize, Box<[u32]>), ScalarInputValueId>,
    inputs: Vec<ScalarSymbolCoordinate>,
    input_nodes: HashMap<ScalarSymbolCoordinate, ScalarInputValueId>,
    symbol_shapes: HashMap<SymbolRef, ValueShape>,
}

impl<I: Clone + Eq> ComponentDagLowering<'_, I> {
    fn lower(
        &mut self,
        value: ExprId,
        component: &[u32],
    ) -> Result<ScalarInputValueId, Diagnostic> {
        let index = node_index(value, self.expression.nodes().len())?;
        let node_type = self
            .node_types
            .get(index)
            .ok_or_else(|| invalid_component_ir("component node has no inferred type"))?;
        validate_component(&node_type.shape, component)?;
        let key = (index, component.into());
        if let Some(mapped) = self.remapped.get(&key) {
            return Ok(*mapped);
        }

        let node = self.expression.nodes()[index].clone();
        let mapped = match node {
            ExprNode::Constant(constant) => self.builder.constant(constant)?,
            ExprNode::Symbol(symbol) => self.input(symbol, &node_type.shape, component)?,
            ExprNode::Neg(operand) => {
                let operand = self.lower_shaped(operand, component)?;
                self.builder.neg(operand)?
            }
            ExprNode::Add(left, right) => {
                let left = self.lower_shaped(left, component)?;
                let right = self.lower_shaped(right, component)?;
                self.builder.add(left, right)?
            }
            ExprNode::Sub(left, right) => {
                let left = self.lower_shaped(left, component)?;
                let right = self.lower_shaped(right, component)?;
                self.builder.sub(left, right)?
            }
            ExprNode::Mul(left, right) => {
                let left = self.lower_shaped(left, component)?;
                let right = self.lower_shaped(right, component)?;
                self.builder.mul(left, right)?
            }
            ExprNode::Div(numerator, denominator) => {
                let numerator = self.lower_shaped(numerator, component)?;
                let denominator = self.lower_shaped(denominator, component)?;
                self.builder.div(numerator, denominator)?
            }
            ExprNode::PowI(base, exponent) => {
                let base = self.lower_shaped(base, component)?;
                self.builder.powi(base, exponent)?
            }
            ExprNode::SymmetricPart(operand) => self.lower_pure_operator(
                StandardPureOperator::SymmetricPart,
                operand,
                component,
                node_type,
            )?,
            ExprNode::IsotropicLift(operand) => self.lower_pure_operator(
                StandardPureOperator::IsotropicLift,
                operand,
                component,
                node_type,
            )?,
            ExprNode::PureOperatorApplication(application) => {
                let definition = self
                    .expression
                    .definition(application.definition())
                    .cloned()
                    .ok_or_else(|| {
                        invalid_component_ir(
                            "pure operator application has no exact expression-local definition",
                        )
                    })?;
                self.lower_pure_definition(
                    &definition,
                    application.arguments(),
                    component,
                    node_type,
                )?
            }
            _ => {
                return Err(invalid_component_ir(
                    "component scalarization requires pointwise algebra or tensor structure",
                ));
            }
        };
        self.remapped.insert(key, mapped);
        Ok(mapped)
    }

    fn lower_pure_operator(
        &mut self,
        operator: StandardPureOperator,
        operand: ExprId,
        component: &[u32],
        expected_result: &ExpressionType<I>,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        let definition = match operator {
            StandardPureOperator::SymmetricPart => PureOperatorDefinition::symmetric_part(),
            StandardPureOperator::IsotropicLift => PureOperatorDefinition::isotropic_lift(),
        }
        .map_err(|error| invalid_component_ir(format!("invalid pure operator: {error}")))?;
        self.lower_pure_definition(&definition, &[operand], component, expected_result)
    }

    fn lower_pure_definition(
        &mut self,
        definition: &PureOperatorDefinition,
        arguments: &[ExprId],
        component: &[u32],
        expected_result: &ExpressionType<I>,
    ) -> Result<ScalarInputValueId, Diagnostic> {
        let argument_types = arguments
            .iter()
            .map(|argument| {
                let index = node_index(*argument, self.expression.nodes().len())?;
                self.node_types.get(index).cloned().ok_or_else(|| {
                    invalid_component_ir("pure operator argument has no inferred type")
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expansion = definition.instantiate(&argument_types).map_err(|error| {
            invalid_component_ir(format!("pure operator typing failed: {error}"))
        })?;
        if expansion.result_type() != expected_result {
            return Err(invalid_component_ir(
                "pure operator expansion differs from the inferred Kernel type",
            ));
        }
        let calculus = expansion.component(component).map_err(|error| {
            invalid_component_ir(format!("pure operator component expansion failed: {error}"))
        })?;
        let mut remapped = vec![None; calculus.nodes().len()];
        self.lower_calculus_component(&calculus, calculus.root(), arguments, &mut remapped)
    }

    fn lower_calculus_component(
        &mut self,
        calculus: &crate::ScalarCalculus<I>,
        value: crate::CalculusNodeId,
        arguments: &[ExprId],
        remapped: &mut [Option<ScalarInputValueId>],
    ) -> Result<ScalarInputValueId, Diagnostic> {
        let index = usize::try_from(value.index())
            .ok()
            .filter(|index| *index < calculus.nodes().len())
            .ok_or_else(|| {
                invalid_component_ir("pure calculus contains an invalid value reference")
            })?;
        if let Some(mapped) = remapped[index] {
            return Ok(mapped);
        }
        let node = calculus.nodes()[index].clone();
        let mapped = match node {
            ScalarCalculusNode::Rational(value) => self.builder.constant(DynQuantity::new(
                value.as_f64(),
                DimExponents::DIMENSIONLESS,
            ))?,
            ScalarCalculusNode::FormalComponent(atom) => {
                let operand = arguments
                    .get(usize::from(atom.formal()))
                    .copied()
                    .ok_or_else(|| {
                        invalid_component_ir("pure calculus referenced an unexpected formal")
                    })?;
                self.lower(operand, atom.component())?
            }
            ScalarCalculusNode::Neg(value) => {
                let value = self.lower_calculus_component(calculus, value, arguments, remapped)?;
                self.builder.neg(value)?
            }
            ScalarCalculusNode::Add(left, right) => {
                let left = self.lower_calculus_component(calculus, left, arguments, remapped)?;
                let right = self.lower_calculus_component(calculus, right, arguments, remapped)?;
                self.builder.add(left, right)?
            }
            ScalarCalculusNode::Mul(left, right) => {
                let left = self.lower_calculus_component(calculus, left, arguments, remapped)?;
                let right = self.lower_calculus_component(calculus, right, arguments, remapped)?;
                self.builder.mul(left, right)?
            }
        };
        remapped[index] = Some(mapped);
        Ok(mapped)
    }

    fn lower_shaped(
        &mut self,
        operand: ExprId,
        result_component: &[u32],
    ) -> Result<ScalarInputValueId, Diagnostic> {
        let operand_index = node_index(operand, self.expression.nodes().len())?;
        let operand_shape = &self
            .node_types
            .get(operand_index)
            .ok_or_else(|| invalid_component_ir("component operand has no inferred type"))?
            .shape;
        if operand_shape.is_scalar() {
            self.lower(operand, &[])
        } else {
            self.lower(operand, result_component)
        }
    }

    fn input(
        &mut self,
        symbol: SymbolRef,
        shape: &ValueShape,
        component: &[u32],
    ) -> Result<ScalarInputValueId, Diagnostic> {
        match self.symbol_shapes.entry(symbol) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(shape.clone());
            }
            std::collections::hash_map::Entry::Occupied(entry) if entry.get() != shape => {
                return Err(invalid_component_ir(format!(
                    "repeated symbol {symbol:?} has inconsistent exact shapes"
                )));
            }
            std::collections::hash_map::Entry::Occupied(_) => {}
        }
        let coordinate = ScalarSymbolCoordinate {
            symbol,
            component_index: component.into(),
        };
        if let Some(mapped) = self.input_nodes.get(&coordinate) {
            return Ok(*mapped);
        }

        let ordinal = u32::try_from(self.inputs.len())
            .map_err(|_| invalid_component_ir("component input slot exceeds portable u32"))?;
        let mapped = self
            .builder
            .input(ScalarInputSlot::new(ordinal, coordinate.clone()))?;
        self.inputs.push(coordinate.clone());
        self.input_nodes.insert(coordinate, mapped);
        Ok(mapped)
    }
}

fn validate_component(shape: &ValueShape, component: &[u32]) -> Result<(), Diagnostic> {
    if shape.rank() != component.len()
        || shape
            .extents()
            .iter()
            .zip(component)
            .any(|(extent, coordinate)| *coordinate >= extent.get())
    {
        Err(invalid_component_ir(
            "component coordinate is outside the exact value shape",
        ))
    } else {
        Ok(())
    }
}

fn node_index(id: ExprId, upper_bound: usize) -> Result<usize, Diagnostic> {
    usize::try_from(id.index())
        .ok()
        .filter(|index| *index < upper_bound)
        .ok_or_else(|| invalid_component_ir("component scalarization ExprId is out of bounds"))
}

fn row_major_index(shape: &ValueShape, mut flat: usize) -> Result<Box<[u32]>, Diagnostic> {
    let mut index = vec![0_u32; shape.rank()];
    for (axis, extent) in shape.extents().iter().enumerate().rev() {
        let extent = usize::try_from(extent.get())
            .map_err(|_| invalid_component_ir("shape extent exceeds local usize"))?;
        index[axis] = u32::try_from(flat % extent)
            .map_err(|_| invalid_component_ir("component index exceeds portable u32"))?;
        flat /= extent;
    }
    if flat != 0 {
        return Err(invalid_component_ir(
            "flat component index exceeds the exact shape",
        ));
    }
    Ok(index.into_boxed_slice())
}

fn invalid_component_ir(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_OPERATOR_IR, message)
        .with_graph_path(GraphPath::new(["operator-ir", "component-scalarization"]))
}

#[cfg(test)]
mod tests {
    use eqiora_core::entity::kinds;
    use eqiora_core::{DimExponents, Id, ValueShape};
    use eqiora_schema::kernel::pure_operator::PureOperatorDefinition;
    use eqiora_schema::kernel::typing::{
        ExpressionType, RootContract, SpatialSupport, TypedResidual,
    };
    use eqiora_schema::kernel::{ExprDagBuilder, SymbolRef, ValueFrame};

    use super::ComponentScalarization;

    #[test]
    fn vector_root_scalarizes_in_root_then_row_major_component_order() {
        let port = Id::<kinds::Port>::new();
        let parameter = Id::<kinds::Parameter>::new();
        let mut expression = ExprDagBuilder::new();
        let trace = expression.symbol(SymbolRef::PortTrace(port)).unwrap();
        let scale = expression.symbol(SymbolRef::Parameter(parameter)).unwrap();
        let scaled = expression.mul(trace, scale).unwrap();
        let dag = expression.finish([scaled]).unwrap();
        let vector = ValueShape::new([2]).unwrap();
        let typed =
            TypedResidual::infer(dag, None, RootContract::ComponentwiseResidual, |symbol| {
                Ok::<_, ()>(match symbol {
                    SymbolRef::PortTrace(_) => ExpressionType::shaped(
                        DimExponents::DIMENSIONLESS,
                        vector.clone(),
                        eqiora_schema::kernel::ValueFrame::SpatialCartesian,
                        None::<eqiora_schema::kernel::typing::SpatialSupport<()>>,
                    ),
                    SymbolRef::Parameter(_) => {
                        ExpressionType::scalar(DimExponents::DIMENSIONLESS, None)
                    }
                    _ => unreachable!(),
                })
            })
            .unwrap();
        let lowering = ComponentScalarization::lower(&typed).unwrap();

        assert_eq!(lowering.rows().len(), 2);
        assert_eq!(lowering.rows()[0].component_index(), [0]);
        assert_eq!(lowering.rows()[1].component_index(), [1]);
        let values = lowering
            .evaluate(|coordinate| match coordinate.symbol() {
                SymbolRef::PortTrace(_) => Some(if coordinate.component_index() == [0] {
                    2.0
                } else {
                    4.0
                }),
                SymbolRef::Parameter(_) => Some(3.0),
                _ => None,
            })
            .unwrap();
        assert_eq!(values, [6.0, 12.0]);
    }

    #[test]
    fn multiple_roots_preserve_root_then_last_axis_fastest_order() {
        let first = Id::<kinds::Port>::new();
        let second = Id::<kinds::Port>::new();
        let mut expression = ExprDagBuilder::new();
        let scalar_root = expression.symbol(SymbolRef::PortTrace(first)).unwrap();
        let tensor_root = expression.symbol(SymbolRef::PortFlux(second)).unwrap();
        let dag = expression.finish([scalar_root, tensor_root]).unwrap();
        let tensor = ValueShape::new([2, 2]).unwrap();
        let typed =
            TypedResidual::infer(dag, None, RootContract::ComponentwiseResidual, |symbol| {
                Ok::<_, ()>(match symbol {
                    SymbolRef::PortTrace(_) => {
                        ExpressionType::scalar(DimExponents::DIMENSIONLESS, None)
                    }
                    SymbolRef::PortFlux(_) => ExpressionType::shaped(
                        DimExponents::DIMENSIONLESS,
                        tensor.clone(),
                        eqiora_schema::kernel::ValueFrame::SpatialCartesian,
                        None::<eqiora_schema::kernel::typing::SpatialSupport<()>>,
                    ),
                    _ => unreachable!(),
                })
            })
            .unwrap();
        let lowering = ComponentScalarization::lower(&typed).unwrap();

        let order = lowering
            .rows()
            .iter()
            .map(|row| (row.root_index(), row.component_index().to_vec()))
            .collect::<Vec<_>>();
        assert_eq!(
            order,
            [
                (0, vec![]),
                (1, vec![0, 0]),
                (1, vec![0, 1]),
                (1, vec![1, 0]),
                (1, vec![1, 1]),
            ]
        );
    }

    #[test]
    fn symmetric_part_reads_direct_and_swapped_tensor_coordinates() {
        let stress = Id::<kinds::Port>::new();
        let mut expression = ExprDagBuilder::new();
        let tensor = expression.symbol(SymbolRef::PortFlux(stress)).unwrap();
        let symmetric = expression.symmetric_part(tensor).unwrap();
        let dag = expression.finish([symmetric]).unwrap();
        let tensor_shape = ValueShape::new([2, 2]).unwrap();
        let support = SpatialSupport::Volume {
            domain: "body",
            dimensions: 2,
        };
        let typed = TypedResidual::infer(
            dag,
            Some(support.clone()),
            RootContract::ComponentwiseResidual,
            |_| {
                Ok::<_, ()>(ExpressionType::shaped(
                    DimExponents::DIMENSIONLESS,
                    tensor_shape.clone(),
                    ValueFrame::SpatialCartesian,
                    Some(support.clone()),
                ))
            },
        )
        .unwrap();
        let lowering = ComponentScalarization::lower(&typed).unwrap();

        assert_eq!(
            lowering.rows()[1]
                .symbols()
                .iter()
                .map(|coordinate| coordinate.component_index().to_vec())
                .collect::<Vec<_>>(),
            [vec![0, 1], vec![1, 0]]
        );
        assert_eq!(
            lowering.rows()[2]
                .symbols()
                .iter()
                .map(|coordinate| coordinate.component_index().to_vec())
                .collect::<Vec<_>>(),
            [vec![1, 0], vec![0, 1]]
        );

        let values = lowering
            .evaluate(|coordinate| match coordinate.component_index() {
                [0, 0] => Some(2.0),
                [0, 1] => Some(6.0),
                [1, 0] => Some(10.0),
                [1, 1] => Some(4.0),
                _ => None,
            })
            .unwrap();
        assert_eq!(values, [2.0, 8.0, 8.0, 4.0]);
    }

    #[test]
    fn isotropic_lift_preserves_ordered_scalar_reads_for_every_component() {
        let pressure = Id::<kinds::Port>::new();
        let mut expression = ExprDagBuilder::new();
        let scalar = expression.symbol(SymbolRef::PortTrace(pressure)).unwrap();
        let isotropic = expression.isotropic_lift(scalar).unwrap();
        let dag = expression.finish([isotropic]).unwrap();
        let dimension =
            DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension");
        let support = SpatialSupport::Volume {
            domain: "body",
            dimensions: 2,
        };
        let typed = TypedResidual::infer(
            dag,
            Some(support.clone()),
            RootContract::ComponentwiseResidual,
            |_| Ok::<_, ()>(ExpressionType::scalar(dimension, Some(support.clone()))),
        )
        .unwrap();
        let lowering = ComponentScalarization::lower(&typed).unwrap();

        for row in lowering.rows() {
            assert_eq!(row.symbols().len(), 1);
            assert_eq!(row.symbols()[0].symbol(), SymbolRef::PortTrace(pressure));
            assert!(row.symbols()[0].component_index().is_empty());
        }

        let values = lowering.evaluate(|_| Some(7.0)).unwrap();
        assert_eq!(values, [7.0, 0.0, 0.0, 7.0]);
    }

    #[test]
    fn generic_dyadic_application_scalarizes_from_its_ordered_definition() {
        let left = Id::<kinds::Field>::new();
        let right = Id::<kinds::Field>::new();
        let definition = PureOperatorDefinition::dyadic_product().unwrap();
        let mut expression = ExprDagBuilder::new();
        let left_value = expression.symbol(SymbolRef::Field(left)).unwrap();
        let right_value = expression.symbol(SymbolRef::Field(right)).unwrap();
        let dyadic = expression
            .pure_operator(&definition, [left_value, right_value])
            .unwrap();
        let dag = expression.finish([dyadic]).unwrap();
        let support = SpatialSupport::Volume {
            domain: "body",
            dimensions: 2,
        };
        let vector_type = ExpressionType::shaped(
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2]).unwrap(),
            ValueFrame::SpatialCartesian,
            Some(support.clone()),
        );
        let typed = TypedResidual::infer(
            dag,
            Some(support),
            RootContract::ComponentwiseResidual,
            |_| Ok::<_, ()>(vector_type.clone()),
        )
        .unwrap();

        let lowering = ComponentScalarization::lower(&typed).unwrap();
        assert_eq!(lowering.rows().len(), 4);
        for row in lowering.rows() {
            assert_eq!(row.symbols().len(), 2);
            assert_eq!(row.symbols()[0].symbol(), SymbolRef::Field(left));
            assert_eq!(row.symbols()[1].symbol(), SymbolRef::Field(right));
            assert_eq!(
                row.symbols()[0].component_index(),
                &row.component_index()[..1]
            );
            assert_eq!(
                row.symbols()[1].component_index(),
                &row.component_index()[1..]
            );
        }

        let values = lowering
            .evaluate(|coordinate| match coordinate.symbol() {
                SymbolRef::Field(field) if field == left => match coordinate.component_index() {
                    [0] => Some(2.0),
                    [1] => Some(3.0),
                    _ => None,
                },
                SymbolRef::Field(field) if field == right => match coordinate.component_index() {
                    [0] => Some(5.0),
                    [1] => Some(7.0),
                    _ => None,
                },
                _ => None,
            })
            .unwrap();
        assert_eq!(values, [10.0, 14.0, 15.0, 21.0]);
    }
}
