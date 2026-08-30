//! Typed, compiler-owned projections of authored mathematical formulations.

use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, DimExponents, Id, RawId, ValueShape};
use eqiora_graph::{EdgeKind, Op, Transaction};
use eqiora_lang::{
    BinaryOp, ComponentDecl, Expr, ExprKind, FormulationDecl, FormulationSyntax, NamePath,
    TextRange, UnaryOp,
};
use eqiora_schema::kernel::{KernelNode, ParameterDef};

use crate::diagnostics::source_error;
use crate::dimensions::{checked_dimensions, checked_scale_dimension, length_dimension};
use crate::lower::ModelSymbols;
use crate::source_identity::AuthoredFormSourceIdentity;

/// One typed expression in an authored Formulation.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthoredFormExpression {
    kind: AuthoredFormExpressionKind,
    dimension: DimExponents,
    shape: ValueShape,
    support: Option<Id<kinds::Domain>>,
}

impl AuthoredFormExpression {
    /// Typed expression topology.
    #[must_use]
    pub const fn kind(&self) -> &AuthoredFormExpressionKind {
        &self.kind
    }

    /// Exact coherent-SI dimension.
    #[must_use]
    pub const fn dimension(&self) -> DimExponents {
        self.dimension
    }

    /// Exact mathematical value shape.
    #[must_use]
    pub const fn shape(&self) -> &ValueShape {
        &self.shape
    }

    /// Spatial support before an enclosing integration removes it.
    #[must_use]
    pub const fn support(&self) -> Option<Id<kinds::Domain>> {
        self.support
    }
}

/// Closed expression vocabulary accepted by the first scalar-primal compiler.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AuthoredFormExpressionKind {
    /// Dimensionless scalar literal.
    Number(f64),
    /// Scalar Field value.
    Field(Id<kinds::Field>),
    /// Scalar Parameter value.
    Parameter(Id<kinds::Parameter>),
    /// One physical Cartesian coordinate in the Relation Domain.
    Coordinate(usize),
    /// Scalar test function associated with one trial Field.
    Test(Id<kinds::Field>),
    /// Arithmetic negation.
    Neg(Box<AuthoredFormExpression>),
    /// Binary addition.
    Add(Box<AuthoredFormExpression>, Box<AuthoredFormExpression>),
    /// Binary subtraction.
    Sub(Box<AuthoredFormExpression>, Box<AuthoredFormExpression>),
    /// Scalar multiplication, or scalar scaling of a vector.
    Mul(Box<AuthoredFormExpression>, Box<AuthoredFormExpression>),
    /// Division by a scalar.
    Div(Box<AuthoredFormExpression>, Box<AuthoredFormExpression>),
    /// Integer power of a scalar.
    Pow(Box<AuthoredFormExpression>, i32),
    /// Spatial gradient.
    Gradient(Box<AuthoredFormExpression>),
    /// Scalar sine.
    Sin(Box<AuthoredFormExpression>),
    /// Euclidean inner product of equal vectors.
    Dot(Box<AuthoredFormExpression>, Box<AuthoredFormExpression>),
    /// Volume integral over one exact Domain.
    Integrate {
        /// Integration Domain.
        domain: Id<kinds::Domain>,
        /// Scalar integrand on that Domain.
        integrand: Box<AuthoredFormExpression>,
    },
}

/// A typed scalar primal equality retained beside a freshly compiled Model.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthoredScalarPrimalForm {
    source_identity: AuthoredFormSourceIdentity,
    relation: Id<kinds::Relation>,
    domain: Id<kinds::Domain>,
    trial: Id<kinds::Field>,
    left: AuthoredFormExpression,
    right: AuthoredFormExpression,
    file: String,
    range: TextRange,
}

impl AuthoredScalarPrimalForm {
    /// Identity of the Component's canonical authored-form source.
    #[must_use]
    pub const fn source_identity(&self) -> AuthoredFormSourceIdentity {
        self.source_identity
    }

    /// Relation represented by this form.
    #[must_use]
    pub const fn relation(&self) -> Id<kinds::Relation> {
        self.relation
    }

