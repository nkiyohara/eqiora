//! Exact sealed topology content, index, and chordal realization predicates.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_artifact::GeometryDefinitionV1;
use eqiora_core::Diagnostic;
use eqiora_geometry::{EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet, PlanarFace, PlanarRegion};
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh};

use super::{
    StokesDissipationBoundaryFacetSource2d, StokesDissipationCellRecord2d,
    StokesDissipationProfileGeometry2d, StokesDissipationTopology2d,
    StokesDissipationTopologySource2d, invalid,
};

/// Exact index, arity, range, and same-index order predicate.
///
/// This owns everything an index, endpoint, connectivity, correspondence, or
/// angle-order fault can break, before any label or content comparison.
pub(super) fn require_topology_indices(
    source: &StokesDissipationTopologySource2d,
    sectors: usize,
) -> Result<(), Diagnostic> {
    let vertex_count = source.vertices.len();
    let facet_count = source.boundary_facets.len();
    if vertex_count == 0 || facet_count == 0 || sectors == 0 {
        return Err(invalid("topology has an empty index stratum"));
    }
    if source
        .vertices
        .iter()
        .enumerate()
        .any(|(index, record)| record.id != index)
    {
        return Err(invalid("topology vertex identities are not contiguous"));
    }
    for (index, cell) in source.cells.iter().enumerate() {
        let [a, b, c] = cell.vertices;
        if cell.id != index
            || a >= vertex_count
            || b >= vertex_count
            || c >= vertex_count
            || a == b
            || b == c
            || c == a
        {
            return Err(invalid(
                "topology cell identity, arity, or vertex index is out of exact range",
            ));
        }
    }
    for (index, facet) in source.boundary_facets.iter().enumerate() {
        let [first, second] = facet.vertices;
        if facet.id != index || first >= vertex_count || second >= vertex_count || first == second {
            return Err(invalid(
                "topology facet identity or endpoint index is out of exact range",
            ));
        }
    }
    if source.ordered_body_angles.len() != sectors
        || source
            .ordered_body_angles
            .iter()
            .enumerate()
            .any(|(angle, turns)| *turns != format!("{angle}/{sectors}"))
    {
        return Err(invalid(
            "ordered body angle content or order differs from the exact sealed sequence",
        ));
    }
    if source.correspondence.len() != sectors
        || source
            .correspondence
            .iter()
            .enumerate()
            .any(|(angle, entry)| {
                entry.angle_index != angle
                    || entry.angle_turns != format!("{angle}/{sectors}")
                    || entry.body_vertex != angle
                    || entry.body_facet != angle
                    || entry.body_vertex >= vertex_count
                    || entry.body_facet >= facet_count
            })
    {
        return Err(invalid(
            "body correspondence differs from the exact same-index angle/vertex/facet tuple",
        ));
    }
    Ok(())
}

/// Exact sealed content predicate over records, cells, and labelled facets.
pub(super) fn require_topology_content(
    source: &StokesDissipationTopologySource2d,
    sectors: usize,
    intervals: usize,
) -> Result<(), Diagnostic> {
    let vertex_count = sectors * (intervals + 1);
    if source.vertex_count != vertex_count
        || source.cell_count != 2 * sectors * intervals
        || source.facet_count != 2 * sectors
        || source.membership_counts != [sectors, sectors * (intervals - 1), sectors]
        || source.vertices.len() != vertex_count
        || source.boundary_facets.len() != source.facet_count
        || source.vertices.iter().enumerate().any(|(id, record)| {
            record.ring_index != id / sectors
                || record.angle_index != id % sectors
                || record.ring_fraction != format!("{}/{}", record.ring_index, intervals)
                || record.angle_turns != format!("{}/{}", record.angle_index, sectors)
                || record.classification
                    != match record.ring_index {
                        0 => "body_boundary",
                        ring if ring == intervals => "outer_boundary",
                        _ => "fluid_interior",
                    }
        })
    {
        return Err(invalid(
            "topology counts or vertex records differ from exact symbolic ring/angle content",
        ));
    }
    if source.cells.len() != source.cell_count || source.cells != expected_cells(sectors, intervals)
    {
        return Err(invalid(
            "topology connectivity differs from the exact ordered square-ring triangulation",
        ));
    }
    if source.boundary_facets != expected_boundary_facets(sectors, intervals)? {
        return Err(invalid(
            "boundary facets differ from the exact oriented five-label inventory",
        ));
    }
    Ok(())
}

