use super::*;

use eqiora_schema::kernel::typing::{self, ExpressionType, SpatialSupport};

impl LoweringExpression {
    pub(crate) fn collect_physical_port_names(&self, names: &mut BTreeSet<String>) -> bool {
        let mut pending = vec![self];
        let mut seen = BTreeSet::new();
        while let Some(expression) = pending.pop() {
            if !seen.insert(Arc::as_ptr(&expression.node) as usize) {
                continue;
            }
            match expression.node.as_ref() {
                LoweringExpressionNode::Call { callee, argument } => {
                    if matches!(callee.as_str(), "across" | "through" | "trace" | "flux")
                        && let LoweringExpressionNode::Name(name) = argument.node.as_ref()
                    {
                        names.insert(name.clone());
                    }
                    pending.push(argument);
                }
                LoweringExpressionNode::Neg(value)
                | LoweringExpressionNode::Index { value, .. }
                | LoweringExpressionNode::Sample { value, .. } => pending.push(value),
                LoweringExpressionNode::Array(elements) => pending.extend(elements),
                LoweringExpressionNode::Complex { real, imag } => pending.extend([real, imag]),
                LoweringExpressionNode::Binary { left, right, .. } => {
                    pending.push(left);
                    pending.push(right);
                }
                LoweringExpressionNode::PureOperator { arguments, .. } => pending.extend(arguments),
                LoweringExpressionNode::Literal(_) | LoweringExpressionNode::Name(_) => {}
                _ => return false,
            }
        }
        true
    }
}

pub(super) fn from_source(expression: &Expr) -> LoweringExpression {
    let kind = match expression.kind() {
        ExprKind::Array(elements) => LoweringExpressionNode::Array(
            elements
                .iter()
                .map(LoweringExpression::from_source)
                .collect(),
        ),
        ExprKind::Index { value, index } => match crate::hierarchy::closed_index(index) {
            Ok(index) => LoweringExpressionNode::Index {
                value: LoweringExpression::from_source(value),
                index,
            },
            Err(_) => LoweringExpressionNode::InvalidValue(
                "channel index requires a constant nonnegative integer",
            ),
        },
        ExprKind::Path(path) if path.as_str() == "math.i" => {
            return LoweringExpression::literal(
                eqiora_core::ValueLiteral::new(
                    eqiora_core::ValueType::scalar(
                        eqiora_core::ScalarDomain::Complex,
                        DimExponents::DIMENSIONLESS,
                    ),
                    [(0.0, 1.0)],
                )
                .expect("imaginary unit"),
                expression.range(),
            );
        }
        ExprKind::Call { callee, arguments } if callee.as_str() == "math.complex" => {
            match arguments.as_slice() {
                [real, imag] => LoweringExpressionNode::Complex {
                    real: LoweringExpression::from_source(real),
                    imag: LoweringExpression::from_source(imag),
                },
                _ => LoweringExpressionNode::InvalidValue(
                    "math.complex requires exactly two real scalar arguments",
                ),
            }
        }
        ExprKind::Quantity { value, unit } => match crate::units::quantity(value, unit) {
            Ok(value) => return LoweringExpression::quantity(value, expression.range()),
            Err(message) => LoweringExpressionNode::InvalidValue(message),
        },
        ExprKind::Number(value) => {
            return LoweringExpression::quantity(
                DynQuantity::new(*value, DimExponents::DIMENSIONLESS),
                expression.range(),
            );
        }
        ExprKind::Path(path) => match crate::math::constant(path) {
            Some(value) => {
                return LoweringExpression::quantity(
                    DynQuantity::new(value, DimExponents::DIMENSIONLESS),
                    expression.range(),
                );
            }
            None if crate::math::is_namespaced(path) => {
                LoweringExpressionNode::UnknownMath(path.as_str().to_owned())
            }
            None => LoweringExpressionNode::Unsupported,
        },
        ExprKind::Name(name) => LoweringExpressionNode::Name(name.clone()),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => return LoweringExpression::neg(from_source(value), expression.range()),
        ExprKind::Binary { op, left, right } => LoweringExpressionNode::Binary {
            operator: *op,
            left: from_source(left),
            right: from_source(right),
        },
        ExprKind::Call { callee, arguments } if callee.as_str() == "sample" => {
            match arguments.as_slice() {
                [value, clock] => match clock.kind() {
                    ExprKind::Name(clock) => LoweringExpressionNode::Sample {
                        value: from_source(value),
                        clock: clock.clone(),
                    },
                    _ => LoweringExpressionNode::InvalidValue("sample requires one clock name"),
                },
                _ => LoweringExpressionNode::InvalidValue(
                    "sample requires a value and one clock name",
                ),
            }
        }
        ExprKind::Call { callee, arguments }
            if (!callee.is_qualified() || crate::math::is_namespaced(callee))
                && arguments.len() == 1 =>
        {
            LoweringExpressionNode::Call {
                callee: callee.as_str().to_owned(),
                argument: from_source(&arguments[0]),
            }
        }
        _ => LoweringExpressionNode::Unsupported,
    };
    LoweringExpression {
        node: Arc::new(kind),
        range: expression.range(),
    }
}

