use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::QuadratureRule;

use super::{ScalarEllipticMethod, ScalarEllipticRunResult, ScalarFieldLocation};

impl ScalarEllipticRunResult {
    /// Quadrature used by accepted-field error metrics.
    ///
    /// This is the tensor product of three-point Gauss--Legendre rules on the
    /// two reference axes. Its nine points integrate polynomials through
    /// degree five on each axis exactly. Callers can inspect
    /// [`QuadratureRule::polynomial_exactness`] and the complete point set;
    /// the rule is returned rather than being an implicit implementation
    /// choice.
    ///
    /// # Errors
    /// Returns `EQ0809` unless this result publishes the supported
    /// two-dimensional, vertex-located Cartesian Q1 field layout.
    pub fn error_quadrature(&self) -> Result<QuadratureRule, Diagnostic> {
        CartesianQ1Layout2d::from_result(self)?;
        scalar_elliptic_error_quadrature()
    }

    /// Continuum L2 error of the accepted Q1 field against an exact solution.
    ///
    /// `exact_solution` receives coherent-SI physical coordinates in the
    /// published Cartesian axis order. The integral uses
    /// [`Self::error_quadrature`], which is exact when the squared pointwise
    /// difference has degree at most five on each cell axis.
    ///
    /// # Errors
    /// Returns `EQ0809` for an unsupported accepted field layout, or `EQ0801`
    /// if the exact solution or accumulated error is non-finite.
    pub fn l2_error<E>(&self, exact_solution: E) -> Result<f64, Diagnostic>
    where
        E: Fn([f64; 2]) -> f64,
    {
        let mut squared_error = 0.0;
        self.for_each_accepted_field_error(&exact_solution, |error, measure| {
            squared_error += measure * error * error;
        })?;
        if !squared_error.is_finite() || squared_error < 0.0 {
            return Err(scalar_elliptic_metric_error(
                "accepted scalar field L2 error accumulation is non-finite or negative",
            ));
        }
        Ok(squared_error.sqrt())
    }

    /// Quadrature-sampled L-infinity error of the accepted Q1 field.
    ///
    /// `exact_solution` receives coherent-SI physical coordinates in the
    /// published Cartesian axis order. This is the largest pointwise error at
    /// the nine per-cell points returned by [`Self::error_quadrature`]; a
    /// finite rule cannot claim the essential supremum of an arbitrary caller
    /// function between those points.
    ///
    /// # Errors
    /// Returns `EQ0809` for an unsupported accepted field layout, or `EQ0801`
    /// if the exact solution or sampled error is non-finite.
    pub fn quadrature_l_infinity_error<E>(&self, exact_solution: E) -> Result<f64, Diagnostic>
    where
        E: Fn([f64; 2]) -> f64,
    {
        let mut maximum_error = 0.0_f64;
        self.for_each_accepted_field_error(&exact_solution, |error, _measure| {
            maximum_error = maximum_error.max(error);
        })?;
        Ok(maximum_error)
    }

    fn for_each_accepted_field_error<E, F>(
        &self,
        exact_solution: &E,
        mut observe: F,
    ) -> Result<(), Diagnostic>
    where
        E: Fn([f64; 2]) -> f64,
        F: FnMut(f64, f64),
    {
        let layout = CartesianQ1Layout2d::from_result(self)?;
        let quadrature = scalar_elliptic_error_quadrature()?;
        let values = self.field_values();
        let stride = layout.cells[1] + 1;

        for slow_cell in 0..layout.cells[0] {
            for fast_cell in 0..layout.cells[1] {
                let corner = slow_cell * stride + fast_cell;
                let cell = [
                    values[corner],
                    values[corner + 1],
                    values[corner + stride],
                    values[corner + stride + 1],
                ];
                for point in quadrature.points() {
                    let slow_offset = 0.5 * (point.coordinates[0] + 1.0);
                    let fast_offset = 0.5 * (point.coordinates[1] + 1.0);
                    let approximation = (1.0 - slow_offset) * (1.0 - fast_offset) * cell[0]
                        + (1.0 - slow_offset) * fast_offset * cell[1]
                        + slow_offset * (1.0 - fast_offset) * cell[2]
                        + slow_offset * fast_offset * cell[3];
                    let coordinates = [
                        layout.coordinate(0, slow_cell, slow_offset),
                        layout.coordinate(1, fast_cell, fast_offset),
                    ];
                    let exact = exact_solution(coordinates);
                    if !exact.is_finite() {
                        return Err(scalar_elliptic_metric_error(
                            "exact scalar solution returned a non-finite value",
                        ));
                    }
                    let error = (approximation - exact).abs();
                    if !error.is_finite() {
                        return Err(scalar_elliptic_metric_error(
                            "accepted scalar field error is non-finite",
                        ));
                    }
                    observe(error, point.weight * layout.cell_jacobian);
                }
            }
        }
        Ok(())
    }
}

