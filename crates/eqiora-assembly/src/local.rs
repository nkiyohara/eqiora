use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

/// Dense local matrix and right-hand-side contribution in local ordering.
///
/// Local rows and columns are intentionally anonymous. A separate
/// [`crate::AssemblyMap`] supplies equations, free unknowns, and fixed values
/// when the contribution is scattered.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalContribution {
    rows: usize,
    columns: usize,
    matrix: Vec<f64>,
    rhs: Vec<f64>,
}

impl LocalContribution {
    /// Construct a finite row-major local contribution.
    ///
    /// # Errors
    /// Returns `EQ0805` for zero rows, shape overflow/mismatch, or non-finite
    /// matrix/right-hand-side entries.
    pub fn new(
        rows: usize,
        columns: usize,
        matrix: Vec<f64>,
        rhs: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        if rows == 0 {
            return Err(invalid_local(
                "local contribution requires at least one row",
            ));
        }
        let entry_count = rows
            .checked_mul(columns)
            .ok_or_else(|| invalid_local("local contribution matrix dimensions overflow usize"))?;
        if matrix.len() != entry_count || rhs.len() != rows {
            return Err(invalid_local(format!(
                "local contribution shape is {rows}x{columns} with {} matrix and {} rhs entries",
                matrix.len(),
                rhs.len()
            )));
        }
        if matrix.iter().chain(&rhs).any(|value| !value.is_finite()) {
            return Err(invalid_local(
                "local contribution entries must all be finite",
            ));
        }
        Ok(Self {
            rows,
            columns,
            matrix,
            rhs,
        })
    }

    /// Local row count.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Local column count.
    #[must_use]
    pub const fn columns(&self) -> usize {
        self.columns
    }

    /// Row-major local matrix.
    #[must_use]
    pub fn matrix(&self) -> &[f64] {
        &self.matrix
    }

    /// Local right-hand side.
    #[must_use]
    pub fn rhs(&self) -> &[f64] {
        &self.rhs
    }

    /// One local matrix entry.
    #[must_use]
    pub fn entry(&self, row: usize, column: usize) -> Option<f64> {
        (row < self.rows && column < self.columns).then(|| self.matrix[row * self.columns + column])
    }
}

fn invalid_local(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_LOCAL_CONTRIBUTION, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_contribution_checks_dense_shape_and_values() {
        assert_eq!(
            LocalContribution::new(2, 2, vec![1.0; 3], vec![0.0; 2])
                .unwrap_err()
                .code(),
            codes::INVALID_LOCAL_CONTRIBUTION
        );
        assert_eq!(
            LocalContribution::new(1, 1, vec![f64::NAN], vec![0.0])
                .unwrap_err()
                .code(),
            codes::INVALID_LOCAL_CONTRIBUTION
        );
    }
}
