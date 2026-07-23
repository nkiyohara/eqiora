//! Canonical design coordinates mapped to realization-local mesh velocities.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::{MeshTopology, SimplicialMesh, SpatialDesignCoordinate};

/// One selected design direction represented as vertex velocities.
///
/// The coordinate carries canonical model identity. Velocities are
/// realization-local data in one mesh revision's vertex order; they never
/// become Semantic Kernel nodes or portable mesh topology.
#[derive(Debug, Clone, PartialEq)]
pub struct SimplicialMeshVelocity {
    coordinate: SpatialDesignCoordinate,
    vertex_velocities: Vec<Vec<f64>>,
}

impl SimplicialMeshVelocity {
    /// Construct one finite velocity vector per mesh vertex.
    ///
    /// # Errors
    /// Returns `EQ0803` for an incompatible shape or non-finite velocity.
    pub fn new(
        mesh: &SimplicialMesh,
        coordinate: SpatialDesignCoordinate,
        vertex_velocities: Vec<Vec<f64>>,
    ) -> Result<Self, Diagnostic> {
        let dimension = mesh.topological_dimension();
        if vertex_velocities.len() != mesh.vertices().len()
            || vertex_velocities.iter().any(|velocity| {
                velocity.len() != dimension || velocity.iter().any(|value| !value.is_finite())
            })
        {
            return Err(invalid(format!(
                "simplex mesh velocity requires {} finite vectors of dimension {dimension}",
                mesh.vertices().len(),
            )));
        }
        Ok(Self {
            coordinate,
            vertex_velocities,
        })
    }

    /// Build the coherent affine motion induced by one Cartesian Domain bound.
    ///
    /// Every vertex preserves its normalized coordinate between the accepted
    /// lower/upper bounds. Connectivity and local affine interpolation remain
    /// fixed even when the mesh is not Cartesian-indexed.
    ///
    /// # Errors
    /// Returns `EQ0803` for a non-bound design coordinate, invalid bounds, a
    /// dimension mismatch, or a vertex outside the accepted box.
    pub fn normalized_box_bound(
        mesh: &SimplicialMesh,
        coordinate: SpatialDesignCoordinate,
        bounds: &[[f64; 2]],
    ) -> Result<Self, Diagnostic> {
        let SpatialDesignCoordinate::CartesianBound { axis, side, .. } = coordinate else {
            return Err(invalid(
                "normalized box motion requires one Cartesian Domain-bound coordinate",
            ));
        };
        let dimension = mesh.topological_dimension();
        if bounds.len() != dimension || axis >= dimension {
            return Err(invalid(
                "normalized box motion dimension differs from the simplex mesh",
            ));
        }
        for bound in bounds {
            if !bound[0].is_finite() || !bound[1].is_finite() || bound[1] <= bound[0] {
                return Err(invalid(
                    "normalized box motion requires finite increasing bounds",
                ));
            }
        }
        let mut velocities = vec![vec![0.0; dimension]; mesh.vertices().len()];
        for (vertex, velocity) in mesh.vertices().iter().zip(&mut velocities) {
            for physical_axis in 0..dimension {
                let [lower, upper] = bounds[physical_axis];
                let tolerance = 256.0 * f64::EPSILON * lower.abs().max(upper.abs()).max(1.0);
                if vertex[physical_axis] < lower - tolerance
                    || vertex[physical_axis] > upper + tolerance
                {
                    return Err(invalid(
                        "simplex mesh vertex lies outside the accepted Cartesian Domain",
                    ));
                }
            }
            let [lower, upper] = bounds[axis];
            let normalized = ((vertex[axis] - lower) / (upper - lower)).clamp(0.0, 1.0);
            velocity[axis] = match side {
                eqiora_schema::kernel::BoundarySide::Lower => 1.0 - normalized,
                eqiora_schema::kernel::BoundarySide::Upper => normalized,
            };
        }
        Self::new(mesh, coordinate, velocities)
    }

    /// Canonical design coordinate producing this realization action.
    #[must_use]
    pub const fn coordinate(&self) -> SpatialDesignCoordinate {
        self.coordinate
    }

    /// Vertex velocities in the mesh revision's canonical vertex order.
    #[must_use]
    pub fn vertex_velocities(&self) -> &[Vec<f64>] {
        &self.vertex_velocities
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_MESH, message)
}
