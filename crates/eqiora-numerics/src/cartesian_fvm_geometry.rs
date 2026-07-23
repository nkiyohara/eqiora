use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_schema::kernel::BoundarySide;

use crate::{CartesianMesh, MeshEntity, MeshGeometry, MeshTopology};

const DIMENSION: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CartesianCellMetrics2d {
    pub(crate) center: [f64; DIMENSION],
    pub(crate) measure: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CartesianFacetAdjacency2d {
    Interior {
        lower: usize,
        upper: usize,
        center_distance: f64,
    },
    Boundary {
        cell: usize,
        side: BoundarySide,
        center_distance: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CartesianFacetMetrics2d {
    pub(crate) center: [f64; DIMENSION],
    pub(crate) measure: f64,
    pub(crate) normal_axis: usize,
    pub(crate) adjacency: CartesianFacetAdjacency2d,
}

/// Traverse a 2D Cartesian mesh without assigning equation or boundary roles.
///
/// Cell/facet order, adjacency, normal axis, and positive metric data are
/// shared numerical geometry. Every physical flux remains with its owning
/// realization.
pub(crate) fn cartesian_fvm_geometry_2d(
    mesh: &CartesianMesh,
) -> Result<(Vec<CartesianCellMetrics2d>, Vec<CartesianFacetMetrics2d>), Diagnostic> {
    let cell_count = mesh
        .entity_count(DIMENSION)
        .expect("2D Cartesian mesh owns top cells");
    let mut cells = Vec::with_capacity(cell_count);
    for index in 0..cell_count {
        let entity = MeshEntity::new(DIMENSION, index);
        let center = point2(
            mesh.entity_center(entity)
                .ok_or_else(|| invalid_numerics("Cartesian FVM cell center is unavailable"))?,
        )?;
        let geometry = mesh
            .geometry_map(entity)
            .ok_or_else(|| invalid_numerics("Cartesian FVM cell geometry is unavailable"))?;
        let measure = 4.0 * geometry.measure_scale();
        if !measure.is_finite() || measure <= 0.0 {
            return Err(invalid_numerics(
                "Cartesian FVM cell measure must be finite and positive",
            ));
        }
        cells.push(CartesianCellMetrics2d { center, measure });
    }

    let facet_count = mesh
        .entity_count(DIMENSION - 1)
        .expect("2D Cartesian mesh owns facets");
    let mut facets = Vec::with_capacity(facet_count);
    for index in 0..facet_count {
        let facet = MeshEntity::new(DIMENSION - 1, index);
        let center = point2(
            mesh.entity_center(facet)
                .ok_or_else(|| invalid_numerics("Cartesian FVM facet center is unavailable"))?,
        )?;
        let geometry = mesh
            .geometry_map(facet)
            .ok_or_else(|| invalid_numerics("Cartesian FVM facet geometry is unavailable"))?;
        let measure = 2.0 * geometry.measure_scale();
        if !measure.is_finite() || measure <= 0.0 {
            return Err(invalid_numerics(
                "Cartesian FVM facet measure must be finite and positive",
            ));
        }
        let free_axes = mesh
            .entity_free_axes(facet)
            .ok_or_else(|| invalid_numerics("Cartesian FVM facet axes are unavailable"))?;
        let normal_axis = (0..DIMENSION)
            .find(|axis| free_axes.binary_search(axis).is_err())
            .ok_or_else(|| invalid_numerics("Cartesian FVM facet has no normal axis"))?;
        let adjacent = mesh
            .incidence(facet, DIMENSION)
            .ok_or_else(|| invalid_numerics("Cartesian FVM facet adjacency is unavailable"))?;
        let adjacency = match adjacent.as_slice() {
            [first, second] => {
                let first = first.entity.index();
                let second = second.entity.index();
                let (lower, upper) =
                    if cells[first].center[normal_axis] < cells[second].center[normal_axis] {
                        (first, second)
                    } else {
                        (second, first)
                    };
                let center_distance =
                    cells[upper].center[normal_axis] - cells[lower].center[normal_axis];
                require_positive_distance(center_distance, "interior")?;
                CartesianFacetAdjacency2d::Interior {
                    lower,
                    upper,
                    center_distance,
                }
            }
            [cell] => {
                let cell = cell.entity.index();
                let side = if center[normal_axis] < cells[cell].center[normal_axis] {
                    BoundarySide::Lower
                } else {
                    BoundarySide::Upper
                };
                let center_distance = (cells[cell].center[normal_axis] - center[normal_axis]).abs();
                require_positive_distance(center_distance, "boundary")?;
                CartesianFacetAdjacency2d::Boundary {
                    cell,
                    side,
                    center_distance,
                }
            }
            _ => {
                return Err(invalid_numerics(
                    "Cartesian FVM facet requires one or two adjacent cells",
                ));
            }
        };
        facets.push(CartesianFacetMetrics2d {
            center,
            measure,
            normal_axis,
            adjacency,
        });
    }
    Ok((cells, facets))
}

fn require_positive_distance(value: f64, role: &str) -> Result<(), Diagnostic> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(invalid_numerics(format!(
            "Cartesian FVM {role} center distance must be finite and positive"
        )))
    }
}

fn point2(point: Vec<f64>) -> Result<[f64; DIMENSION], Diagnostic> {
    point.try_into().map_err(|point: Vec<_>| {
        invalid_numerics(format!(
            "Cartesian FVM geometry requires two coordinates, received {}",
            point.len()
        ))
    })
}

fn invalid_numerics(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "cartesian-fvm-geometry-2d".to_owned(),
    ]))
}
