//! Ordered formal differentiation; normalization remains a separate proof view.
use eqiora_core::{DimExponents, ScalarDomain, ValueLiteral, ValueType};
use eqiora_schema::kernel::typing::ExpressionType;
use eqiora_schema::kernel::{ExprDagBuilder, ExprId};

use super::{CalculusError, ScalarCalculus, ScalarCalculusNode, calculus_index};

#[derive(Clone, Copy)]
struct Jet {
    value: usize,
    first: Option<usize>,
    second: Option<usize>,
}

impl<I: Clone> ScalarCalculus<I> {
    /// Append the ordered first or second formal derivative as an ordinary DAG.
    /// The returned type includes the exact quotient of physical dimensions.
    /// Arguments substitute the corresponding checked formal types. Only work
    /// reachable from the requested derivative is appended. Product-rule terms
    /// retain their order; no floating reassociation occurs.
    ///
    /// # Errors
    /// Rejects non-real or shaped formals/results, unsupported orders, wrong
    /// arity, invalid argument IDs, and unrepresentable derivative dimensions.
    pub fn partial(
        &self,
        builder: &mut ExprDagBuilder,
        arguments: &[ExprId],
        formal: u16,
        order: u8,
    ) -> Result<(ExprId, ExpressionType<I>), CalculusError> {
        let selected = self
            .argument_types
            .get(usize::from(formal))
            .ok_or(CalculusError::InvalidFormal(formal))?;
        if !matches!(order, 1 | 2)
            || arguments.len() != self.argument_types.len()
            || self
                .argument_types
                .iter()
                .chain([&self.result_type])
                .any(|ty| {
                    ty.value_type.scalar_domain() != ScalarDomain::Real || !ty.shape().is_scalar()
                })
        {
            return Err(CalculusError::UnsupportedDerivative);
        }
        for argument in arguments {
            builder
                .validate_prior_operand(*argument)
                .map_err(projection)?;
        }
        let dimension = selected
            .dimension()
            .pow(i32::from(order), 1)
            .and_then(|denominator| self.result_type.dimension().div(denominator))
            .ok_or(CalculusError::UnsupportedDerivative)?;
        let result_type = ExpressionType::scalar(dimension, self.result_type.support.clone());
        let destination = builder;
        let mut plan = PartialPlan::default();
        let builder = &mut plan;
        let arguments = arguments
            .iter()
            .map(|id| builder.push(PartialNode::Argument(*id)))
            .collect::<Vec<_>>();
        let mut jets: Vec<Jet> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let jet = match node {
                ScalarCalculusNode::Rational(value) => Jet {
                    value: constant(builder, DimExponents::DIMENSIONLESS, value.as_f64())?,
                    first: None,
                    second: None,
                },
                ScalarCalculusNode::FormalComponent(atom) => Jet {
                    value: arguments[usize::from(atom.formal())],
                    first: if atom.formal() == formal {
                        Some(constant(builder, DimExponents::DIMENSIONLESS, 1.0)?)
                    } else {
                        None
                    },
                    second: None,
                },
                ScalarCalculusNode::Neg(id) => {
                    let a = jets[calculus_index(*id, jets.len())?];
                    Jet {
                        value: builder.neg(a.value).map_err(projection)?,
                        first: negate(builder, a.first)?,
                        second: negate(builder, a.second)?,
                    }
                }
                ScalarCalculusNode::Add(left, right) => {
                    let a = jets[calculus_index(*left, jets.len())?];
                    let b = jets[calculus_index(*right, jets.len())?];
                    Jet {
                        value: builder.add(a.value, b.value).map_err(projection)?,
                        first: add(builder, a.first, b.first)?,
                        second: add(builder, a.second, b.second)?,
                    }
                }
                ScalarCalculusNode::Mul(left, right) => {
                    let a = jets[calculus_index(*left, jets.len())?];
                    let b = jets[calculus_index(*right, jets.len())?];
                    let first_left = mul(builder, a.first, Some(b.value))?;
                    let first_right = mul(builder, Some(a.value), b.first)?;
                    let first = add(builder, first_left, first_right)?;
                    let second = if order == 2 {
                        let ll = mul(builder, a.second, Some(b.value))?;
                        let lr = mul(builder, a.first, b.first)?;
                        let rl = mul(builder, a.first, b.first)?;
                        let rr = mul(builder, Some(a.value), b.second)?;
                        let left = add(builder, ll, lr)?;
                        let right = add(builder, rl, rr)?;
                        add(builder, left, right)?
                    } else {
                        None
                    };
                    Jet {
                        value: builder.mul(a.value, b.value).map_err(projection)?,
                        first,
                        second,
                    }
                }
            };
            jets.push(jet);
        }
        let jet = jets[calculus_index(self.root, jets.len())?];
        let derivative = if order == 1 { jet.first } else { jet.second };
        let root = match derivative {
            Some(root) => root,
            None => constant(builder, dimension, 0.0)?,
        };
        Ok((plan.append(root, destination)?, result_type))
    }
}

