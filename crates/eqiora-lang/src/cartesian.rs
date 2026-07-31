use crate::ast::TextRange;

/// Source spelling of a Cartesian boundary side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BoundarySideSyntax {
    /// Lower coordinate side.
    Lower,
    /// Upper coordinate side.
    Upper,
}

/// Closed source spelling for one Cartesian coordinate endpoint.
#[derive(Debug, Clone, PartialEq)]
pub enum CartesianCoordinateSyntax {
    /// One fixed coherent-SI length literal.
    Fixed {
        /// Parsed finite numeric value.
        value: f64,
        /// Exact source range of the signed literal.
        range: TextRange,
    },
    /// One unqualified root Model Parameter name.
    Parameter {
        /// Source name resolved by the compiler.
        name: String,
        /// Exact source range of the name.
        range: TextRange,
    },
}

impl CartesianCoordinateSyntax {
    pub(crate) const fn fixed(value: f64, range: TextRange) -> Self {
        Self::Fixed { value, range }
    }

    /// Fixed value, if this endpoint is a literal.
    #[must_use]
    pub const fn fixed_value(&self) -> Option<f64> {
        match self {
            Self::Fixed { value, .. } => Some(*value),
            Self::Parameter { .. } => None,
        }
    }

    /// Parameter name, if this endpoint is a reference.
    #[must_use]
    pub fn parameter_name(&self) -> Option<&str> {
        match self {
            Self::Parameter { name, .. } => Some(name),
            Self::Fixed { .. } => None,
        }
    }

    /// Exact endpoint source range.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        match self {
            Self::Fixed { range, .. } | Self::Parameter { range, .. } => *range,
        }
    }
}
