//! Method-neutral algebra shared by affine finite-element local operators.

pub(crate) fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

pub(crate) fn physical_gradient(
    reference: &[f64],
    inverse_jacobian: &[f64],
    dimension: usize,
) -> Vec<f64> {
    (0..dimension)
        .map(|physical| {
            (0..dimension)
                .map(|reference_axis| {
                    inverse_jacobian[reference_axis * dimension + physical]
                        * reference[reference_axis]
                })
                .sum()
        })
        .collect()
}

pub(crate) fn weighted_gradient(values: &[f64], gradients: &[Vec<f64>]) -> Vec<f64> {
    let dimension = gradients.first().map_or(0, Vec::len);
    let mut result = vec![0.0; dimension];
    for (value, gradient) in values.iter().zip(gradients) {
        for (entry, gradient) in result.iter_mut().zip(gradient) {
            *entry += value * gradient;
        }
    }
    result
}

pub(crate) fn weighted_gradient_tangent(
    values: &[f64],
    value_tangents: &[f64],
    gradients: &[Vec<f64>],
    gradient_tangents: &[Vec<f64>],
) -> Vec<f64> {
    let dimension = gradients.first().map_or(0, Vec::len);
    let mut result = vec![0.0; dimension];
    for (((value, value_tangent), gradient), gradient_tangent) in values
        .iter()
        .zip(value_tangents)
        .zip(gradients)
        .zip(gradient_tangents)
    {
        for ((entry, gradient), gradient_tangent) in
            result.iter_mut().zip(gradient).zip(gradient_tangent)
        {
            *entry += value_tangent * gradient + value * gradient_tangent;
        }
    }
    result
}
