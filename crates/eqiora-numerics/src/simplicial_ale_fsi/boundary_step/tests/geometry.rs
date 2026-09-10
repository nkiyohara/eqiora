//! Exact authored loops of the boundary-word fixture, before mesh subdivision.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_geometry::{NamedEntitySet, PlanarFace, PlanarRegion};

pub(super) fn authored_region(
    points: &[Vec<f64>],
    boundary: &[usize],
    split: usize,
    apex: usize,
) -> PlanarRegion {
    let mut vertices = boundary
        .iter()
        .map(|&index| [points[index][0], points[index][1]])
        .collect::<Vec<_>>();
    let apex_vertex = vertices.len();
    vertices.push([points[apex][0], points[apex][1]]);
    let mut fluid = (0..boundary.len()).collect::<Vec<_>>();
    fluid.insert(split + 1, apex_vertex);
    let solid = vec![apex_vertex, (split + 1) % boundary.len(), split];
    let faces = vec![
        PlanarFace::new(fluid, vec![]),
        PlanarFace::new(solid, vec![]),
    ];
    let canonical = PlanarRegion::new(vertices.clone(), faces.clone(), vec![], 1e-15).unwrap();
    let solid = canonical
        .faces()
        .iter()
        .position(|face| face.outer().len() == 3)
        .unwrap();
    let fluid = 1 - solid;
    let mut edges = BTreeMap::<(usize, usize), Vec<(usize, usize)>>::new();
    let mut ordinal = 0;
    for (face, geometry) in canonical.faces().iter().enumerate() {
        let vertices = geometry.outer();
        for (&a, &b) in vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .take(vertices.len())
        {
            edges
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push((face, ordinal));
            ordinal += 1;
        }
    }
    let mut outer = [BTreeSet::new(), BTreeSet::new()];
    let mut contact = [BTreeSet::new(), BTreeSet::new()];
    for owners in edges.values() {
        for &(face, edge) in owners {
            if owners.len() == 2 {
                contact[face].insert(edge);
            } else {
                outer[face].insert(edge);
            }
        }
    }
    let mut sets = vec![
        NamedEntitySet::new("fluid", 2, vec![fluid]),
        NamedEntitySet::new("solid", 2, vec![solid]),
    ];
    for (name, face) in [("fluid", fluid), ("solid", solid)] {
        sets.push(NamedEntitySet::new(
            format!("{name}_outer"),
            1,
            outer[face].iter().copied().collect(),
        ));
        sets.push(NamedEntitySet::new(
            format!("{name}_contact"),
            1,
            contact[face].iter().copied().collect(),
        ));
    }
    PlanarRegion::new(vertices, faces, sets, 1e-15).unwrap()
}
