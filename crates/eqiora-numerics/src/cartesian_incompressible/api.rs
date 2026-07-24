use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_meshing::{MeshEntity, MeshGeometry, MeshTopology};

use crate::cartesian_mesh::CartesianMesh;

const DIMENSION: usize = 2;

/// Cell-centered physical velocity reconstructed on a two-dimensional Cartesian mesh.
///
/// Values follow the mesh's canonical top-cell order. This type records a physical
/// field only; it does not prescribe how the values were discretized or obtained.
#[derive(Debug, Clone, PartialEq)]
pub struct CellCenteredVelocityField2d {
    mesh: CartesianMesh,
    values: Vec<[f64; DIMENSION]>,
}

impl CellCenteredVelocityField2d {
    /// Bind finite Cartesian velocity values to every top cell of `mesh`.
    ///
    /// # Errors
    /// Returns `EQ0801` unless the mesh is exactly two-dimensional, the value
    /// count equals its top-cell count, and every component is finite.
    pub fn new(mesh: CartesianMesh, values: Vec<[f64; DIMENSION]>) -> Result<Self, Diagnostic> {
        validate_mesh_and_count(&mesh, values.len(), "velocity")?;
        if let Some((cell, component)) = values.iter().enumerate().find_map(|(cell, value)| {
            value
                .iter()
                .position(|component| !component.is_finite())
                .map(|component| (cell, component))
        }) {
            return Err(invalid_field(
                "velocity",
                format!(
                    "cell-centered velocity component {component} at canonical cell {cell} must be finite"
                ),
            ));
        }
        Ok(Self { mesh, values })
    }

    /// Mesh carrying the field.
    #[must_use]
    pub const fn mesh(&self) -> &CartesianMesh {
        &self.mesh
    }

    /// Velocity values in canonical top-cell order.
    #[must_use]
    pub fn values(&self) -> &[[f64; DIMENSION]] {
        &self.values
    }

    /// Compute `sqrt(sum(cell_measure * velocity dot velocity))`.
    ///
    /// # Errors
    /// Returns `EQ0801` if the mesh geometry is unavailable or a geometric or
    /// accumulated value is not finite and positive where required.
    pub fn volume_l2_norm(&self) -> Result<f64, Diagnostic> {
        let mut squared_norm = 0.0;
        for (cell, value) in self.values.iter().enumerate() {
            let measure = cell_measure_2d(&self.mesh, cell, "velocity")?;
            let magnitude_squared = value[0].mul_add(value[0], value[1] * value[1]);
            squared_norm += measure * magnitude_squared;
        }
        if !squared_norm.is_finite() || squared_norm < 0.0 {
            return Err(invalid_field(
                "velocity",
                "cell-centered velocity volume norm must remain finite and non-negative",
            ));
        }
        Ok(squared_norm.sqrt())
    }
}

/// Cell-centered physical pressure reconstructed on a two-dimensional Cartesian mesh.
///
/// Values follow the mesh's canonical top-cell order. This type records a physical
/// field only; it does not prescribe how the values were discretized or obtained.
#[derive(Debug, Clone, PartialEq)]
pub struct CellCenteredPressureField2d {
    mesh: CartesianMesh,
    values: Vec<f64>,
}

impl CellCenteredPressureField2d {
    /// Bind one finite pressure value to every top cell of `mesh`.
    ///
    /// # Errors
    /// Returns `EQ0801` unless the mesh is exactly two-dimensional, the value
    /// count equals its top-cell count, and every value is finite.
    pub fn new(mesh: CartesianMesh, values: Vec<f64>) -> Result<Self, Diagnostic> {
        validate_mesh_and_count(&mesh, values.len(), "pressure")?;
        if let Some(cell) = values.iter().position(|value| !value.is_finite()) {
            return Err(invalid_field(
                "pressure",
                format!("cell-centered pressure at canonical cell {cell} must be finite"),
            ));
        }
        Ok(Self { mesh, values })
    }

    /// Mesh carrying the field.
    #[must_use]
    pub const fn mesh(&self) -> &CartesianMesh {
        &self.mesh
    }

