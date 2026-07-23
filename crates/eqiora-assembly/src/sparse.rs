use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_solver::{
    CompleteCsrStorage, DiagonalAvailability, LinearOperator, RowLinearAction,
    TransposeLinearOperator,
};

use crate::{AssemblyDelta, LocalContribution};

/// Dense global degree-of-freedom index in one assembled system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DofId(usize);

impl DofId {
    /// Construct a zero-based global degree-of-freedom index.
    #[must_use]
    pub const fn new(index: usize) -> Self {
        Self(index)
    }

    /// Zero-based index.
    #[must_use]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// One local trial slot after essential constraints are resolved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LocalUnknown {
    /// Free global unknown.
    Free(DofId),
    /// Fixed affine value eliminated during assembly.
    Fixed(f64),
}

/// Mapping from anonymous local rows/columns to global algebra.
///
/// A missing equation row is skipped. This represents an essential boundary
/// equation without fabricating a global row. Fixed column values are moved
/// to the global right-hand side by the assembler.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyMap {
    equations: Vec<Option<DofId>>,
    unknowns: Vec<LocalUnknown>,
}

impl AssemblyMap {
    /// Construct a local-to-global map.
    ///
    /// # Errors
    /// Returns `EQ0806` if a fixed value is non-finite. Shape compatibility
    /// with a contribution is checked at scatter time.
    pub fn new(
        equations: Vec<Option<DofId>>,
        unknowns: Vec<LocalUnknown>,
    ) -> Result<Self, Diagnostic> {
        if unknowns
            .iter()
            .any(|unknown| matches!(unknown, LocalUnknown::Fixed(value) if !value.is_finite()))
        {
            return Err(assembly_failed("fixed assembly values must be finite"));
        }
        Ok(Self {
            equations,
            unknowns,
        })
    }

    /// Global equation associated with each local row.
    #[must_use]
    pub fn equations(&self) -> &[Option<DofId>] {
        &self.equations
    }

    /// Free or fixed global unknown associated with each local column.
    #[must_use]
    pub fn unknowns(&self) -> &[LocalUnknown] {
        &self.unknowns
    }
}

/// Deterministic coordinate accumulator for local contributions.
///
/// This v0 implementation prioritizes a small, inspectable contract. A
/// distributed or device assembler may consume the same `AssemblyMap` and
/// `LocalContribution` without changing local operators.
#[derive(Debug, Clone)]
pub struct CooAssembler {
    size: usize,
    entries: BTreeMap<(usize, usize), f64>,
    rhs: Vec<f64>,
}

impl CooAssembler {
    /// Construct a square global system.
    ///
    /// # Errors
    /// Returns `EQ0806` when the system has zero free equations.
    pub fn new(size: usize) -> Result<Self, Diagnostic> {
        if size == 0 {
            return Err(assembly_failed(
                "assembled system requires at least one free equation",
            ));
        }
        Ok(Self {
            size,
            entries: BTreeMap::new(),
            rhs: vec![0.0; size],
        })
    }

    /// Add one local contribution using its independent assembly map.
    ///
    /// # Errors
    /// Returns `EQ0806` for shape mismatch, an out-of-range global index, or
    /// a non-finite accumulated value.
    pub fn scatter(
        &mut self,
        map: &AssemblyMap,
        local: &LocalContribution,
    ) -> Result<(), Diagnostic> {
        let delta = AssemblyDelta::from_local(self.size, map, local)?;
        self.scatter_delta(&delta)
    }

    /// Add one already-mapped packet-local delta.
    ///
    /// This is the common accumulation boundary for direct and owner-routed
    /// assembly. The complete delta is checked against current state before
    /// any entry is changed.
    ///
    /// # Errors
    /// Returns `EQ0806` for a target-size mismatch or non-finite accumulated
    /// value. Failure leaves the assembler unchanged.
    pub fn scatter_delta(&mut self, delta: &AssemblyDelta) -> Result<(), Diagnostic> {
        if delta.target_size() != self.size {
            return Err(assembly_failed(format!(
                "assembly delta targets size {} but assembler size is {}",
                delta.target_size(),
                self.size
            )));
        }
        let finite_entries = delta.rows().iter().all(|row| {
            row.entries().iter().all(|(column, value)| {
                (self
                    .entries
                    .get(&(row.row().index(), column.index()))
                    .copied()
                    .unwrap_or(0.0)
                    + value)
                    .is_finite()
            })
        });
        let finite_rhs = delta
            .rows()
            .iter()
            .all(|row| (self.rhs[row.row().index()] + row.rhs()).is_finite());
        if !finite_entries || !finite_rhs {
            return Err(assembly_failed(
                "sparse assembly produced a non-finite accumulated value",
            ));
        }
        for row in delta.rows() {
            for &(column, value) in row.entries() {
                *self
                    .entries
                    .entry((row.row().index(), column.index()))
                    .or_insert(0.0) += value;
            }
            self.rhs[row.row().index()] += row.rhs();
        }
        Ok(())
    }

