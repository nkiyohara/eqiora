use super::*;

pub(crate) fn entity_coordinates<M: MeshGeometry + ?Sized>(
    mesh: &M,
    entity: MeshEntity,
) -> Result<Vec<Vec<f64>>, Diagnostic> {
    mesh.incidence(entity, 0)
        .ok_or_else(|| invalid_artifact("mesh entity has no vertex closure"))?
        .into_iter()
        .map(|incidence| {
            let vertex = incidence.entity;
            let map = mesh.geometry_map(vertex).ok_or_else(|| {
                invalid_artifact("mesh entity references an unavailable vertex geometry")
            })?;
            let mut coordinates = vec![0.0; map.physical_dimension()];
            map.map_point(&[], &mut coordinates)
                .map_err(|error| invalid_artifact(error.message()))?;
            Ok(coordinates)
        })
        .collect()
}

pub(crate) fn point_inside_cartesian(point: &[f64], bounds: &[(f64, f64)], tolerance: f64) -> bool {
    point.len() == bounds.len()
        && point
            .iter()
            .zip(bounds)
            .all(|(&coordinate, &(lower, upper))| {
                coordinate >= lower - tolerance && coordinate <= upper + tolerance
            })
}

pub(crate) fn cartesian_facet_roles(
    points: &[Vec<f64>],
    bounds: &[(f64, f64)],
    tolerance: f64,
) -> Vec<(usize, BoundarySide)> {
    bounds
        .iter()
        .enumerate()
        .flat_map(|(axis, &(lower, upper))| {
            [(BoundarySide::Lower, lower), (BoundarySide::Upper, upper)]
                .into_iter()
                .filter(move |&(_, coordinate)| {
                    points
                        .iter()
                        .all(|point| (point[axis] - coordinate).abs() <= tolerance)
                })
                .map(move |(side, _)| (axis, side))
        })
        .collect()
}

pub(crate) fn validate_parent_outward_cell<M: MeshGeometry + ?Sized>(
    mesh: &M,
    cell: MeshEntity,
    axis: usize,
    side: BoundarySide,
    bounds: &[(f64, f64)],
) -> Result<(), Diagnostic> {
    let vertices = entity_coordinates(mesh, cell)?;
    let centroid = vertices.iter().map(|vertex| vertex[axis]).sum::<f64>() / vertices.len() as f64;
    let valid = match side {
        BoundarySide::Lower => centroid > bounds[axis].0,
        BoundarySide::Upper => centroid < bounds[axis].1,
    };
    if valid {
        Ok(())
    } else {
        Err(invalid_artifact(
            "mesh incidence does not derive the declared parent-outward boundary role",
        ))
    }
}

