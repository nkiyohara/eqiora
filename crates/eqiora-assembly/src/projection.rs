use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::{AssemblyMap, DofId, LocalContribution, LocalUnknown};

/// Canonical additive effect of one mapped local contribution on one row.
///
/// Duplicate local rows and columns have already been folded in the original
/// local row-major order. Entries are then exposed in ascending global-column
/// order so execution adapters can route the row without reimplementing
/// constraint elimination or local scatter semantics.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyRowDelta {
    row: DofId,
    entries: Vec<(DofId, f64)>,
    rhs: f64,
}

impl AssemblyRowDelta {
    /// Global equation receiving this additive row.
    #[must_use]
    pub const fn row(&self) -> DofId {
        self.row
    }

    /// Canonically ordered global-column deltas.
    #[must_use]
    pub fn entries(&self) -> &[(DofId, f64)] {
        &self.entries
    }

    /// Additive right-hand-side value for this row.
    #[must_use]
    pub const fn rhs(&self) -> f64 {
        self.rhs
    }
}

/// Canonical packet-local scatter delta for one square assembly target.
///
/// This is the sole lowering from anonymous local rows and columns into global
/// algebra. Distributed adapters may split its already-mapped rows by owner,
/// but must not repeat the mapping and fixed-column elimination themselves.
#[derive(Debug, Clone, PartialEq)]
pub struct AssemblyDelta {
    target_size: usize,
    rows: Vec<AssemblyRowDelta>,
}

impl AssemblyDelta {
    /// Project one finite local contribution through its independent map.
    ///
    /// Local duplicates are accumulated in local row-major order before rows
    /// and columns are canonicalized. Exact zeros remain present until final
    /// COO compression so all backends share the reference structural gate.
    ///
    /// # Errors
    /// Returns `EQ0806` for a zero target, shape mismatch, an out-of-range
    /// global degree of freedom, or non-finite projected arithmetic.
    pub fn from_local(
        target_size: usize,
        map: &AssemblyMap,
        local: &LocalContribution,
    ) -> Result<Self, Diagnostic> {
        if target_size == 0 {
            return Err(assembly_failed(
                "an assembly delta requires a nonempty target",
            ));
        }
        if map.equations().len() != local.rows() || map.unknowns().len() != local.columns() {
            return Err(assembly_failed(format!(
                "assembly map is {}x{} but local contribution is {}x{}",
                map.equations().len(),
                map.unknowns().len(),
                local.rows(),
                local.columns()
            )));
        }
        for equation in map.equations().iter().flatten() {
            check_dof(target_size, *equation)?;
        }
        for unknown in map.unknowns() {
            if let LocalUnknown::Free(dof) = unknown {
                check_dof(target_size, *dof)?;
            }
        }

        let mut entry_deltas = BTreeMap::<(DofId, DofId), f64>::new();
        let mut rhs_deltas = BTreeMap::<DofId, f64>::new();
        for (local_row, equation) in map.equations().iter().enumerate() {
            let Some(global_row) = equation else {
                continue;
            };
            *rhs_deltas.entry(*global_row).or_insert(0.0) += local.rhs()[local_row];
            for (local_column, unknown) in map.unknowns().iter().enumerate() {
                let value = local
                    .entry(local_row, local_column)
                    .expect("assembly map shape matches local contribution");
                match unknown {
                    LocalUnknown::Free(global_column) => {
                        *entry_deltas
                            .entry((*global_row, *global_column))
                            .or_insert(0.0) += value;
                    }
                    LocalUnknown::Fixed(fixed) => {
                        *rhs_deltas.entry(*global_row).or_insert(0.0) -= value * fixed;
                    }
                }
            }
        }
        if entry_deltas.values().any(|value| !value.is_finite())
            || rhs_deltas.values().any(|value| !value.is_finite())
        {
            return Err(assembly_failed(
                "sparse assembly produced a non-finite projected value",
            ));
        }

        let mut entries = entry_deltas.into_iter().peekable();
        let mut rows = Vec::with_capacity(rhs_deltas.len());
        for (row, rhs) in rhs_deltas {
            let mut row_entries = Vec::new();
            while entries
                .peek()
                .is_some_and(|((entry_row, _), _)| *entry_row == row)
            {
                let ((_, column), value) = entries
                    .next()
                    .expect("peeked assembly entry remains available");
                row_entries.push((column, value));
            }
            rows.push(AssemblyRowDelta {
                row,
                entries: row_entries,
                rhs,
            });
        }
        debug_assert!(entries.next().is_none());
        Ok(Self { target_size, rows })
    }

    /// Dimension of the square target addressed by these deltas.
    #[must_use]
    pub const fn target_size(&self) -> usize {
        self.target_size
    }

    /// Canonically ascending global rows.
    #[must_use]
    pub fn rows(&self) -> &[AssemblyRowDelta] {
        &self.rows
    }
}

fn check_dof(target_size: usize, dof: DofId) -> Result<(), Diagnostic> {
    if dof.index() >= target_size {
        Err(assembly_failed(format!(
            "global degree of freedom {} is outside system size {target_size}",
            dof.index()
        )))
    } else {
        Ok(())
    }
}

fn assembly_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::ASSEMBLY_FAILED, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_preserves_local_fold_before_canonical_ordering() {
        let local = LocalContribution::new(
            2,
            4,
            vec![3.0, 1.0e16, 2.0, 3.0, 5.0, -1.0e16, 4.0, 5.0],
            vec![7.0, 11.0],
        )
        .unwrap();
        let map = AssemblyMap::new(
            vec![Some(DofId::new(1)), Some(DofId::new(1))],
            vec![
                LocalUnknown::Free(DofId::new(1)),
                LocalUnknown::Free(DofId::new(0)),
                LocalUnknown::Free(DofId::new(0)),
                LocalUnknown::Fixed(2.0),
            ],
        )
        .unwrap();

        let delta = AssemblyDelta::from_local(2, &map, &local).unwrap();
        assert_eq!(delta.target_size(), 2);
        assert_eq!(delta.rows().len(), 1);
        assert_eq!(delta.rows()[0].row(), DofId::new(1));
        assert_eq!(
            delta.rows()[0].entries(),
            &[(DofId::new(0), 6.0), (DofId::new(1), 8.0)]
        );
        assert_eq!(delta.rows()[0].rhs(), 2.0);
    }

    #[test]
    fn projection_checks_unknowns_even_when_every_equation_is_skipped() {
        let local = LocalContribution::new(1, 1, vec![1.0], vec![0.0]).unwrap();
        let map = AssemblyMap::new(vec![None], vec![LocalUnknown::Free(DofId::new(1))]).unwrap();
        assert_eq!(
            AssemblyDelta::from_local(1, &map, &local)
                .unwrap_err()
                .code(),
            codes::ASSEMBLY_FAILED
        );
    }
}