    /// Finalize deterministic compressed sparse row storage.
    ///
    /// # Errors
    /// Returns `EQ0806` if a global row has no nonzero entry.
    pub fn finish(self) -> Result<LinearSystem, Diagnostic> {
        let entries = self
            .entries
            .into_iter()
            .filter(|(_, value)| *value != 0.0)
            .collect::<Vec<_>>();
        let mut row_offsets = vec![0; self.size + 1];
        for ((row, _), _) in &entries {
            row_offsets[row + 1] += 1;
        }
        for row in 0..self.size {
            row_offsets[row + 1] += row_offsets[row];
            if row_offsets[row + 1] == row_offsets[row] {
                return Err(assembly_failed(format!(
                    "assembled global row {row} has no nonzero entries"
                )));
            }
        }
        let mut column_indices = Vec::with_capacity(entries.len());
        let mut values = Vec::with_capacity(entries.len());
        for ((_, column), value) in entries {
            column_indices.push(column);
            values.push(value);
        }
        LinearSystem::new(
            CsrMatrix::from_sorted_csr(self.size, self.size, row_offsets, column_indices, values)?,
            self.rhs,
        )
    }
}

/// Immutable compressed sparse row matrix.
#[derive(Debug, Clone, PartialEq)]
pub struct CsrMatrix {
    rows: usize,
    columns: usize,
    row_offsets: Vec<usize>,
    column_indices: Vec<usize>,
    values: Vec<f64>,
}

impl CsrMatrix {
    /// Construct validated CSR storage with strictly ordered columns per row.
    ///
    /// This is the single admission boundary for finalized sparse storage.
    /// Callers retain no mutable access to the admitted arrays, so row lookup
    /// and backend adapters may rely on the checked ordering thereafter.
    ///
    /// # Errors
    /// Returns `EQ0806` for zero or inconsistent shape, invalid row offsets,
    /// out-of-range or non-increasing columns, or a non-finite value.
    pub fn from_sorted_csr(
        rows: usize,
        columns: usize,
        row_offsets: Vec<usize>,
        column_indices: Vec<usize>,
        values: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        if rows == 0 || columns == 0 {
            return Err(assembly_failed(
                "CSR storage requires positive row and column counts",
            ));
        }
        let expected_offsets = rows
            .checked_add(1)
            .ok_or_else(|| assembly_failed("CSR row count overflows its offset table"))?;
        if row_offsets.len() != expected_offsets {
            return Err(assembly_failed(format!(
                "CSR with {rows} rows requires {expected_offsets} row offsets, found {}",
                row_offsets.len()
            )));
        }
        if column_indices.len() != values.len() {
            return Err(assembly_failed(format!(
                "CSR column/value lengths differ: {} versus {}",
                column_indices.len(),
                values.len()
            )));
        }
        if row_offsets.first() != Some(&0) || row_offsets.last() != Some(&values.len()) {
            return Err(assembly_failed(
                "CSR row offsets must start at zero and end at the nonzero count",
            ));
        }
        if values.iter().any(|value| !value.is_finite()) {
            return Err(assembly_failed("CSR values must all be finite"));
        }

        for row in 0..rows {
            let start = row_offsets[row];
            let end = row_offsets[row + 1];
            if start > end || end > column_indices.len() {
                return Err(assembly_failed(format!(
                    "CSR row {row} has invalid offset range {start}..{end}"
                )));
            }
            let columns_in_row = &column_indices[start..end];
            if let Some(column) = columns_in_row.iter().find(|column| **column >= columns) {
                return Err(assembly_failed(format!(
                    "CSR row {row} contains column {column} outside 0..{columns}"
                )));
            }
            if columns_in_row.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(assembly_failed(format!(
                    "CSR row {row} columns must be strictly increasing"
                )));
            }
        }

