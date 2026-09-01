//! Method-neutral small-deformation continuum kinematics.

/// Infinitesimal strain `sym(grad(u))`.
pub(crate) fn symmetric_gradient<const D: usize>(gradient: &[[f64; D]; D]) -> [[f64; D]; D] {
    std::array::from_fn(|row| {
        std::array::from_fn(|column| 0.5 * (gradient[row][column] + gradient[column][row]))
    })
}

/// The viscous/elastic invariant `2 sym(grad(u)) : sym(grad(u))`.
pub(crate) fn twice_symmetric_gradient_squared_norm<const D: usize>(
    gradient: &[[f64; D]; D],
) -> f64 {
    let mut squared_norm = gradient
        .iter()
        .enumerate()
        .map(|(axis, row)| 2.0 * row[axis].powi(2))
        .sum::<f64>();
    for (row, row_values) in gradient.iter().enumerate() {
        for (column, column_values) in gradient.iter().enumerate().skip(row + 1) {
            squared_norm += (row_values[column] + column_values[row]).powi(2);
        }
    }
    squared_norm
}

/// One component entry of `2 sym(grad(v)) : sym(grad(u))`.
pub(crate) fn symmetric_gradient_bilinear_entry(
    row_gradient: &[f64],
    row_component: usize,
    column_gradient: &[f64],
    column_component: usize,
) -> f64 {
    let component_diagonal = if row_component == column_component {
        row_gradient
            .iter()
            .zip(column_gradient)
            .map(|(left, right)| left * right)
            .sum()
    } else {
        0.0
    };
    component_diagonal + row_gradient[column_component] * column_gradient[row_component]
}

#[cfg(test)]
mod tests {
    use super::{
        symmetric_gradient, symmetric_gradient_bilinear_entry,
        twice_symmetric_gradient_squared_norm,
    };

    #[test]
    fn three_dimensional_symmetric_gradient_includes_every_axis_and_shear_pair() {
        let gradient = [[1.0, 2.0, 3.0], [5.0, 7.0, 11.0], [13.0, 17.0, 19.0]];

        assert_eq!(
            symmetric_gradient(&gradient),
            [[1.0, 3.5, 8.0], [3.5, 7.0, 14.0], [8.0, 14.0, 19.0]]
        );
        assert_eq!(
            twice_symmetric_gradient_squared_norm(&gradient),
            2.0 * (1.0_f64.powi(2) + 7.0_f64.powi(2) + 19.0_f64.powi(2))
                + (2.0_f64 + 5.0).powi(2)
                + (3.0_f64 + 13.0).powi(2)
                + (11.0_f64 + 17.0).powi(2)
        );
    }

    #[test]
    fn bilinear_entry_retains_component_diagonal_and_crossed_terms() {
        let row = [2.0, 3.0, 5.0];
        let column = [7.0, 11.0, 13.0];

        assert_eq!(
            symmetric_gradient_bilinear_entry(&row, 1, &column, 1),
            2.0 * 7.0 + 3.0 * 11.0 + 5.0 * 13.0 + 3.0 * 11.0
        );
        assert_eq!(
            symmetric_gradient_bilinear_entry(&row, 0, &column, 2),
            row[2] * column[0]
        );
    }
}
