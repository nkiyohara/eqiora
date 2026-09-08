use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_meshing::{MeshEntity, MeshGeometry, MeshTopology};
use eqiora_schema::kernel::BoundarySide;

use eqiora_meshing::CartesianMesh;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CartesianCellMetrics<const D: usize> {
    pub(crate) center: [f64; D],
    pub(crate) measure: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CartesianFacetAdjacency {
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
pub(crate) struct CartesianFacetMetrics<const D: usize> {
    pub(crate) center: [f64; D],
    pub(crate) measure: f64,
    pub(crate) normal_axis: usize,
    pub(crate) adjacency: CartesianFacetAdjacency,
}

/// Traverse an admitted 1D or 2D Cartesian mesh without assigning equation or boundary roles.
///
/// Cell/facet order, adjacency, normal axis, and positive metric data are
/// shared numerical geometry. Every physical flux remains with its owning
/// realization.
pub(crate) fn cartesian_fvm_geometry<const D: usize>(
    mesh: &CartesianMesh,
) -> Result<(Vec<CartesianCellMetrics<D>>, Vec<CartesianFacetMetrics<D>>), Diagnostic> {
    if !(1..=2).contains(&D) || mesh.topological_dimension() != D {
        return Err(invalid_numerics(
            "Cartesian FVM geometry requires matching 1D or 2D mesh",
        ));
    }
    let cell_count = mesh.entity_count(D).expect("Cartesian mesh owns top cells");
    let mut cells = Vec::new();
    cells
        .try_reserve_exact(cell_count)
        .map_err(|_| invalid_numerics("Cartesian FVM cell allocation exceeds capacity"))?;
    for index in 0..cell_count {
        let entity = MeshEntity::new(D, index);
        let center = point(
            mesh.entity_center(entity)
                .ok_or_else(|| invalid_numerics("Cartesian FVM cell center is unavailable"))?,
        )?;
        let geometry = mesh
            .geometry_map(entity)
            .ok_or_else(|| invalid_numerics("Cartesian FVM cell geometry is unavailable"))?;
        let measure = ((1_u32 << D) as f64) * geometry.measure_scale();
        if !measure.is_finite() || measure <= 0.0 {
            return Err(invalid_numerics(
                "Cartesian FVM cell measure must be finite and positive",
            ));
        }
        cells.push(CartesianCellMetrics { center, measure });
    }

    let facet_count = mesh
        .entity_count(D - 1)
        .expect("Cartesian mesh owns facets");
    let mut facets = Vec::new();
    facets
        .try_reserve_exact(facet_count)
        .map_err(|_| invalid_numerics("Cartesian FVM facet allocation exceeds capacity"))?;
    for index in 0..facet_count {
        let facet = MeshEntity::new(D - 1, index);
        let center = point(
            mesh.entity_center(facet)
                .ok_or_else(|| invalid_numerics("Cartesian FVM facet center is unavailable"))?,
        )?;
        let geometry = mesh
            .geometry_map(facet)
            .ok_or_else(|| invalid_numerics("Cartesian FVM facet geometry is unavailable"))?;
        let measure = ((1_u32 << (D - 1)) as f64) * geometry.measure_scale();
        if !measure.is_finite() || measure <= 0.0 {
            return Err(invalid_numerics(
                "Cartesian FVM facet measure must be finite and positive",
            ));
        }
        let free_axes = mesh
            .entity_free_axes(facet)
            .ok_or_else(|| invalid_numerics("Cartesian FVM facet axes are unavailable"))?;
        let normal_axis = (0..D)
            .find(|axis| free_axes.binary_search(axis).is_err())
            .ok_or_else(|| invalid_numerics("Cartesian FVM facet has no normal axis"))?;
        let adjacent = mesh
            .incidence(facet, D)
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
                CartesianFacetAdjacency::Interior {
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
                CartesianFacetAdjacency::Boundary {
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
        facets.push(CartesianFacetMetrics {
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

fn point<const D: usize>(point: Vec<f64>) -> Result<[f64; D], Diagnostic> {
    point.try_into().map_err(|point: Vec<_>| {
        invalid_numerics(format!(
            "Cartesian FVM geometry requires {D} coordinates, received {}",
            point.len()
        ))
    })
}

fn invalid_numerics(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "cartesian-fvm-geometry".to_owned(),
    ]))
}