pub(super) struct LoweredRelation {
    pub(super) residuals: ExprDag,
    pub(super) dependencies: BTreeSet<RawId>,
    pub(super) ports: BTreeSet<RawId>,
}

pub(super) fn lower_relation(
    file: &str,
    range: TextRange,
    activation: &ActivationSyntax,
    domain: Option<&str>,
    equations: &[LoweringEquation],
    initial: bool,
    bindings: &BTreeMap<String, Binding>,
) -> Result<LoweredRelation, Diagnostic> {
    if let Some(domain) = domain {
        match bindings.get(domain) {
            Some(Binding::Domain(_, DomainContract::Spatial { .. })) => {}
            Some(Binding::Domain(_, DomainContract::ScalarPhysical { .. })) => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "spatial Relation scope cannot be a scalar physical Domain",
                ));
            }
            Some(_) | None => {
                return Err(unresolved(file, range, domain, "Relation Domain"));
            }
        }
    }
    if let ActivationSyntax::Periodic(clock) = activation {
        if !matches!(bindings.get(clock), Some(Binding::Clock(_))) {
            return Err(unresolved(file, range, clock, "periodic ClockDomain"));
        }
    } else if !matches!(activation, ActivationSyntax::Continuous) {
        return Err(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "Activation syntax is newer than this compiler",
        ));
    }

    let support = domain
        .map(|name| relation_support(file, range, name, bindings))
        .transpose()?;
    let discrete = matches!(activation, ActivationSyntax::Periodic(_));
    let mut lowerer = ExpressionLowerer {
        file,
        bindings,
        builder: ExprDagBuilder::new(),
        dependencies: BTreeSet::new(),
        ports: BTreeSet::new(),
        cache: HashMap::new(),
        sampling: false,
        allow_discrete_symbols: discrete || initial,
        activation,
        initial,
    };
    let mut normalized = Vec::with_capacity(equations.len());
    for equation in equations {
        let left_type = expression_type(file, &equation.left, bindings, support.as_ref())?;
        let right_type = expression_type(file, &equation.right, bindings, support.as_ref())?;
        let checked = equality::check(
            left_type,
            right_type,
            equation.contextual_left_zero,
            equation.contextual_right_zero,
        )
        .map_err(|error| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                equation.range,
                error.to_string(),
            )
        })?;
        typing::residual(
            &checked.residual,
            if initial {
                checked.residual.support.as_ref()
            } else {
                support.as_ref()
            },
        )
        .map_err(|error| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                equation.range,
                error.to_string(),
            )
        })?;
        let contextual =
            |expression: &LoweringExpression, is_zero, value_type: &eqiora_core::ValueType| {
                if is_zero {
                    LoweringExpression::literal(
                        eqiora_core::ValueLiteral::from_real(value_type.clone(), 0.0)
                            .expect("zero inhabits every checked mathematical type"),
                        expression.range(),
                    )
                } else {
                    expression.clone()
                }
            };
        let left = contextual(
            &equation.left,
            equation.contextual_left_zero,
            &checked.left.value_type,
        );
        let right = contextual(
            &equation.right,
            equation.contextual_right_zero,
            &checked.right.value_type,
        );
        // This neutral-element rule applies only after both operands and the
        // resulting support/type have passed admission. In particular a real
        // left side cannot absorb the promotion caused by a complex zero.
        let residual = if equation.literal_right_zero && checked.residual == checked.left {
            left
        } else {
            LoweringExpression::binary(BinaryOp::Sub, left, right, equation.range)
        };
        normalized.push(residual);
    }
    // Keep all normalized nodes alive for the pointer-keyed lowering cache.
    let roots = normalized
        .iter()
        .map(|residual| lowerer.lower(residual).map(|value| value.id))
        .collect::<Result<Vec<_>, _>>()?;
    let residuals = lowerer.builder.finish(roots).map_err(|diagnostic| {
        source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            diagnostic.message(),
        )
    })?;
    Ok(LoweredRelation {
        residuals,
        dependencies: lowerer.dependencies,
        ports: lowerer.ports,
    })
}

