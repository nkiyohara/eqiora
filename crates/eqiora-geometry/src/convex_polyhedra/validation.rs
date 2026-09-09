//! Bounded convex-shell predicates; no mesh or nearest-entity interpretation.

use std::collections::{BTreeMap, BTreeSet};

use super::{edges, exact::Predicates, invalid};
use crate::{CanonicalGeometryLimits, NamedEntitySet};
use eqiora_core::Diagnostic;

// Charge geometric point/plane and axis projection predicates, including SAT
// edge-cross-edge axes. Raw loop limits alone do not bound this cubic stage.
const MAX_PREDICATES: usize = 16_777_216;
struct Budget(usize);
impl Budget {
    fn charge(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.0 = self
            .0
            .checked_sub(count)
            .ok_or_else(|| invalid("polyhedral validation predicate budget exceeded"))?;
        Ok(())
    }
}

pub(super) fn check_limits(
    vertices: &[[f64; 3]],
    shells: &[Vec<Vec<usize>>],
    sets: &[NamedEntitySet],
    limits: CanonicalGeometryLimits,
) -> Result<(), Diagnostic> {
    let facets = shells
        .iter()
        .try_fold(0usize, |n, s| n.checked_add(s.len()));
    let loops = shells
        .iter()
        .flatten()
        .try_fold(0usize, |n, p| n.checked_add(p.len()));
    let members = sets
        .iter()
        .try_fold(0usize, |n, s| n.checked_add(s.members().len()));
    if vertices.len() > limits.max_vertices
        || vertices.len() > limits.max_loop_indices
        || shells.is_empty()
        || shells.len() > limits.max_faces
        || facets.is_none_or(|n| n > limits.max_faces)
        || loops.is_none_or(|n| n > limits.max_loop_indices)
        || sets.len() > limits.max_entity_sets
        || members.is_none_or(|n| n > limits.max_entity_set_members)
    {
        return Err(invalid(
            "polyhedral geometry exceeds topology or entity-set limits",
        ));
    }
    Ok(())
}

struct Shell {
    vertices: Vec<usize>,
    edges: Vec<(usize, usize)>,
    normals: Vec<([f64; 3], [usize; 4])>,
}

pub(super) fn validate(
    vertices: &[[f64; 3]],
    shells: &[Vec<Vec<usize>>],
    precision: f64,
) -> Result<(), Diagnostic> {
    let mut budget = Budget(MAX_PREDICATES);
    let mut exact = Predicates::new(vertices);
    for (i, &a) in vertices.iter().enumerate() {
        for &b in &vertices[..i] {
            budget.charge(1)?;
            let d = sub(a, b);
            let distance = norm(d);
            if !distance.is_finite() || distance <= precision {
                return Err(invalid(
                    "polyhedral vertices are coincident or unresolved at geometry precision",
                ));
            }
        }
    }
    let shells = shells
        .iter()
        .map(|shell| validate_shell(vertices, shell, precision, &mut budget, &mut exact))
        .collect::<Result<Vec<_>, _>>()?;
    for (i, a) in shells.iter().enumerate() {
        for b in &shells[..i] {
            if !separated(vertices, a, b, &mut budget, &mut exact)? {
                return Err(invalid("polyhedral volume interiors overlap"));
            }
        }
    }
    Ok(())
}

fn validate_shell(
    vertices: &[[f64; 3]],
    polygons: &[Vec<usize>],
    precision: f64,
    budget: &mut Budget,
    exact: &mut Predicates,
) -> Result<Shell, Diagnostic> {
    if polygons.len() < 4 {
        return Err(invalid("polyhedral shell is not a closed volume"));
    }
    let ids = polygons
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if ids.len() < 4 {
        return Err(invalid("polyhedral shell has fewer than four vertices"));
    }
    // Relative coordinates avoid overflowing the average for translated bodies.
    let origin = vertices[*ids.first().expect("nonempty")];
    let mut centroid = [0.; 3];
    for &id in &ids {
        for (value, delta) in centroid.iter_mut().zip(sub(vertices[id], origin)) {
            *value += delta / ids.len() as f64;
        }
    }
    let mut incidence = BTreeMap::<(usize, usize), Vec<(usize, bool)>>::new();
    for polygon in polygons {
        exact.facet(polygon)?;
    }
    let normals = polygons
        .iter()
        .map(|polygon| polygon_normal(vertices, polygon, precision, budget))
        .collect::<Result<Vec<_>, _>>()?;
    for (facet, polygon) in polygons.iter().enumerate() {
        exact.convex_side(polygon, &ids)?;
        let normal = normals[facet];
        let a = vertices[polygon[0]];
        let interior_distance = dot(normal, sub(centroid, sub(a, origin)));
        if !interior_distance.is_finite() || interior_distance >= -precision {
            return Err(invalid(
                "polyhedral shell is degenerate or its facet orientation is not parent-outward",
            ));
        }
        for &id in &ids {
            budget.charge(1)?;
            let distance = dot(normal, sub(vertices[id], a));
            if !distance.is_finite() {
                return Err(invalid("polyhedral shell is nonconvex"));
            }
        }
        for (a, b) in edges(polygon) {
            incidence
                .entry((a.min(b), a.max(b)))
                .or_default()
                .push((facet, a < b));
        }
    }
    if incidence
        .values()
        .any(|uses| uses.len() != 2 || uses[0].1 == uses[1].1)
    {
        return Err(invalid(
            "polyhedral shell is open, nonmanifold, or has inconsistent facet orientation",
        ));
    }
    let mut reached = BTreeSet::from([0]);
    let mut pending = vec![0];
    while let Some(facet) = pending.pop() {
        for (a, b) in edges(&polygons[facet]) {
            for &(other, _) in &incidence[&(a.min(b), a.max(b))] {
                if reached.insert(other) {
                    pending.push(other);
                }
            }
        }
    }
    if reached.len() != polygons.len() {
        return Err(invalid("polyhedral shell has disconnected components"));
    }
    Ok(Shell {
        vertices: ids,
        edges: incidence.into_keys().collect(),
        normals: normals
            .into_iter()
            .zip(polygons.iter().map(|p| [p[0], p[1], p[0], p[2]]))
            .collect(),
    })
}

