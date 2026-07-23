use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};

/// One shape-homogeneous batch of anonymous entity-local linear maps.
///
/// Coefficients are stored in entity-major, row-major order. Inputs and
/// outputs use the corresponding packed entity-major order. The IR owns no
/// mesh identity, global numbering, gather/scatter rule, or backend type.
/// Heterogeneous discretizations lower to an ordered collection of batches
/// instead of weakening this contract with per-entity dynamic shape.
#[derive(Debug, Clone, PartialEq)]
pub struct LocalLinearActionIr {
    entity_count: usize,
    rows: usize,
    columns: usize,
    coefficients: Vec<f64>,
}

impl LocalLinearActionIr {
    /// Construct a finite, non-empty uniform batch.
    ///
    /// # Errors
    /// Returns `EQ0701` when either local dimension is zero, the coefficient
    /// count is not a positive multiple of the local matrix size, arithmetic
    /// overflows, or a coefficient is non-finite.
    pub fn new(rows: usize, columns: usize, coefficients: Vec<f64>) -> Result<Self, Diagnostic> {
        if rows == 0 || columns == 0 {
            return Err(invalid_ir(
                "local linear action requires positive row and column dimensions",
            ));
        }
        let entries_per_entity = rows
            .checked_mul(columns)
            .ok_or_else(|| invalid_ir("local linear-action dimensions overflow usize"))?;
        if coefficients.is_empty() || !coefficients.len().is_multiple_of(entries_per_entity) {
            return Err(invalid_ir(format!(
                "local linear-action coefficient count {} is not a positive multiple of its {rows}x{columns} shape",
                coefficients.len()
            )));
        }
        if coefficients.iter().any(|value| !value.is_finite()) {
            return Err(invalid_ir(
                "local linear-action coefficients must all be finite",
            ));
        }
        let entity_count = coefficients.len() / entries_per_entity;
        Ok(Self {
            entity_count,
            rows,
            columns,
            coefficients,
        })
    }

    /// Number of local maps in the batch.
    #[must_use]
    pub const fn entity_count(&self) -> usize {
        self.entity_count
    }

    /// Output width of every local map.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Input width of every local map.
    #[must_use]
    pub const fn columns(&self) -> usize {
        self.columns
    }

    /// Entity-major, row-major coefficients.
    #[must_use]
    pub fn coefficients(&self) -> &[f64] {
        &self.coefficients
    }

    /// Required packed input length.
    ///
    /// Construction proves this product is representable.
    #[must_use]
    pub const fn input_len(&self) -> usize {
        self.entity_count * self.columns
    }

    /// Required packed output length.
    ///
    /// Construction proves this product is representable.
    #[must_use]
    pub const fn output_len(&self) -> usize {
        self.entity_count * self.rows
    }

    /// Evaluate with the auditable reference expression order.
    ///
    /// Each output starts from positive zero and accumulates columns in
    /// ascending local order using separate multiplication and addition. This
    /// defines the reproducible oracle; accelerated fast policies may use a
    /// different expression tree and must be compared under a stated
    /// tolerance.
    ///
    /// # Errors
    /// Returns `EQ0702` for incompatible buffer lengths and `EQ0505` for a
    /// non-finite input or result.
    pub fn apply_reference(&self, input: &[f64], output: &mut [f64]) -> Result<(), Diagnostic> {
        if input.len() != self.input_len() || output.len() != self.output_len() {
            return Err(Diagnostic::error(
                codes::OPERATOR_INPUT_MISMATCH,
                format!(
                    "local linear action expects input/output lengths {}/{}, received {}/{}",
                    self.input_len(),
                    self.output_len(),
                    input.len(),
                    output.len()
                ),
            )
            .with_graph_path(GraphPath::new(["local-action", "buffers"])));
        }
        if input.iter().any(|value| !value.is_finite()) {
            return Err(nonfinite("local linear-action input must be finite"));
        }

        for entity in 0..self.entity_count {
            let matrix_offset = entity * self.rows * self.columns;
            let input_offset = entity * self.columns;
            let output_offset = entity * self.rows;
            for row in 0..self.rows {
                let row_offset = matrix_offset + row * self.columns;
                let mut value = 0.0;
                for column in 0..self.columns {
                    value += self.coefficients[row_offset + column] * input[input_offset + column];
                }
                if !value.is_finite() {
                    return Err(nonfinite(format!(
                        "local linear-action output became non-finite at entity {entity}, row {row}"
                    )));
                }
                output[output_offset + row] = value;
            }
        }
        Ok(())
    }
}

fn invalid_ir(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_OPERATOR_IR, message)
        .with_graph_path(GraphPath::new(["local-action"]))
}

fn nonfinite(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NONFINITE_EVALUATION, message)
        .with_graph_path(GraphPath::new(["local-action", "evaluation"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_batch_applies_in_entity_and_column_order() {
        let action =
            LocalLinearActionIr::new(2, 2, vec![2.0, -1.0, -1.0, 2.0, 3.0, 1.0, -2.0, 4.0])
                .unwrap();
        let mut output = vec![0.0; action.output_len()];
        action
            .apply_reference(&[4.0, 5.0, -2.0, 3.0], &mut output)
            .unwrap();

        assert_eq!(action.entity_count(), 2);
        assert_eq!(output, vec![3.0, 6.0, -3.0, 16.0]);
    }

    #[test]
    fn uniform_batch_rejects_ambiguous_shape_and_buffers() {
        assert_eq!(
            LocalLinearActionIr::new(2, 2, vec![1.0; 3])
                .unwrap_err()
                .code(),
            codes::INVALID_OPERATOR_IR
        );
        let action = LocalLinearActionIr::new(1, 2, vec![1.0, 2.0]).unwrap();
        assert_eq!(
            action
                .apply_reference(&[1.0], &mut [0.0])
                .unwrap_err()
                .code(),
            codes::OPERATOR_INPUT_MISMATCH
        );
    }
}