    /// Exact integration and Relation Domain.
    #[must_use]
    pub const fn domain(&self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Scalar trial Field named by every test function.
    #[must_use]
    pub const fn trial(&self) -> Id<kinds::Field> {
        self.trial
    }

    /// Typed left side.
    #[must_use]
    pub const fn left(&self) -> &AuthoredFormExpression {
        &self.left
    }

    /// Typed right side.
    #[must_use]
    pub const fn right(&self) -> &AuthoredFormExpression {
        &self.right
    }

    /// Source filename used for diagnostics and inspection.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Exact source range of the Formulation declaration.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
}

pub(crate) fn compile_component_formulations(
    file: &str,
    component: &ComponentDecl,
    symbols: &ModelSymbols,
    transaction: &Transaction,
    ambient_dimension: usize,
    topological_dimension: usize,
) -> Result<Vec<AuthoredScalarPrimalForm>, Vec<Diagnostic>> {
    if component.formulations().is_empty() {
        return Ok(Vec::new());
    }
    if component.formulations().len() != 1 {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            component.range(),
            "the scalar-primal Formulation compiler accepts exactly one form per Component",
        )]);
    }
    let source_identity = AuthoredFormSourceIdentity::from_component(component)
        .map_err(|diagnostic| vec![diagnostic])?;
    let index = KernelIndex::new(transaction);
    component
        .formulations()
        .iter()
        .map(|form| {
            compile_formulation(
                file,
                form,
                source_identity,
                symbols,
                &index,
                ambient_dimension,
                topological_dimension,
            )
            .map_err(|diagnostic| vec![diagnostic])
        })
        .collect()
}

struct KernelIndex<'a> {
    nodes: BTreeMap<RawId, &'a KernelNode>,
    applies_on: BTreeMap<RawId, RawId>,
    defined_on: BTreeMap<RawId, RawId>,
}

impl<'a> KernelIndex<'a> {
    fn new(transaction: &'a Transaction) -> Self {
        let mut nodes = BTreeMap::new();
        let mut applies_on = BTreeMap::new();
        let mut defined_on = BTreeMap::new();
        for op in transaction.ops() {
            match op {
                Op::DefineKernelNode { node } => {
                    nodes.insert(node.id(), node);
                }
                Op::Connect {
                    from,
                    to,
                    edge: EdgeKind::AppliesOn,
                } => {
                    applies_on.insert(*from, *to);
                }
                Op::Connect {
                    from,
                    to,
                    edge: EdgeKind::DefinedOn,
                } if to.downcast::<kinds::Domain>().is_some() => {
                    defined_on.insert(*from, *to);
                }
                _ => {}
            }
        }
        Self {
            nodes,
            applies_on,
            defined_on,
        }
    }
}

fn compile_formulation(
    file: &str,
    form: &FormulationDecl,
    source_identity: AuthoredFormSourceIdentity,
    symbols: &ModelSymbols,
    index: &KernelIndex<'_>,
    ambient_dimension: usize,
    topological_dimension: usize,
) -> Result<AuthoredScalarPrimalForm, Diagnostic> {
    if form.kind() != FormulationSyntax::Primal {
        return Err(error(
            file,
            form.range(),
            "unsupported authored Formulation kind",
        ));
    }
    let relation_raw = resolve_symbol(file, form.range(), form.relation(), symbols)?;
    let relation = relation_raw.downcast::<kinds::Relation>().ok_or_else(|| {
        error(
            file,
            form.range(),
            format!("`{}` is not a Relation", form.relation()),
        )
    })?;
    let domain = index
        .applies_on
        .get(&relation_raw)
        .copied()
        .and_then(RawId::downcast::<kinds::Domain>)
        .ok_or_else(|| {
            error(
                file,
                form.range(),
                "Formulation Relation has no exact AppliesOn Domain",
            )
        })?;
    let mut context = ExpressionContext {
        file,
        symbols,
        index,
        ambient_dimension,
        topological_dimension,
        relation_domain: domain,
        trial: None,
    };
    let left = context.compile_root(form.left())?;
    let right = context.compile_root(form.right())?;
    if left.dimension != right.dimension || left.shape != right.shape {
        return Err(error(
            file,
            form.range(),
            "Formulation equality sides must have identical dimension and shape",
        ));
    }
    let trial = context.trial.ok_or_else(|| {
        error(
            file,
            form.range(),
            "scalar primal Formulation must contain test(field)",
        )
    })?;
    Ok(AuthoredScalarPrimalForm {
        source_identity,
        relation,
        domain,
        trial,
        left,
        right,
        file: file.to_owned(),
        range: form.range(),
    })
}

