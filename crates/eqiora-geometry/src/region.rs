//! Authored continuous geometry, defined before any Model or mesh exists.
//!
//! [`GeometryIdentity`](crate::GeometryIdentity) derives a geometry catalog
//! *from* a Model, which is why a Model can only own shapes the Kernel can
//! already name. A [`PlanarRegion`] reverses that direction: it owns exact
//! topology, an embedding, and a catalog of named entity sets, and a Model
//! later references it rather than generating it.
//!
//! Shape is Model meaning; mesh, refinement and placement are Realization
//! choices. So a region carries no mesh and no discretization — only the
//! producer's classification precision, which is part of geometry identity and
//! is reused by every membership decision made against it.
//!
//! # Frozen at straight-edged planar v1
//!
//! The shape vocabulary is deliberately narrow so that curved entities extend
//! this identity contract later instead of reopening it:
//!
//! - edges are straight, and a curve will be a different region kind rather
//!   than a flag on this one;
//! - coordinates are `f64` metres, named rather than implied;
//! - loop ordering is canonical, so one region has exactly one encoding;
//! - nothing here claims that two regions of equal area, of equal boundary
//!   length, or related by a rigid motion are the same geometry. Identity is
//!   the canonical form and nothing else.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::identity::GeometryEntity;

/// Topological dimension of a planar region.
pub const FACE_DIMENSION: usize = 2;
/// Topological dimension of a straight edge.
pub const EDGE_DIMENSION: usize = 1;
/// Topological dimension of a vertex.
pub const VERTEX_DIMENSION: usize = 0;

/// Smallest loop that bounds a positive area.
const MINIMUM_LOOP_VERTICES: usize = 3;

/// One closed straight-edged loop, as vertex indices in traversal order.
///
/// The closing edge from the last vertex back to the first is implied and is
/// never written, so a loop has exactly as many edges as vertices and cannot
/// be authored unclosed.
pub type PlanarLoop = Vec<usize>;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// One planar face: an outer loop and the holes cut from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanarFace {
    outer: PlanarLoop,
    holes: Vec<PlanarLoop>,
}

impl PlanarFace {
    /// One face bounded by `outer` with `holes` removed from it.
    #[must_use]
    pub const fn new(outer: PlanarLoop, holes: Vec<PlanarLoop>) -> Self {
        Self { outer, holes }
    }

    /// Outer loop, counter-clockwise once canonicalized.
    #[must_use]
    pub fn outer(&self) -> &[usize] {
        &self.outer
    }

    /// Hole loops, clockwise once canonicalized.
    #[must_use]
    pub fn holes(&self) -> &[PlanarLoop] {
        &self.holes
    }

    fn loops(&self) -> impl Iterator<Item = &PlanarLoop> {
        std::iter::once(&self.outer).chain(self.holes.iter())
    }
}

/// One named, dimension-homogeneous set of entities in a region.
///
/// The name is the geometry's own vocabulary. A Model aliases it; a mesh file
/// never supplies it, because a label read from an untrusted numerical artifact
/// would let that artifact decide physics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedEntitySet {
    name: String,
    dimension: usize,
    members: Vec<usize>,
}

impl NamedEntitySet {
    /// One named set of same-dimension entities.
    #[must_use]
    pub fn new(name: impl Into<String>, dimension: usize, members: Vec<usize>) -> Self {
        Self {
            name: name.into(),
            dimension,
            members,
        }
    }

    /// Region-local set name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Shared topological dimension of every member.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Member indices within their dimension.
    #[must_use]
    pub fn members(&self) -> &[usize] {
        &self.members
    }

    /// Members, as revision-local geometry entities.
    #[must_use]
    pub fn entities(&self) -> Vec<GeometryEntity> {
        self.members
            .iter()
            .map(|index| GeometryEntity::new(self.dimension, *index))
            .collect()
    }
}

/// A canonical straight-edged planar region with named entity sets.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanarRegion {
    vertices: Vec<[f64; 2]>,
    faces: Vec<PlanarFace>,
    entity_sets: Vec<NamedEntitySet>,
    tolerance_m: f64,
}