fn projection(error: eqiora_core::Diagnostic) -> CalculusError {
    CalculusError::DerivativeProjection(error.to_string())
}

fn constant(
    builder: &mut PartialPlan,
    dimension: DimExponents,
    value: f64,
) -> Result<usize, CalculusError> {
    builder
        .constant(
            ValueLiteral::from_real(ValueType::scalar(ScalarDomain::Real, dimension), value)
                .map_err(|error| CalculusError::DerivativeProjection(error.to_string()))?,
        )
        .map_err(projection)
}

fn negate(builder: &mut PartialPlan, value: Option<usize>) -> Result<Option<usize>, CalculusError> {
    value
        .map(|value| builder.neg(value).map_err(projection))
        .transpose()
}

fn add(
    builder: &mut PartialPlan,
    a: Option<usize>,
    b: Option<usize>,
) -> Result<Option<usize>, CalculusError> {
    match (a, b) {
        (Some(a), Some(b)) => builder.add(a, b).map(Some).map_err(projection),
        (a, b) => Ok(a.or(b)),
    }
}

fn mul(
    builder: &mut PartialPlan,
    a: Option<usize>,
    b: Option<usize>,
) -> Result<Option<usize>, CalculusError> {
    match (a, b) {
        (Some(a), Some(b)) => builder.mul(a, b).map(Some).map_err(projection),
        _ => Ok(None),
    }
}

#[derive(Default)]
struct PartialPlan {
    nodes: Vec<PartialNode>,
}

enum PartialNode {
    Argument(ExprId),
    Constant(ValueLiteral),
    Neg(usize),
    Add(usize, usize),
    Mul(usize, usize),
}

impl PartialPlan {
    fn push(&mut self, node: PartialNode) -> usize {
        let id = self.nodes.len();
        self.nodes.push(node);
        id
    }
    fn constant(&mut self, value: ValueLiteral) -> Result<usize, eqiora_core::Diagnostic> {
        Ok(self.push(PartialNode::Constant(value)))
    }
    fn neg(&mut self, value: usize) -> Result<usize, eqiora_core::Diagnostic> {
        Ok(self.push(PartialNode::Neg(value)))
    }
    fn add(&mut self, a: usize, b: usize) -> Result<usize, eqiora_core::Diagnostic> {
        Ok(self.push(PartialNode::Add(a, b)))
    }
    fn mul(&mut self, a: usize, b: usize) -> Result<usize, eqiora_core::Diagnostic> {
        Ok(self.push(PartialNode::Mul(a, b)))
    }

    fn append(self, root: usize, builder: &mut ExprDagBuilder) -> Result<ExprId, CalculusError> {
        let mut reachable = vec![false; self.nodes.len()];
        let mut pending = vec![root];
        while let Some(id) = pending.pop() {
            if std::mem::replace(&mut reachable[id], true) {
                continue;
            }
            match self.nodes[id] {
                PartialNode::Neg(a) => pending.push(a),
                PartialNode::Add(a, b) | PartialNode::Mul(a, b) => pending.extend([a, b]),
                _ => {}
            }
        }
        let mut mapped = vec![None; self.nodes.len()];
        for (id, node) in self.nodes.into_iter().enumerate() {
            if !reachable[id] {
                continue;
            }
            let operand = |id: usize| mapped[id].expect("reachable prior derivative operand");
            mapped[id] = Some(match node {
                PartialNode::Argument(id) => id,
                PartialNode::Constant(value) => builder.constant(value).map_err(projection)?,
                PartialNode::Neg(a) => builder.neg(operand(a)).map_err(projection)?,
                PartialNode::Add(a, b) => {
                    builder.add(operand(a), operand(b)).map_err(projection)?
                }
                PartialNode::Mul(a, b) => {
                    builder.mul(operand(a), operand(b)).map_err(projection)?
                }
            });
        }
        Ok(mapped[root].expect("reachable derivative root"))
    }
}