struct ExpressionContext<'a> {
    file: &'a str,
    symbols: &'a ModelSymbols,
    index: &'a KernelIndex<'a>,
    ambient_dimension: usize,
    topological_dimension: usize,
    relation_domain: Id<kinds::Domain>,
    trial: Option<Id<kinds::Field>>,
}

impl ExpressionContext<'_> {
    fn compile_root(&mut self, expression: &Expr) -> Result<AuthoredFormExpression, Diagnostic> {
        let value = self.compile(expression)?;
        match value.kind {
            AuthoredFormExpressionKind::Integrate { .. } => Ok(value),
            _ => Err(error(
                self.file,
                expression.range(),
                "each scalar-primal equality side must be one integrate(domain, expression) call",
            )),
        }
    }

    fn compile(&mut self, expression: &Expr) -> Result<AuthoredFormExpression, Diagnostic> {
        match expression.kind() {
            ExprKind::Number(value) => Ok(typed(
                AuthoredFormExpressionKind::Number(*value),
                DimExponents::DIMENSIONLESS,
                ValueShape::scalar(),
                None,
            )),
            ExprKind::Name(name) => self.compile_name(expression, name),
            ExprKind::Path(_) | ExprKind::BoundaryPortSelection { .. } => Err(error(
                self.file,
                expression.range(),
                "qualified and boundary-selected names are not accepted in scalar-primal forms",
            )),
            ExprKind::Unary {
                op: UnaryOp::Neg,
                value,
            } => {
                let value = self.compile(value)?;
                Ok(typed(
                    AuthoredFormExpressionKind::Neg(Box::new(value.clone())),
                    value.dimension,
                    value.shape.clone(),
                    value.support,
                ))
            }
            ExprKind::Binary { op, left, right } => {
                self.compile_binary(expression, *op, left, right)
            }
            ExprKind::Call { callee, arguments } => {
                self.compile_call(expression, callee, arguments)
            }
            _ => Err(error(
                self.file,
                expression.range(),
                "unsupported scalar-primal expression",
            )),
        }
    }

    fn compile_name(
        &self,
        expression: &Expr,
        name: &str,
    ) -> Result<AuthoredFormExpression, Diagnostic> {
        let raw = resolve_symbol(self.file, expression.range(), name, self.symbols)?;
        match self.index.nodes.get(&raw).copied() {
            Some(KernelNode::Field(field)) if field.shape().is_scalar() => {
                let support = self.field_support(expression, raw)?;
                Ok(typed(
                    AuthoredFormExpressionKind::Field(field.id()),
                    field.dimension(),
                    ValueShape::scalar(),
                    Some(support),
                ))
            }
            Some(KernelNode::Field(_)) => Err(error(
                self.file,
                expression.range(),
                "scalar-primal forms accept only scalar Fields",
            )),
            Some(KernelNode::Parameter(parameter)) => Ok(parameter_expression(parameter)),
            _ => Err(error(
                self.file,
                expression.range(),
                format!("`{name}` is not a scalar Field or Parameter"),
            )),
        }
    }

    fn field_support(
        &self,
        expression: &Expr,
        field: RawId,
    ) -> Result<Id<kinds::Domain>, Diagnostic> {
        let support = self
            .index
            .defined_on
            .get(&field)
            .copied()
            .and_then(RawId::downcast::<kinds::Domain>)
            .ok_or_else(|| {
                error(
                    self.file,
                    expression.range(),
                    "Formulation Field has no exact DefinedOn Domain",
                )
            })?;
        if support != self.relation_domain {
            return Err(error(
                self.file,
                expression.range(),
                "Formulation Field support differs from the Relation Domain",
            ));
        }
        Ok(support)
    }

    fn compile_binary(
        &mut self,
        expression: &Expr,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
    ) -> Result<AuthoredFormExpression, Diagnostic> {
        if op == BinaryOp::Pow {
            let base = self.compile(left)?;
            require_scalar(self.file, left.range(), &base)?;
            let exponent = integer_literal(right).ok_or_else(|| {
                error(
                    self.file,
                    right.range(),
                    "Formulation power requires an integer literal",
                )
            })?;
            let dimension = checked_scale_dimension(base.dimension, exponent).ok_or_else(|| {
                error(
                    self.file,
                    expression.range(),
                    "Formulation dimension exponent overflows",
                )
            })?;
            return Ok(typed(
                AuthoredFormExpressionKind::Pow(Box::new(base.clone()), exponent),
                dimension,
                ValueShape::scalar(),
                base.support,
            ));
        }
        let left_value = self.compile(left)?;
        let right_value = self.compile(right)?;
        let support = merge_support(
            self.file,
            expression.range(),
            left_value.support,
            right_value.support,
        )?;
        let (kind, dimension, shape) = match op {
            BinaryOp::Add | BinaryOp::Sub => {
                if left_value.dimension != right_value.dimension
                    || left_value.shape != right_value.shape
                {
                    return Err(error(
                        self.file,
                        expression.range(),
                        "addition and subtraction require identical dimension and shape",
                    ));
                }
                let kind = if op == BinaryOp::Add {
                    AuthoredFormExpressionKind::Add(
                        Box::new(left_value.clone()),
                        Box::new(right_value),
                    )
                } else {
                    AuthoredFormExpressionKind::Sub(
                        Box::new(left_value.clone()),
                        Box::new(right_value),
                    )
                };
                (kind, left_value.dimension, left_value.shape.clone())
            }
            BinaryOp::Mul => {
                if !left_value.shape.is_scalar() && !right_value.shape.is_scalar() {
                    return Err(error(
                        self.file,
                        expression.range(),
                        "multiplication accepts at most one non-scalar operand",
                    ));
                }
                let dimension = checked_dimensions(
                    left_value.dimension,
                    right_value.dimension,
                    i8::checked_add,
                )
                .ok_or_else(|| {
                    error(
                        self.file,
                        expression.range(),
                        "Formulation dimension multiplication overflows",
                    )
                })?;
                let shape = if left_value.shape.is_scalar() {
                    right_value.shape.clone()
                } else {
                    left_value.shape.clone()
                };
                (
                    AuthoredFormExpressionKind::Mul(Box::new(left_value), Box::new(right_value)),
                    dimension,
                    shape,
                )
            }
            BinaryOp::Div => {
                require_scalar(self.file, right.range(), &right_value)?;
                let dimension = checked_dimensions(
                    left_value.dimension,
                    right_value.dimension,
                    i8::checked_sub,
                )
                .ok_or_else(|| {
                    error(
                        self.file,
                        expression.range(),
                        "Formulation dimension division overflows",
                    )
                })?;
                let shape = left_value.shape.clone();
                (
                    AuthoredFormExpressionKind::Div(Box::new(left_value), Box::new(right_value)),
                    dimension,
                    shape,
                )
            }
            BinaryOp::Pow => unreachable!(),
        };
        Ok(typed(kind, dimension, shape, support))
    }

    fn compile_call(
        &mut self,
        expression: &Expr,
        callee: &NamePath,
        arguments: &[Expr],
    ) -> Result<AuthoredFormExpression, Diagnostic> {
        let name = unqualified_callee(self.file, expression.range(), callee)?;
        match (name, arguments) {
            ("test", [argument]) => self.compile_test(expression, argument),
            ("coordinate", [axis]) => self.compile_coordinate(expression, axis),
            ("sin", [argument]) => {
                let argument = self.compile(argument)?;
                require_scalar(self.file, expression.range(), &argument)?;
                if argument.dimension != DimExponents::DIMENSIONLESS {
                    return Err(error(
                        self.file,
                        expression.range(),
                        "sin requires a dimensionless argument",
                    ));
                }
                Ok(typed(
                    AuthoredFormExpressionKind::Sin(Box::new(argument.clone())),
                    DimExponents::DIMENSIONLESS,
                    ValueShape::scalar(),
                    argument.support,
                ))
            }
            ("grad", [argument]) => {
                let argument = self.compile(argument)?;
                require_scalar(self.file, expression.range(), &argument)?;
                let support = argument.support.ok_or_else(|| {
                    error(
                        self.file,
                        expression.range(),
                        "grad requires a spatially supported expression",
                    )
                })?;
                let dimension =
                    checked_dimensions(argument.dimension, length_dimension(), i8::checked_sub)
                        .ok_or_else(|| {
                            error(
                                self.file,
                                expression.range(),
                                "gradient dimension overflows",
                            )
                        })?;
                let extent = u32::try_from(self.ambient_dimension)
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| {
                        error(
                            self.file,
                            expression.range(),
                            "Geometry ambient dimension is not representable",
                        )
                    })?;
                let shape = ValueShape::new([extent]).map_err(|_| {
                    error(
                        self.file,
                        expression.range(),
                        "Geometry ambient dimension is not representable",
                    )
                })?;
                Ok(typed(
                    AuthoredFormExpressionKind::Gradient(Box::new(argument)),
                    dimension,
                    shape,
                    Some(support),
                ))
            }
            ("dot", [left, right]) => {
                let left = self.compile(left)?;
                let right = self.compile(right)?;
                if left.shape.is_scalar() || left.shape != right.shape {
                    return Err(error(
                        self.file,
                        expression.range(),
                        "dot requires equal non-scalar vector shapes",
                    ));
                }
                let support =
                    merge_support(self.file, expression.range(), left.support, right.support)?;
                let dimension =
                    checked_dimensions(left.dimension, right.dimension, i8::checked_add)
                        .ok_or_else(|| {
                            error(
                                self.file,
                                expression.range(),
                                "dot-product dimension overflows",
                            )
                        })?;
                Ok(typed(
                    AuthoredFormExpressionKind::Dot(Box::new(left), Box::new(right)),
                    dimension,
                    ValueShape::scalar(),
                    support,
                ))
            }
            ("integrate", [domain, integrand]) => {
                self.compile_integral(expression, domain, integrand)
            }
            _ => Err(error(
                self.file,
                expression.range(),
                format!("unsupported scalar-primal operator `{name}` or arity"),
            )),
        }
    }

    fn compile_test(
        &mut self,
        expression: &Expr,
        argument: &Expr,
    ) -> Result<AuthoredFormExpression, Diagnostic> {
        let ExprKind::Name(name) = argument.kind() else {
            return Err(error(
                self.file,
                argument.range(),
                "test requires one unqualified scalar Field name",
            ));
        };
        let raw = resolve_symbol(self.file, argument.range(), name, self.symbols)?;
        let Some(KernelNode::Field(field)) = self.index.nodes.get(&raw).copied() else {
            return Err(error(
                self.file,
                argument.range(),
                "test argument is not a Field",
            ));
        };
        if !field.shape().is_scalar() {
            return Err(error(
                self.file,
                argument.range(),
                "test requires a scalar Field",
            ));
        }
        let support = self.field_support(expression, raw)?;
        if self
            .trial
            .replace(field.id())
            .is_some_and(|trial| trial != field.id())
        {
            return Err(error(
                self.file,
                expression.range(),
                "one scalar-primal form cannot mix test functions from different Fields",
            ));
        }
        Ok(typed(
            AuthoredFormExpressionKind::Test(field.id()),
            field.dimension(),
            ValueShape::scalar(),
            Some(support),
        ))
    }

    fn compile_coordinate(
        &self,
        expression: &Expr,
        axis: &Expr,
    ) -> Result<AuthoredFormExpression, Diagnostic> {
        let axis = integer_literal(axis)
            .and_then(|axis| usize::try_from(axis).ok())
            .filter(|axis| *axis < self.ambient_dimension)
            .ok_or_else(|| {
                error(
                    self.file,
                    expression.range(),
                    "coordinate axis must be a nonnegative literal below the Geometry ambient dimension",
                )
            })?;
        Ok(typed(
            AuthoredFormExpressionKind::Coordinate(axis),
            length_dimension(),
            ValueShape::scalar(),
            Some(self.relation_domain),
        ))
    }

    fn compile_integral(
        &mut self,
        expression: &Expr,
        domain: &Expr,
        integrand: &Expr,
    ) -> Result<AuthoredFormExpression, Diagnostic> {
        let ExprKind::Name(name) = domain.kind() else {
            return Err(error(
                self.file,
                domain.range(),
                "integrate Domain must be one unqualified name",
            ));
        };
        let raw = resolve_symbol(self.file, domain.range(), name, self.symbols)?;
        let domain_id = raw.downcast::<kinds::Domain>().ok_or_else(|| {
            error(
                self.file,
                domain.range(),
                "integrate first argument is not a Domain",
            )
        })?;
        if !matches!(self.index.nodes.get(&raw), Some(KernelNode::Domain(_)))
            || domain_id != self.relation_domain
        {
            return Err(error(
                self.file,
                domain.range(),
                "integrate Domain must equal the Formulation Relation Domain",
            ));
        }
        let integrand_range = integrand.range();
        let integrand = self.compile(integrand)?;
        require_scalar(self.file, integrand_range, &integrand)?;
        if integrand.support != Some(domain_id) {
            return Err(error(
                self.file,
                expression.range(),
                "integrand support must equal its integration Domain",
            ));
        }
        let topological_dimension = i32::try_from(self.topological_dimension).map_err(|_| {
            error(
                self.file,
                expression.range(),
                "Geometry dimension is not representable",
            )
        })?;
        let measure_dimension = checked_scale_dimension(length_dimension(), topological_dimension)
            .ok_or_else(|| {
                error(
                    self.file,
                    expression.range(),
                    "integration-measure dimension overflows",
                )
            })?;
        let dimension = checked_dimensions(integrand.dimension, measure_dimension, i8::checked_add)
            .ok_or_else(|| {
                error(
                    self.file,
                    expression.range(),
                    "integral dimension overflows",
                )
            })?;
        Ok(typed(
            AuthoredFormExpressionKind::Integrate {
                domain: domain_id,
                integrand: Box::new(integrand),
            },
            dimension,
            ValueShape::scalar(),
            None,
        ))
    }
}

