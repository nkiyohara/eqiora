//! Unique cells, complete exact frontiers, and source topology membership.

#[path = "assignments/frontiers.rs"]
mod frontiers;

use super::exact::{self, Budget, Facet, Point, Scalar};
use super::{Assignment, Frontier, Orientation};
use crate::invalid_artifact;
use eqiora_core::Diagnostic;
use eqiora_geometry::CanonicalGeometryV1;
use eqiora_meshing::{MeshEntity, MeshTopology, SimplicialMesh};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Assignments {
    pub(super) vertices: Vec<Assignment>,
    pub(super) edges: Vec<Assignment>,
    pub(super) volumes: Vec<Assignment>,
    pub(super) frontiers: Vec<Frontier>,
}
struct Volume {
    facets: Vec<Facet>,
    volume6: Scalar,
}

pub(super) fn generate(
    geometry: &CanonicalGeometryV1,
    mesh: &SimplicialMesh,
) -> Result<Assignments, Diagnostic> {
    let source = geometry.polyhedral_vertices().ok_or_else(|| {
        invalid_artifact("correspondence requires exact convex polyhedral Geometry")
    })?;
    if mesh.topological_dimension() != 3
        || mesh.vertices().len() > 16384
        || mesh.cells().len() > 4096
    {
        return Err(invalid_artifact(
            "polyhedral correspondence requires bounded three-dimensional tetrahedra",
        ));
    }
    let mut budget = Budget::new();
    let source = source.iter().map(|p| exact::point(p)).collect::<Vec<_>>();
    let points = mesh
        .vertices()
        .iter()
        .map(|p| exact::point(p))
        .collect::<Vec<_>>();
    let mut volumes = Vec::new();
    let mut edges = BTreeSet::new();
    for volume in 0.. {
        let Some(ids) = geometry.polyhedral_volume_facets(volume) else {
            break;
        };
        let mut facets = Vec::new();
        let mut volume6 = Scalar::default();
        for &id in ids {
            let polygon = geometry
                .polyhedral_outward_facet(id, volume)
                .expect("Geometry-owned incidence");
            for (&a, &b) in polygon
                .iter()
                .zip(polygon.iter().cycle().skip(1))
                .take(polygon.len())
            {
                edges.insert((a.min(b), a.max(b)));
            }
            let facet_points = polygon
                .iter()
                .map(|&i| source[i].clone())
                .collect::<Vec<_>>();
            budget.charge(polygon.len())?;
            volume6 += exact::polygon_volume6(&facet_points);
            facets.push(Facet {
                id,
                normal: exact::polygon_area(&facet_points),
                points: facet_points,
            });
        }
        volumes.push(Volume { facets, volume6 });
    }
    let vertices = assign_vertices(&source, &points, &mut budget)?;
    let edge_assignments = assign_edges(&source, &points, mesh, &edges, &mut budget)?;
    let (cell_assignments, owners) = assign_cells(&points, mesh, &volumes, &mut budget)?;
    let frontiers = frontiers::assign(&points, mesh, &volumes, &owners, &mut budget)?;
    Ok(Assignments {
        vertices,
        edges: edge_assignments,
        volumes: cell_assignments,
        frontiers,
    })
}
fn assign_vertices(
    source: &[Point],
    points: &[Point],
    budget: &mut Budget,
) -> Result<Vec<Assignment>, Diagnostic> {
    source
        .iter()
        .enumerate()
        .map(|(id, point)| {
            budget.charge(points.len())?;
            let ids = points
                .iter()
                .enumerate()
                .filter(|(_, p)| *p == point)
                .map(|(i, _)| i as u64)
                .collect::<Vec<_>>();
            if ids.len() != 1 {
                return Err(invalid_artifact(
                    "each polyhedral vertex must match one unique exact mesh vertex",
                ));
            }
            Ok(Assignment {
                geometry_entity: id as u64,
                mesh_entities: ids,
            })
        })
        .collect()
}
fn mesh_vertices(mesh: &SimplicialMesh, entity: MeshEntity) -> Result<Vec<usize>, Diagnostic> {
    mesh.entity_vertices(entity)
        .map(|vertices| vertices.into_iter().map(|v| v.index()).collect())
        .ok_or_else(|| invalid_artifact("polyhedral mesh entity has no vertex closure"))
}
fn assign_edges(
    source: &[Point],
    points: &[Point],
    mesh: &SimplicialMesh,
    edges: &BTreeSet<(usize, usize)>,
    budget: &mut Budget,
) -> Result<Vec<Assignment>, Diagnostic> {
    let count = mesh
        .entity_count(1)
        .ok_or_else(|| invalid_artifact("polyhedral mesh has no edge stratum"))?;
    let mesh_edges = (0..count)
        .map(|id| mesh_vertices(mesh, MeshEntity::new(1, id)))
        .collect::<Result<Vec<_>, _>>()?;
    edges
        .iter()
        .enumerate()
        .map(|(geometry_entity, &(a, b))| {
            let mut intervals = Vec::new();
            let mut members = Vec::new();
            for (id, edge) in mesh_edges.iter().enumerate() {
                let (Some(t0), Some(t1)) = (
                    exact::segment_parameter(&points[edge[0]], &source[a], &source[b], budget)?,
                    exact::segment_parameter(&points[edge[1]], &source[a], &source[b], budget)?,
                ) else {
                    continue;
                };
                if t0 == t1 {
                    return Err(invalid_artifact(
                        "polyhedral mesh edge is exactly degenerate",
                    ));
                }
                intervals.push((t0.clone().min(t1.clone()), t0.max(t1)));
                members.push(id as u64);
            }
            intervals.sort();
            let mut end = Scalar::default();
            for (start, next) in intervals {
                if start != end {
                    return Err(invalid_artifact(
                        "polyhedral Geometry edge coverage is missing or overlaps",
                    ));
                }
                end = next;
            }
            if end != Scalar::from_integer(1.into()) {
                return Err(invalid_artifact(
                    "polyhedral Geometry edge coverage is incomplete",
                ));
            }
            Ok(Assignment {
                geometry_entity: geometry_entity as u64,
                mesh_entities: members,
            })
        })
        .collect()
}
fn assign_cells(
    points: &[Point],
    mesh: &SimplicialMesh,
    volumes: &[Volume],
    budget: &mut Budget,
) -> Result<(Vec<Assignment>, Vec<usize>), Diagnostic> {
    let mut owners = Vec::new();
    let mut members = vec![Vec::new(); volumes.len()];
    let mut measures = vec![Scalar::default(); volumes.len()];
    let cells = mesh
        .cells()
        .iter()
        .map(|cell| cell.iter().map(|&id| &points[id]).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    for (id, cell) in cells.iter().enumerate() {
        budget.charge(1)?;
        let determinant = exact::determinant(cell);
        if determinant <= Scalar::default() {
            return Err(invalid_artifact(
                "polyhedral mesh tetrahedron is exactly degenerate or inverted",
            ));
        }
        let mut candidates = Vec::new();
        for (parent, volume) in volumes.iter().enumerate() {
            let mut inside = true;
            for facet in &volume.facets {
                for point in cell {
                    inside &= facet.inside(point, budget)?;
                }
            }
            if inside {
                candidates.push(parent);
            }
        }
        let [parent] = candidates.as_slice() else {
            return Err(invalid_artifact(
                "each mesh tetrahedron must belong uniquely to one exact polyhedral parent volume",
            ));
        };
        owners.push(*parent);
        members[*parent].push(id as u64);
        measures[*parent] += determinant;
    }
    for (i, a) in cells.iter().enumerate() {
        for b in &cells[..i] {
            if !exact::separated(a, b, budget)? {
                return Err(invalid_artifact(
                    "polyhedral mesh tetrahedron interiors overlap",
                ));
            }
        }
    }
    for (id, volume) in volumes.iter().enumerate() {
        if members[id].is_empty() || measures[id] != volume.volume6 {
            return Err(invalid_artifact(
                "mesh cells do not cover the complete exact polyhedral volume",
            ));
        }
    }
    Ok((
        members
            .into_iter()
            .enumerate()
            .map(|(id, mesh_entities)| Assignment {
                geometry_entity: id as u64,
                mesh_entities,
            })
            .collect(),
        owners,
    ))
}