impl PlanarRegion {
    /// Canonicalize and admit one straight-edged planar region.
    ///
    /// Vertex order, loop rotation, loop orientation, face order and entity-set
    /// order are all normalized, so two authorings of the same region are the
    /// same region. `tolerance_m` is the producer's coherent-SI classification
    /// precision; it is part of geometry identity and is not mesh quality
    /// policy.
    ///
    /// # Errors
    /// Returns `EQ0901` for a non-finite coordinate, an invalid tolerance, a
    /// degenerate or self-intersecting loop, coincident vertices, a hole that
    /// is not strictly inside its face, nested holes, or an entity set that is
    /// empty, unnamed, duplicated, or names an entity that does not exist.
    pub fn new(
        vertices: Vec<[f64; 2]>,
        faces: Vec<PlanarFace>,
        entity_sets: Vec<NamedEntitySet>,
        tolerance_m: f64,
    ) -> Result<Self, Diagnostic> {
        if !tolerance_m.is_finite() || tolerance_m <= 0.0 {
            return Err(invalid(
                "geometry tolerance must be finite and positive in metres",
            ));
        }
        if vertices.iter().flatten().any(|value| !value.is_finite()) {
            return Err(invalid("geometry vertex coordinates must be finite metres"));
        }
        if faces.is_empty() {
            return Err(invalid("geometry must define at least one face"));
        }

        let (vertices, remap) = canonical_vertices(vertices, tolerance_m)?;
        let faces = canonical_faces(faces, &remap, &vertices)?;
        let edges = edge_count(&faces);
        let entity_sets = canonical_entity_sets(entity_sets, vertices.len(), edges, faces.len())?;

        Ok(Self {
            vertices,
            faces,
            entity_sets,
            tolerance_m,
        })
    }

    /// Canonical vertex coordinates in metres.
    #[must_use]
    pub fn vertices(&self) -> &[[f64; 2]] {
        &self.vertices
    }

    /// Canonical faces.
    #[must_use]
    pub fn faces(&self) -> &[PlanarFace] {
        &self.faces
    }

    /// Canonical named entity sets.
    #[must_use]
    pub fn entity_sets(&self) -> &[NamedEntitySet] {
        &self.entity_sets
    }

    /// One named entity set, if this region defines it.
    #[must_use]
    pub fn entity_set(&self, name: &str) -> Option<&NamedEntitySet> {
        self.entity_sets
            .iter()
            .find(|candidate| candidate.name == name)
    }

    /// Producer classification precision in metres.
    #[must_use]
    pub const fn tolerance_m(&self) -> f64 {
        self.tolerance_m
    }

    /// Total straight edges, enumerated from the canonical loops.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        edge_count(&self.faces)
    }
}

/// Sort vertices lexicographically and return the old-to-new index remap.
///
/// Sorting rather than preserving author order is what makes two authorings of
/// one region canonicalize identically. Coincidence is rejected rather than
/// merged, because merging would silently change the topology the author wrote.
fn canonical_vertices(
    vertices: Vec<[f64; 2]>,
    tolerance_m: f64,
) -> Result<(Vec<[f64; 2]>, Vec<usize>), Diagnostic> {
    let mut order: Vec<usize> = (0..vertices.len()).collect();
    order.sort_by(|left, right| {
        vertices[*left]
            .partial_cmp(&vertices[*right])
            .expect("coordinates are finite")
    });
    let sorted: Vec<[f64; 2]> = order.iter().map(|index| vertices[*index]).collect();
    for pair in sorted.windows(2) {
        if distance(pair[0], pair[1]) <= tolerance_m {
            return Err(invalid(
                "geometry vertices must be separated by more than the classification tolerance",
            ));
        }
    }
    let mut remap = vec![0; vertices.len()];
    for (position, original) in order.iter().enumerate() {
        remap[*original] = position;
    }
    Ok((sorted, remap))
}

/// Rotate, orient and order every loop so one region has one canonical form.
fn canonical_faces(
    faces: Vec<PlanarFace>,
    remap: &[usize],
    vertices: &[[f64; 2]],
) -> Result<Vec<PlanarFace>, Diagnostic> {
    let mut canonical = Vec::with_capacity(faces.len());
    for face in faces {
        let outer = canonical_loop(&face.outer, remap, vertices, true)?;
        let mut holes = face
            .holes
            .iter()
            .map(|hole| canonical_loop(hole, remap, vertices, false))
            .collect::<Result<Vec<_>, _>>()?;
        holes.sort();
        if holes.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid("geometry face repeats a hole loop"));
        }
        canonical.push(PlanarFace::new(outer, holes));
    }
    canonical.sort_by(|left, right| left.outer.cmp(&right.outer));
    if canonical
        .windows(2)
        .any(|pair| pair[0].outer == pair[1].outer)
    {
        return Err(invalid("geometry repeats a face outer loop"));
    }
    for face in &canonical {
        validate_face(face, vertices)?;
    }
    Ok(canonical)
}

