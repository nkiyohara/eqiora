//! Opposite parent frontiers from exact authored planar topology.

use std::collections::{BTreeMap, BTreeSet};

use crate::{NamedEntitySet, PlanarRegion};

/// Canonical face loops keep material on their left, including clockwise holes.
/// Reversed vertex pairs therefore prove opposite parent-outward directions.
/// This proves the selected interface, which may be a strict common-frontier
/// subset. Numerical admission separately proves complete required coverage.
pub(crate) fn opposite_parent_interface(
    topology: &PlanarRegion,
    first_boundary: &NamedEntitySet,
    first_region: &NamedEntitySet,
    second_boundary: &NamedEntitySet,
    second_region: &NamedEntitySet,
) -> bool {
    let first_parents = first_region
        .members()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let second_parents = second_region
        .members()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if first_boundary.dimension() != 1
        || second_boundary.dimension() != 1
        || first_region.dimension() != 2
        || second_region.dimension() != 2
        || !first_parents.is_disjoint(&second_parents)
    {
        return false;
    }
    let frontier = |boundary: &NamedEntitySet, parent_members: &BTreeSet<usize>| {
        let boundary_members = boundary.members().iter().copied().collect::<BTreeSet<_>>();
        let mut selected = BTreeSet::new();
        let mut occurrences = BTreeMap::<(usize, usize), usize>::new();
        let mut index = 0usize;
        for (parent, face) in topology.faces().iter().enumerate() {
            for vertices in
                std::iter::once(face.outer()).chain(face.holes().iter().map(Vec::as_slice))
            {
                for (&a, &b) in vertices
                    .iter()
                    .zip(vertices.iter().cycle().skip(1))
                    .take(vertices.len())
                {
                    let owns = parent_members.contains(&parent);
                    if owns {
                        *occurrences.entry((a.min(b), a.max(b))).or_default() += 1;
                    }
                    if boundary_members.contains(&index) {
                        if !owns || !selected.insert((a, b)) {
                            return None;
                        }
                    }
                    index += 1;
                }
            }
        }
        if selected.is_empty()
            || selected.len() != boundary.members().len()
            || selected
                .iter()
                .any(|&(a, b)| occurrences.get(&(a.min(b), a.max(b))) != Some(&1))
        {
            return None;
        }
        Some(selected)
    };
    let Some(first) = frontier(first_boundary, &first_parents) else {
        return false;
    };
    let Some(second) = frontier(second_boundary, &second_parents) else {
        return false;
    };
    first == second.into_iter().map(|(a, b)| (b, a)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CanonicalGeometryV1, PlanarFace};

    #[test]
    fn diagonal_interface_is_owned_by_exact_opposite_parents() {
        // Canonical vertices: (0,0),(0,1),(1,0),(1,1). The two
        // counter-clockwise triangles share only directed edges 2->1 and 1->2.
        let region = PlanarRegion::new(
            vec![[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]],
            vec![
                PlanarFace::new(vec![0, 2, 1], vec![]),
                PlanarFace::new(vec![1, 2, 3], vec![]),
            ],
            vec![
                NamedEntitySet::new("first", 2, vec![0]),
                NamedEntitySet::new("second", 2, vec![1]),
                NamedEntitySet::new("both", 2, vec![0, 1]),
                NamedEntitySet::new("first_interface", 1, vec![1]),
                NamedEntitySet::new("second_interface", 1, vec![3]),
                NamedEntitySet::new("exterior", 1, vec![0]),
            ],
            1e-12,
        )
        .unwrap();
        let geometry = CanonicalGeometryV1::from_region(&region).unwrap();
        let accepts = |a, b, c, d| geometry.selections_form_opposite_parent_interface(a, b, c, d);
        assert!(accepts(
            "first_interface",
            "first",
            "second_interface",
            "second"
        ));
        assert!(accepts(
            "second_interface",
            "second",
            "first_interface",
            "first"
        ));
        assert!(!accepts(
            "first_interface",
            "second",
            "second_interface",
            "first"
        ));
        assert!(!accepts(
            "first_interface",
            "first",
            "first_interface",
            "first"
        ));
        assert!(!accepts("exterior", "first", "second_interface", "second"));
        assert!(!accepts(
            "first_interface",
            "both",
            "second_interface",
            "second"
        ));
        assert!(!accepts("absent", "first", "second_interface", "second"));
        let replay =
            CanonicalGeometryV1::decode_canonical(geometry.canonical_bytes(), Default::default())
                .unwrap();
        assert!(replay.selections_form_opposite_parent_interface(
            "first_interface",
            "first",
            "second_interface",
            "second"
        ));
    }
    #[test]
    fn arbitrary_parent_count_uses_edge_incidence_and_rejects_internal_frontiers() {
        for count in [2, 3, 5] {
            let vertices = (0..=count)
                .flat_map(|x| [[x as f64, 0.0], [x as f64, 1.0]])
                .collect();
            let faces = (0..count)
                .map(|i| PlanarFace::new(vec![2 * i, 2 * i + 2, 2 * i + 3, 2 * i + 1], vec![]))
                .collect();
            let mut sets = Vec::new();
            for i in 0..count {
                sets.push(NamedEntitySet::new(format!("body{i}"), 2, vec![i]));
                sets.push(NamedEntitySet::new(format!("left{i}"), 1, vec![4 * i + 3]));
                sets.push(NamedEntitySet::new(format!("right{i}"), 1, vec![4 * i + 1]));
            }
            sets.push(NamedEntitySet::new("first_two", 2, vec![0, 1]));
            let topology = PlanarRegion::new(vertices, faces, sets, 1e-12).unwrap();
            let geometry = CanonicalGeometryV1::from_region(&topology).unwrap();
            for i in 0..count - 1 {
                assert!(geometry.selections_form_opposite_parent_interface(
                    &format!("right{i}"),
                    &format!("body{i}"),
                    &format!("left{}", i + 1),
                    &format!("body{}", i + 1)
                ));
            }
            assert!(!geometry.selections_form_opposite_parent_interface(
                "right0",
                "first_two",
                "left1",
                "body1"
            ));
            if count > 2 {
                assert!(geometry.selections_form_opposite_parent_interface(
                    "right1",
                    "first_two",
                    "left2",
                    "body2"
                ));
                assert!(!geometry.selections_form_opposite_parent_interface(
                    "right0",
                    "first_two",
                    "left2",
                    "body2"
                ));
            }
        }
    }
    #[test]
    fn hole_frontiers_reverse_filled_parent_orientation_and_match_exact_selected_extent() {
        let topology = PlanarRegion::new(
            vec![
                [0.0, 0.0],
                [0.0, 3.0],
                [1.0, 1.0],
                [1.0, 2.0],
                [2.0, 1.0],
                [2.0, 2.0],
                [3.0, 0.0],
                [3.0, 3.0],
            ],
            vec![
                PlanarFace::new(vec![0, 6, 7, 1], vec![vec![2, 3, 5, 4]]),
                PlanarFace::new(vec![2, 4, 5, 3], vec![]),
            ],
            vec![
                NamedEntitySet::new("ring", 2, vec![0]),
                NamedEntitySet::new("fill", 2, vec![1]),
                NamedEntitySet::new("hole", 1, vec![4, 5, 6, 7]),
                NamedEntitySet::new("inner", 1, vec![8, 9, 10, 11]),
                NamedEntitySet::new("hole_part", 1, vec![4]),
                NamedEntitySet::new("inner_part", 1, vec![11]),
                NamedEntitySet::new("wrong_part", 1, vec![10]),
            ],
            1e-12,
        )
        .unwrap();
        let geometry = CanonicalGeometryV1::from_region(&topology).unwrap();
        assert!(
            geometry.selections_form_opposite_parent_interface("hole", "ring", "inner", "fill")
        );
        // Separate Connections may cover separate shared-frontier subsets.
        assert!(geometry.selections_form_opposite_parent_interface(
            "hole_part",
            "ring",
            "inner_part",
            "fill"
        ));
        assert!(!geometry.selections_form_opposite_parent_interface(
            "hole",
            "ring",
            "inner_part",
            "fill"
        ));
        assert!(!geometry.selections_form_opposite_parent_interface(
            "hole_part",
            "ring",
            "wrong_part",
            "fill"
        ));
    }
}
