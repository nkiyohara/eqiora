use std::collections::BTreeMap;

use eqiora_assembly::LocalContribution;
use eqiora_core::{Diagnostic, RawId};
use eqiora_meshing::{AffineGeometryMap, GeometryMap, QuadratureRule, ReferenceCellFamily};
use eqiora_realization::SpaceFamily;

use crate::affine_fem::physical_gradient;
use crate::form_compiler::bilinear::Basis;

use super::binding::{BoundRegionForm, basis};
use super::invalid;

impl BoundRegionForm {
    /// Previous coefficients are physical coherent-SI values, not scaled algebraic unknowns.
    pub(crate) fn evaluate(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
        previous: &BTreeMap<RawId, Vec<f64>>,
    ) -> Result<LocalContribution, Diagnostic> {
        if geometry.reference_cell() != self.reference
            || quadrature.reference_cell() != self.reference
            || geometry.physical_dimension() != self.form.dimension
        {
            return Err(invalid(
                "region geometry and quadrature must match bound reference support",
            ));
        }
        let exactness = if self
            .fields
            .iter()
            .any(|layout| layout.space.family() == SpaceFamily::SimplexP1Bubble)
        {
            2 * (self.form.dimension + 1)
        } else if self.reference.family() == ReferenceCellFamily::Hypercube {
            3
        } else {
            2
        };
        if quadrature
            .polynomial_exactness()
            .is_none_or(|order| order < exactness)
        {
            return Err(invalid(
                "quadrature does not cover region basis-product degree",
            ));
        }
        if previous.len() != self.previous.len()
            || self.previous.iter().any(|(field, layout)| {
                previous.get(field).is_none_or(|values| {
                    values.len() != layout.range.len()
                        || values.iter().any(|value| !value.is_finite())
                })
            })
        {
            return Err(invalid(
                "previous coefficients require exact consumed Field coverage, shape and finite values",
            ));
        }
        let count = self.fields.last().expect("nonempty region").range.end;
        let entries = count
            .checked_mul(count)
            .ok_or_else(|| invalid("region matrix size overflow"))?;
        let mut matrix = vec![0.0; entries];
        let mut rhs = vec![0.0; count];
        let spaces = self
            .fields
            .iter()
            .map(|layout| basis(layout.space, self.reference))
            .collect::<Result<Vec<_>, _>>()?;
        let indices = self
            .fields
            .iter()
            .enumerate()
            .map(|(index, layout)| (layout.field, index))
            .collect::<BTreeMap<_, _>>();
        let inverse = geometry.inverse_jacobian()?;
        let mut physical = vec![0.0; self.form.dimension];
        for point in quadrature.points() {
            geometry.map_point(&point.coordinates, &mut physical)?;
            let tabulations = spaces
                .iter()
                .map(|space| space.tabulate(&point.coordinates))
                .collect::<Result<Vec<_>, _>>()?;
            let gradients = tabulations
                .iter()
                .map(|tabulation| {
                    (0..tabulation.values().len())
                        .map(|dof| {
                            physical_gradient(
                                tabulation.gradient(dof).expect("supported basis gradient"),
                                &inverse,
                                self.form.dimension,
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let weight = point.weight * geometry.measure_scale();
            for (row_index, row) in self.form.rows.iter().enumerate() {
                let test_layout = &self.fields[row_index];
                let test_basis = &tabulations[row_index];
                let row_scale = weight * self.row_multipliers[row_index];
                let forcing = row.forcing.evaluate(&physical)?;
                for test in 0..test_basis.values().len() {
                    for component in 0..test_layout.components {
                        rhs[test_layout.range.start + test * test_layout.components + component] +=
                            row_scale * forcing * test_basis.values()[test];
                    }
                }
                for term in &row.terms {
                    let eliminated = self.eliminations.get(&term.trial);
                    let trial_field = eliminated.copied().unwrap_or(term.trial);
                    let column = indices[&trial_field];
                    let trial_layout = &self.fields[column];
                    let trial_basis = &tabulations[column];
                    let (new_factor, old_factor) = match (eliminated, term.derivative) {
                        (Some(_), true) => (1.0, 0.0),
                        (Some(_), false) => (self.step.expect("bound state step"), -1.0),
                        (None, true) => {
                            let inverse_step = self.step.expect("bound derivative step").recip();
                            (inverse_step, inverse_step)
                        }
                        (None, false) => (1.0, 0.0),
                    };
                    let coefficient = row_scale * term.coefficient.evaluate(&physical)?;
                    for test in 0..test_basis.values().len() {
                        for test_component in 0..test_layout.components {
                            let global_test = test_layout.range.start
                                + test * test_layout.components
                                + test_component;
                            for trial in 0..trial_basis.values().len() {
                                for trial_component in 0..trial_layout.components {
                                    let local_trial =
                                        trial * trial_layout.components + trial_component;
                                    let entry = coefficient
                                        * term.pairing.entry(
                                            Basis {
                                                value: test_basis.values()[test],
                                                gradient: &gradients[row_index][test],
                                                component: test_component,
                                            },
                                            Basis {
                                                value: trial_basis.values()[trial],
                                                gradient: &gradients[column][trial],
                                                component: trial_component,
                                            },
                                        );
                                    matrix[global_test * count
                                        + trial_layout.range.start
                                        + local_trial] += entry * new_factor * trial_layout.scale;
                                    if old_factor != 0.0 {
                                        rhs[global_test] +=
                                            entry * old_factor * previous[&term.trial][local_trial];
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        LocalContribution::new(count, count, matrix, rhs)
    }
}