fn expected_cells(sectors: usize, intervals: usize) -> Vec<StokesDissipationCellRecord2d> {
    let mut cells = Vec::with_capacity(2 * sectors * intervals);
    for ring in 0..intervals {
        for angle in 0..sectors {
            let next = (angle + 1) % sectors;
            let inner = ring * sectors + angle;
            let inner_next = ring * sectors + next;
            let outer = (ring + 1) * sectors + angle;
            let outer_next = (ring + 1) * sectors + next;
            cells.push(StokesDissipationCellRecord2d {
                id: cells.len(),
                vertices: [inner, outer, outer_next],
            });
            cells.push(StokesDissipationCellRecord2d {
                id: cells.len(),
                vertices: [inner, outer_next, inner_next],
            });
        }
    }
    cells
}

pub(super) fn reference_vertices(
    area_radius_m: f64,
    sectors: usize,
    intervals: usize,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    if !area_radius_m.is_finite() || area_radius_m <= 0.0 {
        return Err(invalid("topology requires finite positive r_A"));
    }
    let half_width = 10.0 * area_radius_m;
    let mut vertices = Vec::with_capacity(sectors * (intervals + 1));
    for ring in 0..=intervals {
        let fraction = ring as f64 / intervals as f64;
        for angle_index in 0..sectors {
            let angle = std::f64::consts::TAU * angle_index as f64 / sectors as f64;
            let direction = [angle.cos(), angle.sin()];
            let outer_scale = half_width / direction[0].abs().max(direction[1].abs());
            let body = [area_radius_m * direction[0], area_radius_m * direction[1]];
            let outer = [outer_scale * direction[0], outer_scale * direction[1]];
            vertices.push(vec![
                (1.0 - fraction) * body[0] + fraction * outer[0],
                (1.0 - fraction) * body[1] + fraction * outer[1],
            ]);
        }
    }
    Ok(vertices)
}

pub(super) fn expected_boundary_facets(
    sectors: usize,
    intervals: usize,
) -> Result<Vec<StokesDissipationBoundaryFacetSource2d>, Diagnostic> {
    let mut facets = Vec::with_capacity(2 * sectors);
    for angle in 0..sectors {
        facets.push(StokesDissipationBoundaryFacetSource2d {
            id: facets.len(),
            vertices: [(angle + 1) % sectors, angle],
            kind: "body".to_owned(),
            label: "body_no_slip".to_owned(),
            orientation: "fluid_domain_boundary_clockwise".to_owned(),
        });
    }
    let outer_start = intervals * sectors;
    for angle in 0..sectors {
        let endpoints = [outer_start + angle, outer_start + (angle + 1) % sectors];
        facets.push(StokesDissipationBoundaryFacetSource2d {
            id: facets.len(),
            vertices: endpoints,
            kind: "outer".to_owned(),
            label: outer_source_label(angle, sectors)?.to_owned(),
            orientation: "fluid_domain_boundary_counterclockwise".to_owned(),
        });
    }
    Ok(facets)
}

fn outer_source_label(angle: usize, sectors: usize) -> Result<&'static str, Diagnostic> {
    if !sectors.is_multiple_of(8) || angle >= sectors {
        return Err(invalid(
            "outer facet has no exact indexed square-side label",
        ));
    }
    match 8 * angle / sectors {
        0 | 7 => Ok("outer_x_plus"),
        1 | 2 => Ok("outer_y_plus"),
        3 | 4 => Ok("outer_x_minus"),
        5 | 6 => Ok("outer_y_minus"),
        _ => unreachable!("angle is in exact sector range"),
    }
}

pub(super) fn semantic_role(source: &str) -> Result<&'static str, Diagnostic> {
    match source {
        "body_no_slip" => Ok("body"),
        "outer_x_minus" => Ok("outer_x_lower"),
        "outer_x_plus" => Ok("outer_x_upper"),
        "outer_y_minus" => Ok("outer_y_lower"),
        "outer_y_plus" => Ok("outer_y_upper"),
        _ => Err(invalid(
            "topology contains an unknown source boundary label",
        )),
    }
}

