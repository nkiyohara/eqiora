//! The expression DAG on the wire.
//!
//! Operands are indices into one flat node list, so an expression naming a node
//! outside its own list is refused rather than resolved elsewhere.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::entity::kinds;
use eqiora_schema::kernel::pure_operator::PureOperatorDefinition;
use eqiora_schema::kernel::{
    ExprDag, ExprDagBuilder, ExprId, ExprNode, SymbolRef, UnaryMathFunction,
};
use serde::{Deserialize, Serialize};

use crate::invalid_artifact;

use super::*;
use super::{node::*, primitive::*, vocabulary::*};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireExpression {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) definitions: Vec<WirePureOperatorDefinition>,
    pub(crate) nodes: Vec<WireExpressionNode>,
    pub(crate) roots: Vec<u32>,
}

impl WireExpression {
    pub(crate) fn encode(expression: &ExprDag, version: WireVersion) -> Result<Self, Diagnostic> {
        Ok(Self {
            definitions: if version.supports_pure_operators() {
                expression
                    .definitions()
                    .values()
                    .map(WirePureOperatorDefinition::encode)
                    .collect()
            } else {
                Vec::new()
            },
            nodes: expression
                .nodes()
                .iter()
                .map(|node| WireExpressionNode::encode(node, version))
                .collect::<Result<Vec<_>, _>>()?,
            roots: expression.roots().iter().map(|root| root.index()).collect(),
        })
    }

