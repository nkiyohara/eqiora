//! Pure, identity-parametric typing rules for residual expressions.
//!
//! The rules in this module know dimensions, scalar/tensor shape, and spatial
//! support, but nothing about source spans, graph paths, syntax, or storage.
//! Compilers and semantic validators supply their own identity type and map a
//! [`TypeViolation`] to the diagnostic location owned by their layer.

use core::fmt;

use eqiora_core::{DimExponents, ValueShape};

use super::pure_operator::PureOperatorError;
use super::{ExprDag, ExprId, ExprNode, SymbolRef, UnaryMathFunction, ValueFrame};

/// Exact spatial support carried by an expression value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SpatialSupport<I> {
    /// A Cartesian volume Domain.
    Volume {
        /// Nominal Domain identity.
        domain: I,
        /// Number of coordinate axes.
        dimensions: usize,
    },
    /// One boundary of a Cartesian volume Domain.
    Boundary {
        /// Nominal boundary Domain identity.
        domain: I,
        /// Exact nominal parent volume Domain.
        parent: I,
        /// Number of axes on the parent volume.
        dimensions: usize,
    },
    /// One validated maximal field-interface class.
    ///
    /// This is a derived typing support identified by its conserving
    /// Connection. It does not invent a canonical Domain and carries no mesh
    /// or transfer data.
    Interface {
        /// Exact maximal Connection identity.
        connection: I,
        /// Ambient Cartesian dimension of the coincident member boundaries.
        dimensions: usize,
    },
}

impl<I> SpatialSupport<I> {
    /// Nominal identity of this support.
    #[must_use]
    pub const fn domain(&self) -> &I {
        match self {
            Self::Volume { domain, .. } | Self::Boundary { domain, .. } => domain,
            Self::Interface { connection, .. } => connection,
        }
    }

    /// Ambient Cartesian dimension.
    #[must_use]
    pub const fn dimensions(&self) -> usize {
        match self {
            Self::Volume { dimensions, .. }
            | Self::Boundary { dimensions, .. }
            | Self::Interface { dimensions, .. } => *dimensions,
        }
    }

    /// Exact parent volume for a boundary support.
    #[must_use]
    pub const fn parent(&self) -> Option<&I> {
        match self {
            Self::Volume { .. } | Self::Interface { .. } => None,
            Self::Boundary { parent, .. } => Some(parent),
        }
    }
}

/// Complete static type of one residual-expression value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionType<I> {
    /// SI base-dimension exponents.
    pub dimension: DimExponents,
    /// Exact mathematical value shape.
    pub shape: ValueShape,
    /// Coordinate-frame meaning of the value components.
    pub frame: ValueFrame,
    /// Exact nominal spatial support, absent for global scalars.
    pub support: Option<SpatialSupport<I>>,
}

impl<I> ExpressionType<I> {
    /// A scalar with the supplied physical dimension and spatial support.
    #[must_use]
    pub fn scalar(dimension: DimExponents, support: Option<SpatialSupport<I>>) -> Self {
        Self {
            dimension,
            shape: ValueShape::scalar(),
            frame: ValueFrame::Invariant,
            support,
        }
    }

    /// A value with an exact shape, frame, physical dimension, and support.
    #[must_use]
    pub fn shaped(
        dimension: DimExponents,
        shape: ValueShape,
        frame: ValueFrame,
        support: Option<SpatialSupport<I>>,
    ) -> Self {
        Self {
            dimension,
            shape,
            frame,
            support,
        }
    }
}

/// Pure typing failure, retaining identities without choosing diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeViolation<I> {
    /// Additive operands have unequal dimensions or shapes.
    AdditiveTypeMismatch {
        /// Left operand type.
        left: Box<ExpressionType<I>>,
        /// Right operand type.
        right: Box<ExpressionType<I>>,
    },
    /// Two spatial supports are nominally different.
    IncompatibleSupport {
        /// Left spatial support.
        left: Box<SpatialSupport<I>>,
        /// Right spatial support.
        right: Box<SpatialSupport<I>>,
    },
    /// Physical-dimension exponent arithmetic overflowed.
    DimensionOverflow {
        /// Operation whose dimension arithmetic overflowed.
        operation: &'static str,
    },
    /// Multiplication received two non-scalar operands.
    MultiplicationRequiresScalar,
    /// Scalar scaling or a spatial operator received an incompatible frame.
    IncompatibleFrame,
    /// A division denominator was not scalar.
    DivisionDenominatorNotScalar,
    /// Integer power received a non-scalar base.
    PowerRequiresScalar,
    /// Sine received a dimensioned or non-scalar operand.
    SinRequiresDimensionlessScalar,
    /// A coordinate was used without a spatial Relation scope.
    CoordinateRequiresSpatialScope,
    /// A coordinate axis is outside the ambient dimension.
    CoordinateAxisOutOfRange {
        /// Requested zero-based axis.
        axis: usize,
        /// Ambient Cartesian dimension.
        dimensions: usize,
    },
    /// Gradient received an expression without spatial support.
    GradientRequiresSpatialSupport,
    /// Gradient support was not a volume Domain.
    GradientRequiresVolume,
    /// A spatial extent cannot be represented by the portable shape contract.
    SpatialExtentInvalid,
    /// Divergence received an expression without spatial support.
    DivergenceRequiresSpatialSupport,
    /// Divergence support was not a volume Domain.
    DivergenceRequiresVolume,
    /// Divergence received a scalar.
    DivergenceRequiresTensor,
    /// Symmetric part received an unsupported or non-volume value.
    SymmetricPartRequiresVolume,
    /// Symmetric part received something other than an exact `[d,d]`
    /// Cartesian tensor.
    SymmetricPartRequiresSquareSpatialTensor,
    /// Isotropic lift received an unsupported or non-volume value.
    IsotropicLiftRequiresVolume,
    /// Isotropic lift received something other than an invariant scalar.
    IsotropicLiftRequiresInvariantScalar,
    /// Trace or normal was used outside a boundary-scoped Relation.
    BoundaryOperatorRequiresBoundaryScope,
    /// Boundary operator input is not supported on the exact parent volume.
    BoundaryOperandSupportMismatch,
    /// Normal component received a scalar.
    NormalRequiresTensor,
    /// A content-addressed pure definition rejected its exact application.
    PureOperatorApplication(PureOperatorError),
    /// A root whose consumer requires one scalar value was shaped.
    RootRequiresScalar,
    /// Residual support differs from its Relation scope.
    ResidualSupportMismatch {
        /// Support inferred for the residual root.
        residual: Box<Option<SpatialSupport<I>>>,
        /// Support declared by the Relation.
        relation: Box<Option<SpatialSupport<I>>>,
    },
}