        Ok(Self {
            rows,
            columns,
            row_offsets,
            column_indices,
            values,
        })
    }

    /// Row count.
    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    /// Column count.
    #[must_use]
    pub const fn columns(&self) -> usize {
        self.columns
    }

    /// CSR row offsets.
    #[must_use]
    pub fn row_offsets(&self) -> &[usize] {
        &self.row_offsets
    }

    /// CSR column indices.
    #[must_use]
    pub fn column_indices(&self) -> &[usize] {
        &self.column_indices
    }

    /// CSR nonzero values.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Multiply by a dense vector.
    ///
    /// # Errors
    /// Returns `EQ0802` for the wrong input size or a non-finite result.
    pub fn multiply(&self, input: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        let mut output = vec![0.0; self.rows];
        self.multiply_into(input, &mut output)?;
        Ok(output)
    }

    /// Multiply into caller-owned storage without allocating.
    ///
    /// # Errors
    /// Returns `EQ0802` for a shape mismatch or non-finite result.
    pub fn multiply_into(&self, input: &[f64], output: &mut [f64]) -> Result<(), Diagnostic> {
        if input.len() != self.columns || output.len() != self.rows {
            return Err(solve_failed(format!(
                "sparse matrix is {}x{} but input/output have {}/{} values",
                self.rows,
                self.columns,
                input.len(),
                output.len()
            )));
        }
        self.apply_rows(0..self.rows, input, output)
    }

    /// Read one matrix entry; structural zeros return zero.
    #[must_use]
    pub fn entry(&self, row: usize, column: usize) -> Option<f64> {
        if row >= self.rows || column >= self.columns {
            return None;
        }
        let range = self.row_offsets[row]..self.row_offsets[row + 1];
        match self.column_indices[range.clone()].binary_search(&column) {
            Ok(offset) => Some(self.values[range.start + offset]),
            Err(_) => Some(0.0),
        }
    }
}

impl LinearOperator for CsrMatrix {
    fn rows(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.columns
    }

    fn apply(&self, input: &[f64], output: &mut [f64]) -> Result<(), Diagnostic> {
        self.multiply_into(input, output)
    }

    fn row_action(&self) -> Option<&dyn RowLinearAction> {
        Some(self)
    }

    fn diagonal(&self, output: &mut [f64]) -> Result<DiagonalAvailability, Diagnostic> {
        if self.rows != self.columns || output.len() != self.rows {
            return Err(solve_failed(
                "CSR diagonal output must match a square matrix",
            ));
        }
        for (row, value) in output.iter_mut().enumerate() {
            *value = self.entry(row, row).expect("row and column are in range");
        }
        if output.iter().any(|value| !value.is_finite()) {
            return Err(solve_failed("CSR diagonal contains a non-finite value"));
        }
        Ok(DiagonalAvailability::Available)
    }
}

impl RowLinearAction for CsrMatrix {
    fn apply_rows(
        &self,
        rows: std::ops::Range<usize>,
        input: &[f64],
        output: &mut [f64],
    ) -> Result<(), Diagnostic> {
        let row_count = rows
            .end
            .checked_sub(rows.start)
            .ok_or_else(|| solve_failed("sparse row action requires a nondecreasing row range"))?;
        if rows.end > self.rows || input.len() != self.columns || output.len() != row_count {
            return Err(solve_failed(format!(
                "sparse row action for {:?} of {}x{} has input/output sizes {}/{}",
                rows,
                self.rows,
                self.columns,
                input.len(),
                output.len()
            )));
        }
        for (row, output_value) in rows.zip(output.iter_mut()) {
            *output_value = 0.0;
            for entry in self.row_offsets[row]..self.row_offsets[row + 1] {
                *output_value += self.values[entry] * input[self.column_indices[entry]];
            }
        }
        if output.iter().any(|value| !value.is_finite()) {
            return Err(solve_failed(
                "sparse matrix-vector product produced a non-finite value",
            ));
        }
        Ok(())
    }
}

impl TransposeLinearOperator for CsrMatrix {
    fn apply_transpose(&self, input: &[f64], output: &mut [f64]) -> Result<(), Diagnostic> {
        if input.len() != self.rows || output.len() != self.columns {
            return Err(solve_failed(format!(
                "transposed sparse matrix is {}x{} but input/output have {}/{} values",
                self.columns,
                self.rows,
                input.len(),
                output.len()
            )));
        }
        output.fill(0.0);
        for (row, input_value) in input.iter().enumerate() {
            for entry in self.row_offsets[row]..self.row_offsets[row + 1] {
                output[self.column_indices[entry]] += self.values[entry] * input_value;
            }
        }
        if output.iter().any(|value| !value.is_finite()) {
            return Err(solve_failed(
                "transposed sparse matrix-vector product produced a non-finite value",
            ));
        }
        Ok(())
    }
}

