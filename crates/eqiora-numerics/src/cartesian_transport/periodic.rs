use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, GraphPath};
use eqiora_realization::CellCenteredConvectionScheme;

use super::api::TransportFace2d;
use super::reconstruction::AffineFaceTrace;
use crate::{CartesianMesh, MeshEntity, MeshGeometry, MeshTopology};

const DIMENSION: usize = 2;

/// One generated-Cartesian identification across opposite sides of an axis.
///
/// Facet and cell identities are retained so a seam can be re-admitted against
/// the exact mesh before it contributes an operator action. The tangential
/// index is the Cartesian cell index shared by both boundary facets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CartesianPeriodicFacetPair2d {
    axis: usize,
    tangential_index: usize,
    lower_facet: MeshEntity,
    upper_facet: MeshEntity,
    lower_cell: MeshEntity,
    upper_cell: MeshEntity,
}

/// Derive one conservative interior-face action for every periodic seam pair.
///
/// This primitive owns numerical identification only. It does not infer a
/// periodic boundary from canonical boundary laws. The caller must already
/// have resolved that meaning and supplies the constant normal velocity and
/// diffusivity selected for this generated-Cartesian reference path.
///
/// # Errors
/// Rejects non-2D or incomplete Cartesian seams, stale or forged facet pairs,
/// non-finite coefficients, and every reconstruction other than endpoint
/// first-order upwind before producing any face action.
pub(super) fn periodic_seam_faces(
    mesh: &CartesianMesh,
    axis: usize,
    normal_velocity: f64,
    diffusivity: f64,
    scheme: CellCenteredConvectionScheme,
) -> Result<Vec<TransportFace2d>, Diagnostic> {
    if scheme != CellCenteredConvectionScheme::ImplicitFirstOrderUpwind {
        return Err(invalid_realization(
            "Cartesian periodic seams currently require implicit first-order upwind reconstruction",
        ));
    }
    let pairs = derive_periodic_facet_pairs(mesh, axis)?;
    periodic_seam_faces_from_pairs(mesh, axis, &pairs, normal_velocity, diffusivity)
}

fn periodic_seam_faces_from_pairs(
    mesh: &CartesianMesh,
    axis: usize,
    pairs: &[CartesianPeriodicFacetPair2d],
    normal_velocity: f64,
    diffusivity: f64,
) -> Result<Vec<TransportFace2d>, Diagnostic> {
    if !normal_velocity.is_finite() || !diffusivity.is_finite() || diffusivity <= 0.0 {
        return Err(invalid_numerics(
            "Cartesian periodic seam coefficients require finite normal velocity and positive diffusivity",
        ));
    }
    admit_periodic_facet_pairs(mesh, axis, pairs)?;
    let mut faces = Vec::with_capacity(pairs.len());
    for pair in pairs {
        let lower_facet_center =
            point2(mesh.entity_center(pair.lower_facet).ok_or_else(|| {
                invalid_numerics("Cartesian periodic lower-facet center is unavailable")
            })?)?;
        let upper_facet_center =
            point2(mesh.entity_center(pair.upper_facet).ok_or_else(|| {
                invalid_numerics("Cartesian periodic upper-facet center is unavailable")
            })?)?;
        let lower_cell_center = point2(mesh.entity_center(pair.lower_cell).ok_or_else(|| {
            invalid_numerics("Cartesian periodic lower-cell center is unavailable")
        })?)?;
        let upper_cell_center = point2(mesh.entity_center(pair.upper_cell).ok_or_else(|| {
            invalid_numerics("Cartesian periodic upper-cell center is unavailable")
        })?)?;

        let lower_area = facet_measure(mesh, pair.lower_facet)?;
        let upper_area = facet_measure(mesh, pair.upper_facet)?;
        if lower_area.to_bits() != upper_area.to_bits() {
            return Err(invalid_numerics(
                "paired Cartesian periodic facets require identical measures",
            ));
        }
        let distance = lower_cell_center[axis] - lower_facet_center[axis]
            + upper_facet_center[axis]
            - upper_cell_center[axis];
        // `lower` is the cell adjacent to the coordinate-lower boundary. Its
        // outward seam normal is therefore the negative physical-axis basis.
        let outward_from_lower_flux = -normal_velocity * lower_area;
        let transmissibility = diffusivity * lower_area / distance;
        if !distance.is_finite()
            || distance <= 0.0
            || !outward_from_lower_flux.is_finite()
            || !transmissibility.is_finite()
            || transmissibility <= 0.0
        {
            return Err(invalid_numerics(
                "Cartesian periodic two-point face geometry is invalid",
            ));
        }

        let lower = pair.lower_cell.index();
        let upper = pair.upper_cell.index();
        let donor = if outward_from_lower_flux >= 0.0 {
            lower
        } else {
            upper
        };
        faces.push(TransportFace2d::Interior {
            lower,
            upper,
            outward_from_lower_flux,
            transmissibility,
            advective_trace: AffineFaceTrace::cell(donor),
        });
    }
    Ok(faces)
}