mod types;
use types::expression_type;
pub(super) use types::relation_support;

fn spatial_type_error(
    file: &str,
    expression: &LoweringExpression,
    error: impl std::fmt::Display,
) -> Diagnostic {
    source_error(
        codes::LANGUAGE_TYPE_ERROR,
        file,
        expression.range(),
        error.to_string(),
    )
}

struct ExpressionLowerer<'a> {
    file: &'a str,
    bindings: &'a BTreeMap<String, Binding>,
    builder: ExprDagBuilder,
    dependencies: BTreeSet<RawId>,
    ports: BTreeSet<RawId>,
    cache: HashMap<(usize, bool), TypedExpression>,
    sampling: bool,
    allow_discrete_symbols: bool,
    activation: &'a ActivationSyntax,
    initial: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TypedExpression {
    id: ExprId,
    pub(super) dimension: DimExponents,
}

impl ExpressionLowerer<'_> {
    fn lower(&mut self, expression: &LoweringExpression) -> Result<TypedExpression, Diagnostic> {
        let key = (Arc::as_ptr(&expression.node) as usize, self.sampling);
        if let Some(lowered) = self.cache.get(&key) {
            return Ok(*lowered);
        }
        let lowered = match expression.node.as_ref() {
            LoweringExpressionNode::Sample { value, clock } => {
                let Some(Binding::Clock(id)) = self.bindings.get(clock) else {
                    return Err(unresolved(
                        self.file,
                        expression.range(),
                        clock,
                        "sample ClockDomain",
                    ));
                };
                let id = *id;
                if self.sampling
                    || self.initial
                    || self.activation != &ActivationSyntax::Periodic(clock.clone())
                {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        expression.range(),
                        "sample requires its exact clock's update relation",
                    ));
                }
                self.sampling = true;
                let operand = self.lower(value);
                self.sampling = false;
                let operand = operand?;
                self.dependencies.insert(id.erase());
                self.builder
                    .sample(operand.id, id)
                    .map(|id| TypedExpression {
                        id,
                        dimension: operand.dimension,
                    })
                    .map_err(|error| self.builder_error(expression, error))
            }
            LoweringExpressionNode::Array(elements) => {
                let elements = elements
                    .iter()
                    .map(|value| self.lower(value))
                    .collect::<Result<Vec<_>, _>>()?;
                self.builder
                    .array(elements.iter().map(|element| element.id))
                    .map(|id| TypedExpression {
                        id,
                        dimension: elements[0].dimension,
                    })
                    .map_err(|diagnostic| self.builder_error(expression, diagnostic))
            }
            LoweringExpressionNode::Index { value, index } => {
                let value = self.lower(value)?;
                self.builder
                    .index(value.id, *index)
                    .map(|id| TypedExpression {
                        id,
                        dimension: value.dimension,
                    })
                    .map_err(|diagnostic| self.builder_error(expression, diagnostic))
            }
            LoweringExpressionNode::Complex { real, imag } => {
                let real = self.lower(real)?;
                let imag = self.lower(imag)?;
                self.builder
                    .complex(real.id, imag.id)
                    .map(|id| TypedExpression {
                        id,
                        dimension: real.dimension,
                    })
                    .map_err(|diagnostic| self.builder_error(expression, diagnostic))
            }
            LoweringExpressionNode::InvalidValue(message) => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                *message,
            )),
            LoweringExpressionNode::Literal(value) => self
                .builder
                .constant(value.clone())
                .map(|id| TypedExpression {
                    id,
                    dimension: value.value_type().dimension(),
                })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic)),
            LoweringExpressionNode::Name(name) if name == "time" => self
                .builder
                .symbol(SymbolRef::Time)
                .map(|id| TypedExpression {
                    id,
                    dimension: time_dimension(),
                })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic)),
            LoweringExpressionNode::Name(name) => self.lower_name(expression, name),
            LoweringExpressionNode::Neg(value) => {
                let value = self.lower(value)?;
                self.builder
                    .neg(value.id)
                    .map(|id| TypedExpression {
                        id,
                        dimension: value.dimension,
                    })
                    .map_err(|diagnostic| self.builder_error(expression, diagnostic))
            }
            LoweringExpressionNode::Binary {
                operator,
                left,
                right,
            } => self.lower_binary(expression, *operator, left, right),
            LoweringExpressionNode::Call { callee, argument } => {
                self.lower_call(expression, callee, argument)
            }
            LoweringExpressionNode::PureOperator {
                definition,
                arguments,
            } => self.lower_pure_operator(expression, definition, arguments),
            LoweringExpressionNode::UnknownMath(path) => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                format!("unknown compiler-owned scalar mathematics member `{path}`"),
            )),
            LoweringExpressionNode::Unsupported => Err(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                self.file,
                expression.range(),
                "expression syntax is newer than this compiler",
            )),
        }?;
        self.cache.insert(key, lowered);
        Ok(lowered)
    }

    fn lower_name(
        &mut self,
        expression: &LoweringExpression,
        name: &str,
    ) -> Result<TypedExpression, Diagnostic> {
        let Some(binding) = self.bindings.get(name).cloned() else {
            return Err(unresolved(
                self.file,
                expression.range(),
                name,
                "expression symbol",
            ));
        };
        let (symbol, id, dimension) = match binding {
            Binding::Field(id, contract) => {
                if self.sampling && !matches!(contract.activation, ActivationSyntax::Continuous) {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        expression.range(),
                        "sample operand must be continuous",
                    ));
                }
                (SymbolRef::Field(id), id.erase(), contract.dimension)
            }
            Binding::Parameter(id, value_type) => {
                (SymbolRef::Parameter(id), id.erase(), value_type.dimension())
            }
            Binding::Port(id, contract) => match resolve_port_contract(
                self.file,
                expression.range(),
                &contract,
                self.bindings,
            )? {
                ResolvedPortContract::Signal {
                    value_type, clock, ..
                } => {
                    let expected = if self.sampling {
                        None
                    } else {
                        match self.activation {
                            ActivationSyntax::Periodic(name) => match self.bindings.get(name) {
                                Some(Binding::Clock(id)) => Some(*id),
                                _ => None,
                            },
                            _ => None,
                        }
                    };
                    if clock != expected {
                        return Err(source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.file,
                            expression.range(),
                            "signal Port read requires the exact declared activation; use an explicit transition",
                        ));
                    }
                    self.ports.insert(id.erase());
                    (SymbolRef::Port(id), id.erase(), value_type.dimension())
                }
                ResolvedPortContract::ScalarPhysical { .. } => {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        expression.range(),
                        format!(
                            "scalar physical Port `{name}` must be read as `across({name})` or `through({name})`"
                        ),
                    ));
                }
                ResolvedPortContract::BoundaryPhysical { .. } => {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        expression.range(),
                        format!(
                            "field-physical Port `{name}` must be read as `trace({name})` or `flux({name})`"
                        ),
                    ));
                }
            },
            Binding::Domain(_, _)
            | Binding::Representation(_)
            | Binding::Clock(_)
            | Binding::Relation { .. } => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    format!("`{name}` is not a scalar Field, Parameter, or Port"),
                ));
            }
        };
        self.dependencies.insert(id);
        self.builder
            .symbol(symbol)
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn lower_call(
        &mut self,
        expression: &LoweringExpression,
        callee: &str,
        argument: &LoweringExpression,
    ) -> Result<TypedExpression, Diagnostic> {
        if callee == "sin" {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                "bare `sin` is not language vocabulary; use compiler-owned `math.sin`",
            ));
        }
        if callee.starts_with("math.") && !matches!(callee, "math.sin" | "math.sqrt") {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                format!("unknown compiler-owned scalar mathematics member `{callee}`"),
            ));
        }
        let boundary_trace = callee == "trace"
            && matches!(
                argument.node.as_ref(),
                LoweringExpressionNode::Name(name)
                    if matches!(
                        self.bindings.get(name),
                        Some(Binding::Port(_, PortContract::BoundaryPhysical { .. }))
                    )
            );
        if matches!(callee, "across" | "through" | "flux") || boundary_trace {
            return self.lower_physical_accessor(expression, callee, argument);
        }
        if callee == "coordinate" {
            let axis = lowering_integer_literal(argument)
                .and_then(|axis| usize::try_from(axis).ok())
                .ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        argument.range(),
                        "coordinate(...) requires a non-negative integer literal axis",
                    )
                })?;
            return self
                .builder
                .spatial_coordinate(axis)
                .map(|id| TypedExpression {
                    id,
                    dimension: length_dimension(),
                })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic));
        }
        if matches!(callee, "math.sin" | "math.sqrt") {
            let operand = self.lower(argument)?;
            if callee == "math.sin" && operand.dimension != DimExponents::DIMENSIONLESS {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    argument.range(),
                    format!(
                        "math.sin(...) requires a dimensionless scalar, received [{}]",
                        operand.dimension
                    ),
                ));
            }
            let (function, dimension) = if callee == "math.sqrt" {
                (
                    UnaryMathFunction::Sqrt,
                    operand
                        .dimension
                        .pow(1, 2)
                        .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
                )
            } else {
                (UnaryMathFunction::Sin, DimExponents::DIMENSIONLESS)
            };
            return self
                .builder
                .unary_math(function, operand.id)
                .map(|id| TypedExpression { id, dimension })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic));
        }
        if matches!(
            callee,
            "grad" | "div" | "symmetric_part" | "isotropic_lift" | "trace" | "normal"
        ) {
            let operand = self.lower(argument)?;
            let (result, dimension) = match callee {
                "grad" => (
                    self.builder.gradient(operand.id),
                    operand
                        .dimension
                        .div(length_dimension())
                        .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
                ),
                "div" => (
                    self.builder.divergence(operand.id),
                    operand
                        .dimension
                        .div(length_dimension())
                        .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
                ),
                "symmetric_part" => (self.builder.symmetric_part(operand.id), operand.dimension),
                "isotropic_lift" => (self.builder.isotropic_lift(operand.id), operand.dimension),
                "trace" => (self.builder.trace(operand.id), operand.dimension),
                "normal" => (self.builder.normal_component(operand.id), operand.dimension),
                _ => unreachable!("spatial operator was matched"),
            };
            return result
                .map(|id| TypedExpression { id, dimension })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic));
        }
        let LoweringExpressionNode::Name(name) = argument.node.as_ref() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                argument.range(),
                format!("{callee}(...) requires one Field name"),
            ));
        };
        let Some(Binding::Field(field, contract)) = self.bindings.get(name).cloned() else {
            return Err(unresolved(
                self.file,
                argument.range(),
                name,
                "Field operator argument",
            ));
        };
        if callee == "hold" {
            if contract.role != eqiora_lang::FieldRoleSyntax::State
                || !matches!(contract.activation, ActivationSyntax::Periodic(_))
            {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    "hold requires one periodic State",
                ));
            }
            self.dependencies.insert(field.erase());
            let symbol = self
                .builder
                .symbol(SymbolRef::Field(field))
                .map_err(|error| self.builder_error(expression, error))?;
            return self
                .builder
                .hold(symbol)
                .map(|id| TypedExpression {
                    id,
                    dimension: contract.dimension,
                })
                .map_err(|error| self.builder_error(expression, error));
        }
        if self.sampling {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                "sample operand cannot contain an evolution operator",
            ));
        }
        if matches!(callee, "derivative" | "pre" | "next") {
            let eligible = contract.role == eqiora_lang::FieldRoleSyntax::State
                && match callee {
                    "derivative" => matches!(contract.activation, ActivationSyntax::Continuous),
                    "pre" => {
                        matches!(contract.activation, ActivationSyntax::Periodic(_))
                            && (self.initial || contract.activation == *self.activation)
                    }
                    "next" => {
                        !self.initial
                            && matches!(contract.activation, ActivationSyntax::Periodic(_))
                            && contract.activation == *self.activation
                    }
                    _ => false,
                };
            if !eligible {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    "evolution operator requires an eligible declared state at the exact clock",
                ));
            }
        }
        if matches!(callee, "pre" | "next") && !self.allow_discrete_symbols {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                format!("continuous Relation cannot use `{callee}`"),
            ));
        }
        if callee == "derivative" && self.allow_discrete_symbols && !self.initial {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                "clocked Relation cannot use `derivative`",
            ));
        }
        let (symbol, dimension) = match callee {
            "derivative" => (
                SymbolRef::Derivative(field),
                contract.dimension.div(time_dimension()).ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.file,
                        expression.range(),
                        "derivative dimension exponent exceeds rational exponent bounds",
                    )
                })?,
            ),
            "pre" => (SymbolRef::Pre(field), contract.dimension),
            "next" => (SymbolRef::Next(field), contract.dimension),
            _ => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    format!("unknown scalar operator `{callee}`"),
                ));
            }
        };
        self.dependencies.insert(field.erase());
        self.builder
            .symbol(symbol)
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn lower_pure_operator(
        &mut self,
        expression: &LoweringExpression,
        definition: &PureOperatorDefinition,
        arguments: &[LoweringExpression],
    ) -> Result<TypedExpression, Diagnostic> {
        let arguments = arguments
            .iter()
            .map(|argument| self.lower(argument))
            .collect::<Result<Vec<_>, _>>()?;
        let dimension = instantiate_pure_dimension(definition, &arguments).ok_or_else(|| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                "pure-operator result dimension overflows the portable SI exponent range",
            )
        })?;
        self.builder
            .pure_operator(definition, arguments.iter().map(|argument| argument.id))
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn lower_physical_accessor(
        &mut self,
        expression: &LoweringExpression,
        callee: &str,
        argument: &LoweringExpression,
    ) -> Result<TypedExpression, Diagnostic> {
        let LoweringExpressionNode::Name(name) = argument.node.as_ref() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                argument.range(),
                if matches!(callee, "across" | "through") {
                    format!("`{callee}(...)` requires one bare scalar physical Port name")
                } else {
                    format!("`{callee}(...)` requires one bare field-physical Port name")
                },
            ));
        };
        let Some(binding) = self.bindings.get(name) else {
            return Err(unresolved(
                self.file,
                argument.range(),
                name,
                "scalar physical Port",
            ));
        };
        let Binding::Port(port, contract) = binding else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                argument.range(),
                format!("`{name}` is not a scalar physical Port"),
            ));
        };
        let contract = resolve_port_contract(self.file, argument.range(), contract, self.bindings)?;
        self.dependencies.insert(port.erase());
        self.ports.insert(port.erase());
        let (symbol, dimension) = match (callee, contract) {
            ("across", ResolvedPortContract::ScalarPhysical { across_type, .. }) => {
                (SymbolRef::Across(*port), across_type.dimension())
            }
            ("through", ResolvedPortContract::ScalarPhysical { through_type, .. }) => {
                (SymbolRef::Through(*port), through_type.dimension())
            }
            ("trace", ResolvedPortContract::BoundaryPhysical { trace_type, .. }) => {
                (SymbolRef::PortTrace(*port), trace_type.dimension())
            }
            ("flux", ResolvedPortContract::BoundaryPhysical { flux_type, .. }) => {
                (SymbolRef::PortFlux(*port), flux_type.dimension())
            }
            _ => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    argument.range(),
                    if matches!(callee, "across" | "through") {
                        format!("`{name}` is not a scalar physical Port")
                    } else {
                        format!("`{name}` is not a field-physical Port")
                    },
                ));
            }
        };
        self.builder
            .symbol(symbol)
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn lower_binary(
        &mut self,
        expression: &LoweringExpression,
        operator: BinaryOp,
        left: &LoweringExpression,
        right: &LoweringExpression,
    ) -> Result<TypedExpression, Diagnostic> {
        if operator == BinaryOp::Pow {
            let base = self.lower(left)?;
            let exponent = lowering_integer_literal(right).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    right.range(),
                    "power exponent must be an i32 integer literal",
                )
            })?;
            let dimension = base.dimension.pow(exponent, 1).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    "power dimension exponent exceeds rational exponent bounds",
                )
            })?;
            return self
                .builder
                .powi(base.id, exponent)
                .map(|id| TypedExpression { id, dimension })
                .map_err(|diagnostic| self.builder_error(expression, diagnostic));
        }

        let left = self.lower(left)?;
        let right = self.lower(right)?;
        let dimension = match operator {
            BinaryOp::Add | BinaryOp::Sub if left.dimension == right.dimension => left.dimension,
            BinaryOp::Add | BinaryOp::Sub => {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    format!(
                        "addition/subtraction combines dimensions [{}] and [{}]",
                        left.dimension, right.dimension
                    ),
                ));
            }
            BinaryOp::Mul => left
                .dimension
                .mul(right.dimension)
                .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
            BinaryOp::Div => left
                .dimension
                .div(right.dimension)
                .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
            BinaryOp::Pow => unreachable!("power handled above"),
        };
        let result = match operator {
            BinaryOp::Add => self.builder.add(left.id, right.id),
            BinaryOp::Sub => self.builder.sub(left.id, right.id),
            BinaryOp::Mul => self.builder.mul(left.id, right.id),
            BinaryOp::Div => self.builder.div(left.id, right.id),
            BinaryOp::Pow => unreachable!("power handled above"),
        };
        result
            .map(|id| TypedExpression { id, dimension })
            .map_err(|diagnostic| self.builder_error(expression, diagnostic))
    }

    fn builder_error(&self, expression: &LoweringExpression, diagnostic: Diagnostic) -> Diagnostic {
        source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            self.file,
            expression.range(),
            diagnostic.message(),
        )
    }
}