fn parameter_expression(parameter: &ParameterDef) -> AuthoredFormExpression {
    typed(
        AuthoredFormExpressionKind::Parameter(parameter.id()),
        parameter.value().dim(),
        ValueShape::scalar(),
        None,
    )
}

fn typed(
    kind: AuthoredFormExpressionKind,
    dimension: DimExponents,
    shape: ValueShape,
    support: Option<Id<kinds::Domain>>,
) -> AuthoredFormExpression {
    AuthoredFormExpression {
        kind,
        dimension,
        shape,
        support,
    }
}

fn resolve_symbol(
    file: &str,
    range: TextRange,
    name: &str,
    symbols: &ModelSymbols,
) -> Result<RawId, Diagnostic> {
    if let Some(id) = symbols.get(name) {
        return Ok(id);
    }
    let suffix = format!(".{name}");
    let mut matches = symbols
        .iter()
        .filter_map(|(candidate, id)| candidate.ends_with(&suffix).then_some(id));
    let Some(id) = matches.next() else {
        return Err(error(
            file,
            range,
            format!("unknown Formulation symbol `{name}`"),
        ));
    };
    if matches.next().is_some() {
        return Err(error(
            file,
            range,
            format!("ambiguous Formulation symbol `{name}`"),
        ));
    }
    Ok(id)
}

