use super::*;

/// Source expression with its exact byte range.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub(crate) resolved_nominal: Option<Box<eqiora_core::ValueType>>,
    pub(crate) kind: ExprKind,
    pub(crate) range: TextRange,
}

impl Expr {
    /// Checked nominal constructor type supplied by lexical declaration resolution.
    #[must_use]
    pub fn resolved_nominal(&self) -> Option<&eqiora_core::ValueType> {
        self.resolved_nominal.as_deref()
    }

    /// Expression form.
    #[must_use]
    pub const fn kind(&self) -> &ExprKind {
        &self.kind
    }

    /// Full expression range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Rewrite structurally named references without reparsing source text.
    ///
    /// The callback visits bare [`ExprKind::Name`] and qualified
    /// [`ExprKind::Path`] occurrences as one [`NamePath`] abstraction. Returning
    /// `None` retains the occurrence; returning a path replaces it. Expression
    /// topology and every expression/source range are preserved. Call callees
    /// are structural [`NamePath`] values and are visited before their ordered
    /// arguments.
    /// Reduction set names are visited, while occurrences rooted in the local
    /// binder are protected from outer rewrites. Unit-catalog names inside
    /// quantity literals are not value-name occurrences.
    #[must_use]
    pub fn rewrite_name_paths(
        &self,
        mut rewrite: impl FnMut(&NamePath) -> Option<NamePath>,
    ) -> Self {
        self.rewrite_name_paths_with(&mut rewrite)
    }

    fn rewrite_name_paths_with(
        &self,
        rewrite: &mut dyn FnMut(&NamePath) -> Option<NamePath>,
    ) -> Self {
        let kind = match &self.kind {
            ExprKind::Member { value, member } => ExprKind::Member {
                value: Box::new(value.rewrite_name_paths_with(rewrite)),
                member: member.clone(),
            },
            ExprKind::Boolean(value) => ExprKind::Boolean(*value),
            ExprKind::Number(value) => ExprKind::Number(value.clone()),
            ExprKind::Quantity { value, unit } => ExprKind::Quantity {
                value: value.clone(),
                unit: unit.clone(),
            },
            ExprKind::Name(name) => {
                let path = NamePath::single(name.clone(), self.range);
                rewrite(&path).map_or_else(
                    || ExprKind::Name(name.clone()),
                    |replacement| expression_name(replacement.with_range(self.range)),
                )
            }
            ExprKind::Path(path) => rewrite(path).map_or_else(
                || ExprKind::Path(path.clone()),
                |replacement| expression_name(replacement.with_range(self.range)),
            ),
            ExprKind::BoundaryPortSelection { port, selector } => ExprKind::BoundaryPortSelection {
                port: Box::new(rewrite(port).unwrap_or_else(|| port.as_ref().clone())),
                selector: selector.clone(),
            },
            ExprKind::Unary { op, value } => ExprKind::Unary {
                op: *op,
                value: Box::new(value.rewrite_name_paths_with(rewrite)),
            },
            ExprKind::Binary { op, left, right } => ExprKind::Binary {
                op: *op,
                left: Box::new(left.rewrite_name_paths_with(rewrite)),
                right: Box::new(right.rewrite_name_paths_with(rewrite)),
            },
            ExprKind::Array(elements) => ExprKind::Array(
                elements
                    .iter()
                    .map(|element| element.rewrite_name_paths_with(rewrite))
                    .collect(),
            ),
            ExprKind::Index { value, index } => ExprKind::Index {
                value: Box::new(value.rewrite_name_paths_with(rewrite)),
                index: Box::new(index.rewrite_name_paths_with(rewrite)),
            },
            ExprKind::Reduction {
                operation,
                binder,
                value,
            } => {
                let set = rewrite(&binder.set)
                    .unwrap_or_else(|| binder.set.clone())
                    .with_range(binder.set.range());
                let mut scoped = |path: &NamePath| {
                    if path.segments().next() == Some(binder.member.as_str()) {
                        None
                    } else {
                        rewrite(path)
                    }
                };
                ExprKind::Reduction {
                    operation: *operation,
                    binder: FamilyBinderSyntax {
                        member: binder.member.clone(),
                        set,
                        range: binder.range,
                    },
                    value: Box::new(value.rewrite_name_paths_with(&mut scoped)),
                }
            }
            ExprKind::Call { callee, arguments } => ExprKind::Call {
                callee: rewrite(callee).map_or_else(
                    || callee.clone(),
                    |replacement| replacement.with_range(callee.range()),
                ),
                arguments: match arguments {
                    CallArguments::Positional(values) => CallArguments::Positional(
                        values
                            .iter()
                            .map(|value| value.rewrite_name_paths_with(rewrite))
                            .collect(),
                    ),
                    CallArguments::Named(bindings) => CallArguments::Named(
                        bindings
                            .iter()
                            .map(|binding| NamedBindingDecl {
                                value: binding.value.rewrite_name_paths_with(rewrite),
                                ..binding.clone()
                            })
                            .collect(),
                    ),
                },
            },
        };
        Self {
            resolved_nominal: self.resolved_nominal.clone(),
            kind,
            range: self.range,
        }
    }
}

