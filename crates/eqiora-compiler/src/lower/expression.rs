use super::*;

use eqiora_schema::kernel::typing::{self, ExpressionType, SpatialSupport};

pub(super) fn from_source(expression: &Expr) -> LoweringExpression {
    let kind = match expression.kind() {
        ExprKind::Number(value) => LoweringExpressionNode::Quantity(DynQuantity::new(
            normalize_zero(*value),
            DimExponents::DIMENSIONLESS,
        )),
        ExprKind::Path(path) => {
            crate::math::constant(path).map_or(LoweringExpressionNode::Unsupported, |value| {
                LoweringExpressionNode::Quantity(DynQuantity::new(
                    value,
                    DimExponents::DIMENSIONLESS,
                ))
            })
        }
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
        ExprKind::Call { callee, arguments } if !callee.is_qualified() && arguments.len() == 1 => {
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
    residuals: &[LoweringExpression],
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

    let mut lowerer = ExpressionLowerer {
        file,
        bindings,
        builder: ExprDagBuilder::new(),
        dependencies: BTreeSet::new(),
        ports: BTreeSet::new(),
        cache: HashMap::new(),
        allow_discrete_symbols: matches!(activation, ActivationSyntax::Periodic(_)),
    };
    let mut roots = Vec::new();
    for residual in residuals {
        validate_spatial_operator_types(file, residual, bindings)?;
        roots.push(lowerer.lower(residual)?.id);
    }
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

/// Apply the shared shape/frame rules before the legacy flat DAG lowerer
/// erases that information. `None` means this bounded pass encountered a
/// physical accessor or another expression family whose existing owner still
/// performs admission; nested spatial operators have already been checked.
fn validate_spatial_operator_types(
    file: &str,
    expression: &LoweringExpression,
    bindings: &BTreeMap<String, Binding>,
) -> Result<Option<ExpressionType<RawId>>, Diagnostic> {
    let typed = match expression.node.as_ref() {
        LoweringExpressionNode::Quantity(value) => Some(ExpressionType::scalar(value.dim(), None)),
        LoweringExpressionNode::Name(name) if name == "time" => {
            Some(ExpressionType::scalar(time_dimension(), None))
        }
        LoweringExpressionNode::Name(name) => match bindings.get(name) {
            Some(Binding::Field(_, contract)) => Some(field_expression_type(
                file,
                expression.range(),
                contract,
                bindings,
            )?),
            Some(Binding::Parameter(_, dimension)) => {
                Some(ExpressionType::scalar(*dimension, None))
            }
            _ => None,
        },
        LoweringExpressionNode::Neg(value) => {
            validate_spatial_operator_types(file, value, bindings)?
        }
        LoweringExpressionNode::Binary {
            operator,
            left,
            right,
        } => {
            let exponent = (*operator == BinaryOp::Pow)
                .then(|| lowering_integer_literal(right))
                .flatten();
            let left = validate_spatial_operator_types(file, left, bindings)?;
            let right = validate_spatial_operator_types(file, right, bindings)?;
            match (left, right) {
                (Some(left), Some(right)) => {
                    let result = match operator {
                        BinaryOp::Add | BinaryOp::Sub if left.dimension != right.dimension => {
                            // Preserve the established flat-lowerer diagnostic,
                            // which reports the two dimensions compactly.
                            return Ok(None);
                        }
                        BinaryOp::Add | BinaryOp::Sub => typing::additive(&left, &right),
                        BinaryOp::Mul => typing::multiply(&left, &right),
                        BinaryOp::Div => typing::divide(&left, &right),
                        BinaryOp::Pow => {
                            let Some(exponent) = exponent else {
                                return Ok(None);
                            };
                            typing::power(&left, exponent)
                        }
                    };
                    // Arithmetic diagnostics retain their established owner
                    // and graph path. Successful composition is needed here
                    // only to type a surrounding spatial operator.
                    result.ok()
                }
                _ => None,
            }
        }
        LoweringExpressionNode::Call { callee, argument } => {
            let operand = validate_spatial_operator_types(file, argument, bindings)?;
            match (callee.as_str(), operand) {
                ("grad", Some(operand)) => Some(
                    typing::gradient(&operand)
                        .map_err(|error| spatial_type_error(file, expression, error))?,
                ),
                ("div", Some(operand)) => Some(
                    typing::divergence(&operand)
                        .map_err(|error| spatial_type_error(file, expression, error))?,
                ),
                ("symmetric_part", Some(operand)) => Some(
                    typing::symmetric_part(&operand)
                        .map_err(|error| spatial_type_error(file, expression, error))?,
                ),
                ("isotropic_lift", Some(operand)) => Some(
                    typing::isotropic_lift(&operand)
                        .map_err(|error| spatial_type_error(file, expression, error))?,
                ),
                ("sin", Some(operand)) => typing::sine(&operand).ok(),
                _ => None,
            }
        }
        LoweringExpressionNode::PureOperator { arguments, .. } => {
            for argument in arguments {
                validate_spatial_operator_types(file, argument, bindings)?;
            }
            None
        }
        LoweringExpressionNode::Unsupported => None,
    };
    Ok(typed)
}

fn field_expression_type(
    file: &str,
    range: TextRange,
    contract: &FieldContract,
    bindings: &BTreeMap<String, Binding>,
) -> Result<ExpressionType<RawId>, Diagnostic> {
    let resolved = resolve_field_contract(file, range, contract, bindings)?;
    let support = contract.domain.as_deref().and_then(|domain| {
        let Binding::Domain(
            id,
            DomainContract::Spatial {
                dimensions: Some(dimensions),
            },
        ) = bindings.get(domain)?
        else {
            return None;
        };
        Some(SpatialSupport::Volume {
            domain: id.erase(),
            dimensions: *dimensions,
        })
    });
    Ok(ExpressionType::shaped(
        resolved.dimension,
        resolved.shape,
        resolved.frame,
        support,
    ))
}

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
    cache: HashMap<usize, TypedExpression>,
    allow_discrete_symbols: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TypedExpression {
    id: ExprId,
    pub(super) dimension: DimExponents,
}

impl ExpressionLowerer<'_> {
    fn lower(&mut self, expression: &LoweringExpression) -> Result<TypedExpression, Diagnostic> {
        let key = Arc::as_ptr(&expression.node) as usize;
        if let Some(lowered) = self.cache.get(&key) {
            return Ok(*lowered);
        }
        let lowered = match expression.node.as_ref() {
            LoweringExpressionNode::Quantity(value) => self
                .builder
                .constant(*value)
                .map(|id| TypedExpression {
                    id,
                    dimension: value.dim(),
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
            Binding::Field(id, contract) => (SymbolRef::Field(id), id.erase(), contract.dimension),
            Binding::Parameter(id, dimension) => (SymbolRef::Parameter(id), id.erase(), dimension),
            Binding::Port(id, contract) => match resolve_port_contract(
                self.file,
                expression.range(),
                &contract,
                self.bindings,
            )? {
                ResolvedPortContract::Signal { dimension, .. }
                | ResolvedPortContract::ConservingMarker { dimension } => {
                    self.ports.insert(id.erase());
                    (SymbolRef::Port(id), id.erase(), dimension)
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
        if callee == "sin" {
            let operand = self.lower(argument)?;
            if operand.dimension != DimExponents::DIMENSIONLESS {
                return Err(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    argument.range(),
                    format!(
                        "sin(...) requires a dimensionless scalar, received [{}]",
                        operand.dimension
                    ),
                ));
            }
            return self
                .builder
                .unary_math(UnaryMathFunction::Sin, operand.id)
                .map(|id| TypedExpression {
                    id,
                    dimension: DimExponents::DIMENSIONLESS,
                })
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
                    checked_dimensions(operand.dimension, length_dimension(), i8::checked_sub)
                        .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
                ),
                "div" => (
                    self.builder.divergence(operand.id),
                    checked_dimensions(operand.dimension, length_dimension(), i8::checked_sub)
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
        if matches!(callee, "pre" | "next") && !self.allow_discrete_symbols {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.file,
                expression.range(),
                format!("continuous Relation cannot use `{callee}`"),
            ));
        }
        let (symbol, dimension) = match callee {
            "derivative" => (
                SymbolRef::Derivative(field),
                checked_dimensions(contract.dimension, time_dimension(), i8::checked_sub)
                    .ok_or_else(|| {
                        source_error(
                            codes::LANGUAGE_TYPE_ERROR,
                            self.file,
                            expression.range(),
                            "derivative dimension exponent overflows i8",
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
            (
                "across",
                ResolvedPortContract::ScalarPhysical {
                    across_dimension, ..
                },
            ) => (SymbolRef::Across(*port), across_dimension),
            (
                "through",
                ResolvedPortContract::ScalarPhysical {
                    through_dimension, ..
                },
            ) => (SymbolRef::Through(*port), through_dimension),
            (
                "trace",
                ResolvedPortContract::BoundaryPhysical {
                    trace_dimension, ..
                },
            ) => (SymbolRef::PortTrace(*port), trace_dimension),
            ("flux", ResolvedPortContract::BoundaryPhysical { flux_dimension, .. }) => {
                (SymbolRef::PortFlux(*port), flux_dimension)
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
            let dimension = checked_scale_dimension(base.dimension, exponent).ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    self.file,
                    expression.range(),
                    "power dimension exponent overflows i8",
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
            BinaryOp::Mul => checked_dimensions(left.dimension, right.dimension, i8::checked_add)
                .ok_or_else(|| dimension_overflow(self.file, expression.range()))?,
            BinaryOp::Div => checked_dimensions(left.dimension, right.dimension, i8::checked_sub)
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
