use eqiora_assembly::LocalContribution;
use eqiora_core::Diagnostic;
use eqiora_meshing::{AffineGeometryMap, GeometryMap, QuadratureRule};

use crate::affine_fem::physical_gradient;
use crate::discrete_space::{DiscreteSpace, HypercubeQ1Space};

/// Field-major rows and columns, with the Q1 bit order within each Field.
pub(in crate::form_compiler) fn integrate(
    dimension: usize,
    fields: usize,
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
    values: impl Fn(&[f64], &mut [f64], &mut [f64], &mut [f64]) -> Result<(), Diagnostic>,
) -> Result<LocalContribution, Diagnostic> {
    let space = HypercubeQ1Space::new(dimension)?;
    if fields == 0
        || geometry.reference_cell() != space.reference_cell()
        || quadrature.reference_cell() != space.reference_cell()
        || geometry.physical_dimension() != dimension
    {
        return Err(super::invalid(
            "linear Q1 geometry, quadrature or Field count mismatch",
        ));
    }
    let basis_count = space.local_dofs().len();
    let count = fields
        .checked_mul(basis_count)
        .ok_or_else(|| super::invalid("local DOF count overflow"))?;
    let entries = count
        .checked_mul(count)
        .ok_or_else(|| super::invalid("local matrix size overflow"))?;
    let inverse = geometry.inverse_jacobian()?;
    let mut matrix = vec![0.0; entries];
    let mut rhs = vec![0.0; count];
    let mut diffusion = vec![0.0; fields];
    let mut reaction = vec![0.0; fields * fields];
    let mut forcing = vec![0.0; fields];
    let mut physical = vec![0.0; dimension];
    for point in quadrature.points() {
        let basis = space.tabulate(&point.coordinates)?;
        geometry.map_point(&point.coordinates, &mut physical)?;
        diffusion.fill(0.0);
        reaction.fill(0.0);
        forcing.fill(0.0);
        values(&physical, &mut diffusion, &mut reaction, &mut forcing)?;
        if diffusion
            .iter()
            .any(|value| !value.is_finite() || *value <= 0.0)
            || reaction
                .iter()
                .chain(&forcing)
                .any(|value| !value.is_finite())
        {
            return Err(super::invalid(
                "linear Q1 requires positive finite diffusion and finite reaction/forcing",
            ));
        }
        let scale = point.weight * geometry.measure_scale();
        let gradients = (0..basis_count)
            .map(|dof| {
                physical_gradient(
                    basis.gradient(dof).expect("Q1 gradient"),
                    &inverse,
                    dimension,
                )
            })
            .collect::<Vec<_>>();
        for row in 0..fields {
            for test in 0..basis_count {
                let global_test = row * basis_count + test;
                rhs[global_test] += scale * forcing[row] * basis.values()[test];
                for column in 0..fields {
                    for trial in 0..basis_count {
                        let gradient_pairing = if row == column {
                            diffusion[row]
                                * gradients[test]
                                    .iter()
                                    .zip(&gradients[trial])
                                    .map(|(left, right)| left * right)
                                    .sum::<f64>()
                        } else {
                            0.0
                        };
                        matrix[global_test * count + column * basis_count + trial] += scale
                            * (gradient_pairing
                                + reaction[row * fields + column]
                                    * basis.values()[test]
                                    * basis.values()[trial]);
                    }
                }
            }
        }
    }
    LocalContribution::new(count, count, matrix, rhs)
}
