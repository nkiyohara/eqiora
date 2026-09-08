/// Mathematical scalar domain, independent of numerical storage precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarDomain {
    /// Logical truth values, with no numeric embedding.
    Boolean,
    /// Closed nominal alternatives, without numeric embedding.
    Enum,
    /// Exact signed 64-bit integers, with no implicit real embedding.
    Integer,
    /// Real-valued mathematics.
    Real,
    /// Complex-valued mathematics.
    Complex,
}

impl ScalarDomain {
    /// Smallest scalar domain into which both operands embed without loss.
    #[must_use]
    pub const fn common(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Enum, Self::Enum) => Some(Self::Enum),
            (Self::Enum, _) | (_, Self::Enum) => None,
            (Self::Boolean, Self::Boolean) => Some(Self::Boolean),
            (Self::Boolean, _) | (_, Self::Boolean) => None,
            (Self::Integer, Self::Integer) => Some(Self::Integer),
            (Self::Integer, _) | (_, Self::Integer) => None,
            (Self::Real, Self::Real) => Some(Self::Real),
            _ => Some(Self::Complex),
        }
    }
}

/// Scalar storage representation shared by lowering, realization, and
/// execution contracts.
///
/// This is physical representation, not a mathematical assertion about a
/// field. Keeping it in the L0 vocabulary lets host, distributed, and device
/// contracts negotiate the same type without depending on one another.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarType {
    /// IEEE-754 binary32.
    F32,
    /// IEEE-754 binary64.
    F64,
}

impl ScalarType {
    /// Size of one stored scalar in bytes.
    #[must_use]
    pub const fn byte_width(self) -> usize {
        match self {
            Self::F32 => size_of::<f32>(),
            Self::F64 => size_of::<f64>(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_width_is_part_of_shared_storage_vocabulary() {
        assert_eq!(ScalarType::F32.byte_width(), 4);
        assert_eq!(ScalarType::F64.byte_width(), 8);
    }
}