impl<I> TypeViolation<I> {
    /// Whether this violation is a numeric dimension/shape contract failure.
    #[must_use]
    pub const fn is_dimension_or_shape(&self) -> bool {
        matches!(
            self,
            Self::AdditiveTypeMismatch { .. }
                | Self::DimensionOverflow { .. }
                | Self::MultiplicationRequiresScalar
                | Self::IncompatibleFrame
                | Self::DivisionDenominatorNotScalar
                | Self::PowerRequiresScalar
                | Self::SinRequiresDimensionlessScalar
                | Self::SpatialExtentInvalid
                | Self::DivergenceRequiresTensor
                | Self::SymmetricPartRequiresSquareSpatialTensor
                | Self::IsotropicLiftRequiresInvariantScalar
                | Self::NormalRequiresTensor
                | Self::PureOperatorApplication(
                    PureOperatorError::FormalTypeMismatch
                        | PureOperatorError::ResultDimensionOverflow
                )
                | Self::RootRequiresScalar
        )
    }
}

impl<I: fmt::Debug> fmt::Display for TypeViolation<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AdditiveTypeMismatch { left, right } => write!(
                formatter,
                "addition/subtraction combines incompatible types {left:?} and {right:?}"
            ),
            Self::IncompatibleSupport { left, right } => {
                write!(
                    formatter,
                    "expression combines incompatible supports {left:?} and {right:?}"
                )
            }
            Self::DimensionOverflow { operation } => write!(
                formatter,
                "{operation} overflows the physical-dimension exponent range"
            ),
            Self::MultiplicationRequiresScalar => {
                formatter.write_str("multiplication requires at least one scalar operand")
            }
            Self::IncompatibleFrame => formatter
                .write_str("value frame is incompatible with this scalar or spatial operation"),
            Self::DivisionDenominatorNotScalar => {
                formatter.write_str("division denominator must be scalar")
            }
            Self::PowerRequiresScalar => {
                formatter.write_str("integer power requires a scalar operand")
            }
            Self::SinRequiresDimensionlessScalar => {
                formatter.write_str("sin requires a dimensionless scalar operand")
            }
            Self::CoordinateRequiresSpatialScope => {
                formatter.write_str("coordinate operator requires a Cartesian Relation scope")
            }
            Self::CoordinateAxisOutOfRange { axis, dimensions } => write!(
                formatter,
                "coordinate axis {axis} is outside Domain dimension {dimensions}"
            ),
            Self::GradientRequiresSpatialSupport => {
                formatter.write_str("gradient operand has no spatial Domain support")
            }
            Self::GradientRequiresVolume => {
                formatter.write_str("gradient requires a Cartesian volume Domain")
            }
            Self::SpatialExtentInvalid => {
                formatter.write_str("spatial extent is not a positive portable u32")
            }
            Self::DivergenceRequiresSpatialSupport => {
                formatter.write_str("divergence operand has no spatial Domain support")
            }
            Self::DivergenceRequiresVolume => {
                formatter.write_str("divergence requires a Cartesian volume Domain")
            }
            Self::DivergenceRequiresTensor => {
                formatter.write_str("divergence requires a spatial tensor operand")
            }
            Self::SymmetricPartRequiresVolume => {
                formatter.write_str("symmetric_part requires a Cartesian volume operand")
            }
            Self::SymmetricPartRequiresSquareSpatialTensor => formatter
                .write_str("symmetric_part requires an exact [d,d] spatial Cartesian tensor"),
            Self::IsotropicLiftRequiresVolume => {
                formatter.write_str("isotropic_lift requires a Cartesian volume operand")
            }
            Self::IsotropicLiftRequiresInvariantScalar => formatter
                .write_str("isotropic_lift requires an invariant scalar on its Cartesian volume"),
            Self::BoundaryOperatorRequiresBoundaryScope => {
                formatter.write_str("trace/normal operator requires an AppliesOn boundary Domain")
            }
            Self::BoundaryOperandSupportMismatch => formatter.write_str(
                "boundary operator operand must be supported on its exact parent Domain",
            ),
            Self::NormalRequiresTensor => {
                formatter.write_str("normal component requires a spatial tensor")
            }
            Self::PureOperatorApplication(error) => {
                write!(formatter, "pure operator application is invalid: {error}")
            }
            Self::RootRequiresScalar => {
                formatter.write_str("this expression root must be an invariant scalar")
            }
            Self::ResidualSupportMismatch { residual, relation } => write!(
                formatter,
                "residual support {residual:?} differs from Relation scope {relation:?}"
            ),
        }
    }
}