pub(super) fn mesh_facet_for_vertices(
    mesh: &SimplicialMesh,
    endpoints: [usize; 2],
) -> Result<MeshEntity, Diagnostic> {
    let target = BTreeSet::from(endpoints);
    let facet_count = mesh
        .entity_count(1)
        .ok_or_else(|| invalid("topology has no facet stratum"))?;
    let matches = (0..facet_count)
        .map(|index| MeshEntity::new(1, index))
        .filter(|facet| {
            mesh.entity_vertices(*facet).is_some_and(|vertices| {
                vertices
                    .into_iter()
                    .map(|vertex| vertex.index())
                    .collect::<BTreeSet<_>>()
                    == target
            })
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [facet] => Ok(*facet),
        _ => Err(invalid(
            "boundary source facet is missing or duplicated in exact topology",
        )),
    }
}

fn realized_facet_role<'a>(
    topology: &'a StokesDissipationTopology2d,
    mesh: &SimplicialMesh,
    coordinates: &[[f64; 2]],
    endpoints: [usize; 2],
) -> Result<&'a str, Diagnostic> {
    let expected = [coordinates[endpoints[0]], coordinates[endpoints[1]]];
    let facets = (0..mesh.entity_count(1).expect("2D mesh facets"))
        .map(|index| MeshEntity::new(1, index))
        .filter(|facet| {
            let vertices = mesh.entity_vertices(*facet).expect("mesh facet vertices");
            let actual = [
                mesh.vertices()[vertices[0].index()].as_slice(),
                mesh.vertices()[vertices[1].index()].as_slice(),
            ];
            (actual[0] == expected[0] && actual[1] == expected[1])
                || (actual[0] == expected[1] && actual[1] == expected[0])
        })
        .collect::<Vec<_>>();
    let [facet] = facets.as_slice() else {
        return Err(invalid(
            "chordal Geometry edge has no unique topology facet",
        ));
    };
    let roles = topology
        .entity_sets()
        .iter()
        .filter_map(|(role, members)| members.contains(facet).then_some(role.as_str()))
        .collect::<Vec<_>>();
    match roles.as_slice() {
        [role] => Ok(*role),
        _ => Err(invalid(
            "chordal Geometry facet has no unique semantic role",
        )),
    }
}

pub(super) fn chordal_geometry(
    profile: &StokesDissipationProfileGeometry2d,
    topology: &StokesDissipationTopology2d,
    mesh: &SimplicialMesh,
    sectors: usize,
    coordinate_tolerance_m: f64,
) -> Result<GeometryDefinitionV1, Diagnostic> {
    let outer_start = mesh.vertices().len() - sectors;
    let outer = (0..sectors).map(|index| outer_start + index);
    let body = 0..sectors;
    let mut compact = outer
        .chain(body)
        .map(|index| [mesh.vertices()[index][0], mesh.vertices()[index][1]])
        .collect::<Vec<_>>();
    let first = PlanarRegion::new(
        compact.clone(),
        vec![PlanarFace::new(
            (0..sectors).collect(),
            vec![(sectors..2 * sectors).collect()],
        )],
        Vec::new(),
        coordinate_tolerance_m,
    )?;
    compact = first.vertices().to_vec();
    let face = first.faces()[0].clone();
    let outer_loop = face.outer();
    let body_loop = &face.holes()[0];
    let mut sets = BTreeMap::<String, Vec<usize>>::new();
    for edge in 0..outer_loop.len() {
        let endpoints = [outer_loop[edge], outer_loop[(edge + 1) % outer_loop.len()]];
        let role = realized_facet_role(topology, mesh, &compact, endpoints)?;
        if role == "body" {
            return Err(invalid(
                "chordal outer loop is associated with the body role",
            ));
        }
        sets.entry(role.to_owned()).or_default().push(edge);
    }
    for edge in 0..body_loop.len() {
        let endpoints = [body_loop[edge], body_loop[(edge + 1) % body_loop.len()]];
        if realized_facet_role(topology, mesh, &compact, endpoints)? != "body" {
            return Err(invalid(
                "chordal body loop is associated with an outer role",
            ));
        }
    }
    sets.insert(
        "body".to_owned(),
        (outer_loop.len()..outer_loop.len() + body_loop.len()).collect(),
    );
    let mut named = sets
        .into_iter()
        .map(|(name, members)| NamedEntitySet::new(name, EDGE_DIMENSION, members))
        .collect::<Vec<_>>();
    named.push(NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]));
    let region = PlanarRegion::new(compact, vec![face], named, coordinate_tolerance_m)?;
    let geometry = GeometryDefinitionV1::from_region(&region);
    if profile.analytic_area_m2() <= 0.0 {
        return Err(invalid("analytic profile area is not positive"));
    }
    Ok(geometry)
}

/// Signed chordal polygon area of the realized body loop.
///
/// This is a named finite-element diagnostic. It never owns or replaces the
/// analytic `pi r_A^2` identity held by the profile.
pub(super) fn chordal_body_polygon_area_m2(vertices: &[usize], coordinates: &[Vec<f64>]) -> f64 {
    0.5 * vertices
        .iter()
        .zip(vertices.iter().cycle().skip(1))
        .take(vertices.len())
        .map(|(&first, &second)| {
            coordinates[first][0].mul_add(
                coordinates[second][1],
                -(coordinates[first][1] * coordinates[second][0]),
            )
        })
        .sum::<f64>()
}
