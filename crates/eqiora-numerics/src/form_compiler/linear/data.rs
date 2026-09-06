use std::collections::BTreeMap;
use std::sync::Arc;

use eqiora_core::{Diagnostic, RawId};
use eqiora_schema::kernel::{ExprDag, ExprId, ExprNode, SymbolRef, UnaryMathFunction};
use eqiora_sem::KernelProgram;

use crate::spatial_expression::{self, ScalarSpatialExpression};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Data(Arc<Node>);

#[derive(Debug, PartialEq)]
enum Node {
    Tape(ScalarSpatialExpression),
    Add(Data, Data),
    Mul(Data, Data),
    Div(Data, Data),
    Pow(Data, i32),
    Math(UnaryMathFunction, Data),
}

impl Data {
    pub(super) fn bind_parameter_point(
        &self,
        fields: &[eqiora_core::Id<eqiora_core::entity::kinds::Parameter>],
        values: &[f64],
    ) -> Result<Self, Diagnostic> {
        let bind = |data: &Self| data.bind_parameter_point(fields, values);
        Ok(Self(Arc::new(match self.0.as_ref() {
            Node::Tape(tape) => Node::Tape(tape.bind_parameter_point(fields, values)?),
            Node::Add(a, b) => Node::Add(bind(a)?, bind(b)?),
            Node::Mul(a, b) => Node::Mul(bind(a)?, bind(b)?),
            Node::Div(a, b) => Node::Div(bind(a)?, bind(b)?),
            Node::Pow(a, power) => Node::Pow(bind(a)?, *power),
            Node::Math(function, a) => Node::Math(*function, bind(a)?),
        })))
    }

    /// Compare symbolic coefficient products without sampling or erasing Parameters.
    pub(super) fn same_coefficient(&self, other: &Self) -> bool {
        fn product<'a>(data: &'a Data, scale: &mut f64, factors: &mut Vec<&'a Data>) {
            match data.0.as_ref() {
                Node::Mul(a, b) => {
                    product(a, scale, factors);
                    product(b, scale, factors);
                }
                Node::Tape(tape) if tape.parameter_fields().is_empty() => {
                    if let Some(value) = tape.constant_value() {
                        *scale *= value;
                    } else {
                        factors.push(data);
                    }
                }
                _ => factors.push(data),
            }
        }
        fn factor(a: &Data, b: &Data) -> bool {
            match (a.0.as_ref(), b.0.as_ref()) {
                (Node::Tape(a), Node::Tape(b)) => a.is_same_coefficient_as(b),
                (Node::Add(a, b), Node::Add(c, d)) => {
                    (a.same_coefficient(c) && b.same_coefficient(d))
                        || (a.same_coefficient(d) && b.same_coefficient(c))
                }
                (Node::Div(a, b), Node::Div(c, d)) => {
                    a.same_coefficient(c) && b.same_coefficient(d)
                }
                (Node::Pow(a, n), Node::Pow(b, m)) => n == m && a.same_coefficient(b),
                (Node::Math(f, a), Node::Math(g, b)) => f == g && a.same_coefficient(b),
                _ => false,
            }
        }
        let (mut left_scale, mut right_scale) = (1.0, 1.0);
        let (mut left, mut right) = (Vec::new(), Vec::new());
        product(self, &mut left_scale, &mut left);
        product(other, &mut right_scale, &mut right);
        if !left_scale.is_finite() || left_scale != right_scale || left.len() != right.len() {
            return false;
        }
        for candidate in left {
            let Some(index) = right.iter().position(|other| factor(candidate, other)) else {
                return false;
            };
            right.remove(index);
        }
        true
    }

    pub(super) fn constant(dimension: usize, value: f64) -> Self {
        Self(Arc::new(Node::Tape(ScalarSpatialExpression::constant(
            dimension, value,
        ))))
    }
    pub(super) fn add(self, right: Self) -> Self {
        let zero = |data: &Self| matches!(data.0.as_ref(), Node::Tape(tape) if tape.parameter_fields().is_empty() && tape.constant_value() == Some(0.0));
        if zero(&self) {
            return right;
        }
        if zero(&right) {
            return self;
        }
        Self(Arc::new(Node::Add(self, right)))
    }
    pub(super) fn multiply(self, right: Self) -> Self {
        Self(Arc::new(Node::Mul(self, right)))
    }
    pub(super) fn divide(self, right: Self) -> Self {
        Self(Arc::new(Node::Div(self, right)))
    }
    pub(super) fn spatial(&self) -> bool {
        match self.0.as_ref() {
            Node::Tape(tape) => tape.is_coordinate_dependent(),
            Node::Add(a, b) | Node::Mul(a, b) | Node::Div(a, b) => a.spatial() || b.spatial(),
            Node::Pow(a, _) | Node::Math(_, a) => a.spatial(),
        }
    }
    pub(super) fn evaluate(&self, point: &[f64]) -> Result<f64, Diagnostic> {
        let value = match self.0.as_ref() {
            Node::Tape(tape) => tape.evaluate(point)?,
            Node::Add(a, b) => a.evaluate(point)? + b.evaluate(point)?,
            Node::Mul(a, b) => a.evaluate(point)? * b.evaluate(point)?,
            Node::Div(a, b) => a.evaluate(point)? / b.evaluate(point)?,
            Node::Pow(a, n) => a.evaluate(point)?.powi(*n),
            Node::Math(UnaryMathFunction::Sin, a) => a.evaluate(point)?.sin(),
            Node::Math(UnaryMathFunction::Sqrt, a) => a.evaluate(point)?.sqrt(),
            _ => return Err(super::invalid("unsupported coefficient mathematics")),
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(super::invalid("non-finite linear coefficient data"))
        }
    }
}

