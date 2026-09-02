//! Realization-neutral observations over interleaved vector degrees of freedom.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}

/// A complete vector-valued field stored in entity-major, component-minor order.
pub(crate) struct InterleavedDofValues<'a, const D: usize> {
    values: &'a [f64],
}

impl<'a, const D: usize> InterleavedDofValues<'a, D> {
    pub(crate) fn new(values: &'a [f64]) -> Result<Self, Diagnostic> {
        if D == 0 || !values.len().is_multiple_of(D) {
            return Err(invalid(
                "interleaved degree-of-freedom values do not contain complete components",
            ));
        }
        Ok(Self { values })
    }

    pub(crate) fn sum(&self) -> [f64; D] {
        self.sum_where(|_| true)
    }

    pub(crate) fn sum_where(&self, mut include: impl FnMut(usize) -> bool) -> [f64; D] {
        let mut sum = [0.0; D];
        for (global, value) in self.values.iter().enumerate() {
            if include(global) {
                sum[global % D] += value;
            }
        }
        sum
    }

    pub(crate) fn sum_entities(
        &self,
        entities: impl IntoIterator<Item = usize>,
    ) -> Result<[f64; D], Diagnostic> {
        let mut sum = [0.0; D];
        let component_values = self.values.as_chunks::<D>().0;
        for entity in entities {
            let values = component_values.get(entity).ok_or_else(|| {
                invalid("observed entity is outside the interleaved degree-of-freedom values")
            })?;
            for (result, value) in sum.iter_mut().zip(values) {
                *result += value;
            }
        }
        Ok(sum)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_complete_and_selected_vector_entities() {
        let values = InterleavedDofValues::<2>::new(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
            .expect("complete interleaved values");
        assert_eq!(values.sum(), [9.0, 12.0]);
        assert_eq!(values.sum_entities([2, 0]).unwrap(), [6.0, 8.0]);
        assert_eq!(values.sum_where(|global| global >= 2), [8.0, 10.0]);
    }

    #[test]
    fn rejects_incomplete_components_and_unknown_entities() {
        assert!(InterleavedDofValues::<2>::new(&[1.0]).is_err());
        let values = InterleavedDofValues::<2>::new(&[1.0, 2.0]).unwrap();
        assert!(values.sum_entities([1]).is_err());
    }
}