struct CartesianQ1Layout2d {
    cells: [usize; 2],
    origin: [f64; 2],
    step: [f64; 2],
    cell_jacobian: f64,
}

impl CartesianQ1Layout2d {
    fn from_result(result: &ScalarEllipticRunResult) -> Result<Self, Diagnostic> {
        let projection = result.plan().field_projection();
        let shape = projection.logical_shape();
        if result.plan().intent().method() != ScalarEllipticMethod::FiniteElement
            || projection.location() != ScalarFieldLocation::Vertex
            || shape.len() != 2
        {
            return Err(unsupported_scalar_elliptic_metric_layout(
                "accepted-field error metrics require a two-dimensional vertex-located Cartesian Q1 field",
            ));
        }
        let cells = [
            shape[0].checked_sub(1).ok_or_else(|| {
                unsupported_scalar_elliptic_metric_layout(
                    "accepted Cartesian Q1 field has no cells on axis zero",
                )
            })?,
            shape[1].checked_sub(1).ok_or_else(|| {
                unsupported_scalar_elliptic_metric_layout(
                    "accepted Cartesian Q1 field has no cells on axis one",
                )
            })?,
        ];
        if cells.contains(&0) {
            return Err(unsupported_scalar_elliptic_metric_layout(
                "accepted Cartesian Q1 field requires at least one cell on each axis",
            ));
        }
        let expected_values = shape[0].checked_mul(shape[1]).ok_or_else(|| {
            unsupported_scalar_elliptic_metric_layout(
                "accepted Cartesian Q1 field value count overflows its logical shape",
            )
        })?;
        if projection.value_count() != expected_values
            || result.field().value_count() != expected_values
            || result.field_values().len() != expected_values
        {
            return Err(unsupported_scalar_elliptic_metric_layout(
                "accepted Cartesian Q1 field values do not match its published logical shape",
            ));
        }

        let bounds = projection.bounds();
        let mut origin = [0.0; 2];
        let mut step = [0.0; 2];
        for axis in 0..2 {
            let [lower, upper] = bounds[axis];
            if !lower.is_finite() || !upper.is_finite() || upper <= lower {
                return Err(unsupported_scalar_elliptic_metric_layout(
                    "accepted Cartesian Q1 field requires finite increasing bounds on each axis",
                ));
            }
            origin[axis] = lower;
            step[axis] = (upper - lower) / cells[axis] as f64;
            if !step[axis].is_finite() || step[axis] <= 0.0 {
                return Err(unsupported_scalar_elliptic_metric_layout(
                    "accepted Cartesian Q1 field has an invalid physical cell size",
                ));
            }
        }
        let cell_jacobian = 0.25 * step[0] * step[1];
        if !cell_jacobian.is_finite() || cell_jacobian <= 0.0 {
            return Err(unsupported_scalar_elliptic_metric_layout(
                "accepted Cartesian Q1 field has an invalid cell Jacobian",
            ));
        }
        Ok(Self {
            cells,
            origin,
            step,
            cell_jacobian,
        })
    }

    fn coordinate(&self, axis: usize, cell: usize, offset: f64) -> f64 {
        self.origin[axis] + (cell as f64 + offset) * self.step[axis]
    }
}

fn scalar_elliptic_error_quadrature() -> Result<QuadratureRule, Diagnostic> {
    QuadratureRule::tensor_product_gauss_legendre(2, 3)
}

fn unsupported_scalar_elliptic_metric_layout(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETE_FIELD, message)
}

fn scalar_elliptic_metric_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use eqiora_realization::RealizationRevision;

    use super::*;
    use crate::{ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent};

    const POISSON_2D: &str =
        include_str!("../../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");
    const POISSON_1D: &str =
        include_str!("../../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");
    const ZERO_POISSON_2D: &str = r#"