fn polygon_normal(
    vertices: &[[f64; 3]],
    polygon: &[usize],
    precision: f64,
    budget: &mut Budget,
) -> Result<[f64; 3], Diagnostic> {
    let origin = vertices[polygon[0]];
    let normal = unit(cross(
        sub(vertices[polygon[1]], origin),
        sub(vertices[polygon[2]], origin),
    ))
    .ok_or_else(|| invalid("polyhedral facet is degenerate"))?;
    for &id in polygon {
        budget.charge(1)?;
        let distance = dot(normal, sub(vertices[id], origin));
        if !distance.is_finite() || distance.abs() > precision {
            return Err(invalid(
                "polyhedral facet is not planar at geometry precision",
            ));
        }
    }
    // Every other vertex must lie strictly to the left of each directed edge
    // in the face plane: this also rejects crossed and collinear polygon loops.
    for (a, b) in edges(polygon) {
        let direction = unit(sub(vertices[b], vertices[a]))
            .ok_or_else(|| invalid("polyhedral facet edge is degenerate"))?;
        let inward = cross(normal, direction);
        for &id in polygon {
            if id == a || id == b {
                continue;
            }
            budget.charge(1)?;
            let distance = dot(inward, sub(vertices[id], vertices[a]));
            if !distance.is_finite() || distance <= precision {
                return Err(invalid(
                    "polyhedral facet is degenerate, crossed, or nonconvex at geometry precision",
                ));
            }
        }
    }
    Ok(normal)
}

fn separated(
    vertices: &[[f64; 3]],
    a: &Shell,
    b: &Shell,
    budget: &mut Budget,
    exact: &mut Predicates,
) -> Result<bool, Diagnostic> {
    for &(axis, ends) in a.normals.iter().chain(&b.normals) {
        if separating_axis(vertices, a, b, axis, budget)?
            && exact.separated(&a.vertices, &b.vertices, ends)?
        {
            return Ok(true);
        }
    }
    // Convex polyhedra can separate on an edge-edge axis even when no face
    // plane separates them. Conversely, vertex-containment tests miss crosses.
    for &(a0, a1) in &a.edges {
        for &(b0, b1) in &b.edges {
            budget.charge(1)?;
            let axis = cross(
                sub(vertices[a1], vertices[a0]),
                sub(vertices[b1], vertices[b0]),
            );
            if axis.iter().any(|x| !x.is_finite()) {
                return Err(invalid("polyhedral edge-axis arithmetic is nonfinite"));
            }
            if let Some(axis) = unit(axis)
                && separating_axis(vertices, a, b, axis, budget)?
                && exact.separated(&a.vertices, &b.vertices, [a0, a1, b0, b1])?
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn separating_axis(
    vertices: &[[f64; 3]],
    a: &Shell,
    b: &Shell,
    axis: [f64; 3],
    budget: &mut Budget,
) -> Result<bool, Diagnostic> {
    budget.charge(a.vertices.len() + b.vertices.len())?;
    let origin = vertices[a.vertices[0]];
    let project = |ids: &[usize]| -> Result<(f64, f64), Diagnostic> {
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for &id in ids {
            let x = dot(axis, sub(vertices[id], origin));
            if !x.is_finite() {
                return Err(invalid("polyhedral projection arithmetic is nonfinite"));
            }
            low = low.min(x);
            high = high.max(x);
        }
        Ok((low, high))
    };
    let (al, ah) = project(&a.vertices)?;
    let (bl, bh) = project(&b.vertices)?;
    // Precision never excuses positive overlap. Exact contact is admitted.
    Ok(ah <= bl || bh <= al)
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn norm(a: [f64; 3]) -> f64 {
    a[0].hypot(a[1]).hypot(a[2])
}
fn unit(a: [f64; 3]) -> Option<[f64; 3]> {
    let n = norm(a);
    (n.is_finite() && n > 0.).then(|| a.map(|x| x / n))
}