fn derive_periodic_facet_pairs(
    mesh: &CartesianMesh,
    axis: usize,
) -> Result<Vec<CartesianPeriodicFacetPair2d>, Diagnostic> {
    if mesh.topological_dimension() != DIMENSION || axis >= DIMENSION {
        return Err(invalid_numerics(
            "Cartesian periodic seam requires a valid axis of a two-dimensional mesh",
        ));
    }
    let tangential_axis = 1 - axis;
    let expected_count = mesh.axis_cell_count(tangential_axis).ok_or_else(|| {
        invalid_numerics("Cartesian periodic tangential cell count is unavailable")
    })?;
    let axis_cell_count = mesh
        .axis_cell_count(axis)
        .ok_or_else(|| invalid_numerics("Cartesian periodic normal cell count is unavailable"))?;
    let bounds = mesh
        .axis_bounds(axis)
        .ok_or_else(|| invalid_numerics("Cartesian periodic axis bounds are unavailable"))?;
    let facet_count = mesh
        .entity_count(DIMENSION - 1)
        .ok_or_else(|| invalid_numerics("Cartesian periodic facet stratum is unavailable"))?;
    let mut lower = BTreeMap::new();
    let mut upper = BTreeMap::new();

    for index in 0..facet_count {
        let facet = MeshEntity::new(DIMENSION - 1, index);
        let free_axes = mesh
            .entity_free_axes(facet)
            .ok_or_else(|| invalid_numerics("Cartesian periodic facet axes are unavailable"))?;
        if free_axes.binary_search(&axis).is_ok() {
            continue;
        }
        let adjacent = mesh
            .incidence(facet, DIMENSION)
            .ok_or_else(|| invalid_numerics("Cartesian periodic facet adjacency is unavailable"))?;
        let [cell] = adjacent.as_slice() else {
            continue;
        };
        let cell_indices = mesh.cell_multi_index(cell.entity).ok_or_else(|| {
            invalid_numerics("Cartesian periodic adjacent-cell index is unavailable")
        })?;
        let tangential_index = cell_indices[tangential_axis];
        let facet_center =
            point2(mesh.entity_center(facet).ok_or_else(|| {
                invalid_numerics("Cartesian periodic facet center is unavailable")
            })?)?;
        let side = if cell_indices[axis] == 0 && facet_center[axis] == bounds[0] {
            &mut lower
        } else if cell_indices[axis] + 1 == axis_cell_count && facet_center[axis] == bounds[1] {
            &mut upper
        } else {
            return Err(invalid_numerics(
                "Cartesian periodic boundary facet is inconsistent with its adjacent cell",
            ));
        };
        if side
            .insert(tangential_index, (facet, cell.entity))
            .is_some()
        {
            return Err(invalid_numerics(
                "Cartesian periodic seam contains a duplicate tangential facet index",
            ));
        }
    }

    if lower.len() != expected_count || upper.len() != expected_count {
        return Err(invalid_numerics(
            "Cartesian periodic seam does not cover both axis sides one-to-one",
        ));
    }
    (0..expected_count)
        .map(|tangential_index| {
            let (lower_facet, lower_cell) =
                lower.get(&tangential_index).copied().ok_or_else(|| {
                    invalid_numerics("Cartesian periodic lower-side tangential facet is missing")
                })?;
            let (upper_facet, upper_cell) =
                upper.get(&tangential_index).copied().ok_or_else(|| {
                    invalid_numerics("Cartesian periodic upper-side tangential facet is missing")
                })?;
            Ok(CartesianPeriodicFacetPair2d {
                axis,
                tangential_index,
                lower_facet,
                upper_facet,
                lower_cell,
                upper_cell,
            })
        })
        .collect()
}