fn expression_name(path: NamePath) -> ExprKind {
    if path.is_qualified() {
        ExprKind::Path(path)
    } else {
        ExprKind::Name(path.as_str().to_owned())
    }
}

/// Recursive parser AST. Canonical residual storage is the lowered DAG.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ExprKind {
    /// Boolean truth value, distinct from numeric literals.
    Boolean(bool),
    /// Exact decimal literal, interpreted in its required value-domain context.
    Number(crate::DecimalLiteral),
    /// Numeric literal with an explicit input-unit expression.
    Quantity {
        /// Exact decimal value before unit conversion.
        value: crate::DecimalLiteral,
        /// Unit-catalog expression, independent of value names and dimension aliases.
        unit: Box<Expr>,
    },
    /// Nonempty ordered channel-array elements; rectangularity is checked during lowering.
    Array(Vec<Expr>),
    /// Select one channel from a value; index legality is checked during lowering.
    Index {
        /// Value being indexed.
        value: Box<Expr>,
        /// Authored index expression.
        index: Box<Expr>,
    },
    /// Member of a statically selected indexed component occurrence.
    Member { value: Box<Expr>, member: String },
    /// Source identifier.
    Name(String),
    /// Qualified lexical or instance-member name.
    Path(NamePath),
    /// One boundary-family Port selected by an exact boundary spelling.
    BoundaryPortSelection {
        /// Port family path.
        port: Box<NamePath>,
        /// Closed boundary selector.
        selector: Box<BoundaryPortSelectorSyntax>,
    },
    /// Prefix operator.
    Unary {
        /// Operator.
        op: UnaryOp,
        /// Operand.
        value: Box<Expr>,
    },
    /// Infix operator.
    Binary {
        /// Operator.
        op: BinaryOp,
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
    },
    /// Ordered reduction over one nonempty bounded nominal index set.
    Reduction {
        /// Scalar reduction operation, evaluated in index-set order.
        operation: ReductionOp,
        /// Lexical member binding and exact set name.
        binder: FamilyBinderSyntax,
        /// Scalar expression evaluated for each member.
        value: Box<Expr>,
    },
    /// Qualified named operator with one or more ordered arguments.
    Call {
        /// Structurally qualified operator name.
        callee: NamePath,
        /// Nonempty ordered arguments.
        arguments: CallArguments,
    },
}

/// Scalar operation for an ordered finite reduction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReductionOp {
    /// Ordered addition.
    Sum,
    /// Ordered multiplication.
    Product,
    /// Ordered minimum, retaining the first equal operand.
    Min,
    /// Ordered maximum, retaining the first equal operand.
    Max,
}

/// Prefix expression operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// Arithmetic negation.
    Neg,
    /// Boolean negation.
    Not,
}

/// Infix expression operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    /// Addition.
    Add,
    /// Subtraction.
    Sub,
    /// Multiplication.
    Mul,
    /// Division.
    Div,
    /// Integer power (validated during lowering).
    Pow,
    /// Equality comparison.
    Equal,
    /// Inequality comparison.
    NotEqual,
    /// Strict order comparison.
    Less,
    /// Inclusive order comparison.
    LessEqual,
    /// Strict reverse order comparison.
    Greater,
    /// Inclusive reverse order comparison.
    GreaterEqual,
    /// Short-circuit Boolean conjunction.
    And,
    /// Short-circuit Boolean disjunction.
    Or,
}

/// One homogeneous, authored-order argument list for a shared expression call.
#[derive(Debug, Clone, PartialEq)]
pub enum CallArguments {
    /// Ordered arguments of a positional primitive.
    Positional(Vec<Expr>),
    /// Explicit bindings to the target signature, retaining authored order.
    Named(Vec<NamedBindingDecl>),
}

impl CallArguments {
    /// Expression values in authored argument order, without erasing binding names.
    pub fn expressions(&self) -> impl ExactSizeIterator<Item = &Expr> {
        let len = match self {
            Self::Positional(values) => values.len(),
            Self::Named(bindings) => bindings.len(),
        };
        (0..len).map(move |index| match self {
            Self::Positional(values) => &values[index],
            Self::Named(bindings) => bindings[index].value(),
        })
    }
    /// Positional values, only when this call uses positional syntax.
    #[must_use]
    pub fn positional(&self) -> Option<&[Expr]> {
        match self {
            Self::Positional(values) => Some(values),
            Self::Named(_) => None,
        }
    }
    /// Named bindings, only when this call uses named syntax.
    #[must_use]
    pub fn named(&self) -> Option<&[NamedBindingDecl]> {
        match self {
            Self::Named(bindings) => Some(bindings),
            Self::Positional(_) => None,
        }
    }
}