/// Meaning assigned to the roots of one typed expression DAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootContract {
    /// Every exact component of every root is an equation equal to zero.
    ComponentwiseResidual,
    /// Every root supplies one invariant scalar activation condition.
    ScalarActivation,
}

/// One fully inferred residual DAG.
///
/// Node types are constructed only by [`Self::infer`]. Operator lowerings can
/// therefore consume this value without accepting an unrelated shape
/// sidecar from their caller.
#[derive(Debug, Clone, PartialEq)]
pub struct TypedResidual<I> {
    expression: ExprDag,
    node_types: Box<[ExpressionType<I>]>,
}

impl<I> TypedResidual<I> {
    /// Structurally validated expression DAG.
    #[must_use]
    pub const fn expression(&self) -> &ExprDag {
        &self.expression
    }

    /// Exact inferred type of every DAG node in arena order.
    #[must_use]
    pub const fn node_types(&self) -> &[ExpressionType<I>] {
        &self.node_types
    }

    /// Exact inferred type of one DAG node.
    #[must_use]
    pub fn node_type(&self, id: ExprId) -> Option<&ExpressionType<I>> {
        usize::try_from(id.index())
            .ok()
            .and_then(|index| self.node_types.get(index))
    }
}

/// One local failure while inferring a [`TypedResidual`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypedResidualError<I, E> {
    /// A symbol could not be typed by the owning semantic environment.
    Symbol {
        /// Zero-based expression node index.
        node_index: u32,
        /// Exact symbol whose semantic environment rejected it.
        symbol: SymbolRef,
        /// Environment-owned reason.
        error: E,
    },
    /// A pure expression typing rule failed.
    Type {
        /// Zero-based expression node or root index.
        node_index: u32,
        /// Exact rule violation.
        error: TypeViolation<I>,
    },
}

impl<I: Clone + Eq> TypedResidual<I> {
    /// Infer one complete DAG from a semantic symbol resolver.
    ///
    /// Independent errors are accumulated in arena/root order. Nodes whose
    /// operands failed do not emit cascading diagnostics. A value is returned
    /// only when every node and root satisfies the selected contract.
    pub fn infer<E>(
        expression: ExprDag,
        relation_support: Option<SpatialSupport<I>>,
        root_contract: RootContract,
        mut symbol_type: impl FnMut(SymbolRef) -> Result<ExpressionType<I>, E>,
    ) -> Result<Self, Vec<TypedResidualError<I, E>>> {
        let mut inferred =
            Vec::<Option<ExpressionType<I>>>::with_capacity(expression.nodes().len());
        let mut errors = Vec::new();

        for (index, node) in expression.nodes().iter().enumerate() {
            let node_index = u32::try_from(index).expect("ExprDag indices are portable u32");
            let result = infer_node(
                &expression,
                node,
                &inferred,
                relation_support.as_ref(),
                &mut symbol_type,
            );
            let value = match result {
                NodeInference::Typed(value) => Some(value),
                NodeInference::Unavailable => None,
                NodeInference::Symbol { symbol, error } => {
                    errors.push(TypedResidualError::Symbol {
                        node_index,
                        symbol,
                        error,
                    });
                    None
                }
                NodeInference::Type(error) => {
                    errors.push(TypedResidualError::Type { node_index, error });
                    None
                }
            };
            inferred.push(value);
        }

        for root in expression.roots() {
            let Some(root_type) = inferred_type(&inferred, *root) else {
                continue;
            };
            let result = match root_contract {
                RootContract::ComponentwiseResidual => {
                    residual(&root_type, relation_support.as_ref())
                }
                RootContract::ScalarActivation => {
                    scalar_root(&root_type, relation_support.as_ref())
                }
            };
            if let Err(error) = result {
                errors.push(TypedResidualError::Type {
                    node_index: root.index(),
                    error,
                });
            }
        }

        if errors.is_empty() {
            Ok(Self {
                expression,
                node_types: inferred
                    .into_iter()
                    .map(|value| value.expect("error-free inference types every node"))
                    .collect(),
            })
        } else {
            Err(errors)
        }
    }
}

enum NodeInference<I, E> {
    Typed(ExpressionType<I>),
    Unavailable,
    Symbol { symbol: SymbolRef, error: E },
    Type(TypeViolation<I>),
}