fn admit_periodic_facet_pairs(
    mesh: &CartesianMesh,
    axis: usize,
    pairs: &[CartesianPeriodicFacetPair2d],
) -> Result<(), Diagnostic> {
    let expected = derive_periodic_facet_pairs(mesh, axis)?;
    if pairs != expected {
        return Err(invalid_numerics(
            "Cartesian periodic facet pairing does not match the generated mesh",
        ));
    }
    Ok(())
}

fn facet_measure(mesh: &CartesianMesh, facet: MeshEntity) -> Result<f64, Diagnostic> {
    let geometry = mesh
        .geometry_map(facet)
        .ok_or_else(|| invalid_numerics("Cartesian periodic facet geometry is unavailable"))?;
    let measure = 2.0 * geometry.measure_scale();
    if !measure.is_finite() || measure <= 0.0 {
        return Err(invalid_numerics(
            "Cartesian periodic facet measure must be finite and positive",
        ));
    }
    Ok(measure)
}

fn point2(values: Vec<f64>) -> Result<[f64; 2], Diagnostic> {
    values.try_into().map_err(|values: Vec<_>| {
        invalid_numerics(format!(
            "Cartesian periodic geometry expected two coordinates, received {}",
            values.len()
        ))
    })
}

fn invalid_realization(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message).with_graph_path(GraphPath::new([
        "realization".to_owned(),
        "scalar-transport-fvm-2d".to_owned(),
        "periodic-seam".to_owned(),
    ]))
}

fn invalid_numerics(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::NUMERICAL_SOLVE_FAILED, message).with_graph_path(GraphPath::new([
        "numerics".to_owned(),
        "scalar-transport-fvm-2d".to_owned(),
        "periodic-seam".to_owned(),
    ]))
}

#[cfg(test)]
mod tests {
    use eqiora_assembly::LocalUnknown;

    use super::*;
    use crate::cartesian_transport::faces::face_packet;

    fn mesh() -> CartesianMesh {
        CartesianMesh::from_axes(vec![vec![-2.0, -1.0, 1.0, 4.0], vec![3.0, 3.5, 5.0]]).unwrap()
    }

