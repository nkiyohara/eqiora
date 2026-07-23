use eqiora_core::Diagnostic;

use crate::{AffineGeometryMap, DiscreteSpace, LocalContribution, LocalOperator, SimplexP1Space};

use super::acceptance::require_local_geometry;
use super::{CONSTRAINT_LOCAL_DOF_COUNT, CONSTRAINT_LOCAL_GAUGE, DIMENSION, P1_BASIS_COUNT};

/// Cell-local occurrence of one global zero-integral pressure constraint.
pub(crate) struct MiniPressureMeanConstraintCell;

impl LocalOperator<AffineGeometryMap> for MiniPressureMeanConstraintCell {
    fn evaluate(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &crate::QuadratureRule,
    ) -> Result<LocalContribution, Diagnostic> {
        require_local_geometry(geometry, quadrature)?;
        let pressure_space = SimplexP1Space::new(DIMENSION)?;
        let mut matrix = vec![0.0; CONSTRAINT_LOCAL_DOF_COUNT * CONSTRAINT_LOCAL_DOF_COUNT];
        for point in quadrature.points() {
            let basis = pressure_space.tabulate(&point.coordinates)?;
            let scale = point.weight * geometry.measure_scale();
            for pressure in 0..P1_BASIS_COUNT {
                let value = scale * basis.values()[pressure];
                matrix[pressure * CONSTRAINT_LOCAL_DOF_COUNT + CONSTRAINT_LOCAL_GAUGE] += value;
                matrix[CONSTRAINT_LOCAL_GAUGE * CONSTRAINT_LOCAL_DOF_COUNT + pressure] += value;
            }
        }
        LocalContribution::new(
            CONSTRAINT_LOCAL_DOF_COUNT,
            CONSTRAINT_LOCAL_DOF_COUNT,
            matrix,
            vec![0.0; CONSTRAINT_LOCAL_DOF_COUNT],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocalOperator, triangle_duffy_gauss_legendre};

    #[test]
    fn pressure_constraint_is_one_independent_symmetric_local_relation() {
        let geometry = AffineGeometryMap::new(
            eqiora_meshing::ReferenceCell::simplex(DIMENSION).unwrap(),
            DIMENSION,
            vec![0.0, 0.0],
            vec![1.0, 0.0, 0.0, 1.0],
        )
        .unwrap();
        let local = MiniPressureMeanConstraintCell
            .evaluate(&geometry, &triangle_duffy_gauss_legendre(3).unwrap())
            .unwrap();
        assert_eq!(local.rows(), CONSTRAINT_LOCAL_DOF_COUNT);
        assert!(local.rhs().iter().all(|value| *value == 0.0));
        for pressure in 0..P1_BASIS_COUNT {
            assert!(
                (local.entry(pressure, CONSTRAINT_LOCAL_GAUGE).unwrap() - 1.0 / 6.0).abs()
                    < 2.0e-15
            );
            assert_eq!(
                local.entry(pressure, CONSTRAINT_LOCAL_GAUGE),
                local.entry(CONSTRAINT_LOCAL_GAUGE, pressure)
            );
        }
        for row in 0..CONSTRAINT_LOCAL_DOF_COUNT {
            for column in 0..CONSTRAINT_LOCAL_DOF_COUNT {
                if row != CONSTRAINT_LOCAL_GAUGE && column != CONSTRAINT_LOCAL_GAUGE {
                    assert_eq!(local.entry(row, column), Some(0.0));
                }
            }
        }
    }
}
