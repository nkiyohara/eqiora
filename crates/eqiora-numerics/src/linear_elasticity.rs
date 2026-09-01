//! Method-neutral isotropic linear-elasticity primitives.

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
    let diagonal = if row_component == column_component {
        row_gradient
            .iter()
            .zip(column_gradient)
            .map(|(left, right)| left * right)
            .sum()
    } else {
        0.0
    };
    let crossed = row_gradient[column_component] * column_gradient[row_component];
    shear_modulus * (diagonal + crossed)
        + first_lame_parameter * row_gradient[row_component] * column_gradient[column_component]
}

#[cfg(test)]
mod tests {
    use super::is_coercive_isotropic_material;

    #[test]
    fn coercivity_uses_the_admitted_spatial_dimension_and_rejects_overflow() {
        assert!(is_coercive_isotropic_material::<1>(2.0, -3.0));
        assert!(!is_coercive_isotropic_material::<2>(2.0, -3.0));
        assert!(!is_coercive_isotropic_material::<3>(2.0, -3.0));
        assert!(!is_coercive_isotropic_material::<4>(2.0, 1.0));
        assert!(!is_coercive_isotropic_material::<2>(f64::MAX, f64::MAX));
    }
}
