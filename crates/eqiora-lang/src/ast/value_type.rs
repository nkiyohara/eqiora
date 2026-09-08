use eqiora_core::ScalarDomain;

use super::{Expr, TextRange};

/// Source-level mathematical type, before dimensions and support are resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueTypeSyntax {
    pub(crate) kind: Box<ValueTypeSyntaxKind>,
    pub(crate) range: TextRange,
    pub(crate) resolved_nominal: Option<Box<eqiora_core::ValueType>>,
}

/// Closed mathematical type constructors. Arrays retain their element type.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueTypeSyntaxKind {
    /// Unresolved type or dimension name, classified only by lexical resolution.
    Named(super::NamePath),
    /// Integer coordinates in an exact declared atomic finite space.
    Coordinates(super::NamePath),
    /// Nonnegative exact counts indexed by an exact atomic finite space.
    Counts(super::NamePath),
    /// One bounded ordinal carrying its exact index-set identity.
    Index(super::NamePath),
    /// A real or complex scalar with a structural dimension expression.
    Scalar {
        /// Mathematical domain, unrelated to numerical precision.
        domain: ScalarDomain,
        /// Physical dimension expression.
        dimension: Expr,
    },
    /// Spatial vector; its scalar element cannot itself be an array or tensor.
    Vector {
        /// Scalar component type.
        scalar: Box<ValueTypeSyntax>,
        /// Exact positive spatial extent.
        extent: u32,
    },
    /// Spatial tensor with ordered axes and scalar components.
    Tensor {
        /// Scalar component type.
        scalar: Box<ValueTypeSyntax>,
        /// Exact ordered spatial extents.
        extents: Vec<u32>,
    },
    /// Indexed channels, not a spatial frame or finite component basis.
    Array {
        /// Complete checked element type.
        element: Box<ValueTypeSyntax>,
        /// Exact positive channel count.
        extent: u32,
    },
}

impl ValueTypeSyntax {
    /// Exact source type constructor.
    #[must_use]
    pub const fn kind(&self) -> &ValueTypeSyntaxKind {
        &self.kind
    }

    /// Complete source type range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// Whether the type is a mathematical scalar.
    #[must_use]
    pub const fn is_scalar(&self) -> bool {
        matches!(
            *self.kind,
            ValueTypeSyntaxKind::Scalar { .. } | ValueTypeSyntaxKind::Index(_)
        )
    }

    /// Physical dimension of each scalar component.
    #[must_use]
    pub fn dimension(&self) -> Option<&Expr> {
        match self.kind.as_ref() {
            ValueTypeSyntaxKind::Named(_) => None,
            ValueTypeSyntaxKind::Coordinates(_)
            | ValueTypeSyntaxKind::Counts(_)
            | ValueTypeSyntaxKind::Index(_) => Some(dimensionless_syntax()),
            ValueTypeSyntaxKind::Scalar { dimension, .. } => Some(dimension),
            ValueTypeSyntaxKind::Vector { scalar, .. }
            | ValueTypeSyntaxKind::Tensor { scalar, .. } => scalar.dimension(),
            ValueTypeSyntaxKind::Array { element, .. } => element.dimension(),
        }
    }

    /// Mathematical domain of each scalar component.
    #[must_use]
    pub fn scalar_domain(&self) -> Option<ScalarDomain> {
        match self.kind.as_ref() {
            ValueTypeSyntaxKind::Named(_) => None,
            ValueTypeSyntaxKind::Coordinates(_)
            | ValueTypeSyntaxKind::Counts(_)
            | ValueTypeSyntaxKind::Index(_) => Some(ScalarDomain::Integer),
            ValueTypeSyntaxKind::Scalar { domain, .. } => Some(*domain),
            ValueTypeSyntaxKind::Vector { scalar, .. }
            | ValueTypeSyntaxKind::Tensor { scalar, .. } => scalar.scalar_domain(),
            ValueTypeSyntaxKind::Array { element, .. } => element.scalar_domain(),
        }
    }

    pub(crate) fn real(dimension: Expr) -> Self {
        Self {
            resolved_nominal: None,
            range: dimension.range(),
            kind: Box::new(ValueTypeSyntaxKind::Scalar {
                domain: ScalarDomain::Real,
                dimension,
            }),
        }
    }

    /// Checked nominal binding attached by the lexical source-type resolution pass.
    #[must_use]
    pub fn resolved_nominal(&self) -> Option<&eqiora_core::ValueType> {
        self.resolved_nominal.as_deref()
    }

    pub(crate) fn rewrite_dimension(&mut self, rewrite: &mut impl FnMut(&Expr) -> Expr) {
        match self.kind.as_mut() {
            ValueTypeSyntaxKind::Scalar { dimension, .. } => *dimension = rewrite(dimension),
            ValueTypeSyntaxKind::Vector { scalar, .. }
            | ValueTypeSyntaxKind::Tensor { scalar, .. } => scalar.rewrite_dimension(rewrite),
            ValueTypeSyntaxKind::Array { element, .. } => element.rewrite_dimension(rewrite),
            ValueTypeSyntaxKind::Named(_)
            | ValueTypeSyntaxKind::Coordinates(_)
            | ValueTypeSyntaxKind::Counts(_)
            | ValueTypeSyntaxKind::Index(_) => {}
        }
    }
}

fn dimensionless_syntax() -> &'static Expr {
    static ONE: std::sync::LazyLock<Expr> = std::sync::LazyLock::new(|| Expr {
        resolved_enum: None,
        resolved_nominal: None,
        kind: super::ExprKind::Number(crate::DecimalLiteral::parse("1").expect("one")),
        range: TextRange::new(0, 0),
    });
    &ONE
}
