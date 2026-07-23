use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::{LineMesh, VertexId};

const MIN_PIVOT_SCALE: f64 = 64.0 * f64::EPSILON;

/// Constant-coefficient scalar diffusion on a uniform one-dimensional line.
///
/// The operator advances `u_t = diffusivity * u_xx + source` with centered
/// second differences, Crank–Nicolson time integration, and time-dependent
/// Dirichlet boundary values. It is deliberately a numerical realization,
/// not a canonical heat-equation model.
#[derive(Debug, Clone, PartialEq)]
pub struct Diffusion1d {
    mesh: LineMesh,
    diffusivity: f64,
}

impl Diffusion1d {
    /// Construct a scalar diffusion operator.
    ///
    /// # Errors
    /// Returns `EQ0801` when the mesh is not uniform with at least two cells,
    /// or diffusivity is not finite and strictly positive.
    pub fn new(mesh: LineMesh, diffusivity: f64) -> Result<Self, Diagnostic> {
        if mesh.cell_count() < 2 || mesh.uniform_spacing().is_none() {
            return Err(invalid(
                "Diffusion1d requires a uniform mesh with at least two cells",
            ));
        }
        if !diffusivity.is_finite() || diffusivity <= 0.0 {
            return Err(invalid("diffusivity must be finite and positive"));
        }
        Ok(Self { mesh, diffusivity })
    }

    /// Underlying validated uniform mesh.
    #[must_use]
    pub const fn mesh(&self) -> &LineMesh {
        &self.mesh
    }

    /// Constant diffusion coefficient.
    #[must_use]
    pub const fn diffusivity(&self) -> f64 {
        self.diffusivity
    }

    /// Advance one Crank–Nicolson step with Dirichlet boundary values.
    ///
    /// `current` contains both endpoint values at `time`. `source(x, t)` and
    /// `boundary(t)` are sampled at both ends of the time interval.
    ///
    /// # Errors
    /// Returns `EQ0801` for invalid state/time/callback data and `EQ0802` when
    /// the tridiagonal system is singular or produces a non-finite result.
    pub fn crank_nicolson_step<S, B>(
        &self,
        current: &[f64],
        time: f64,
        dt: f64,
        source: S,
        boundary: B,
    ) -> Result<Vec<f64>, Diagnostic>
    where
        S: Fn(f64, f64) -> f64,
        B: Fn(f64) -> (f64, f64),
    {
        if current.len() != self.mesh.vertex_count() {
            return Err(invalid(format!(
                "diffusion state has {} points; expected {}",
                current.len(),
                self.mesh.vertex_count()
            )));
        }
        if current.iter().any(|value| !value.is_finite()) {
            return Err(invalid("diffusion state values must be finite"));
        }
        if !time.is_finite() || !dt.is_finite() || dt <= 0.0 {
            return Err(invalid("step time must be finite and dt must be positive"));
        }
        let next_time = time + dt;
        if !next_time.is_finite() {
            return Err(invalid("step end time must be finite"));
        }

        let (left_old, right_old) = boundary(time);
        let (left_new, right_new) = boundary(next_time);
        if [left_old, right_old, left_new, right_new]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(invalid("Dirichlet boundary values must be finite"));
        }
        let boundary_scale = current[0]
            .abs()
            .max(current[current.len() - 1].abs())
            .max(left_old.abs())
            .max(right_old.abs())
            .max(1.0);
        let boundary_tolerance = MIN_PIVOT_SCALE * boundary_scale;
        if (current[0] - left_old).abs() > boundary_tolerance
            || (current[current.len() - 1] - right_old).abs() > boundary_tolerance
        {
            return Err(invalid(
                "state endpoints do not match the Dirichlet boundary at step time",
            ));
        }

        let spacing = self
            .mesh
            .uniform_spacing()
            .expect("Diffusion1d constructor validates a uniform mesh");
        let ratio = self.diffusivity * dt / spacing.powi(2);
        if !ratio.is_finite() || ratio <= 0.0 {
            return Err(invalid(
                "dimensionless diffusion step ratio must be finite and positive",
            ));
        }

        let interior = self.mesh.cell_count() - 1;
        let off_diagonal = -0.5 * ratio;
        let mut diagonal = vec![1.0 + ratio; interior];
        let lower = vec![off_diagonal; interior.saturating_sub(1)];
        let upper = lower.clone();
        let mut rhs = Vec::with_capacity(interior);

        for interior_index in 0..interior {
            let point = interior_index + 1;
            let x = self
                .mesh
                .vertex_coordinate(VertexId::new(point))
                .expect("interior vertex is in mesh");
            let source_old = source(x, time);
            let source_new = source(x, next_time);
            if !source_old.is_finite() || !source_new.is_finite() {
                return Err(invalid("diffusion source values must be finite"));
            }
            let left_value = if point == 1 {
                left_old
            } else {
                current[point - 1]
            };
            let right_value = if point + 1 == self.mesh.cell_count() {
                right_old
            } else {
                current[point + 1]
            };
            rhs.push(
                (1.0 - ratio) * current[point]
                    + 0.5 * ratio * (left_value + right_value)
                    + 0.5 * dt * (source_old + source_new),
            );
        }

        rhs[0] += 0.5 * ratio * left_new;
        rhs[interior - 1] += 0.5 * ratio * right_new;
        solve_tridiagonal(&lower, &mut diagonal, &upper, &mut rhs)?;