/// Sparse square matrix and matching right-hand side.
#[derive(Debug, Clone, PartialEq)]
pub struct LinearSystem {
    matrix: CsrMatrix,
    rhs: Vec<f64>,
}

impl LinearSystem {
    /// Admit one canonical square sparse matrix and matching finite RHS.
    ///
    /// [`CsrMatrix`] deliberately supports rectangular operators and empty
    /// rows. A complete assembled linear system is narrower: it is square,
    /// omits explicit zeros, and has at least one nonzero in every row.
    ///
    /// # Errors
    /// Returns `EQ0806` for a nonsquare matrix, RHS shape/value mismatch,
    /// explicit stored zero, or structurally empty equation row.
    pub fn new(matrix: CsrMatrix, rhs: Vec<f64>) -> Result<Self, Diagnostic> {
        if matrix.rows != matrix.columns {
            return Err(assembly_failed(format!(
                "linear system matrix must be square, found {}x{}",
                matrix.rows, matrix.columns
            )));
        }
        if rhs.len() != matrix.rows || rhs.iter().any(|value| !value.is_finite()) {
            return Err(assembly_failed(format!(
                "linear system with {} rows requires {} finite right-hand-side values, found {}",
                matrix.rows,
                matrix.rows,
                rhs.len()
            )));
        }
        if matrix.values.contains(&0.0) {
            return Err(assembly_failed(
                "linear system CSR must omit explicit zero entries",
            ));
        }
        if let Some(row) =
            (0..matrix.rows).find(|row| matrix.row_offsets[*row] == matrix.row_offsets[*row + 1])
        {
            return Err(assembly_failed(format!(
                "assembled global row {row} has no nonzero entries"
            )));
        }
        Ok(Self { matrix, rhs })
    }

    /// Sparse operator.
    #[must_use]
    pub const fn matrix(&self) -> &CsrMatrix {
        &self.matrix
    }

    /// Right-hand side.
    #[must_use]
    pub fn rhs(&self) -> &[f64] {
        &self.rhs
    }
}

impl CompleteCsrStorage for LinearSystem {
    fn rows(&self) -> usize {
        self.matrix.rows()
    }

    fn columns(&self) -> usize {
        self.matrix.columns()
    }

    fn row_offsets(&self) -> &[usize] {
        self.matrix.row_offsets()
    }

    fn column_indices(&self) -> &[usize] {
        self.matrix.column_indices()
    }

    fn values(&self) -> &[f64] {
        self.matrix.values()
    }

    fn right_hand_side(&self) -> &[f64] {
        self.rhs()
    }
}

fn assembly_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::ASSEMBLY_FAILED, message)
}

