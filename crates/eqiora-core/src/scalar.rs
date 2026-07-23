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