pub(super) fn lowering_integer_literal(expression: &LoweringExpression) -> Option<i32> {
    let value = match expression.node.as_ref() {
        LoweringExpressionNode::Literal(value)
            if value.value_type().dimension() == DimExponents::DIMENSIONLESS =>
        {
            value.real_scalar_value()?.value()
        }
        LoweringExpressionNode::Neg(value) => match value.node.as_ref() {
            LoweringExpressionNode::Literal(value)
                if value.value_type().dimension() == DimExponents::DIMENSIONLESS =>
            {
                -value.real_scalar_value()?.value()
            }
            _ => return None,
        },
        _ => return None,
    };
    (value.fract() == 0.0 && value >= f64::from(i32::MIN) && value <= f64::from(i32::MAX))
        .then_some(value as i32)
}

fn instantiate_pure_dimension(
    definition: &PureOperatorDefinition,
    arguments: &[TypedExpression],
) -> Option<DimExponents> {
    if arguments.len() != definition.formals().len() {
        return None;
    }
    arguments
        .iter()
        .zip(definition.dimension_monomial().exponents())
        .try_fold(
            DimExponents::DIMENSIONLESS,
            |result, (argument, exponent)| {
                let term = argument.dimension.pow(i32::from(*exponent), 1)?;
                result.mul(term)
            },
        )
}
