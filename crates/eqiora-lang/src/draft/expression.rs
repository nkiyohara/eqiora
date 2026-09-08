//! Projection of native expressions into the shared AST.

use super::*;

impl DraftExpression {
    /// Project an authored expression into the shared AST with synthetic ranges.
    #[doc(hidden)]
    /// # Errors
    /// Rejects invalid literal syntax and enum members without exact lexical declarations.
    pub fn source_ast<'a>(
        &self,
        mut resolve: impl FnMut(eqiora_core::RawId) -> Option<NamePath>,
        mut resolve_enum: impl FnMut(eqiora_core::RawId) -> Option<&'a eqiora_schema::kernel::EnumDef>,
    ) -> Result<Expr, crate::AstConstructionError> {
        if self.contains_invalid_literal() {
            return Err(crate::AstConstructionError::new(
                "invalid native expression literal",
            ));
        }
        self.ast(
            &GraphPath::new(["argument".to_owned()]),
            &mut RangeAllocator::default(),
            &mut HashMap::new(),
            &mut resolve,
            &mut resolve_enum,
        )
    }

    pub(super) fn ast<'a>(
        &self,
        path: &GraphPath,
        ranges: &mut RangeAllocator,
        paths: &mut HashMap<TextRange, GraphPath>,
        resolve: &mut dyn FnMut(eqiora_core::RawId) -> Option<NamePath>,
        resolve_enum: &mut dyn FnMut(
            eqiora_core::RawId,
        ) -> Option<&'a eqiora_schema::kernel::EnumDef>,
    ) -> Result<Expr, crate::AstConstructionError> {
        let kind = match &self.kind {
            DraftExpressionKind::EnumValue(value) => {
                return crate::SourceAstFactory::value_literal(
                    value,
                    None,
                    ranges.allocate(path, paths),
                    resolve,
                    resolve_enum,
                );
            }
            DraftExpressionKind::Select {
                condition,
                then_value,
                else_value,
            } => ExprKind::Select {
                condition: Box::new(condition.ast(path, ranges, paths, resolve, resolve_enum)?),
                then_value: Box::new(then_value.ast(path, ranges, paths, resolve, resolve_enum)?),
                else_value: Box::new(else_value.ast(path, ranges, paths, resolve, resolve_enum)?),
            },
            DraftExpressionKind::Boolean(value) => ExprKind::Boolean(*value),
            DraftExpressionKind::Constant(value) => ExprKind::Number(value.clone()),
            DraftExpressionKind::Complex(real, imaginary) => ExprKind::Call {
                callee: NamePath::from_parsed_segments(
                    vec!["math".to_owned(), "complex".to_owned()],
                    ranges.allocate(path, paths),
                ),
                arguments: crate::CallArguments::Positional(
                    [*real, *imaginary]
                        .into_iter()
                        .map(|number| Expr {
                            resolved_enum: None,
                            resolved_nominal: None,
                            kind: ExprKind::Number(
                                crate::DecimalLiteral::from_f64(number)
                                    .expect("validated native literal"),
                            ),
                            range: ranges.allocate(path, paths),
                        })
                        .collect(),
                ),
            },
            DraftExpressionKind::Array(values) => ExprKind::Array(
                values
                    .iter()
                    .map(|value| value.ast(path, ranges, paths, resolve, resolve_enum))
                    .collect::<Result<_, _>>()?,
            ),
            DraftExpressionKind::Index { value, index } => ExprKind::Index {
                value: Box::new(value.ast(path, ranges, paths, resolve, resolve_enum)?),
                index: Box::new(Expr {
                    resolved_enum: None,
                    resolved_nominal: None,
                    kind: ExprKind::Number(
                        crate::DecimalLiteral::parse(&index.to_string()).expect("u32 index"),
                    ),
                    range: ranges.allocate(path, paths),
                }),
            },
            DraftExpressionKind::Reference(reference) => ExprKind::Name(reference.name.clone()),
            DraftExpressionKind::Derivative(reference) => ExprKind::Call {
                callee: NamePath::single("derivative".to_owned(), ranges.allocate(path, paths)),
                arguments: crate::CallArguments::Positional(vec![Expr {
                    resolved_enum: None,
                    resolved_nominal: None,
                    kind: ExprKind::Name(reference.name.clone()),
                    range: ranges.allocate(path, paths),
                }]),
            },
            DraftExpressionKind::Across(reference) => {
                physical_accessor_ast(&reference.across_name, reference, path, ranges, paths)
            }
            DraftExpressionKind::Through(reference) => {
                physical_accessor_ast(&reference.through_name, reference, path, ranges, paths)
            }
            DraftExpressionKind::SpatialCall { operator, value } => ExprKind::Call {
                callee: NamePath::single(
                    operator.source_name().to_owned(),
                    ranges.allocate(path, paths),
                ),
                arguments: crate::CallArguments::Positional(vec![value.ast(
                    path,
                    ranges,
                    paths,
                    resolve,
                    resolve_enum,
                )?]),
            },
            DraftExpressionKind::Unary { operator, value } => ExprKind::Unary {
                op: *operator,
                value: Box::new(value.ast(path, ranges, paths, resolve, resolve_enum)?),
            },
            DraftExpressionKind::Binary {
                operator,
                left,
                right,
            } => ExprKind::Binary {
                op: *operator,
                left: Box::new(left.ast(path, ranges, paths, resolve, resolve_enum)?),
                right: Box::new(right.ast(path, ranges, paths, resolve, resolve_enum)?),
            },
        };
        crate::SourceAstFactory::expression(kind, ranges.allocate(path, paths))
    }
}

