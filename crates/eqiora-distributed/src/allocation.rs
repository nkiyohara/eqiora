use eqiora_core::Diagnostic;

use crate::error::{invalid_realization, solve_failed};

pub(crate) fn checked_count_add(
    count: usize,
    increment: usize,
    purpose: &'static str,
) -> Result<usize, Diagnostic> {
    count
        .checked_add(increment)
        .ok_or_else(|| invalid_realization(format!("{purpose} overflowed usize")))
}

pub(crate) fn realization_vector<T>(
    capacity: usize,
    purpose: &'static str,
) -> Result<Vec<T>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| invalid_realization(format!("could not reserve {purpose}")))?;
    Ok(values)
}

pub(crate) fn realization_copy<T: Copy>(
    source: &[T],
    purpose: &'static str,
) -> Result<Vec<T>, Diagnostic> {
    let mut values = realization_vector(source.len(), purpose)?;
    // `realization_vector` reserves the complete extent first, so this copy
    // cannot trigger a hidden capacity growth after admission begins.
    values.extend_from_slice(source);
    Ok(values)
}

pub(crate) fn solve_vector<T>(
    capacity: usize,
    purpose: &'static str,
) -> Result<Vec<T>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| solve_failed(format!("could not reserve {purpose}")))?;
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn impossible_counts_and_capacities_fail_as_diagnostics() {
        assert!(checked_count_add(usize::MAX, 1, "test count").is_err());
        assert!(realization_vector::<u8>(usize::MAX, "test realization").is_err());
        assert!(solve_vector::<u8>(usize::MAX, "test solve").is_err());
    }
}
