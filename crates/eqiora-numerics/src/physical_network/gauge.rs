//! Deny a proved unreferenced uniform Across shift before solver admission.
//!
//! This is not a general rank test. Groups are exact-Domain Across columns
//! coupled by nonzero coefficients in one residual row, not whole Relations.

use std::collections::BTreeMap;

use eqiora_core::{Diagnostic, RawId};
use eqiora_schema::kernel::KernelNode;
use eqiora_sem::{KernelProgram, PhysicalUnknown};
use eqiora_solver::CompleteCsrStorage;

use super::affine_error;

pub(super) fn reject_unreferenced_across_groups(
    program: &KernelProgram,
    unknowns: &[PhysicalUnknown],
    storage: &impl CompleteCsrStorage,
) -> Result<(), Diagnostic> {
    let domains = unknowns
        .iter()
        .map(|unknown| match unknown {
            PhysicalUnknown::Across(id) => match program.node(id.erase()) {
                Some(KernelNode::Port(port)) => port
                    .physical_domain()
                    .map(|id| id.erase())
                    .map(Some)
                    .ok_or_else(|| affine_error("Across unknown has no exact physical Domain")),
                _ => Err(affine_error("Across unknown is not an exact Port")),
            },
            PhysicalUnknown::Through(_) => Ok(None),
        })
        .collect::<Result<Vec<_>, _>>()?;
    reject_shift_groups(&domains, storage)
}

fn root(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}

fn reject_shift_groups(
    domains: &[Option<RawId>],
    storage: &impl CompleteCsrStorage,
) -> Result<(), Diagnostic> {
    let mut parents = (0..domains.len()).collect::<Vec<_>>();
    let mut ranks = vec![0u8; domains.len()];
    for row in storage.row_offsets().windows(2) {
        let mut first = BTreeMap::new();
        for offset in row[0]..row[1] {
            let column = storage.column_indices()[offset];
            if storage.values()[offset] == 0.0 {
                continue;
            }
            let Some(domain) = domains[column] else {
                continue;
            };
            if let Some(previous) = first.insert(domain, column) {
                let mut left = root(&mut parents, previous);
                let mut right = root(&mut parents, column);
                if left == right {
                    continue;
                }
                if ranks[left] < ranks[right] {
                    core::mem::swap(&mut left, &mut right);
                }
                parents[right] = left;
                if ranks[left] == ranks[right] {
                    ranks[left] += 1;
                }
            }
        }
    }
    for index in 0..parents.len() {
        parents[index] = root(&mut parents, index);
    }
    let mut anchored = vec![false; parents.len()];
    for row in storage.row_offsets().windows(2) {
        let mut sums = BTreeMap::<usize, ExactCoefficientSum>::new();
        for offset in row[0]..row[1] {
            let column = storage.column_indices()[offset];
            if domains[column].is_some() {
                sums.entry(parents[column])
                    .or_default()
                    .add(storage.values()[offset])?;
            }
        }
        for (group, sum) in sums {
            anchored[group] |= !sum.is_zero();
        }
    }
    if domains
        .iter()
        .enumerate()
        .any(|(column, domain)| domain.is_some() && !anchored[parents[column]])
    {
        return Err(affine_error(
            "physical Across group has an unreferenced uniform shift; add an explicit reference equation",
        ));
    }
    Ok(())
}

// Every finite binary64 coefficient is an integer multiple of 2^-1074.
// Its highest bit in those units is <=2097. On the admitted <=64-bit usize
// platforms, at most usize::MAX coefficients add <64 bits: 34*64 = 2176
// bits therefore suffice without rounding, including subnormals and -0.
// The sum is private to this null-shift proof, not a new numeric value type.
struct ExactCoefficientSum {
    positive: [u64; 34],
    negative: [u64; 34],
}

impl Default for ExactCoefficientSum {
    fn default() -> Self {
        Self {
            positive: [0; 34],
            negative: [0; 34],
        }
    }
}

impl ExactCoefficientSum {
    fn add(&mut self, value: f64) -> Result<(), Diagnostic> {
        if usize::BITS > 64 || !value.is_finite() {
            return Err(affine_error(
                "exact physical reference check requires finite coefficients and at most 64-bit counts",
            ));
        }
        let bits = value.to_bits();
        let exponent = (bits >> 52) & 0x7ff;
        let fraction = bits & ((1u64 << 52) - 1);
        let (significand, shift) = if exponent == 0 {
            (fraction, 0)
        } else {
            (fraction | (1u64 << 52), (exponent - 1) as usize)
        };
        let words = if bits >> 63 == 0 {
            &mut self.positive
        } else {
            &mut self.negative
        };
        let word = shift / 64;
        let offset = shift % 64;
        add_word(words, word, significand << offset)?;
        if offset != 0 {
            add_word(words, word + 1, significand >> (64 - offset))?;
        }
        Ok(())
    }

    fn is_zero(&self) -> bool {
        self.positive == self.negative
    }
}

fn add_word(words: &mut [u64; 34], mut index: usize, mut value: u64) -> Result<(), Diagnostic> {
    while value != 0 {
        let slot = words.get_mut(index).ok_or_else(|| {
            affine_error("physical coefficient count exceeded the exact reference-check bound")
        })?;
        let (next, carry) = slot.overflowing_add(value);
        *slot = next;
        value = u64::from(carry);
        index += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zero(values: &[f64]) -> bool {
        let mut sum = ExactCoefficientSum::default();
        for &value in values {
            sum.add(value).unwrap();
        }
        sum.is_zero()
    }

    #[test]
    fn coefficient_zero_is_exact_across_signs_exponents_and_order() {
        assert!(zero(&[0.0, -0.0]));
        assert!(zero(&[f64::MAX, f64::MAX, -f64::MAX, -f64::MAX]));
        assert!(zero(&[2.0, -1.0, -1.0]));
        assert!(zero(&[f64::from_bits(1), -f64::from_bits(1)]));
        assert!(!zero(&[1.0, f64::from_bits(1), -1.0]));
        assert!(!zero(&[f64::MAX, 1e-300, -f64::MAX]));
        assert!(!zero(&[-1.0, -f64::from_bits(1), 1.0]));
        assert!(ExactCoefficientSum::default().add(f64::NAN).is_err());
        assert!(ExactCoefficientSum::default().add(f64::INFINITY).is_err());
    }

    #[test]
    fn count_carry_and_word_bound_are_checked() {
        let mut words = [0u64; 34];
        words[0] = u64::MAX;
        add_word(&mut words, 0, 1).unwrap();
        assert_eq!(&words[..2], &[0, 1]);
        words = [u64::MAX; 34];
        assert!(add_word(&mut words, 0, 1).is_err());
        assert!(2097 + (u64::BITS as usize) < 34 * 64);
    }
}