impl DraftExpression {
    /// Retain one checked nominal enum member without storing source names.
    ///
    /// # Errors
    /// Rejects values that are not enum members.
    pub fn enum_value(
        value: eqiora_core::ValueLiteral,
    ) -> Result<Self, crate::AstConstructionError> {
        if value.enum_tag().is_none() {
            return Err(crate::AstConstructionError::new(
                "enum expression requires an exact enum member",
            ));
        }
        Ok(Self {
            kind: DraftExpressionKind::EnumValue(value),
        })
    }
    /// Boolean literal with no numeric coercion.
    #[must_use]
    pub const fn boolean(value: bool) -> Self {
        Self {
            kind: DraftExpressionKind::Boolean(value),
        }
    }
    /// Select one value lazily from a Boolean condition and two authored branches.
    #[must_use]
    pub fn select(condition: Self, then_value: Self, else_value: Self) -> Self {
        Self {
            kind: DraftExpressionKind::Select {
                condition: Box::new(condition),
                then_value: Box::new(then_value),
                else_value: Box::new(else_value),
            },
        }
    }
    /// Boolean negation.
    #[must_use]
    pub fn logical_not(self) -> Self {
        Self {
            kind: DraftExpressionKind::Unary {
                operator: UnaryOp::Not,
                value: Box::new(self),
            },
        }
    }
    /// Construct a `equal` predicate.
    #[must_use]
    pub fn equal(self, right: Self) -> Self {
        self.binary(BinaryOp::Equal, right)
    }
    /// Construct a `not_equal` predicate.
    #[must_use]
    pub fn not_equal(self, right: Self) -> Self {
        self.binary(BinaryOp::NotEqual, right)
    }
    /// Construct a `less` predicate.
    #[must_use]
    pub fn less(self, right: Self) -> Self {
        self.binary(BinaryOp::Less, right)
    }
    /// Construct a `less_equal` predicate.
    #[must_use]
    pub fn less_equal(self, right: Self) -> Self {
        self.binary(BinaryOp::LessEqual, right)
    }
    /// Construct a `greater` predicate.
    #[must_use]
    pub fn greater(self, right: Self) -> Self {
        self.binary(BinaryOp::Greater, right)
    }
    /// Construct a `greater_equal` predicate.
    #[must_use]
    pub fn greater_equal(self, right: Self) -> Self {
        self.binary(BinaryOp::GreaterEqual, right)
    }
    /// Construct a `logical_and` predicate.
    #[must_use]
    pub fn logical_and(self, right: Self) -> Self {
        self.binary(BinaryOp::And, right)
    }
    /// Construct a `logical_or` predicate.
    #[must_use]
    pub fn logical_or(self, right: Self) -> Self {
        self.binary(BinaryOp::Or, right)
    }
}