fn unqualified_callee<'a>(
    file: &str,
    range: TextRange,
    callee: &'a NamePath,
) -> Result<&'a str, Diagnostic> {
    if callee.is_qualified() {
        Err(error(
            file,
            range,
            "Formulation operators must be unqualified",
        ))
    } else {
        Ok(callee.as_str())
    }
}

fn merge_support(
    file: &str,
    range: TextRange,
    left: Option<Id<kinds::Domain>>,
    right: Option<Id<kinds::Domain>>,
) -> Result<Option<Id<kinds::Domain>>, Diagnostic> {
    match (left, right) {
        (Some(left), Some(right)) if left != right => Err(error(
            file,
            range,
            "expression operands have different spatial supports",
        )),
        (Some(value), _) | (_, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn require_scalar(
    file: &str,
    range: TextRange,
    value: &AuthoredFormExpression,
) -> Result<(), Diagnostic> {
    if value.shape.is_scalar() {
        Ok(())
    } else {
        Err(error(file, range, "operator requires a scalar expression"))
    }
}

fn integer_literal(expression: &Expr) -> Option<i32> {
    let value = match expression.kind() {
        ExprKind::Number(value) => *value,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => match value.kind() {
            ExprKind::Number(value) => -*value,
            _ => return None,
        },
        _ => return None,
    };
    (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

fn error(file: &str, range: TextRange, message: impl Into<String>) -> Diagnostic {
    source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message)
}