fn infer_node<I: Clone + Eq, E>(
    expression: &ExprDag,
    node: &ExprNode,
    inferred: &[Option<ExpressionType<I>>],
    relation_support: Option<&SpatialSupport<I>>,
    symbol_type: &mut impl FnMut(SymbolRef) -> Result<ExpressionType<I>, E>,
) -> NodeInference<I, E> {
    let typed = match node {
        ExprNode::Constant(value) => Ok(ExpressionType::scalar(value.dim(), None)),
        ExprNode::Symbol(symbol) => {
            return match symbol_type(*symbol) {
                Ok(value) => NodeInference::Typed(value),
                Err(error) => NodeInference::Symbol {
                    symbol: *symbol,
                    error,
                },
            };
        }
        ExprNode::Neg(value) => {
            return inferred_type(inferred, *value)
                .map_or(NodeInference::Unavailable, NodeInference::Typed);
        }
        ExprNode::Add(left, right) | ExprNode::Sub(left, right) => {
            let Some((left, right)) = inferred_binary(inferred, *left, *right) else {
                return NodeInference::Unavailable;
            };
            additive(&left, &right)
        }
        ExprNode::Mul(left, right) => {
            let Some((left, right)) = inferred_binary(inferred, *left, *right) else {
                return NodeInference::Unavailable;
            };
            multiply(&left, &right)
        }
        ExprNode::Div(left, right) => {
            let Some((left, right)) = inferred_binary(inferred, *left, *right) else {
                return NodeInference::Unavailable;
            };
            divide(&left, &right)
        }
        ExprNode::PowI(base, exponent) => {
            let Some(base) = inferred_type(inferred, *base) else {
                return NodeInference::Unavailable;
            };
            power(&base, *exponent)
        }
        ExprNode::SpatialCoordinate(axis) => coordinate(*axis, relation_support),
        ExprNode::UnaryMath(UnaryMathFunction::Sin, value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            sine(&value)
        }
        ExprNode::Gradient(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            gradient(&value)
        }
        ExprNode::Divergence(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            divergence(&value)
        }
        ExprNode::SymmetricPart(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            symmetric_part(&value)
        }
        ExprNode::IsotropicLift(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            isotropic_lift(&value)
        }
        ExprNode::Trace(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            trace(&value, relation_support)
        }
        ExprNode::NormalComponent(value) => {
            let Some(value) = inferred_type(inferred, *value) else {
                return NodeInference::Unavailable;
            };
            normal(&value, relation_support)
        }
        ExprNode::PureOperatorApplication(application) => {
            let mut arguments = Vec::with_capacity(application.arguments().len());
            for argument in application.arguments() {
                let Some(argument_type) = inferred_type(inferred, *argument) else {
                    return NodeInference::Unavailable;
                };
                arguments.push(argument_type);
            }
            let definition = expression
                .definition(application.definition())
                .expect("ExprDag keeps every pure application definition in its closed table");
            definition
                .instantiate(&arguments)
                .map(|application| application.result_type().clone())
                .map_err(TypeViolation::PureOperatorApplication)
        }
    };
    match typed {
        Ok(value) => NodeInference::Typed(value),
        Err(error) => NodeInference::Type(error),
    }
}

fn inferred_binary<I: Clone>(
    inferred: &[Option<ExpressionType<I>>],
    left: ExprId,
    right: ExprId,
) -> Option<(ExpressionType<I>, ExpressionType<I>)> {
    Some((
        inferred_type(inferred, left)?,
        inferred_type(inferred, right)?,
    ))
}

fn inferred_type<I: Clone>(
    inferred: &[Option<ExpressionType<I>>],
    id: ExprId,
) -> Option<ExpressionType<I>> {
    usize::try_from(id.index())
        .ok()
        .and_then(|index| inferred.get(index))
        .cloned()
        .flatten()
}

/// Add or subtract two typed expressions.
pub fn additive<I: Clone + Eq>(
    left: &ExpressionType<I>,
    right: &ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    if left.dimension != right.dimension || left.shape != right.shape || left.frame != right.frame {
        return Err(TypeViolation::AdditiveTypeMismatch {
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        });
    }
    Ok(ExpressionType {
        dimension: left.dimension,
        shape: left.shape.clone(),
        frame: left.frame,
        support: combine_support(&left.support, &right.support)?,
    })
}

/// Multiply two typed expressions using scalar-times-tensor v0 semantics.
pub fn multiply<I: Clone + Eq>(
    left: &ExpressionType<I>,
    right: &ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let (shape, frame) = if left.shape.is_scalar() && left.frame == ValueFrame::Invariant {
        (right.shape.clone(), right.frame)
    } else if right.shape.is_scalar() && right.frame == ValueFrame::Invariant {
        (left.shape.clone(), left.frame)
    } else {
        return Err(TypeViolation::MultiplicationRequiresScalar);
    };
    Ok(ExpressionType {
        dimension: combine_dimensions(left.dimension, right.dimension, i8::checked_add).ok_or(
            TypeViolation::DimensionOverflow {
                operation: "multiplication",
            },
        )?,
        shape,
        frame,
        support: combine_support(&left.support, &right.support)?,
    })
}