impl DraftExpression {
    pub(super) fn references<'a>(&'a self, output: &mut Vec<DraftExpressionReference<'a>>) {
        match &self.kind {
            DraftExpressionKind::Select {
                condition,
                then_value,
                else_value,
            } => {
                condition.references(output);
                then_value.references(output);
                else_value.references(output);
            }
            DraftExpressionKind::EnumValue(value) => {
                output.push(DraftExpressionReference::EnumValue(value))
            }
            DraftExpressionKind::Boolean(_)
            | DraftExpressionKind::Constant(_)
            | DraftExpressionKind::Complex(_, _) => {}
            DraftExpressionKind::Array(values) => {
                for value in values {
                    value.references(output);
                }
            }
            DraftExpressionKind::Index { value, .. } => value.references(output),
            DraftExpressionKind::Reference(reference)
            | DraftExpressionKind::Derivative(reference) => {
                output.push(DraftExpressionReference::Value(reference));
            }
            DraftExpressionKind::Across(reference) | DraftExpressionKind::Through(reference) => {
                output.push(DraftExpressionReference::Port(reference));
            }
            DraftExpressionKind::Unary { value, .. }
            | DraftExpressionKind::SpatialCall { value, .. } => {
                value.references(output);
            }
            DraftExpressionKind::Binary { left, right, .. } => {
                left.references(output);
                right.references(output);
            }
        }
    }

    pub(super) fn contains_invalid_literal(&self) -> bool {
        match &self.kind {
            DraftExpressionKind::Select {
                condition,
                then_value,
                else_value,
            } => {
                condition.contains_invalid_literal()
                    || then_value.contains_invalid_literal()
                    || else_value.contains_invalid_literal()
            }
            DraftExpressionKind::EnumValue(_)
            | DraftExpressionKind::Boolean(_)
            | DraftExpressionKind::Constant(_) => false,
            DraftExpressionKind::Complex(real, imaginary) => {
                !real.is_finite() || !imaginary.is_finite()
            }
            DraftExpressionKind::Array(values) => {
                values.is_empty() || values.iter().any(Self::contains_invalid_literal)
            }
            DraftExpressionKind::Index { value, .. } => value.contains_invalid_literal(),
            DraftExpressionKind::Reference(_)
            | DraftExpressionKind::Derivative(_)
            | DraftExpressionKind::Across(_)
            | DraftExpressionKind::Through(_) => false,
            DraftExpressionKind::Unary { value, .. }
            | DraftExpressionKind::SpatialCall { value, .. } => value.contains_invalid_literal(),
            DraftExpressionKind::Binary { left, right, .. } => {
                left.contains_invalid_literal() || right.contains_invalid_literal()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum DraftExpressionKind {
    Select {
        condition: Box<DraftExpression>,
        then_value: Box<DraftExpression>,
        else_value: Box<DraftExpression>,
    },
    EnumValue(eqiora_core::ValueLiteral),
    Boolean(bool),
    Constant(crate::DecimalLiteral),
    Complex(f64, f64),
    Array(Vec<DraftExpression>),
    Index {
        value: Box<DraftExpression>,
        index: u32,
    },
    Reference(DraftReference),
    Derivative(DraftReference),
    Across(DraftPortReference),
    Through(DraftPortReference),
    SpatialCall {
        operator: DraftSpatialOperator,
        value: Box<DraftExpression>,
    },
    Unary {
        operator: UnaryOp,
        value: Box<DraftExpression>,
    },
    Binary {
        operator: BinaryOp,
        left: Box<DraftExpression>,
        right: Box<DraftExpression>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) enum DraftExpressionReference<'a> {
    EnumValue(&'a eqiora_core::ValueLiteral),
    Value(&'a DraftReference),
    Port(&'a DraftPortReference),
}
