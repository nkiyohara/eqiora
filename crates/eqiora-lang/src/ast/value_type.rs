use eqiora_core::ScalarDomain;

use super::{Expr, TextRange};

/// Source-level mathematical type, before dimensions and support are resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueTypeSyntax {
    pub(crate) kind: ValueTypeSyntaxKind,
    pub(crate) range: TextRange,
}

/// Closed mathematical type constructors. Arrays retain their element type.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueTypeSyntaxKind {
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
        matches!(self.kind, ValueTypeSyntaxKind::Scalar { .. })
    }

    /// Physical dimension of each scalar component.
    #[must_use]
    pub fn dimension(&self) -> &Expr {
        match &self.kind {
            ValueTypeSyntaxKind::Scalar { dimension, .. } => dimension,
            ValueTypeSyntaxKind::Vector { scalar, .. }
            | ValueTypeSyntaxKind::Tensor { scalar, .. } => scalar.dimension(),
            ValueTypeSyntaxKind::Array { element, .. } => element.dimension(),
        }
    }

    /// Mathematical domain of each scalar component.
    #[must_use]
    pub fn scalar_domain(&self) -> ScalarDomain {
        match &self.kind {
            ValueTypeSyntaxKind::Scalar { domain, .. } => *domain,
            ValueTypeSyntaxKind::Vector { scalar, .. }
            | ValueTypeSyntaxKind::Tensor { scalar, .. } => scalar.scalar_domain(),
            ValueTypeSyntaxKind::Array { element, .. } => element.scalar_domain(),
        }
    }

    pub(crate) fn real(dimension: Expr) -> Self {
        Self {
            range: dimension.range(),
            kind: ValueTypeSyntaxKind::Scalar {
                domain: ScalarDomain::Real,
                dimension,
            },
        }
    }

    pub(crate) fn dimension_mut(&mut self) -> &mut Expr {
        match &mut self.kind {
            ValueTypeSyntaxKind::Scalar { dimension, .. } => dimension,
            ValueTypeSyntaxKind::Vector { scalar, .. }
            | ValueTypeSyntaxKind::Tensor { scalar, .. } => scalar.dimension_mut(),
            ValueTypeSyntaxKind::Array { element, .. } => element.dimension_mut(),
        }
    }
}