fn solve_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scatter_eliminates_fixed_columns_and_skips_fixed_rows() {
        let local =
            LocalContribution::new(2, 2, vec![1.0, -1.0, -1.0, 1.0], vec![0.0, 0.0]).unwrap();
        let map = AssemblyMap::new(
            vec![None, Some(DofId::new(0))],
            vec![LocalUnknown::Fixed(2.0), LocalUnknown::Free(DofId::new(0))],
        )
        .unwrap();
        let mut assembler = CooAssembler::new(1).unwrap();
        assembler.scatter(&map, &local).unwrap();
        let system = assembler.finish().unwrap();
        assert_eq!(system.matrix().entry(0, 0), Some(1.0));
        assert_eq!(system.rhs(), &[2.0]);
    }

    #[test]
    fn scatter_rejects_shape_and_global_index_mismatch() {
        let local = LocalContribution::new(1, 1, vec![1.0], vec![0.0]).unwrap();
        let map = AssemblyMap::new(
            vec![Some(DofId::new(1))],
            vec![LocalUnknown::Free(DofId::new(1))],
        )
        .unwrap();
        let mut assembler = CooAssembler::new(1).unwrap();
        assert_eq!(
            assembler.scatter(&map, &local).unwrap_err().code(),
            codes::ASSEMBLY_FAILED
        );
    }

    #[test]
    fn failed_scatter_is_atomic() {
        let local = LocalContribution::new(1, 1, vec![f64::MAX], vec![0.0]).unwrap();
        let map = AssemblyMap::new(
            vec![Some(DofId::new(0))],
            vec![LocalUnknown::Free(DofId::new(0))],
        )
        .unwrap();
        let mut assembler = CooAssembler::new(1).unwrap();
        assembler.scatter(&map, &local).unwrap();
        let before = assembler.clone().finish().unwrap();
        assert_eq!(
            assembler.scatter(&map, &local).unwrap_err().code(),
            codes::ASSEMBLY_FAILED
        );
        assert_eq!(assembler.finish().unwrap(), before);
    }

    #[test]
    fn mismatched_delta_is_rejected_atomically() {
        let local = LocalContribution::new(1, 1, vec![2.0], vec![3.0]).unwrap();
        let map = AssemblyMap::new(
            vec![Some(DofId::new(0))],
            vec![LocalUnknown::Free(DofId::new(0))],
        )
        .unwrap();
        let mut assembler = CooAssembler::new(1).unwrap();
        assembler.scatter(&map, &local).unwrap();
        let before = assembler.clone().finish().unwrap();
        let foreign = AssemblyDelta::from_local(2, &map, &local).unwrap();

        assert_eq!(
            assembler.scatter_delta(&foreign).unwrap_err().code(),
            codes::ASSEMBLY_FAILED
        );
        assert_eq!(assembler.finish().unwrap(), before);
    }

    #[test]
    fn sorted_csr_constructor_admits_rectangular_and_empty_rows() {
        let matrix =
            CsrMatrix::from_sorted_csr(2, 3, vec![0, 0, 2], vec![0, 2], vec![2.0, -1.0]).unwrap();
        assert_eq!(matrix.rows(), 2);
        assert_eq!(matrix.columns(), 3);
        assert_eq!(matrix.entry(0, 1), Some(0.0));
        assert_eq!(matrix.entry(1, 2), Some(-1.0));
    }

    #[test]
    fn sorted_csr_constructor_rejects_every_broken_invariant() {
        let cases = [
            CsrMatrix::from_sorted_csr(0, 1, vec![0], vec![], vec![]),
            CsrMatrix::from_sorted_csr(1, 1, vec![0], vec![], vec![]),
            CsrMatrix::from_sorted_csr(1, 1, vec![1, 1], vec![0], vec![1.0]),
            CsrMatrix::from_sorted_csr(1, 1, vec![0, 1], vec![], vec![1.0]),
            CsrMatrix::from_sorted_csr(1, 1, vec![0, 1], vec![1], vec![1.0]),
            CsrMatrix::from_sorted_csr(1, 2, vec![0, 2], vec![1, 0], vec![1.0, 1.0]),
            CsrMatrix::from_sorted_csr(1, 1, vec![0, 1], vec![0], vec![f64::NAN]),
        ];
        for result in cases {
            assert_eq!(result.unwrap_err().code(), codes::ASSEMBLY_FAILED);
        }
    }

    #[test]
    fn linear_system_constructor_enforces_complete_canonical_rows() {
        let rectangular = CsrMatrix::from_sorted_csr(1, 2, vec![0, 1], vec![0], vec![1.0]).unwrap();
        let wrong_rhs = CsrMatrix::from_sorted_csr(1, 1, vec![0, 1], vec![0], vec![1.0]).unwrap();
        let non_finite_rhs = wrong_rhs.clone();
        let empty_row =
            CsrMatrix::from_sorted_csr(2, 2, vec![0, 1, 1], vec![0], vec![1.0]).unwrap();
        let explicit_zero =
            CsrMatrix::from_sorted_csr(1, 1, vec![0, 1], vec![0], vec![0.0]).unwrap();

        let cases = [
            LinearSystem::new(rectangular, vec![0.0]),
            LinearSystem::new(wrong_rhs, vec![]),
            LinearSystem::new(non_finite_rhs, vec![f64::NAN]),
            LinearSystem::new(empty_row, vec![0.0, 0.0]),
            LinearSystem::new(explicit_zero, vec![0.0]),
        ];
        for result in cases {
            assert_eq!(result.unwrap_err().code(), codes::ASSEMBLY_FAILED);
        }

        let matrix = CsrMatrix::from_sorted_csr(1, 1, vec![0, 1], vec![0], vec![2.0]).unwrap();
        assert_eq!(LinearSystem::new(matrix, vec![3.0]).unwrap().rhs(), &[3.0]);
    }
}
