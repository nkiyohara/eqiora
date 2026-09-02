//! Realization-neutral strong-constraint algebra for assembled systems.

use eqiora_assembly::{AssemblyMap, DofId, LinearSystem, LocalUnknown};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::interleaved_dofs::InterleavedDofValues;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}

fn fallible_zeroed(length: usize, message: &'static str) -> Result<Vec<f64>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message))?;
    values.resize(length, 0.0);
    Ok(values)
}

/// One exact partition of global degrees of freedom into fixed values and
/// reduced-system equations.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConstrainedDofLayout {
    fixed_values: Vec<Option<f64>>,
    free_indices: Vec<Option<DofId>>,
    free_count: usize,
}

impl ConstrainedDofLayout {
    pub(crate) fn new(fixed_values: Vec<Option<f64>>) -> Result<Self, Diagnostic> {
        if fixed_values
            .iter()
            .flatten()
            .any(|value| !value.is_finite())
        {
            return Err(invalid("fixed degree-of-freedom value is non-finite"));
        }
        let mut free_count = 0_usize;
        let free_indices = fixed_values
            .iter()
            .map(|fixed| {
                fixed.is_none().then(|| {
                    let equation = DofId::new(free_count);
                    free_count += 1;
                    equation
                })
            })
            .collect();
        Ok(Self {
            fixed_values,
            free_indices,
            free_count,
        })
    }

    pub(crate) const fn free_count(&self) -> usize {
        self.free_count
    }

    pub(crate) fn is_free(&self, global: usize) -> Result<bool, Diagnostic> {
        self.free_indices
            .get(global)
            .map(Option::is_some)
            .ok_or_else(|| invalid("degree of freedom is outside the constrained layout"))
    }

    pub(crate) fn free_globals(&self) -> Vec<usize> {
        let mut globals = vec![0; self.free_count];
        for (global, free) in self.free_indices.iter().enumerate() {
            if let Some(free) = free {
                globals[free.index()] = global;
            }
        }
        globals
    }

    pub(crate) fn reduced_map(&self, global_dofs: &[usize]) -> Result<AssemblyMap, Diagnostic> {
        let mut equations = Vec::with_capacity(global_dofs.len());
        let mut unknowns = Vec::with_capacity(global_dofs.len());
        for &global in global_dofs {
            let fixed = self
                .fixed_values
                .get(global)
                .ok_or_else(|| invalid("local degree of freedom is outside the global layout"))?;
            let free = self.free_indices[global];
            equations.push(free);
            unknowns.push(match fixed {
                Some(value) => LocalUnknown::Fixed(*value),
                None => LocalUnknown::Free(
                    free.expect("every unfixed degree of freedom owns a reduced equation"),
                ),
            });
        }
        AssemblyMap::new(equations, unknowns)
    }

    pub(crate) fn full_map(&self, global_dofs: &[usize]) -> Result<AssemblyMap, Diagnostic> {
        for &global in global_dofs {
            if global >= self.fixed_values.len() {
                return Err(invalid(
                    "local degree of freedom is outside the full global layout",
                ));
            }
        }
        AssemblyMap::new(
            global_dofs
                .iter()
                .map(|&global| Some(DofId::new(global)))
                .collect(),
            global_dofs
                .iter()
                .map(|&global| LocalUnknown::Free(DofId::new(global)))
                .collect(),
        )
    }

    pub(crate) fn lift(&self, free_values: &[f64]) -> Result<Vec<f64>, Diagnostic> {
        if free_values.len() != self.free_count {
            return Err(invalid(
                "reduced solution shape differs from its constrained layout",
            ));
        }
        let mut values = fallible_zeroed(
            self.fixed_values.len(),
            "constrained solution allocation exceeds platform capacity",
        )?;
        for ((fixed, free), value) in self
            .fixed_values
            .iter()
            .zip(&self.free_indices)
            .zip(&mut values)
        {
            *value = fixed.unwrap_or_else(|| {
                free_values[free
                    .expect("every unfixed degree of freedom owns a reduced equation")
                    .index()]
            });
        }
        Ok(values)
    }

    pub(crate) fn full_residual(
        &self,
        system: &LinearSystem,
        values: &[f64],
    ) -> Result<Vec<f64>, Diagnostic> {
        if values.len() != self.fixed_values.len() {
            return Err(invalid(
                "full solution shape differs from its constrained layout",
            ));
        }
        let mut residual = fallible_zeroed(
            system.matrix().rows(),
            "constrained residual allocation exceeds platform capacity",
        )?;
        system.matrix().multiply_into(values, &mut residual)?;
        for (value, right_hand_side) in residual.iter_mut().zip(system.rhs()) {
            *value -= right_hand_side;
        }
        Ok(residual)
    }

    pub(crate) fn reaction_sum<const D: usize>(
        &self,
        residual: &[f64],
    ) -> Result<[f64; D], Diagnostic> {
        if residual.len() != self.fixed_values.len() {
            return Err(invalid(
                "reaction residual shape differs from its constrained layout",
            ));
        }
        Ok(InterleavedDofValues::<D>::new(residual)?
            .sum_where(|global| self.fixed_values[global].is_some()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constrained_allocation_failure_is_stable() {
        let error = fallible_zeroed(
            usize::MAX,
            "constrained allocation exceeds platform capacity",
        )
        .unwrap_err();
        assert_eq!(error.code(), codes::NUMERICAL_SOLVE_FAILED);
        assert_eq!(
            error.message(),
            "constrained allocation exceeds platform capacity"
        );
    }
}