model zero_poisson_plane {
  domain square = box(0, 1, 0, 1);
  domain x_lower = boundary(square, axis = 0, side = lower);
  domain x_upper = boundary(square, axis = 0, side = upper);
  domain y_lower = boundary(square, axis = 1, side = lower);
  domain y_upper = boundary(square, axis = 1, side = upper);
  representation scalar_space = continuum;

  field potential on square as scalar_space: 1 = 0;
  parameter source_scale: 1 / m ^ 2 = 0;

  relation balance continuous on square {
    -div(grad(potential)) - source_scale = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) = 0; }
}
"#;
    const CONSTANT_POISSON_2D: &str = r#"
model constant_poisson_plane {
  domain square = box(0, 1, 0, 1);
  domain x_lower = boundary(square, axis = 0, side = lower);
  domain x_upper = boundary(square, axis = 0, side = upper);
  domain y_lower = boundary(square, axis = 1, side = lower);
  domain y_upper = boundary(square, axis = 1, side = upper);
  representation scalar_space = continuum;

  field potential on square as scalar_space: 1 = 1;
  parameter source_scale: 1 / m ^ 2 = 0;

  relation balance continuous on square {
    -div(grad(potential)) - source_scale = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) - 1 = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) - 1 = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) - 1 = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) - 1 = 0; }
}
"#;

    fn document() -> ModelDocument {
        ModelDocument::compile("poisson.eqi", POISSON_2D).unwrap()
    }

    fn zero_document() -> ModelDocument {
        ModelDocument::compile("zero-poisson.eqi", ZERO_POISSON_2D).unwrap()
    }

    fn constant_document() -> ModelDocument {
        ModelDocument::compile("constant-poisson.eqi", CONSTANT_POISSON_2D).unwrap()
    }

    fn intent(method: ScalarEllipticMethod, cells: usize) -> ScalarEllipticIntent {
        ScalarEllipticIntent::new(
            RealizationRevision::new(7),
            method,
            NonZeroUsize::new(cells).unwrap(),
            NonZeroUsize::MIN,
        )
    }

    fn run_result(
        document: &ModelDocument,
        method: ScalarEllipticMethod,
        cells: usize,
    ) -> ScalarEllipticRunResult {
        let environment = ScalarEllipticExecutionEnvironment::host_serial();
        document
            .run_scalar_elliptic_plan(
                document
                    .preview_scalar_elliptic_run(intent(method, cells), environment)
                    .unwrap(),
                environment,
            )
            .unwrap()
    }

    #[test]
    fn accepted_field_l2_matches_the_retired_poisson_example_to_ten_significant_figures() {
        let result = run_result(&document(), ScalarEllipticMethod::FiniteElement, 16);
        let reported = result
            .l2_error(|[x, y]| (std::f64::consts::PI * x).sin() * (std::f64::consts::PI * y).sin())
            .unwrap();
        let retired = retired_poisson_l2_error(&result);
        let relative_difference = (reported - retired).abs() / retired;

        assert!(
            relative_difference <= 1.0e-10,
            "reported {reported:.16e}, retired {retired:.16e}"
        );
        assert_eq!(format!("{reported:.6e}"), "1.899742e-3");
    }

    #[test]
    fn represented_constant_has_machine_zero_accepted_field_error() {
        let result = run_result(&constant_document(), ScalarEllipticMethod::FiniteElement, 2);

        // One is a nonzero degree-zero polynomial in Q1, and its squared error
        // is inside the rule's declared degree-five-per-axis exactness.
        assert_eq!(result.l2_error(|_| 1.0).unwrap(), 0.0);
        assert_eq!(result.quadrature_l_infinity_error(|_| 1.0).unwrap(), 0.0);
    }

    #[test]
    fn accepted_field_l2_converges_at_the_q1_order() {
        let document = document();
        let exact =
            |[x, y]: [f64; 2]| (std::f64::consts::PI * x).sin() * (std::f64::consts::PI * y).sin();
        let coarse = run_result(&document, ScalarEllipticMethod::FiniteElement, 8)
            .l2_error(exact)
            .unwrap();
        let fine = run_result(&document, ScalarEllipticMethod::FiniteElement, 16)
            .l2_error(exact)
            .unwrap();
        let observed_order = (coarse / fine).log2();

        // The independently fixed ±0.2 band admits finite-mesh asymptotic
        // contamination while separating Q1's L2 order two from order one.
        assert!(
            (1.8..=2.2).contains(&observed_order),
            "observed L2 order was {observed_order}"
        );
    }

    #[test]
    fn quadrature_sampled_l_infinity_distinguishes_a_local_peak_from_l2() {
        let result = run_result(&zero_document(), ScalarEllipticMethod::FiniteElement, 2);
        let peak = |[x, y]: [f64; 2]| 16.0 * x * (1.0 - x) * y * (1.0 - y);
        let l2 = result.l2_error(peak).unwrap();
        let l_infinity = result.quadrature_l_infinity_error(peak).unwrap();

        assert!(l_infinity > l2);
        assert!(l_infinity >= l2);
    }

    #[test]
    fn accepted_field_error_fails_closed_for_unsupported_location_and_shape() {
        let cell_centered = run_result(&zero_document(), ScalarEllipticMethod::FiniteVolume, 2);
        let location_error = cell_centered.l2_error(|_| 0.0).unwrap_err();
        assert_eq!(location_error.code(), codes::INVALID_DISCRETE_FIELD);
        assert!(
            location_error
                .message()
                .contains("two-dimensional vertex-located Cartesian Q1")
        );

        let line = ModelDocument::compile("poisson-1d.eqi", POISSON_1D).unwrap();
        let one_dimensional = run_result(&line, ScalarEllipticMethod::FiniteElement, 4);
        let shape_error = one_dimensional
            .quadrature_l_infinity_error(|_| 0.0)
            .unwrap_err();
        assert_eq!(shape_error.code(), codes::INVALID_DISCRETE_FIELD);
        assert!(
            shape_error
                .message()
                .contains("two-dimensional vertex-located Cartesian Q1")
        );
    }

    #[test]
    fn accepted_field_error_quadrature_is_publicly_readable() {
        let result = run_result(&zero_document(), ScalarEllipticMethod::FiniteElement, 2);
        let rule = result.error_quadrature().unwrap();

        assert_eq!(
            rule,
            QuadratureRule::tensor_product_gauss_legendre(2, 3).unwrap()
        );
        assert_eq!(rule.polynomial_exactness(), Some(5));
        assert_eq!(rule.points().len(), 9);
    }

    fn retired_poisson_l2_error(result: &ScalarEllipticRunResult) -> f64 {
        const NODES: [f64; 3] = [0.112_701_665_379_258_3, 0.5, 0.887_298_334_620_741_7];
        const WEIGHTS: [f64; 3] = [
            0.277_777_777_777_777_8,
            0.444_444_444_444_444_4,
            0.277_777_777_777_777_8,
        ];

        let projection = result.plan().field_projection();
        let shape = projection.logical_shape();
        let bounds = projection.bounds();
        assert_eq!(projection.location(), ScalarFieldLocation::Vertex);
        assert_eq!(shape.len(), 2);
        let cells = [shape[0] - 1, shape[1] - 1];
        let origin = [bounds[0][0], bounds[1][0]];
        let step = [
            (bounds[0][1] - bounds[0][0]) / cells[0] as f64,
            (bounds[1][1] - bounds[1][0]) / cells[1] as f64,
        ];
        let values = result.field_values();
        let stride = cells[1] + 1;
        let mut squared = 0.0;

        for slow in 0..cells[0] {
            for fast in 0..cells[1] {
                let corner = slow * stride + fast;
                let cell = [
                    values[corner],
                    values[corner + 1],
                    values[corner + stride],
                    values[corner + stride + 1],
                ];
                for (offset_slow, weight_slow) in NODES.iter().zip(WEIGHTS) {
                    for (offset_fast, weight_fast) in NODES.iter().zip(WEIGHTS) {
                        let approximation = (1.0 - offset_slow) * (1.0 - offset_fast) * cell[0]
                            + (1.0 - offset_slow) * offset_fast * cell[1]
                            + offset_slow * (1.0 - offset_fast) * cell[2]
                            + offset_slow * offset_fast * cell[3];
                        let x = origin[0] + (slow as f64 + offset_slow) * step[0];
                        let y = origin[1] + (fast as f64 + offset_fast) * step[1];
                        let exact =
                            (std::f64::consts::PI * x).sin() * (std::f64::consts::PI * y).sin();
                        squared += weight_slow
                            * weight_fast
                            * (approximation - exact)
                            * (approximation - exact);
                    }
                }
            }
        }
        (squared * step[0] * step[1]).sqrt()
    }
}
