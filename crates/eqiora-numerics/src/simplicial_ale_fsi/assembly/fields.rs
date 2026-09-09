//! Local Field projections and their power normalization.

use super::*;

pub(super) fn local_velocity_coefficients<const D: usize>(
    vertices: &[MeshEntity],
    vertex_values: &[[f64; D]],
    bubble: [f64; D],
) -> Result<Vec<[f64; D]>, Diagnostic> {
    if vertices.len() != D + 1 {
        return Err(invalid(format!(
            "{D}D ALE FSI velocity closure must contain exactly {} vertices",
            D + 1
        )));
    }
    let mut coefficients = Vec::new();
    coefficients
        .try_reserve_exact(D + 2)
        .map_err(|_| invalid("ALE FSI local velocity coefficient allocation failed"))?;
    for vertex in vertices {
        coefficients.push(
            vertex_values.get(vertex.index()).copied().ok_or_else(|| {
                invalid("ALE FSI velocity closure references a missing mesh vertex")
            })?,
        );
    }
    coefficients.push(bubble);
    Ok(coefficients)
}

pub(super) fn local_pressure_coefficients<const D: usize>(
    vertices: &[MeshEntity],
    partition: &FixedReferenceFsiPartition<D>,
    pressure: &[f64],
) -> Result<Vec<f64>, Diagnostic> {
    let values = vertices
        .iter()
        .map(|vertex| {
            let position = partition
                .fluid_vertices()
                .binary_search_by_key(&vertex.index(), |candidate| candidate.index())
                .map_err(|_| {
                    invalid("ALE FSI fluid cell vertex has no canonical pressure position")
                })?;
            pressure.get(position).copied().ok_or_else(|| {
                invalid("ALE FSI pressure field differs from its partition ordering")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() != D + 1 {
        return Err(invalid(format!(
            "{D}D ALE FSI simplex pressure closure must contain {} vertices",
            D + 1
        )));
    }
    Ok(values)
}

pub(super) fn fluid_row_scales<const D: usize>(plan: AleFsiStepPlan<D>) -> Vec<f64> {
    let scale = plan.scale();
    let power = scale.power();
    (0..fluid_local_size::<D>())
        .map(|row| {
            if row < fluid_pressure_offset::<D>() {
                scale.velocity() / power
            } else {
                scale.pressure() / power
            }
        })
        .collect()
}

pub(super) const fn fluid_pressure_offset<const D: usize>() -> usize {
    (D + 2) * D
}

pub(super) const fn fluid_local_size<const D: usize>() -> usize {
    fluid_pressure_offset::<D>() + D + 1
}

pub(super) const fn solid_local_size<const D: usize>() -> usize {
    (D + 1) * D
}