/// Divide by one typed scalar expression.
pub fn divide<I: Clone + Eq>(
    numerator: &ExpressionType<I>,
    denominator: &ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    if !denominator.shape.is_scalar() || denominator.frame != ValueFrame::Invariant {
        return Err(TypeViolation::DivisionDenominatorNotScalar);
    }
    Ok(ExpressionType {
        dimension: combine_dimensions(numerator.dimension, denominator.dimension, i8::checked_sub)
            .ok_or(TypeViolation::DimensionOverflow {
                operation: "division",
            })?,
        shape: numerator.shape.clone(),
        frame: numerator.frame,
        support: combine_support(&numerator.support, &denominator.support)?,
    })
}

/// Raise one scalar expression to an integer power.
pub fn power<I: Clone>(
    base: &ExpressionType<I>,
    exponent: i32,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    if !base.shape.is_scalar() || base.frame != ValueFrame::Invariant {
        return Err(TypeViolation::PowerRequiresScalar);
    }
    Ok(ExpressionType {
        dimension: scale_dimension(base.dimension, exponent).ok_or(
            TypeViolation::DimensionOverflow {
                operation: "integer power",
            },
        )?,
        shape: base.shape.clone(),
        frame: base.frame,
        support: base.support.clone(),
    })
}

/// Type one Cartesian coordinate in the Relation scope.
pub fn coordinate<I: Clone>(
    axis: usize,
    relation: Option<&SpatialSupport<I>>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let support = relation.ok_or(TypeViolation::CoordinateRequiresSpatialScope)?;
    if axis >= support.dimensions() {
        return Err(TypeViolation::CoordinateAxisOutOfRange {
            axis,
            dimensions: support.dimensions(),
        });
    }
    Ok(ExpressionType::scalar(
        DimExponents {
            length: 1,
            ..DimExponents::DIMENSIONLESS
        },
        Some(support.clone()),
    ))
}

/// Type a sine application.
pub fn sine<I: Clone>(operand: &ExpressionType<I>) -> Result<ExpressionType<I>, TypeViolation<I>> {
    if !operand.shape.is_scalar()
        || operand.dimension != DimExponents::DIMENSIONLESS
        || operand.frame != ValueFrame::Invariant
    {
        return Err(TypeViolation::SinRequiresDimensionlessScalar);
    }
    Ok(operand.clone())
}

/// Type a physical-space gradient.
pub fn gradient<I: Clone>(
    operand: &ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let support = operand
        .support
        .as_ref()
        .ok_or(TypeViolation::GradientRequiresSpatialSupport)?;
    if !matches!(support, SpatialSupport::Volume { .. }) {
        return Err(TypeViolation::GradientRequiresVolume);
    }
    if (operand.shape.is_scalar() && operand.frame != ValueFrame::Invariant)
        || (!operand.shape.is_scalar() && operand.frame != ValueFrame::SpatialCartesian)
    {
        return Err(TypeViolation::IncompatibleFrame);
    }
    let extent = u32::try_from(support.dimensions())
        .ok()
        .filter(|extent| *extent > 0)
        .ok_or(TypeViolation::SpatialExtentInvalid)?;
    let shape = operand
        .shape
        .appended(extent)
        .map_err(|_| TypeViolation::SpatialExtentInvalid)?;
    Ok(ExpressionType {
        dimension: spatial_derivative_dimension(operand.dimension)?,
        shape,
        frame: ValueFrame::SpatialCartesian,
        support: operand.support.clone(),
    })
}

/// Type a physical-space divergence.
pub fn divergence<I: Clone>(
    operand: &ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let support = operand
        .support
        .as_ref()
        .ok_or(TypeViolation::DivergenceRequiresSpatialSupport)?;
    if !matches!(support, SpatialSupport::Volume { .. }) {
        return Err(TypeViolation::DivergenceRequiresVolume);
    }
    let Some((shape, last)) = operand.shape.remove_last() else {
        return Err(TypeViolation::DivergenceRequiresTensor);
    };
    if operand.frame != ValueFrame::SpatialCartesian {
        return Err(TypeViolation::IncompatibleFrame);
    }
    if usize::try_from(last.get()).ok() != Some(support.dimensions()) {
        return Err(TypeViolation::DivergenceRequiresTensor);
    }
    Ok(ExpressionType {
        dimension: spatial_derivative_dimension(operand.dimension)?,
        frame: if shape.is_scalar() {
            ValueFrame::Invariant
        } else {
            ValueFrame::SpatialCartesian
        },
        shape,
        support: operand.support.clone(),
    })
}

/// Type the symmetric part of an exact square Cartesian tensor.
pub fn symmetric_part<I: Clone>(
    operand: &ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let Some(SpatialSupport::Volume { dimensions, .. }) = operand.support.as_ref() else {
        return Err(TypeViolation::SymmetricPartRequiresVolume);
    };
    let extents = operand.shape.extents();
    if operand.frame != ValueFrame::SpatialCartesian
        || extents.len() != 2
        || usize::try_from(extents[0].get()).ok() != Some(*dimensions)
        || usize::try_from(extents[1].get()).ok() != Some(*dimensions)
    {
        return Err(TypeViolation::SymmetricPartRequiresSquareSpatialTensor);
    }
    Ok(operand.clone())
}

