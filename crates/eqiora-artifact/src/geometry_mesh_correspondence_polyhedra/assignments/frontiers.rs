use super::*;

pub(super) fn assign(
    points: &[Point],
    mesh: &SimplicialMesh,
    volumes: &[Volume],
    owners: &[usize],
    budget: &mut Budget,
) -> Result<Vec<Frontier>, Diagnostic> {
    let count = mesh
        .entity_count(2)
        .ok_or_else(|| invalid_artifact("polyhedral mesh has no facet stratum"))?;
    let mut result = Vec::new();
    let mut shared = BTreeMap::<usize, (Vec<u64>, Vec<Orientation>)>::new();
    let mut source_by_mesh_facet = BTreeMap::<usize, usize>::new();
    for (parent, volume) in volumes.iter().enumerate() {
        let mut assignments = volume
            .facets
            .iter()
            .map(|f| (f.id, Vec::<(usize, Orientation)>::new()))
            .collect::<BTreeMap<_, _>>();
        let mut areas = volume
            .facets
            .iter()
            .map(|f| (f.id, Point::default()))
            .collect::<BTreeMap<_, _>>();
        for id in 0..count {
            let entity = MeshEntity::new(2, id);
            let adjacent = mesh
                .incidence(entity, 3)
                .ok_or_else(|| invalid_artifact("mesh facet has no cell incidence"))?;
            let cells = adjacent
                .iter()
                .filter(|entry| owners[entry.entity.index()] == parent)
                .collect::<Vec<_>>();
            match cells.len() {
                0 | 2 => continue,
                1 => {}
                _ => {
                    return Err(invalid_artifact(
                        "polyhedral relative frontier has nonmanifold cell incidence",
                    ));
                }
            }
            let ids = mesh_vertices(mesh, entity)?;
            let triangle = ids
                .iter()
                .map(|&vertex| &points[vertex])
                .collect::<Vec<_>>();
            let mut candidates = Vec::new();
            for facet in &volume.facets {
                if facet.contains_triangle(&triangle, budget)? {
                    candidates.push(facet);
                }
            }
            let [facet] = candidates.as_slice() else {
                return Err(invalid_artifact(
                    "each relative frontier triangle must match exactly one complete source facet",
                ));
            };
            if source_by_mesh_facet
                .insert(id, facet.id)
                .is_some_and(|previous| previous != facet.id)
            {
                return Err(invalid_artifact(
                    "shared mesh incidence crosses distinct source Geometry facets",
                ));
            }
            budget.charge(1)?;
            let mut area = exact::triangle(triangle[0], triangle[1], triangle[2]);
            let sign = exact::dot(&area, &facet.normal);
            let orientation = if sign > Scalar::default() {
                Orientation::AlongCanonicalFacetNormal
            } else if sign < Scalar::default() {
                area = area.map(|x| -x);
                Orientation::AgainstCanonicalFacetNormal
            } else {
                return Err(invalid_artifact(
                    "polyhedral frontier triangle is exactly degenerate",
                ));
            };
            // The exact source halfspace owns the sign. At least one vertex of
            // the sole incident positive tetrahedron must be strictly interior.
            let cell = &mesh.cells()[cells[0].entity.index()];
            let interior = cell.iter().any(|&v| {
                exact::dot(&facet.normal, &exact::sub(&points[v], &facet.points[0]))
                    < Scalar::default()
            });
            budget.charge(cell.len())?;
            if !interior {
                return Err(invalid_artifact(
                    "polyhedral frontier parent-outward incidence is invalid",
                ));
            }
            exact::add(areas.get_mut(&facet.id).expect("source facet"), &area);
            assignments
                .get_mut(&facet.id)
                .expect("source facet")
                .push((id, orientation));
        }
        for facet in &volume.facets {
            let members = &assignments[&facet.id];
            if members.is_empty() || areas[&facet.id] != facet.normal {
                return Err(invalid_artifact(
                    "polyhedral source facet coverage is incomplete or overlaps",
                ));
            }
            let ids = members.iter().map(|(id, _)| *id as u64).collect::<Vec<_>>();
            let orientations = members.iter().map(|(_, o)| *o).collect::<Vec<_>>();
            if let Some((first_ids, first_orientations)) =
                shared.insert(facet.id, (ids.clone(), orientations.clone()))
                && (first_ids != ids
                    || first_orientations
                        .iter()
                        .zip(&orientations)
                        .any(|(a, b)| a == b))
            {
                return Err(invalid_artifact(
                    "shared polyhedral facet requires identical mesh membership and opposite parent incidence",
                ));
            }
            result.push(Frontier {
                parent_volume: parent as u64,
                geometry_facet: facet.id as u64,
                facet_indices: ids,
                parent_outward: orientations,
            });
        }
    }
    Ok(result)
}