/// Remap, check simplicity, rotate to the smallest vertex, and fix orientation.
///
/// Orientation is normalized rather than rejected: which way round an author
/// listed a loop is not information, and demanding it be right would make one
/// region have two spellings.
fn canonical_loop(
    original: &[usize],
    remap: &[usize],
    vertices: &[[f64; 2]],
    counter_clockwise: bool,
) -> Result<PlanarLoop, Diagnostic> {
    if original.len() < MINIMUM_LOOP_VERTICES {
        return Err(invalid("geometry loop must have at least three vertices"));
    }
    let mut remapped = Vec::with_capacity(original.len());
    for index in original {
        let Some(mapped) = remap.get(*index) else {
            return Err(invalid(
                "geometry loop names a vertex outside the vertex list",
            ));
        };
        remapped.push(*mapped);
    }
    if remapped.iter().collect::<BTreeSet<_>>().len() != remapped.len() {
        return Err(invalid("geometry loop repeats a vertex"));
    }
    let area = signed_area(&remapped, vertices);
    if area == 0.0 {
        return Err(invalid("geometry loop encloses no area"));
    }
    if (area > 0.0) != counter_clockwise {
        remapped.reverse();
    }
    let start = remapped
        .iter()
        .enumerate()
        .min_by_key(|(_, vertex)| **vertex)
        .map(|(position, _)| position)
        .expect("loop is non-empty");
    remapped.rotate_left(start);
    Ok(remapped)
}

fn validate_face(face: &PlanarFace, vertices: &[[f64; 2]]) -> Result<(), Diagnostic> {
    for candidate in face.loops() {
        if self_intersects(candidate, vertices) {
            return Err(invalid("geometry loop intersects itself"));
        }
    }
    let loops: Vec<&PlanarLoop> = face.loops().collect();
    for (position, left) in loops.iter().enumerate() {
        for right in loops.iter().skip(position + 1) {
            if loops_intersect(left, right, vertices) {
                return Err(invalid("geometry loops of one face intersect"));
            }
        }
    }
    for hole in &face.holes {
        if !loop_strictly_inside(hole, &face.outer, vertices) {
            return Err(invalid("geometry hole must lie strictly inside its face"));
        }
    }
    for (position, left) in face.holes.iter().enumerate() {
        for right in face.holes.iter().skip(position + 1) {
            if loop_strictly_inside(left, right, vertices)
                || loop_strictly_inside(right, left, vertices)
            {
                return Err(invalid("geometry holes must not contain each other"));
            }
        }
    }
    Ok(())
}

/// Total straight edges across every canonical loop.
fn edge_count(faces: &[PlanarFace]) -> usize {
    faces.iter().flat_map(PlanarFace::loops).map(Vec::len).sum()
}

fn canonical_entity_sets(
    entity_sets: Vec<NamedEntitySet>,
    vertex_count: usize,
    edge_count: usize,
    face_count: usize,
) -> Result<Vec<NamedEntitySet>, Diagnostic> {
    let mut canonical = Vec::with_capacity(entity_sets.len());
    for set in entity_sets {
        if set.name.trim().is_empty() {
            return Err(invalid("geometry entity set must be named"));
        }
        let limit = match set.dimension {
            VERTEX_DIMENSION => vertex_count,
            EDGE_DIMENSION => edge_count,
            FACE_DIMENSION => face_count,
            _ => return Err(invalid("geometry entity set dimension must be 0, 1, or 2")),
        };
        let mut members = set.members;
        members.sort_unstable();
        members.dedup();
        if members.is_empty() {
            return Err(invalid("geometry entity set must not be empty"));
        }
        if members.iter().any(|index| *index >= limit) {
            return Err(invalid(
                "geometry entity set names an entity that does not exist",
            ));
        }
        canonical.push(NamedEntitySet::new(set.name, set.dimension, members));
    }
    canonical
        .sort_by(|left, right| (left.dimension, &left.name).cmp(&(right.dimension, &right.name)));
    if canonical
        .windows(2)
        .any(|pair| pair[0].name == pair[1].name)
    {
        return Err(invalid("geometry entity set names must be unique"));
    }
    Ok(canonical)
}