    pub(crate) fn decode(&self) -> Result<ExprDag, Diagnostic> {
        if self.nodes.is_empty() || self.roots.is_empty() {
            return Err(invalid_artifact(
                "wire expression requires non-empty nodes and roots",
            ));
        }
        let definitions = self.decode_definitions()?;
        let mut builder = ExprDagBuilder::new();
        let mut ids = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let id = node.decode(&mut builder, &ids, &definitions)?;
            ids.push(id);
        }
        let referenced_definitions = self
            .nodes
            .iter()
            .filter_map(|node| match node {
                WireExpressionNode::PureOperatorApplication { definition, .. } => {
                    Some(definition.as_str())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let supplied_definitions = definitions
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        if referenced_definitions != supplied_definitions {
            return Err(invalid_artifact(
                "wire expression must supply exactly its referenced pure-operator definitions",
            ));
        }
        let roots = self
            .roots
            .iter()
            .map(|&root| operand(&ids, root))
            .collect::<Result<Vec<_>, _>>()?;
        builder
            .finish(roots)
            .map_err(|error| invalid_artifact(error.message()))
    }

    pub(crate) fn pure_operator_counts(&self) -> Result<PureOperatorWireCounts, Diagnostic> {
        let formals = checked_count_sum(
            self.definitions
                .iter()
                .map(|definition| definition.formals.len()),
            "pure-operator formal count",
        )?;
        let calculus_nodes = checked_count_sum(
            self.definitions
                .iter()
                .map(|definition| definition.nodes.len()),
            "pure-operator calculus-node count",
        )?;
        let application_arguments = checked_count_sum(
            self.nodes.iter().map(WireExpressionNode::application_arity),
            "pure-operator application-argument count",
        )?;
        Ok(PureOperatorWireCounts {
            definitions: self.definitions.len(),
            formals,
            calculus_nodes,
            application_arguments,
        })
    }

    pub(crate) fn validate_v5_features(&self) -> Result<(), Diagnostic> {
        for definition in &self.definitions {
            definition.validate_features()?;
        }
        Ok(())
    }

    pub(crate) fn canonicalize_v5_definitions(&mut self) -> Result<(), Diagnostic> {
        for definition in &self.definitions {
            definition.rebuild_and_validate_digest()?;
        }
        self.definitions
            .sort_by(|left, right| left.digest.cmp(&right.digest));
        if self
            .definitions
            .windows(2)
            .any(|pair| pair[0].digest == pair[1].digest)
        {
            return Err(invalid_artifact(
                "wire expression contains a duplicate pure-operator definition digest",
            ));
        }
        Ok(())
    }

    pub(crate) fn decode_definitions(
        &self,
    ) -> Result<BTreeMap<String, PureOperatorDefinition>, Diagnostic> {
        let mut definitions = BTreeMap::new();
        for definition in &self.definitions {
            let rebuilt = definition.rebuild_and_validate_digest()?;
            if definitions
                .insert(definition.digest.clone(), rebuilt)
                .is_some()
            {
                return Err(invalid_artifact(
                    "wire expression contains a duplicate pure-operator definition digest",
                ));
            }
        }
        Ok(definitions)
    }

    pub(crate) fn ensure_v1(&self) -> Result<(), Diagnostic> {
        if self.nodes.iter().any(|node| {
            matches!(
                node,
                WireExpressionNode::Symbol {
                    symbol: WireSymbol::Across { .. }
                        | WireSymbol::Through { .. }
                        | WireSymbol::PortTrace { .. }
                        | WireSymbol::PortFlux { .. }
                } | WireExpressionNode::SymmetricPart { .. }
                    | WireExpressionNode::IsotropicLift { .. }
                    | WireExpressionNode::PureOperatorApplication { .. }
            )
        }) || !self.definitions.is_empty()
        {
            Err(invalid_artifact(
                "model wire v1 cannot contain physical interface symbols or tensor operators",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_v2(&self) -> Result<(), Diagnostic> {
        if self.nodes.iter().any(|node| {
            matches!(
                node,
                WireExpressionNode::Symbol {
                    symbol: WireSymbol::PortTrace { .. } | WireSymbol::PortFlux { .. }
                } | WireExpressionNode::SymmetricPart { .. }
                    | WireExpressionNode::IsotropicLift { .. }
                    | WireExpressionNode::PureOperatorApplication { .. }
            )
        }) || !self.definitions.is_empty()
        {
            Err(invalid_artifact(
                "model wire v2 cannot contain boundary symbols or tensor operators",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_v3(&self) -> Result<(), Diagnostic> {
        if self.nodes.iter().any(|node| {
            matches!(
                node,
                WireExpressionNode::SymmetricPart { .. }
                    | WireExpressionNode::IsotropicLift { .. }
                    | WireExpressionNode::PureOperatorApplication { .. }
            )
        }) || !self.definitions.is_empty()
        {
            Err(invalid_artifact(
                "model wire v3 cannot contain tensor operators introduced by model wire v4",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn ensure_v4(&self) -> Result<(), Diagnostic> {
        if !self.definitions.is_empty()
            || self
                .nodes
                .iter()
                .any(|node| matches!(node, WireExpressionNode::PureOperatorApplication { .. }))
        {
            Err(invalid_artifact(
                "model wire v4 cannot contain pure-operator definitions or generic applications",
            ))
        } else {
            Ok(())
        }
    }

    pub(crate) fn semantic_references(&self) -> Vec<&WireId> {
        self.nodes
            .iter()
            .filter_map(|node| match node {
                WireExpressionNode::Symbol { symbol } => symbol.id(),
                _ => None,
            })
            .collect()
    }
}

/// Aggregate resource counters for the v5 pure-operator wire extension.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PureOperatorWireCounts {
    pub(crate) definitions: usize,
    pub(crate) formals: usize,
    pub(crate) calculus_nodes: usize,
    pub(crate) application_arguments: usize,
}

impl PureOperatorWireCounts {
    pub(crate) fn checked_add(self, other: Self) -> Result<Self, Diagnostic> {
        Ok(Self {
            definitions: self
                .definitions
                .checked_add(other.definitions)
                .ok_or_else(|| {
                    invalid_artifact("pure-operator definition count overflows usize")
                })?,
            formals: self
                .formals
                .checked_add(other.formals)
                .ok_or_else(|| invalid_artifact("pure-operator formal count overflows usize"))?,
            calculus_nodes: self
                .calculus_nodes
                .checked_add(other.calculus_nodes)
                .ok_or_else(|| {
                    invalid_artifact("pure-operator calculus-node count overflows usize")
                })?,
            application_arguments: self
                .application_arguments
                .checked_add(other.application_arguments)
                .ok_or_else(|| {
                    invalid_artifact("pure-operator application-argument count overflows usize")
                })?,
        })
    }

    pub(crate) fn ensure_limits(
        self,
        limits: ModelDecoderLimits,
        label: &str,
    ) -> Result<(), Diagnostic> {
        require_decoder_count(
            &format!("{label} pure-operator definitions"),
            self.definitions,
            limits.max_pure_operator_definitions,
        )?;
        require_decoder_count(
            &format!("{label} pure-operator formals"),
            self.formals,
            limits.max_pure_operator_formals,
        )?;
        require_decoder_count(
            &format!("{label} pure-operator calculus nodes"),
            self.calculus_nodes,
            limits.max_pure_operator_calculus_nodes,
        )?;
        require_decoder_count(
            &format!("{label} pure-operator application arguments"),
            self.application_arguments,
            limits.max_pure_operator_application_arguments,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireExpressionNode {
    Constant {
        value: WireQuantity,
    },
    Symbol {
        symbol: WireSymbol,
    },
    Neg {
        value: u32,
    },
    Add {
        left: u32,
        right: u32,
    },
    Sub {
        left: u32,
        right: u32,
    },
    Mul {
        left: u32,
        right: u32,
    },
    Div {
        left: u32,
        right: u32,
    },
    PowI {
        base: u32,
        exponent: i32,
    },
    SpatialCoordinate {
        axis: usize,
    },
    UnaryMath {
        function: WireUnaryMath,
        value: u32,
    },
    Gradient {
        value: u32,
    },
    Divergence {
        value: u32,
    },
    Trace {
        value: u32,
    },
    NormalComponent {
        value: u32,
    },
    SymmetricPart {
        value: u32,
    },
    IsotropicLift {
        value: u32,
    },
    PureOperatorApplication {
        definition: String,
        arguments: Vec<u32>,
    },
}

impl WireExpressionNode {
    pub(crate) fn encode(node: &ExprNode, version: WireVersion) -> Result<Self, Diagnostic> {
        Ok(match node {
            ExprNode::Constant(value) => Self::Constant {
                value: WireQuantity::encode(*value),
            },
            ExprNode::Symbol(symbol) => Self::Symbol {
                symbol: WireSymbol::encode(*symbol, version)?,
            },
            ExprNode::Neg(value) => Self::Neg {
                value: value.index(),
            },
            ExprNode::Add(left, right) => Self::Add {
                left: left.index(),
                right: right.index(),
            },
            ExprNode::Sub(left, right) => Self::Sub {
                left: left.index(),
                right: right.index(),
            },
            ExprNode::Mul(left, right) => Self::Mul {
                left: left.index(),
                right: right.index(),
            },
            ExprNode::Div(left, right) => Self::Div {
                left: left.index(),
                right: right.index(),
            },
            ExprNode::PowI(base, exponent) => Self::PowI {
                base: base.index(),
                exponent: *exponent,
            },
            ExprNode::SpatialCoordinate(axis) => Self::SpatialCoordinate { axis: *axis },
            ExprNode::UnaryMath(function, value) => Self::UnaryMath {
                function: WireUnaryMath::encode(*function)?,
                value: value.index(),
            },
            ExprNode::Gradient(value) => Self::Gradient {
                value: value.index(),
            },
            ExprNode::Divergence(value) => Self::Divergence {
                value: value.index(),
            },
            ExprNode::Trace(value) => Self::Trace {
                value: value.index(),
            },
            ExprNode::NormalComponent(value) => Self::NormalComponent {
                value: value.index(),
            },
            ExprNode::SymmetricPart(value) if version.supports_tensor_operators() => {
                Self::SymmetricPart {
                    value: value.index(),
                }
            }
            ExprNode::IsotropicLift(value) if version.supports_tensor_operators() => {
                Self::IsotropicLift {
                    value: value.index(),
                }
            }
            ExprNode::PureOperatorApplication(application) if version.supports_pure_operators() => {
                Self::PureOperatorApplication {
                    definition: application.definition().to_string(),
                    arguments: application
                        .arguments()
                        .iter()
                        .map(|argument| argument.index())
                        .collect(),
                }
            }
            _ => {
                return Err(invalid_artifact(
                    "expression node is newer than the supported model wire vocabulary",
                ));
            }
        })
    }

    pub(crate) fn decode(
        &self,
        builder: &mut ExprDagBuilder,
        ids: &[ExprId],
        definitions: &BTreeMap<String, PureOperatorDefinition>,
    ) -> Result<ExprId, Diagnostic> {
        let result = match self {
            Self::Constant { value } => builder.constant(value.decode()?),
            Self::Symbol { symbol } => builder.symbol(symbol.decode()?),
            Self::Neg { value } => builder.neg(operand(ids, *value)?),
            Self::Add { left, right } => builder.add(operand(ids, *left)?, operand(ids, *right)?),
            Self::Sub { left, right } => builder.sub(operand(ids, *left)?, operand(ids, *right)?),
            Self::Mul { left, right } => builder.mul(operand(ids, *left)?, operand(ids, *right)?),
            Self::Div { left, right } => builder.div(operand(ids, *left)?, operand(ids, *right)?),
            Self::PowI { base, exponent } => builder.powi(operand(ids, *base)?, *exponent),
            Self::SpatialCoordinate { axis } => builder.spatial_coordinate(*axis),
            Self::UnaryMath { function, value } => {
                builder.unary_math(function.decode(), operand(ids, *value)?)
            }
            Self::Gradient { value } => builder.gradient(operand(ids, *value)?),
            Self::Divergence { value } => builder.divergence(operand(ids, *value)?),
            Self::Trace { value } => builder.trace(operand(ids, *value)?),
            Self::NormalComponent { value } => builder.normal_component(operand(ids, *value)?),
            Self::SymmetricPart { value } => builder.symmetric_part(operand(ids, *value)?),
            Self::IsotropicLift { value } => builder.isotropic_lift(operand(ids, *value)?),
            Self::PureOperatorApplication {
                definition,
                arguments,
            } => {
                validate_operator_digest(definition)?;
                let definition = definitions.get(definition).ok_or_else(|| {
                    invalid_artifact(
                        "pure-operator application references a missing local definition",
                    )
                })?;
                let arguments = arguments
                    .iter()
                    .map(|argument| operand(ids, *argument))
                    .collect::<Result<Vec<_>, _>>()?;
                builder.pure_operator(definition, arguments)
            }
        };
        result.map_err(|error| invalid_artifact(error.message()))
    }

    pub(crate) fn application_arity(&self) -> usize {
        match self {
            Self::PureOperatorApplication { arguments, .. } => arguments.len(),
            _ => 0,
        }
    }
}

pub(crate) fn operand(ids: &[ExprId], index: u32) -> Result<ExprId, Diagnostic> {
    usize::try_from(index)
        .ok()
        .and_then(|index| ids.get(index))
        .copied()
        .ok_or_else(|| {
            invalid_artifact(format!(
                "wire expression operand {index} is not topologically prior"
            ))
        })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum WireSymbol {
    Field { id: WireId },
    Derivative { id: WireId },
    Pre { id: WireId },
    Next { id: WireId },
    Parameter { id: WireId },
    Port { id: WireId },
    Across { id: WireId },
    Through { id: WireId },
    PortTrace { id: WireId },
    PortFlux { id: WireId },
    Time,
}

impl WireSymbol {
    pub(crate) fn encode(value: SymbolRef, version: WireVersion) -> Result<Self, Diagnostic> {
        match value {
            SymbolRef::Field(id) => Ok(Self::Field {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Derivative(id) => Ok(Self::Derivative {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Pre(id) => Ok(Self::Pre {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Next(id) => Ok(Self::Next {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Parameter(id) => Ok(Self::Parameter {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Port(id) => Ok(Self::Port {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Across(id) if version.supports_scalar_physical() => Ok(Self::Across {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Through(id) if version.supports_scalar_physical() => Ok(Self::Through {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::PortTrace(id) if version.supports_boundary_physical() => {
                Ok(Self::PortTrace {
                    id: WireId::from_raw(id.erase()),
                })
            }
            SymbolRef::PortFlux(id) if version.supports_boundary_physical() => Ok(Self::PortFlux {
                id: WireId::from_raw(id.erase()),
            }),
            SymbolRef::Time => Ok(Self::Time),
            _ => Err(invalid_artifact(
                "symbol kind is newer than the supported model wire vocabulary",
            )),
        }
    }

    pub(crate) fn decode(&self) -> Result<SymbolRef, Diagnostic> {
        Ok(match self {
            Self::Field { id } => SymbolRef::Field(id.typed::<kinds::Field>()?),
            Self::Derivative { id } => SymbolRef::Derivative(id.typed::<kinds::Field>()?),
            Self::Pre { id } => SymbolRef::Pre(id.typed::<kinds::Field>()?),
            Self::Next { id } => SymbolRef::Next(id.typed::<kinds::Field>()?),
            Self::Parameter { id } => SymbolRef::Parameter(id.typed::<kinds::Parameter>()?),
            Self::Port { id } => SymbolRef::Port(id.typed::<kinds::Port>()?),
            Self::Across { id } => SymbolRef::Across(id.typed::<kinds::Port>()?),
            Self::Through { id } => SymbolRef::Through(id.typed::<kinds::Port>()?),
            Self::PortTrace { id } => SymbolRef::PortTrace(id.typed::<kinds::Port>()?),
            Self::PortFlux { id } => SymbolRef::PortFlux(id.typed::<kinds::Port>()?),
            Self::Time => SymbolRef::Time,
        })
    }

    pub(crate) fn id(&self) -> Option<&WireId> {
        match self {
            Self::Field { id }
            | Self::Derivative { id }
            | Self::Pre { id }
            | Self::Next { id }
            | Self::Parameter { id }
            | Self::Port { id }
            | Self::Across { id }
            | Self::Through { id }
            | Self::PortTrace { id }
            | Self::PortFlux { id } => Some(id),
            Self::Time => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireUnaryMath {
    Sin,
}

impl WireUnaryMath {
    pub(crate) fn encode(value: UnaryMathFunction) -> Result<Self, Diagnostic> {
        match value {
            UnaryMathFunction::Sin => Ok(Self::Sin),
            _ => Err(invalid_artifact(
                "unary math function is newer than model wire v1",
            )),
        }
    }

    pub(crate) const fn decode(self) -> UnaryMathFunction {
        match self {
            Self::Sin => UnaryMathFunction::Sin,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireValue {
    pub(crate) target: WireId,
    pub(crate) value: WireQuantity,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireQuantity {
    pub(crate) value: f64,
    pub(crate) dimension: WireDimension,
}
