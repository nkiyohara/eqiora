//! Recover selected component reactions from the same prepared assembly packets.

use std::collections::BTreeSet;

use eqiora_assembly::{AssemblyDelta, AssemblyRowDelta, AssemblyTargetId, AssemblyWork};
use eqiora_core::Diagnostic;

use super::invalid;

/// Partial row actions, not a square system submitted to a solver.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ReactionRows {
    size: usize,
    rows: Vec<AssemblyRowDelta>,
}

impl ReactionRows {
    pub(crate) fn residual(&self, values: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        if values.len() != self.size || values.iter().any(|value| !value.is_finite()) {
            return Err(invalid(
                "reaction values differ from the exact finite full solution",
            ));
        }
        let mut residual = vec![0.0; self.size];
        for row in &self.rows {
            residual[row.row().index()] += row
                .entries()
                .iter()
                .map(|(column, coefficient)| coefficient * values[column.index()])
                .sum::<f64>()
                - row.rhs();
        }
        if residual.iter().any(|value| !value.is_finite()) {
            return Err(invalid("component reaction accumulation is non-finite"));
        }
        Ok(residual)
    }
}

pub(crate) fn prepare_reaction_rows(
    work: &dyn AssemblyWork,
    source_target: AssemblyTargetId,
    size: usize,
    groups: &[Vec<usize>],
    rows: &BTreeSet<usize>,
) -> Result<Vec<ReactionRows>, Diagnostic> {
    if size == 0 || groups.is_empty() || rows.iter().any(|&row| row >= size) {
        return Err(invalid(
            "reaction groups or selected rows differ from the full system",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut reactions = Vec::new();
    for packets in groups {
        let mut selected = Vec::new();
        for &index in packets {
            if index >= work.packet_count() || !seen.insert(index) {
                return Err(invalid("reaction groups need unique valid cell packets"));
            }
            let packet = work.evaluate(index)?;
            let mapping = packet
                .mappings()
                .iter()
                .find(|mapping| mapping.target() == source_target)
                .ok_or_else(|| invalid("reaction packet omits its full-system map"))?;
            let delta = AssemblyDelta::from_local(size, mapping.map(), packet.local())?;
            selected.extend(
                delta
                    .rows()
                    .iter()
                    .filter(|row| rows.contains(&row.row().index()))
                    .cloned(),
            );
        }
        reactions.push(ReactionRows {
            size,
            rows: selected,
        });
    }
    Ok(reactions)
}
