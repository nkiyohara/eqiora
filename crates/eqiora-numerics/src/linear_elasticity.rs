//! Method-neutral isotropic linear-elasticity primitives.

use crate::continuum_kinematics::symmetric_gradient_bilinear_entry;

pub(crate) fn is_coercive_isotropic_material<const D: usize>(
    shear_modulus: f64,
    first_lame_parameter: f64,
) -> bool {
    if !(1..=3).contains(&D) {
        return false;
    }
    let volumetric_modulus = first_lame_parameter + 2.0 * shear_modulus / D as f64;
    shear_modulus.is_finite()
        && shear_modulus > 0.0
        && first_lame_parameter.is_finite()
        && volumetric_modulus.is_finite()
        && volumetric_modulus > 0.0
}

pub(crate) fn isotropic_stiffness_entry(
    row_gradient: &[f64],
    row_component: usize,
    column_gradient: &[f64],
    column_component: usize,
    shear_modulus: f64,
    first_lame_parameter: f64,
) -> f64 {
    shear_modulus
        * symmetric_gradient_bilinear_entry(
            row_gradient,
            row_component,
            column_gradient,
            column_component,
        )
        + first_lame_parameter * row_gradient[row_component] * column_gradient[column_component]
}

pub(crate) fn isotropic_stress<const D: usize>(
    strain: &[[f64; D]; D],
    shear_modulus: f64,
    first_lame_parameter: f64,
) -> [[f64; D]; D] {
    let trace = tensor_trace(strain);
    std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            2.0 * shear_modulus * strain[row][column]
                + if row == column {
                    first_lame_parameter * trace
                } else {
                    0.0
                }
        })
    })
}

pub(crate) fn isotropic_strain_energy_density<const D: usize>(
    strain: &[[f64; D]; D],
    shear_modulus: f64,
    first_lame_parameter: f64,
) -> f64 {
    let squared_norm = strain
        .iter()
        .flatten()
        .map(|value| value * value)
        .sum::<f64>();
    let trace = tensor_trace(strain);
    shear_modulus * squared_norm + 0.5 * first_lame_parameter * trace * trace
}

fn tensor_trace<const D: usize>(tensor: &[[f64; D]; D]) -> f64 {
    (0..D).map(|axis| tensor[axis][axis]).sum()
}

#[cfg(test)]
mod tests {
    use super::{
        is_coercive_isotropic_material, isotropic_strain_energy_density, isotropic_stress,
    };
    use crate::continuum_kinematics::symmetric_gradient;

    #[test]
    fn coercivity_uses_the_admitted_spatial_dimension_and_rejects_overflow() {
        assert!(is_coercive_isotropic_material::<1>(2.0, -3.0));
        assert!(!is_coercive_isotropic_material::<2>(2.0, -3.0));
        assert!(!is_coercive_isotropic_material::<3>(2.0, -3.0));
        assert!(!is_coercive_isotropic_material::<4>(2.0, 1.0));
        assert!(!is_coercive_isotropic_material::<2>(f64::MAX, f64::MAX));
    }

    #[test]
    fn isotropic_state_uses_symmetric_strain_in_every_dimension() {
        let gradient = [[1.0, 2.0, 3.0], [5.0, 7.0, 11.0], [13.0, 17.0, 19.0]];
        let strain = symmetric_gradient(&gradient);
        let stress = isotropic_stress(&strain, 2.0, 3.0);

        assert_eq!(
            strain,
            [[1.0, 3.5, 8.0], [3.5, 7.0, 14.0], [8.0, 14.0, 19.0]]
        );
        assert_eq!(stress[0][1], 14.0);
        assert_eq!(stress[1][0], stress[0][1]);
        assert_eq!(stress[2][2], 4.0 * 19.0 + 3.0 * 27.0);
        assert_eq!(
            isotropic_strain_energy_density(&strain, 2.0, 3.0),
            2.0 * strain
                .iter()
                .flatten()
                .map(|value| value * value)
                .sum::<f64>()
                + 1.5 * 27.0_f64.powi(2)
        );
    }
}