    /// Pressure values in canonical top-cell order.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Compute the geometry-weighted pressure integral over all cells.
    ///
    /// # Errors
    /// Returns `EQ0801` if the mesh geometry is unavailable or a geometric or
    /// accumulated value is not finite and positive where required.
    pub fn volume_integral(&self) -> Result<f64, Diagnostic> {
        let mut integral = 0.0;
        for (cell, value) in self.values.iter().enumerate() {
            integral += cell_measure_2d(&self.mesh, cell, "pressure")? * value;
        }
        if !integral.is_finite() {
            return Err(invalid_field(
                "pressure",
                "cell-centered pressure volume integral must remain finite",
            ));
        }
        Ok(integral)
    }
}

fn validate_mesh_and_count(
    mesh: &CartesianMesh,
    value_count: usize,
    field: &'static str,
) -> Result<(), Diagnostic> {
    let dimension = mesh.topological_dimension();
    if dimension != DIMENSION {
        return Err(invalid_field(
            field,
            format!(
                "cell-centered {field} requires a two-dimensional mesh, received dimension {dimension}"
            ),
        ));
    }
    let cell_count = mesh.entity_count(DIMENSION).ok_or_else(|| {
        invalid_field(
            field,
            "two-dimensional Cartesian mesh has no top-cell stratum",
        )
    })?;
    if value_count != cell_count {
        return Err(invalid_field(
            field,
            format!(
                "cell-centered {field} requires {cell_count} canonical top-cell values, received {value_count}"
            ),
        ));
    }
    Ok(())
}

fn cell_measure_2d(
    mesh: &CartesianMesh,
    cell: usize,
    field: &'static str,
) -> Result<f64, Diagnostic> {
    let geometry = mesh
        .geometry_map(MeshEntity::new(DIMENSION, cell))
        .ok_or_else(|| {
            invalid_field(
                field,
                format!("Cartesian geometry is unavailable for canonical cell {cell}"),
            )
        })?;
    let measure = 4.0 * geometry.measure_scale();
    if !measure.is_finite() || measure <= 0.0 {
        return Err(invalid_field(
            field,
            format!("Cartesian measure for canonical cell {cell} must be finite and positive"),
        ));
    }
    Ok(measure)
}

fn invalid_field(field: &'static str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "cartesian-incompressible".to_owned(),
        field.to_owned(),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonuniform_mesh() -> CartesianMesh {
        CartesianMesh::from_axes(vec![vec![0.0, 1.0, 3.0], vec![-1.0, 1.0]])
            .expect("test mesh is valid")
    }

    #[test]
    fn fields_preserve_canonical_values_and_apply_cell_measures() {
        let velocity =
            CellCenteredVelocityField2d::new(nonuniform_mesh(), vec![[1.0, 0.0], [0.0, 2.0]])
                .expect("velocity field is valid");
        let pressure = CellCenteredPressureField2d::new(nonuniform_mesh(), vec![1.0, 2.0])
            .expect("pressure field is valid");

        assert_eq!(velocity.values(), &[[1.0, 0.0], [0.0, 2.0]]);
        assert_eq!(pressure.values(), &[1.0, 2.0]);
        assert!((velocity.volume_l2_norm().unwrap() - 18.0_f64.sqrt()).abs() < 1.0e-14);
        assert_eq!(pressure.volume_integral().unwrap(), 10.0);
    }

    #[test]
    fn constructors_reject_dimension_count_and_non_finite_data() {
        let one_dimensional = CartesianMesh::uniform(&[[0.0, 1.0]], &[1])
            .expect("one-dimensional test mesh is valid");
        let dimension_error =
            CellCenteredPressureField2d::new(one_dimensional, vec![0.0]).unwrap_err();
        assert_eq!(dimension_error.code(), codes::INVALID_DISCRETIZATION);

        let count_error =
            CellCenteredPressureField2d::new(nonuniform_mesh(), vec![0.0]).unwrap_err();
        assert_eq!(count_error.code(), codes::INVALID_DISCRETIZATION);

        let pressure_error =
            CellCenteredPressureField2d::new(nonuniform_mesh(), vec![0.0, f64::NAN]).unwrap_err();
        assert_eq!(pressure_error.code(), codes::INVALID_DISCRETIZATION);

        let velocity_error = CellCenteredVelocityField2d::new(
            nonuniform_mesh(),
            vec![[0.0, 0.0], [f64::INFINITY, 0.0]],
        )
        .unwrap_err();
        assert_eq!(velocity_error.code(), codes::INVALID_DISCRETIZATION);
    }
}