fn distance(left: [f64; 2], right: [f64; 2]) -> f64 {
    ((left[0] - right[0]).powi(2) + (left[1] - right[1]).powi(2)).sqrt()
}

/// Twice the signed area, positive when the loop runs counter-clockwise.
fn signed_area(loop_vertices: &[usize], vertices: &[[f64; 2]]) -> f64 {
    let mut total = 0.0;
    for position in 0..loop_vertices.len() {
        let current = vertices[loop_vertices[position]];
        let next = vertices[loop_vertices[(position + 1) % loop_vertices.len()]];
        total += current[0].mul_add(next[1], -(next[0] * current[1]));
    }
    total
}

fn segment(loop_vertices: &[usize], position: usize) -> (usize, usize) {
    (
        loop_vertices[position],
        loop_vertices[(position + 1) % loop_vertices.len()],
    )
}

fn self_intersects(loop_vertices: &[usize], vertices: &[[f64; 2]]) -> bool {
    let count = loop_vertices.len();
    (0..count).any(|left| {
        ((left + 1)..count).any(|right| {
            let adjacent = right == left + 1 || (left == 0 && right + 1 == count);
            !adjacent
                && segments_cross(
                    segment(loop_vertices, left),
                    segment(loop_vertices, right),
                    vertices,
                )
        })
    })
}

fn loops_intersect(left: &[usize], right: &[usize], vertices: &[[f64; 2]]) -> bool {
    (0..left.len()).any(|first| {
        (0..right.len())
            .any(|second| segments_cross(segment(left, first), segment(right, second), vertices))
    })
}

fn orientation(origin: [f64; 2], first: [f64; 2], second: [f64; 2]) -> f64 {
    (first[0] - origin[0]).mul_add(
        second[1] - origin[1],
        -((second[0] - origin[0]) * (first[1] - origin[1])),
    )
}

fn segments_cross(left: (usize, usize), right: (usize, usize), vertices: &[[f64; 2]]) -> bool {
    if left.0 == right.0 || left.0 == right.1 || left.1 == right.0 || left.1 == right.1 {
        return false;
    }
    let (start, end) = (vertices[left.0], vertices[left.1]);
    let (other_start, other_end) = (vertices[right.0], vertices[right.1]);
    let first = orientation(start, end, other_start);
    let second = orientation(start, end, other_end);
    let third = orientation(other_start, other_end, start);
    let fourth = orientation(other_start, other_end, end);
    (first > 0.0) != (second > 0.0) && (third > 0.0) != (fourth > 0.0)
}

/// Whether every vertex of `inner` lies strictly inside `outer`.
///
/// Loop intersection is checked separately, so a strictly-interior vertex set
/// is enough to place the loop.
fn loop_strictly_inside(inner: &[usize], outer: &[usize], vertices: &[[f64; 2]]) -> bool {
    inner
        .iter()
        .all(|vertex| point_strictly_inside(vertices[*vertex], outer, vertices))
}