pub(super) struct Context<'a> {
    pub(super) program: &'a KernelProgram,
    pub(super) dag: &'a ExprDag,
    pub(super) owner: RawId,
    pub(super) dimension: usize,
    pub(super) coefficients: &'a BTreeMap<RawId, Data>,
}

impl Context<'_> {
    pub(super) fn data(&self, id: ExprId, depth: usize) -> Result<Data, Diagnostic> {
        if depth > 128 {
            return Err(super::invalid("linear expression nesting exceeds 128"));
        }
        let data = |id| self.data(id, depth + 1);
        Ok(match self.dag.node(id) {
            Some(
                ExprNode::Constant(_)
                | ExprNode::SpatialCoordinate(_)
                | ExprNode::Symbol(SymbolRef::Parameter(_)),
            ) => Data(Arc::new(Node::Tape(spatial_expression::lower(
                self.program,
                self.dag,
                id,
                self.owner,
                self.dimension,
            )?))),
            Some(ExprNode::Symbol(SymbolRef::Field(field))) => self
                .coefficients
                .get(&field.erase())
                .cloned()
                .ok_or_else(|| {
                    super::invalid(
                        "unknown-dependent coefficient or unresolved coefficient definition",
                    )
                })?,
            Some(ExprNode::Neg(a)) => data(*a)?.multiply(Data::constant(self.dimension, -1.0)),
            Some(ExprNode::Add(a, b)) => data(*a)?.add(data(*b)?),
            Some(ExprNode::Sub(a, b)) => {
                data(*a)?.add(data(*b)?.multiply(Data::constant(self.dimension, -1.0)))
            }
            Some(ExprNode::Mul(a, b)) => data(*a)?.multiply(data(*b)?),
            Some(ExprNode::Div(a, b)) => data(*a)?.divide(data(*b)?),
            Some(ExprNode::PowI(a, n)) => Data(Arc::new(Node::Pow(data(*a)?, *n))),
            Some(ExprNode::UnaryMath(function, a)) => {
                Data(Arc::new(Node::Math(*function, data(*a)?)))
            }
            _ => return Err(super::invalid("unsupported linear coefficient expression")),
        })
    }
}