    #[test]
    fn pairs_opposite_facets_by_tangential_cartesian_index() {
        let mesh = mesh();
        let x_pairs = derive_periodic_facet_pairs(&mesh, 0).unwrap();
        assert_eq!(x_pairs.len(), 2);
        assert_eq!(
            x_pairs
                .iter()
                .map(|pair| pair.tangential_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        for pair in &x_pairs {
            let lower = mesh.cell_multi_index(pair.lower_cell).unwrap();
            let upper = mesh.cell_multi_index(pair.upper_cell).unwrap();
            assert_eq!(lower, &[0, pair.tangential_index]);
            assert_eq!(upper, &[2, pair.tangential_index]);
        }

        let y_pairs = derive_periodic_facet_pairs(&mesh, 1).unwrap();
        assert_eq!(y_pairs.len(), 3);
        for pair in &y_pairs {
            let lower = mesh.cell_multi_index(pair.lower_cell).unwrap();
            let upper = mesh.cell_multi_index(pair.upper_cell).unwrap();
            assert_eq!(lower, &[pair.tangential_index, 0]);
            assert_eq!(upper, &[pair.tangential_index, 1]);
        }
    }

    #[test]
    fn seam_uses_one_equal_and_opposite_interior_packet_per_pair() {
        let mesh = CartesianMesh::uniform(&[[0.0, 2.0], [-1.0, 1.0]], &[2, 1]).unwrap();
        let faces = periodic_seam_faces(
            &mesh,
            0,
            3.0,
            0.5,
            CellCenteredConvectionScheme::ImplicitFirstOrderUpwind,
        )
        .unwrap();
        assert_eq!(faces.len(), 1);
        assert!(matches!(faces[0], TransportFace2d::Interior { .. }));

        let (packet, map) = face_packet(&faces[0], 1.0, 1.0).unwrap();
        assert_eq!(packet.rows(), 2);
        assert_eq!(packet.columns(), 2);
        assert_eq!(packet.rhs(), &[0.0, 0.0]);
        assert_eq!(map.equations().len(), 2);
        assert_ne!(map.equations()[0], map.equations()[1]);
        assert!(
            map.unknowns()
                .iter()
                .all(|unknown| matches!(unknown, LocalUnknown::Free(_)))
        );
        for column in 0..packet.columns() {
            assert_eq!(
                packet.entry(0, column).unwrap() + packet.entry(1, column).unwrap(),
                0.0
            );
        }
        let values = [2.0, -1.0];
        let action = |row| {
            (0..packet.columns())
                .map(|column| packet.entry(row, column).unwrap() * values[column])
                .sum::<f64>()
                - packet.rhs()[row]
        };
        assert_eq!(action(0) + action(1), 0.0);
    }

    #[test]
    fn seam_upwind_donor_tracks_oriented_flux() {
        let mesh = CartesianMesh::uniform(&[[0.0, 2.0], [0.0, 1.0]], &[2, 1]).unwrap();
        for (velocity, expected_donor) in [(1.0, 1), (-1.0, 0)] {
            let face = periodic_seam_faces(
                &mesh,
                0,
                velocity,
                1.0,
                CellCenteredConvectionScheme::ImplicitFirstOrderUpwind,
            )
            .unwrap()
            .pop()
            .unwrap();
            let TransportFace2d::Interior {
                advective_trace, ..
            } = face
            else {
                panic!("periodic seam must lower to one interior action")
            };
            assert_eq!(
                advective_trace.terms(),
                &[(crate::DofId::new(expected_donor), 1.0)]
            );
            assert_eq!(advective_trace.offset(), 0.0);
        }
    }

    #[test]
    fn seam_rejects_minmod_forged_pairs_and_nonfinite_coefficients() {
        let mesh = mesh();
        assert_eq!(
            periodic_seam_faces(
                &mesh,
                0,
                1.0,
                1.0,
                CellCenteredConvectionScheme::ExplicitPreviousStateCartesianMinmod,
            )
            .unwrap_err()
            .code(),
            codes::INVALID_REALIZATION
        );

        let mut forged = derive_periodic_facet_pairs(&mesh, 0).unwrap();
        forged[0].upper_cell = forged[1].upper_cell;
        assert_eq!(
            periodic_seam_faces_from_pairs(&mesh, 0, &forged, 1.0, 1.0)
                .unwrap_err()
                .code(),
            codes::NUMERICAL_SOLVE_FAILED
        );

        for (velocity, diffusivity) in [(f64::NAN, 1.0), (1.0, f64::INFINITY), (1.0, 0.0)] {
            assert_eq!(
                periodic_seam_faces(
                    &mesh,
                    0,
                    velocity,
                    diffusivity,
                    CellCenteredConvectionScheme::ImplicitFirstOrderUpwind,
                )
                .unwrap_err()
                .code(),
                codes::NUMERICAL_SOLVE_FAILED
            );
        }
    }
}