pub(super) fn reject_ambiguous_entity_sets(region: &PlanarRegion) -> Result<(), Diagnostic> {
    let mut owners = BTreeMap::<(usize, usize), &str>::new();
    for set in region.entity_sets() {
        for &member in set.members() {
            if let Some(first) = owners.insert((set.dimension(), member), set.name()) {
                let entity = match set.dimension() {
                    VERTEX_DIMENSION => "vertex",
                    EDGE_DIMENSION => "facet",
                    FACE_DIMENSION => "cell",
                    _ => "entity",
                };
                return Err(invalid_artifact(format!(
                    "mesh {entity} is ambiguous between region entity sets '{first}' and '{}'",
                    set.name()
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn region_edges(region: &PlanarRegion) -> Vec<RegionEdge> {
    let mut edges = Vec::with_capacity(region.edge_count());
    for (face_index, face) in region.faces().iter().enumerate() {
        for boundary in std::iter::once(face.outer()).chain(face.holes().iter().map(Vec::as_slice))
        {
            for position in 0..boundary.len() {
                edges.push(RegionEdge {
                    index: edges.len(),
                    parent_face: face_index,
                    start: region.vertices()[boundary[position]],
                    end: region.vertices()[boundary[(position + 1) % boundary.len()]],
                });
            }
        }
    }
    edges
}

pub(super) fn assign_vertices(
    region: &PlanarRegion,
    mesh: &SimplicialMesh,
) -> Result<Vec<WireVertexAssignment>, Diagnostic> {
    let mut used = BTreeSet::new();
    region
        .vertices()
        .iter()
        .enumerate()
        .map(|(geometry_vertex, &point)| {
            let candidates = mesh
                .vertices()
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    distance(point, to_point(candidate)) <= region.tolerance_m()
                })
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            if candidates.len() != 1 || !used.insert(candidates[0]) {
                return Err(invalid_artifact(format!(
                    "region vertex {geometry_vertex} must match exactly one unique mesh vertex"
                )));
            }
            Ok(WireVertexAssignment {
                geometry_vertex: portable(geometry_vertex, "geometry vertex")?,
                mesh_vertex: portable(candidates[0], "mesh vertex")?,
            })
        })
        .collect()
}

pub(super) fn assign_cells(
    region: &PlanarRegion,
    mesh: &SimplicialMesh,
) -> Result<(Vec<WireFaceAssignment>, Vec<usize>), Diagnostic> {
    let cell_count = mesh
        .entity_count(FACE_DIMENSION)
        .ok_or_else(|| invalid_artifact("mesh has no triangle stratum"))?;
    let mut by_face = vec![Vec::new(); region.faces().len()];
    let mut owners = Vec::with_capacity(cell_count);
    for cell_index in 0..cell_count {
        let triangle = entity_triangle(mesh, MeshEntity::new(FACE_DIMENSION, cell_index))?;
        let candidates = region
            .faces()
            .iter()
            .enumerate()
            .filter(|(_, face)| triangle_within_face(&triangle, face, region))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if candidates.len() != 1 {
            return Err(invalid_artifact(format!(
                "mesh cell {cell_index} must be owned by exactly one region face; found {}",
                candidates.len()
            )));
        }
        owners.push(candidates[0]);
        by_face[candidates[0]].push(portable(cell_index, "mesh cell")?);
    }
    by_face
        .into_iter()
        .enumerate()
        .map(|(face, cell_indices)| {
            if cell_indices.is_empty() {
                return Err(invalid_artifact(format!(
                    "region face {face} has no mesh cells"
                )));
            }
            Ok(WireFaceAssignment {
                geometry_face: portable(face, "geometry face")?,
                cell_indices,
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|faces| (faces, owners))
}

pub(super) fn assign_frontiers(
    region: &PlanarRegion,
    mesh: &SimplicialMesh,
    edges: &[RegionEdge],
    cell_owners: &[usize],
) -> Result<Vec<WireFrontierAssignment>, Diagnostic> {
    let facet_count = mesh
        .entity_count(EDGE_DIMENSION)
        .ok_or_else(|| invalid_artifact("mesh has no edge stratum"))?;
    let mut assignments = vec![Vec::<(usize, WireFacetOutward)>::new(); edges.len()];
    for face in 0..region.faces().len() {
        for facet_index in 0..facet_count {
            let facet = MeshEntity::new(EDGE_DIMENSION, facet_index);
            let adjacent = mesh
                .incidence(facet, FACE_DIMENSION)
                .ok_or_else(|| invalid_artifact("mesh facet incidence is unavailable"))?;
            let parent_cells = adjacent
                .iter()
                .filter(|entry| cell_owners[entry.entity.index()] == face)
                .collect::<Vec<_>>();
            match parent_cells.len() {
                0 | 2 => continue,
                1 => {}
                count => {
                    return Err(invalid_artifact(format!(
                        "region face {face} facet {facet_index} has {count} adjacent parent cells"
                    )));
                }
            }
            let points = entity_segment(mesh, facet)?;
            let candidates = edges
                .iter()
                .filter(|edge| {
                    edge.parent_face == face
                        && segment_lies_on_edge(points, **edge, region.tolerance_m())
                })
                .collect::<Vec<_>>();
            if candidates.len() != 1 {
                let names = region
                    .entity_sets()
                    .iter()
                    .filter(|set| {
                        set.dimension() == EDGE_DIMENSION
                            && candidates
                                .iter()
                                .any(|edge| set.members().contains(&edge.index))
                    })
                    .map(|set| set.name())
                    .collect::<Vec<_>>();
                if names.len() > 1 {
                    return Err(invalid_artifact(format!(
                        "frontier facet {facet_index} is ambiguous between region entity sets '{}' and '{}'",
                        names[0], names[1]
                    )));
                }
                return Err(invalid_artifact(format!(
                    "parent-relative frontier facet {facet_index} of face {face} matches {} region edges",
                    candidates.len()
                )));
            }
            let edge = *candidates[0];
            validate_parent_incidence(mesh, parent_cells[0].entity, edge, region.tolerance_m())?;
            assignments[edge.index].push((
                facet_index,
                outward_orientation(points, edge, region.tolerance_m())?,
            ));
        }
    }
    validate_shared_frontier_incidence(mesh, edges, &assignments, region.tolerance_m())?;
    assignments
        .into_iter()
        .enumerate()
        .map(|(edge_index, mut members)| {
            members.sort_by_key(|entry| entry.0);
            if members.is_empty() {
                return Err(invalid_artifact(format!(
                    "region edge {edge_index} has no parent-relative mesh facets"
                )));
            }
            validate_edge_coverage(mesh, edges[edge_index], &members, region.tolerance_m())?;
            Ok(WireFrontierAssignment {
                parent_face: portable(edges[edge_index].parent_face, "parent face")?,
                geometry_edge: portable(edge_index, "geometry edge")?,
                facet_indices: members
                    .iter()
                    .map(|entry| portable(entry.0, "mesh facet"))
                    .collect::<Result<Vec<_>, _>>()?,
                parent_outward: members.iter().map(|entry| entry.1).collect(),
            })
        })
        .collect()
}

fn validate_shared_frontier_incidence(
    mesh: &SimplicialMesh,
    edges: &[RegionEdge],
    assignments: &[Vec<(usize, WireFacetOutward)>],
    tolerance: f64,
) -> Result<(), Diagnostic> {
    for edge in edges {
        for &(facet, _) in &assignments[edge.index] {
            let points = entity_segment(mesh, MeshEntity::new(EDGE_DIMENSION, facet))?;
            for other in edges.iter().filter(|other| {
                other.parent_face != edge.parent_face
                    && segment_lies_on_edge(points, **other, tolerance)
            }) {
                if !assignments[other.index]
                    .iter()
                    .any(|&(candidate, _)| candidate == facet)
                {
                    return Err(invalid_artifact(format!(
                        "coincident region edges {} and {} require identical shared mesh-facet incidence",
                        edge.index, other.index
                    )));
                }
            }
        }
    }
    Ok(())
}

fn validate_parent_incidence(
    mesh: &SimplicialMesh,
    cell: MeshEntity,
    edge: RegionEdge,
    tolerance: f64,
) -> Result<(), Diagnostic> {
    let triangle = entity_triangle(mesh, cell)?;
    let centroid = triangle_centroid(&triangle);
    let signed_distance = cross(edge.start, edge.end, centroid) / distance(edge.start, edge.end);
    if signed_distance <= tolerance {
        return Err(invalid_artifact(format!(
            "mesh incidence does not preserve parent-outward orientation for region edge {}",
            edge.index
        )));
    }
    Ok(())
}

fn outward_orientation(
    points: [[f64; 2]; 2],
    edge: RegionEdge,
    tolerance: f64,
) -> Result<WireFacetOutward, Diagnostic> {
    let edge_vector = [edge.end[0] - edge.start[0], edge.end[1] - edge.start[1]];
    let facet_vector = [points[1][0] - points[0][0], points[1][1] - points[0][1]];
    let projection = edge_vector[0].mul_add(facet_vector[0], edge_vector[1] * facet_vector[1]);
    if projection.abs() <= tolerance * distance(edge.start, edge.end) {
        return Err(invalid_artifact(format!(
            "mesh facet on region edge {} has ambiguous direction",
            edge.index
        )));
    }
    Ok(if projection > 0.0 {
        WireFacetOutward::RightOfCanonicalFacet
    } else {
        WireFacetOutward::LeftOfCanonicalFacet
    })
}

fn validate_edge_coverage(
    mesh: &SimplicialMesh,
    edge: RegionEdge,
    members: &[(usize, WireFacetOutward)],
    tolerance: f64,
) -> Result<(), Diagnostic> {
    let length = distance(edge.start, edge.end);
    let tangent = [
        (edge.end[0] - edge.start[0]) / length,
        (edge.end[1] - edge.start[1]) / length,
    ];
    let mut intervals = members
        .iter()
        .map(|(facet, _)| {
            let points = entity_segment(mesh, MeshEntity::new(EDGE_DIMENSION, *facet))?;
            let mut values = points.map(|point| {
                ((point[0] - edge.start[0]) * tangent[0] + (point[1] - edge.start[1]) * tangent[1])
                    / length
            });
            values.sort_by(f64::total_cmp);
            Ok(values)
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    intervals.sort_by(|left, right| left[0].total_cmp(&right[0]));
    let normalized_tolerance = tolerance / length;
    if intervals[0][0].abs() > normalized_tolerance
        || (intervals.last().expect("nonempty intervals")[1] - 1.0).abs() > normalized_tolerance
        || intervals
            .windows(2)
            .any(|pair| (pair[0][1] - pair[1][0]).abs() > normalized_tolerance)
    {
        return Err(invalid_artifact(format!(
            "mesh facets do not exactly cover region edge {}",
            edge.index
        )));
    }
    Ok(())
}

fn triangle_within_face(
    triangle: &[[f64; 2]; 3],
    face: &PlanarFace,
    region: &PlanarRegion,
) -> bool {
    let tolerance = region.tolerance_m();
    if triangle
        .iter()
        .any(|&point| !point_in_face_closed(point, face, region, tolerance))
        || !point_in_face_strict(triangle_centroid(triangle), face, region, tolerance)
    {
        return false;
    }
    let boundaries = std::iter::once(face.outer()).chain(face.holes().iter().map(Vec::as_slice));
    for boundary in boundaries {
        for position in 0..boundary.len() {
            let start = region.vertices()[boundary[position]];
            let end = region.vertices()[boundary[(position + 1) % boundary.len()]];
            if point_strictly_inside_triangle(start, triangle, tolerance) {
                return false;
            }
            for triangle_edge in 0..3 {
                let left = triangle[triangle_edge];
                let right = triangle[(triangle_edge + 1) % 3];
                if segments_properly_cross(left, right, start, end, tolerance)
                    || point_on_segment_interior(start, left, right, tolerance)
                {
                    return false;
                }
            }
        }
    }
    true
}

fn point_in_face_closed(
    point: [f64; 2],
    face: &PlanarFace,
    region: &PlanarRegion,
    tolerance: f64,
) -> bool {
    if point_on_loop(point, face.outer(), region.vertices(), tolerance)
        || face
            .holes()
            .iter()
            .any(|hole| point_on_loop(point, hole, region.vertices(), tolerance))
    {
        return true;
    }
    point_in_loop(point, face.outer(), region.vertices())
        && !face
            .holes()
            .iter()
            .any(|hole| point_in_loop(point, hole, region.vertices()))
}

fn point_in_face_strict(
    point: [f64; 2],
    face: &PlanarFace,
    region: &PlanarRegion,
    tolerance: f64,
) -> bool {
    !point_on_loop(point, face.outer(), region.vertices(), tolerance)
        && !face
            .holes()
            .iter()
            .any(|hole| point_on_loop(point, hole, region.vertices(), tolerance))
        && point_in_loop(point, face.outer(), region.vertices())
        && !face
            .holes()
            .iter()
            .any(|hole| point_in_loop(point, hole, region.vertices()))
}

fn point_on_loop(
    point: [f64; 2],
    boundary: &[usize],
    vertices: &[[f64; 2]],
    tolerance: f64,
) -> bool {
    (0..boundary.len()).any(|position| {
        point_on_segment(
            point,
            vertices[boundary[position]],
            vertices[boundary[(position + 1) % boundary.len()]],
            tolerance,
        )
    })
}

fn point_in_loop(point: [f64; 2], boundary: &[usize], vertices: &[[f64; 2]]) -> bool {
    let mut inside = false;
    for position in 0..boundary.len() {
        let current = vertices[boundary[position]];
        let next = vertices[boundary[(position + 1) % boundary.len()]];
        if (current[1] > point[1]) != (next[1] > point[1]) {
            let crossing = (next[0] - current[0]) * (point[1] - current[1])
                / (next[1] - current[1])
                + current[0];
            if point[0] < crossing {
                inside = !inside;
            }
        }
    }
    inside
}

fn point_on_segment(point: [f64; 2], start: [f64; 2], end: [f64; 2], tolerance: f64) -> bool {
    let length = distance(start, end);
    cross(start, end, point).abs() <= tolerance * length
        && point[0] >= start[0].min(end[0]) - tolerance
        && point[0] <= start[0].max(end[0]) + tolerance
        && point[1] >= start[1].min(end[1]) - tolerance
        && point[1] <= start[1].max(end[1]) + tolerance
}

fn point_on_segment_interior(
    point: [f64; 2],
    start: [f64; 2],
    end: [f64; 2],
    tolerance: f64,
) -> bool {
    point_on_segment(point, start, end, tolerance)
        && distance(point, start) > tolerance
        && distance(point, end) > tolerance
}

fn segments_properly_cross(
    first_start: [f64; 2],
    first_end: [f64; 2],
    second_start: [f64; 2],
    second_end: [f64; 2],
    tolerance: f64,
) -> bool {
    let first_scale = distance(first_start, first_end);
    let second_scale = distance(second_start, second_end);
    let a = cross(first_start, first_end, second_start);
    let b = cross(first_start, first_end, second_end);
    let c = cross(second_start, second_end, first_start);
    let d = cross(second_start, second_end, first_end);
    opposite_beyond(a, b, tolerance * first_scale)
        && opposite_beyond(c, d, tolerance * second_scale)
}

fn opposite_beyond(left: f64, right: f64, tolerance: f64) -> bool {
    (left > tolerance && right < -tolerance) || (left < -tolerance && right > tolerance)
}

fn point_strictly_inside_triangle(
    point: [f64; 2],
    triangle: &[[f64; 2]; 3],
    tolerance: f64,
) -> bool {
    (0..3).all(|edge| {
        cross(triangle[edge], triangle[(edge + 1) % 3], point)
            > tolerance * distance(triangle[edge], triangle[(edge + 1) % 3])
    })
}

fn segment_lies_on_edge(points: [[f64; 2]; 2], edge: RegionEdge, tolerance: f64) -> bool {
    points
        .iter()
        .all(|&point| point_on_segment(point, edge.start, edge.end, tolerance))
}

fn entity_triangle(mesh: &SimplicialMesh, entity: MeshEntity) -> Result<[[f64; 2]; 3], Diagnostic> {
    let points = entity_points(mesh, entity)?;
    points.try_into().map_err(|points: Vec<[f64; 2]>| {
        invalid_artifact(format!(
            "mesh cell requires three vertices, found {}",
            points.len()
        ))
    })
}

fn entity_segment(mesh: &SimplicialMesh, entity: MeshEntity) -> Result<[[f64; 2]; 2], Diagnostic> {
    let points = entity_points(mesh, entity)?;
    points.try_into().map_err(|points: Vec<[f64; 2]>| {
        invalid_artifact(format!(
            "mesh facet requires two vertices, found {}",
            points.len()
        ))
    })
}

fn entity_points(mesh: &SimplicialMesh, entity: MeshEntity) -> Result<Vec<[f64; 2]>, Diagnostic> {
    mesh.entity_vertices(entity)
        .ok_or_else(|| invalid_artifact("mesh entity has no vertex closure"))?
        .into_iter()
        .map(|vertex| {
            mesh.vertices()
                .get(vertex.index())
                .map(|coordinates| to_point(coordinates))
                .ok_or_else(|| invalid_artifact("mesh entity references an unavailable vertex"))
        })
        .collect()
}

pub(super) fn resolve_entity_set(
    wire: &WireAuthoredRegionCorrespondenceV1,
    dimension: usize,
    members: &[usize],
) -> Result<Vec<MeshEntity>, Diagnostic> {
    let mut entities = BTreeSet::new();
    match dimension {
        VERTEX_DIMENSION => {
            for &member in members {
                let assignment = wire
                    .vertices
                    .iter()
                    .find(|entry| usize::try_from(entry.geometry_vertex) == Ok(member))
                    .ok_or_else(|| invalid_artifact("region vertex set is not fully realized"))?;
                entities.insert(MeshEntity::new(
                    VERTEX_DIMENSION,
                    local(assignment.mesh_vertex, "mesh vertex")?,
                ));
            }
        }
        EDGE_DIMENSION => {
            for &member in members {
                let assignment = wire
                    .frontiers
                    .iter()
                    .find(|entry| usize::try_from(entry.geometry_edge) == Ok(member))
                    .ok_or_else(|| invalid_artifact("region edge set is not fully realized"))?;
                for &facet in &assignment.facet_indices {
                    entities.insert(MeshEntity::new(EDGE_DIMENSION, local(facet, "mesh facet")?));
                }
            }
        }
        FACE_DIMENSION => {
            for &member in members {
                let assignment = wire
                    .faces
                    .iter()
                    .find(|entry| usize::try_from(entry.geometry_face) == Ok(member))
                    .ok_or_else(|| invalid_artifact("region face set is not fully realized"))?;
                for &cell in &assignment.cell_indices {
                    entities.insert(MeshEntity::new(FACE_DIMENSION, local(cell, "mesh cell")?));
                }
            }
        }
        _ => {
            return Err(invalid_artifact(
                "region entity set dimension is unsupported",
            ));
        }
    }
    Ok(entities.into_iter().collect())
}

fn triangle_centroid(triangle: &[[f64; 2]; 3]) -> [f64; 2] {
    [
        (triangle[0][0] + triangle[1][0] + triangle[2][0]) / 3.0,
        (triangle[0][1] + triangle[1][1] + triangle[2][1]) / 3.0,
    ]
}

fn cross(start: [f64; 2], end: [f64; 2], point: [f64; 2]) -> f64 {
    (end[0] - start[0]).mul_add(
        point[1] - start[1],
        -((end[1] - start[1]) * (point[0] - start[0])),
    )
}

fn distance(left: [f64; 2], right: [f64; 2]) -> f64 {
    ((left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2)).sqrt()
}

fn to_point(coordinates: &[f64]) -> [f64; 2] {
    [coordinates[0], coordinates[1]]
}

fn portable(value: usize, label: &str) -> Result<u64, Diagnostic> {
    u64::try_from(value)
        .map_err(|_| invalid_artifact(format!("{label} index exceeds portable u64")))
}

pub(super) fn local(value: u64, label: &str) -> Result<usize, Diagnostic> {
    usize::try_from(value)
        .map_err(|_| invalid_artifact(format!("{label} index exceeds local usize")))
}