fn point_strictly_inside(point: [f64; 2], boundary: &[usize], vertices: &[[f64; 2]]) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Unit square with a centred square hole, authored the obvious way.
    fn square_with_hole() -> PlanarRegion {
        PlanarRegion::new(
            vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.25, 0.25],
                [0.75, 0.25],
                [0.75, 0.75],
                [0.25, 0.75],
            ],
            vec![PlanarFace::new(vec![0, 1, 2, 3], vec![vec![4, 5, 6, 7]])],
            vec![
                NamedEntitySet::new("exterior", EDGE_DIMENSION, vec![0, 1, 2, 3]),
                NamedEntitySet::new("hole", EDGE_DIMENSION, vec![4, 5, 6, 7]),
                NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
            ],
            1.0e-9,
        )
        .expect("a square with a square hole is a region")
    }

    fn filled_square() -> PlanarRegion {
        PlanarRegion::new(
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            vec![PlanarFace::new(vec![0, 1, 2, 3], Vec::new())],
            vec![NamedEntitySet::new(
                "exterior",
                EDGE_DIMENSION,
                vec![0, 1, 2, 3],
            )],
            1.0e-9,
        )
        .expect("a square is a region")
    }

    #[test]
    fn a_square_with_a_hole_canonicalizes_stably() {
        assert_eq!(square_with_hole(), square_with_hole());
        assert_eq!(square_with_hole().edge_count(), 8);
    }

    #[test]
    fn filling_the_hole_changes_the_identity() {
        // The hole is part of what the geometry is, not decoration on it.
        assert_ne!(square_with_hole(), filled_square());
    }

    #[test]
    fn author_order_rotation_and_orientation_do_not_change_the_identity() {
        // Same region, authored with vertices listed in a different order, the
        // outer loop rotated and reversed, and the hole reversed.
        let rotated = PlanarRegion::new(
            vec![
                [0.75, 0.75],
                [0.0, 1.0],
                [1.0, 1.0],
                [0.25, 0.25],
                [1.0, 0.0],
                [0.75, 0.25],
                [0.0, 0.0],
                [0.25, 0.75],
            ],
            vec![PlanarFace::new(vec![2, 4, 6, 1], vec![vec![3, 7, 0, 5]])],
            vec![
                NamedEntitySet::new("hole", EDGE_DIMENSION, vec![4, 5, 6, 7]),
                NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
                NamedEntitySet::new("exterior", EDGE_DIMENSION, vec![0, 1, 2, 3]),
            ],
            1.0e-9,
        )
        .expect("the same region authored differently is still a region");
        assert_eq!(rotated, square_with_hole());
    }

    #[test]
    fn a_self_intersecting_loop_is_rejected() {
        // Asymmetric on purpose. A symmetric bowtie has zero signed area and is
        // caught by the degeneracy check first, which would leave the
        // self-intersection check untested.
        let bowtie = PlanarRegion::new(
            vec![[0.0, 0.0], [2.0, 0.0], [0.0, 2.0], [1.0, 3.0]],
            vec![PlanarFace::new(vec![0, 1, 2, 3], Vec::new())],
            vec![NamedEntitySet::new("edges", EDGE_DIMENSION, vec![0])],
            1.0e-9,
        );
        assert!(bowtie.unwrap_err().message().contains("intersects itself"));
    }

    #[test]
    fn a_hole_outside_its_face_is_rejected() {
        let outside = PlanarRegion::new(
            vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [2.0, 2.0],
                [3.0, 2.0],
                [3.0, 3.0],
                [2.0, 3.0],
            ],
            vec![PlanarFace::new(vec![0, 1, 2, 3], vec![vec![4, 5, 6, 7]])],
            vec![NamedEntitySet::new("edges", EDGE_DIMENSION, vec![0])],
            1.0e-9,
        );
        assert!(
            outside
                .unwrap_err()
                .message()
                .contains("strictly inside its face")
        );
    }

    #[test]
    fn coincident_vertices_are_rejected_rather_than_merged() {
        // Merging would silently change the topology the author wrote.
        let coincident = PlanarRegion::new(
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [1.0, 1.0 + 1.0e-12]],
            vec![PlanarFace::new(vec![0, 1, 2, 3], Vec::new())],
            vec![NamedEntitySet::new("edges", EDGE_DIMENSION, vec![0])],
            1.0e-9,
        );
        assert!(
            coincident
                .unwrap_err()
                .message()
                .contains("classification tolerance")
        );
    }

    #[test]
    fn a_degenerate_loop_is_rejected() {
        let collinear = PlanarRegion::new(
            vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
            vec![PlanarFace::new(vec![0, 1, 2], Vec::new())],
            vec![NamedEntitySet::new("edges", EDGE_DIMENSION, vec![0])],
            1.0e-9,
        );
        assert!(
            collinear
                .unwrap_err()
                .message()
                .contains("encloses no area")
        );
    }

    #[test]
    fn an_entity_set_naming_a_missing_entity_is_rejected() {
        let missing = PlanarRegion::new(
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            vec![PlanarFace::new(vec![0, 1, 2, 3], Vec::new())],
            vec![NamedEntitySet::new("edges", EDGE_DIMENSION, vec![9])],
            1.0e-9,
        );
        assert!(missing.unwrap_err().message().contains("does not exist"));
    }

    #[test]
    fn entity_sets_resolve_to_revision_local_entities() {
        let region = square_with_hole();
        let hole = region.entity_set("hole").expect("the hole is named");
        assert_eq!(hole.entities().len(), 4);
        assert!(
            hole.entities()
                .iter()
                .all(|entity| entity.dimension() == EDGE_DIMENSION)
        );
        assert!(region.entity_set("nose").is_none());
    }
}
