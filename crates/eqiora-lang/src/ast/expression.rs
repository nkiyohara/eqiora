use super::*;

/// Source expression with its exact byte range.
#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub(crate) resolved_nominal: Option<eqiora_core::ValueType>,
    pub(crate) kind: ExprKind,
    pub(crate) range: TextRange,
}

impl Expr {
    /// Checked nominal constructor type supplied by lexical declaration resolution.
    #[must_use]
    pub fn resolved_nominal(&self) -> Option<&eqiora_core::ValueType> {
        self.resolved_nominal.as_ref()
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
    /// Unit-catalog names inside quantity literals are not value-name occurrences.
    #[must_use]
    pub fn rewrite_name_paths(
        &self,
        mut rewrite: impl FnMut(&NamePath) -> Option<NamePath>,
    ) -> Self {
        self.rewrite_name_paths_with(&mut rewrite)
    }

    fn rewrite_name_paths_with(
        &self,
        rewrite: &mut impl FnMut(&NamePath) -> Option<NamePath>,
    ) -> Self {
        let kind = match &self.kind {
            ExprKind::Member { value, member } => ExprKind::Member {
                value: Box::new(value.rewrite_name_paths_with(rewrite)),
                member: member.clone(),
            },
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
            ExprKind::Call { callee, arguments } => ExprKind::Call {
                callee: rewrite(callee).map_or_else(
                    || callee.clone(),
                    |replacement| replacement.with_range(callee.range()),
                ),
                arguments: arguments
                    .iter()
                    .map(|argument| argument.rewrite_name_paths_with(rewrite))
                    .collect(),
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
    /// Qualified named operator with one or more ordered arguments.
    Call {
        /// Structurally qualified operator name.
        callee: NamePath,
        /// Nonempty ordered arguments.
        arguments: Vec<Expr>,
    },
}

/// Prefix expression operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// Arithmetic negation.
    Neg,
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
}