/// Type an isotropic lift whose tensor extent comes solely from volume
/// support.
pub fn isotropic_lift<I: Clone>(
    operand: &ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let Some(SpatialSupport::Volume { dimensions, .. }) = operand.support.as_ref() else {
        return Err(TypeViolation::IsotropicLiftRequiresVolume);
    };
    if !operand.shape.is_scalar() || operand.frame != ValueFrame::Invariant {
        return Err(TypeViolation::IsotropicLiftRequiresInvariantScalar);
    }
    let extent = u32::try_from(*dimensions)
        .ok()
        .filter(|extent| *extent > 0)
        .ok_or(TypeViolation::SpatialExtentInvalid)?;
    let shape =
        ValueShape::new([extent, extent]).map_err(|_| TypeViolation::SpatialExtentInvalid)?;
    Ok(ExpressionType {
        dimension: operand.dimension,
        shape,
        frame: ValueFrame::SpatialCartesian,
        support: operand.support.clone(),
    })
}

/// Type a boundary trace.
pub fn trace<I: Clone + Eq>(
    operand: &ExpressionType<I>,
    relation: Option<&SpatialSupport<I>>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    boundary_operator(operand, relation, false)
}

/// Type an outward-normal contraction.
pub fn normal<I: Clone + Eq>(
    operand: &ExpressionType<I>,
    relation: Option<&SpatialSupport<I>>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    boundary_operator(operand, relation, true)
}

/// Check one residual root against its Relation scope.
pub fn residual<I: Clone + Eq>(
    root: &ExpressionType<I>,
    relation: Option<&SpatialSupport<I>>,
) -> Result<(), TypeViolation<I>> {
    if root.support.as_ref().map(SpatialSupport::domain) != relation.map(SpatialSupport::domain) {
        return Err(TypeViolation::ResidualSupportMismatch {
            residual: Box::new(root.support.clone()),
            relation: Box::new(relation.cloned()),
        });
    }
    Ok(())
}

/// Check one activation root, which must remain an invariant scalar.
pub fn scalar_root<I: Clone + Eq>(
    root: &ExpressionType<I>,
    relation: Option<&SpatialSupport<I>>,
) -> Result<(), TypeViolation<I>> {
    if !root.shape.is_scalar() || root.frame != ValueFrame::Invariant {
        return Err(TypeViolation::RootRequiresScalar);
    }
    residual(root, relation)
}

/// Divide a dimension by time for a Field derivative.
pub fn time_derivative<I: Clone>(
    operand: &ExpressionType<I>,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    Ok(ExpressionType {
        dimension: combine_dimensions(
            operand.dimension,
            DimExponents {
                time: 1,
                ..DimExponents::DIMENSIONLESS
            },
            i8::checked_sub,
        )
        .ok_or(TypeViolation::DimensionOverflow {
            operation: "Field derivative",
        })?,
        shape: operand.shape.clone(),
        frame: operand.frame,
        support: operand.support.clone(),
    })
}

fn boundary_operator<I: Clone + Eq>(
    operand: &ExpressionType<I>,
    relation: Option<&SpatialSupport<I>>,
    normal_component: bool,
) -> Result<ExpressionType<I>, TypeViolation<I>> {
    let Some(SpatialSupport::Boundary {
        parent,
        domain,
        dimensions,
    }) = relation
    else {
        return Err(TypeViolation::BoundaryOperatorRequiresBoundaryScope);
    };
    if operand.support.as_ref().map(SpatialSupport::domain) != Some(parent) {
        return Err(TypeViolation::BoundaryOperandSupportMismatch);
    }
    let shape = if normal_component {
        let Some((shape, last)) = operand.shape.remove_last() else {
            return Err(TypeViolation::NormalRequiresTensor);
        };
        if usize::try_from(last.get()).ok() != Some(*dimensions) {
            return Err(TypeViolation::NormalRequiresTensor);
        }
        if operand.frame != ValueFrame::SpatialCartesian {
            return Err(TypeViolation::IncompatibleFrame);
        }
        shape
    } else {
        operand.shape.clone()
    };
    Ok(ExpressionType {
        dimension: operand.dimension,
        frame: if shape.is_scalar() {
            ValueFrame::Invariant
        } else {
            operand.frame
        },
        shape,
        support: Some(SpatialSupport::Boundary {
            domain: domain.clone(),
            parent: parent.clone(),
            dimensions: *dimensions,
        }),
    })
}

fn combine_support<I: Clone + Eq>(
    left: &Option<SpatialSupport<I>>,
    right: &Option<SpatialSupport<I>>,
) -> Result<Option<SpatialSupport<I>>, TypeViolation<I>> {
    match (left, right) {
        (None, support) | (support, None) => Ok(support.clone()),
        (Some(left), Some(right)) if left == right => Ok(Some(left.clone())),
        (Some(left), Some(right)) => Err(TypeViolation::IncompatibleSupport {
            left: Box::new(left.clone()),
            right: Box::new(right.clone()),
        }),
    }
}