        let mut next = Vec::with_capacity(self.mesh.vertex_count());
        next.push(left_new);
        next.extend(rhs);
        next.push(right_new);
        if next.iter().any(|value| !value.is_finite()) {
            return Err(solve_failed("diffusion step produced a non-finite state"));
        }
        Ok(next)
    }
}

fn solve_tridiagonal(
    lower: &[f64],
    diagonal: &mut [f64],
    upper: &[f64],
    rhs: &mut [f64],
) -> Result<(), Diagnostic> {
    let size = diagonal.len();
    if size == 0 || rhs.len() != size || lower.len() + 1 != size || upper.len() + 1 != size {
        return Err(solve_failed("invalid tridiagonal system dimensions"));
    }
    let scale = diagonal
        .iter()
        .chain(lower)
        .chain(upper)
        .fold(1.0_f64, |acc, value| acc.max(value.abs()));
    let pivot_tolerance = MIN_PIVOT_SCALE * scale;

    for row in 1..size {
        let pivot = diagonal[row - 1];
        if !pivot.is_finite() || pivot.abs() <= pivot_tolerance {
            return Err(solve_failed(
                "tridiagonal solve encountered a singular pivot",
            ));
        }
        let multiplier = lower[row - 1] / pivot;
        diagonal[row] -= multiplier * upper[row - 1];
        rhs[row] -= multiplier * rhs[row - 1];
    }

    let last_pivot = diagonal[size - 1];
    if !last_pivot.is_finite() || last_pivot.abs() <= pivot_tolerance {
        return Err(solve_failed(
            "tridiagonal solve encountered a singular pivot",
        ));
    }
    rhs[size - 1] /= last_pivot;
    for row in (0..size - 1).rev() {
        let pivot = diagonal[row];
        if !pivot.is_finite() || pivot.abs() <= pivot_tolerance {
            return Err(solve_failed(
                "tridiagonal solve encountered a singular pivot",
            ));
        }
        rhs[row] = (rhs[row] - upper[row] * rhs[row + 1]) / pivot;
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}

fn solve_failed(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message)
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    #[test]
    fn rejects_invalid_grid_operator_and_state() {
        assert_eq!(
            Diffusion1d::new(LineMesh::uniform(0.0, 1.0, 1).unwrap(), 0.1)
                .unwrap_err()
                .code(),
            codes::INVALID_DISCRETIZATION,
        );
        let grid = LineMesh::uniform(0.0, 1.0, 10).unwrap();
        assert_eq!(
            Diffusion1d::new(grid.clone(), 0.0).unwrap_err().code(),
            codes::INVALID_DISCRETIZATION
        );
        let operator = Diffusion1d::new(grid, 0.1).unwrap();
        assert_eq!(
            operator
                .crank_nicolson_step(&[0.0; 3], 0.0, 0.1, |_, _| 0.0, |_| (0.0, 0.0))
                .unwrap_err()
                .code(),
            codes::INVALID_DISCRETIZATION
        );
    }

    #[test]
    fn preserves_a_linear_steady_profile() {
        let grid = LineMesh::uniform(-1.0, 2.0, 12).unwrap();
        let operator = Diffusion1d::new(grid, 0.3).unwrap();
        let profile = operator
            .mesh()
            .vertices()
            .map(|vertex| 2.0 * operator.mesh().vertex_coordinate(vertex).unwrap() - 0.5)
            .collect::<Vec<_>>();
        let next = operator
            .crank_nicolson_step(&profile, 0.0, 0.07, |_, _| 0.0, |_| (-2.5, 3.5))
            .unwrap();
        for (actual, expected) in next.iter().zip(profile) {
            assert!((actual - expected).abs() < 2.0e-15);
        }
    }

    #[test]
    fn manufactured_solution_converges_at_second_order_in_space() {
        let coarse = manufactured_error(10);
        let medium = manufactured_error(20);
        let fine = manufactured_error(40);
        let coarse_rate = (coarse / medium).log2();
        let fine_rate = (medium / fine).log2();

        assert!(coarse > medium && medium > fine);
        assert!(coarse_rate > 1.9, "coarse-to-medium rate: {coarse_rate}");
        assert!(fine_rate > 1.9, "medium-to-fine rate: {fine_rate}");
    }

    fn manufactured_error(intervals: usize) -> f64 {
        let diffusivity = 0.1;
        let final_time = 0.1;
        let grid = LineMesh::uniform(0.0, 1.0, intervals).unwrap();
        let operator = Diffusion1d::new(grid, diffusivity).unwrap();
        let spacing = operator.mesh().uniform_spacing().unwrap();
        let dt = 0.2 * spacing.powi(2) / diffusivity;
        let steps = (final_time / dt).round() as usize;
        assert!((steps as f64 * dt - final_time).abs() < 1.0e-14);

        let mut state = operator
            .mesh()
            .vertices()
            .map(|vertex| (PI * operator.mesh().vertex_coordinate(vertex).unwrap()).sin())
            .collect::<Vec<_>>();
        let mut time = 0.0;
        for _ in 0..steps {
            state = operator
                .crank_nicolson_step(&state, time, dt, |_, _| 0.0, |_| (0.0, 0.0))
                .unwrap();
            time += dt;
        }

        let decay = (-diffusivity * PI.powi(2) * final_time).exp();
        let squared_error = state
            .iter()
            .enumerate()
            .map(|(index, actual)| {
                let exact = (PI
                    * operator
                        .mesh()
                        .vertex_coordinate(VertexId::new(index))
                        .unwrap())
                .sin()
                    * decay;
                (actual - exact).powi(2)
            })
            .sum::<f64>();
        (spacing * squared_error).sqrt()
    }
}