fn spatial_derivative_dimension<I>(
    dimension: DimExponents,
) -> Result<DimExponents, TypeViolation<I>> {
    combine_dimensions(
        dimension,
        DimExponents {
            length: 1,
            ..DimExponents::DIMENSIONLESS
        },
        i8::checked_sub,
    )
    .ok_or(TypeViolation::DimensionOverflow {
        operation: "spatial derivative",
    })
}

fn combine_dimensions(
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

fn scale_dimension(dimension: DimExponents, exponent: i32) -> Option<DimExponents> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::Id;
    use eqiora_core::entity::kinds;

    fn volume(name: &'static str) -> SpatialSupport<&'static str> {
        SpatialSupport::Volume {
            domain: name,
            dimensions: 2,
        }
    }

    #[test]
    fn spatial_rules_are_identity_parametric_and_shape_aware() {
        let scalar = ExpressionType::scalar(DimExponents::DIMENSIONLESS, Some(volume("left")));
        let gradient = gradient(&scalar).expect("gradient");
        assert_eq!(gradient.shape.extents()[0].get(), 2);
        assert!(divergence(&scalar).is_err());

        let other = ExpressionType::scalar(DimExponents::DIMENSIONLESS, Some(volume("right")));
        assert!(matches!(
            additive(&scalar, &other),
            Err(TypeViolation::IncompatibleSupport { .. })
        ));
    }

    #[test]
    fn tensor_structure_comes_only_from_exact_spatial_types() {
        let dimension = DimExponents {
            mass: 1,
            length: -1,
            time: -2,
            ..DimExponents::DIMENSIONLESS
        };
        let tensor = ExpressionType::shaped(
            dimension,
            ValueShape::new([2, 2]).unwrap(),
            ValueFrame::SpatialCartesian,
            Some(volume("body")),
        );
        assert_eq!(symmetric_part(&tensor).unwrap(), tensor);

        for shape in [
            ValueShape::new([2]).unwrap(),
            ValueShape::new([2, 3]).unwrap(),
        ] {
            let invalid = ExpressionType::shaped(
                dimension,
                shape,
                ValueFrame::SpatialCartesian,
                Some(volume("body")),
            );
            assert!(matches!(
                symmetric_part(&invalid),
                Err(TypeViolation::SymmetricPartRequiresSquareSpatialTensor)
            ));
        }
        let wrong_frame = ExpressionType::shaped(
            dimension,
            ValueShape::new([2, 2]).unwrap(),
            ValueFrame::Invariant,
            Some(volume("body")),
        );
        assert!(symmetric_part(&wrong_frame).is_err());

        let scalar = ExpressionType::scalar(dimension, Some(volume("body")));
        let isotropic = isotropic_lift(&scalar).unwrap();
        assert_eq!(isotropic.dimension, dimension);
        assert_eq!(isotropic.shape, ValueShape::new([2, 2]).unwrap());
        assert_eq!(isotropic.frame, ValueFrame::SpatialCartesian);
        assert_eq!(isotropic.support, scalar.support);

        let global = ExpressionType::<&str>::scalar(dimension, None);
        assert!(matches!(
            isotropic_lift(&global),
            Err(TypeViolation::IsotropicLiftRequiresVolume)
        ));
        assert!(matches!(
            isotropic_lift(&tensor),
            Err(TypeViolation::IsotropicLiftRequiresInvariantScalar)
        ));
    }

    #[test]
    fn tensor_structure_rejects_boundary_support() {
        let boundary = SpatialSupport::Boundary {
            domain: "wall",
            parent: "body",
            dimensions: 2,
        };
        let tensor = ExpressionType::shaped(
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2, 2]).unwrap(),
            ValueFrame::SpatialCartesian,
            Some(boundary.clone()),
        );
        let scalar = ExpressionType::scalar(DimExponents::DIMENSIONLESS, Some(boundary));
        assert!(matches!(
            symmetric_part(&tensor),
            Err(TypeViolation::SymmetricPartRequiresVolume)
        ));
        assert!(matches!(
            isotropic_lift(&scalar),
            Err(TypeViolation::IsotropicLiftRequiresVolume)
        ));
    }

    #[test]
    fn typed_residual_separates_componentwise_relations_from_scalar_activations() {
        let port = Id::<kinds::Port>::new();
        let mut builder = super::super::ExprDagBuilder::new();
        let root = builder.symbol(SymbolRef::PortTrace(port)).unwrap();
        let expression = builder.finish([root]).unwrap();
        let vector = ExpressionType::shaped(
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2]).unwrap(),
            ValueFrame::SpatialCartesian,
            None::<SpatialSupport<RawTestId>>,
        );

        let typed = TypedResidual::infer(
            expression.clone(),
            None,
            RootContract::ComponentwiseResidual,
            |_| Ok::<_, ()>(vector.clone()),
        )
        .unwrap();
        assert_eq!(typed.node_type(root).unwrap().shape.extents()[0].get(), 2);

        let errors = TypedResidual::infer(expression, None, RootContract::ScalarActivation, |_| {
            Ok::<_, ()>(vector.clone())
        })
        .unwrap_err();
        assert!(matches!(
            errors.as_slice(),
            [TypedResidualError::Type {
                error: TypeViolation::RootRequiresScalar,
                ..
            }]
        ));
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct RawTestId;

    #[test]
    fn coordinate_and_boundary_rules_use_relation_support() {
        assert!(matches!(
            coordinate::<&str>(0, None),
            Err(TypeViolation::CoordinateRequiresSpatialScope)
        ));
        assert!(matches!(
            coordinate(2, Some(&volume("body"))),
            Err(TypeViolation::CoordinateAxisOutOfRange { .. })
        ));

        let boundary = SpatialSupport::Boundary {
            domain: "wall",
            parent: "body",
            dimensions: 2,
        };
        let body = ExpressionType::scalar(DimExponents::DIMENSIONLESS, Some(volume("body")));
        assert_eq!(
            trace(&body, Some(&boundary))
                .expect("trace")
                .support
                .as_ref()
                .map(SpatialSupport::domain),
            Some(&"wall")
        );
        assert!(normal(&body, Some(&boundary)).is_err());
    }

    #[test]
    fn generic_pure_application_derives_shape_support_and_dimension_from_its_table() {
        let left = Id::<kinds::Field>::new();
        let right = Id::<kinds::Field>::new();
        let definition = crate::kernel::pure_operator::PureOperatorDefinition::dyadic_product()
            .expect("standard definition");
        let mut builder = super::super::ExprDagBuilder::new();
        let left_value = builder.symbol(SymbolRef::Field(left)).unwrap();
        let right_value = builder.symbol(SymbolRef::Field(right)).unwrap();
        let product = builder
            .pure_operator(&definition, [left_value, right_value])
            .unwrap();
        let expression = builder.finish([product]).unwrap();
        let length = DimExponents {
            length: 1,
            ..DimExponents::DIMENSIONLESS
        };
        let inverse_time = DimExponents {
            time: -1,
            ..DimExponents::DIMENSIONLESS
        };

        let typed = TypedResidual::infer(
            expression,
            Some(volume("body")),
            RootContract::ComponentwiseResidual,
            |symbol| {
                let dimension = match symbol {
                    SymbolRef::Field(field) if field == left => length,
                    SymbolRef::Field(field) if field == right => inverse_time,
                    _ => unreachable!(),
                };
                Ok::<_, ()>(ExpressionType::shaped(
                    dimension,
                    ValueShape::new([2]).unwrap(),
                    ValueFrame::SpatialCartesian,
                    Some(volume("body")),
                ))
            },
        )
        .unwrap();

        let result = typed.node_type(product).unwrap();
        assert_eq!(result.shape, ValueShape::new([2, 2]).unwrap());
        assert_eq!(result.support, Some(volume("body")));
        assert_eq!(
            result.dimension,
            DimExponents {
                length: 1,
                time: -1,
                ..DimExponents::DIMENSIONLESS
            }
        );
    }

    #[test]
    fn generic_pure_application_rejects_argument_type_and_support_mismatches() {
        let left = Id::<kinds::Field>::new();
        let right = Id::<kinds::Field>::new();
        let definition = crate::kernel::pure_operator::PureOperatorDefinition::dyadic_product()
            .expect("standard definition");
        let expression = {
            let mut builder = super::super::ExprDagBuilder::new();
            let left = builder.symbol(SymbolRef::Field(left)).unwrap();
            let right = builder.symbol(SymbolRef::Field(right)).unwrap();
            let product = builder.pure_operator(&definition, [left, right]).unwrap();
            builder.finish([product]).unwrap()
        };
        let vector = |domain| {
            ExpressionType::shaped(
                DimExponents::DIMENSIONLESS,
                ValueShape::new([2]).unwrap(),
                ValueFrame::SpatialCartesian,
                Some(volume(domain)),
            )
        };

        let support_errors = TypedResidual::infer(
            expression.clone(),
            Some(volume("body")),
            RootContract::ComponentwiseResidual,
            |symbol| {
                Ok::<_, ()>(match symbol {
                    SymbolRef::Field(field) if field == left => vector("body"),
                    SymbolRef::Field(field) if field == right => vector("other"),
                    _ => unreachable!(),
                })
            },
        )
        .unwrap_err();
        assert!(matches!(
            support_errors.as_slice(),
            [TypedResidualError::Type {
                error: TypeViolation::PureOperatorApplication(
                    PureOperatorError::CommonVolumeMismatch
                ),
                ..
            }]
        ));

        let type_errors = TypedResidual::infer(
            expression,
            Some(volume("body")),
            RootContract::ComponentwiseResidual,
            |symbol| {
                Ok::<_, ()>(match symbol {
                    SymbolRef::Field(field) if field == left => vector("body"),
                    SymbolRef::Field(field) if field == right => {
                        ExpressionType::scalar(DimExponents::DIMENSIONLESS, Some(volume("body")))
                    }
                    _ => unreachable!(),
                })
            },
        )
        .unwrap_err();
        assert!(matches!(
            type_errors.as_slice(),
            [TypedResidualError::Type {
                error: TypeViolation::PureOperatorApplication(
                    PureOperatorError::FormalTypeMismatch
                ),
                ..
            }]
        ));
    }
}
