//! Binary-rational geometry predicates for one explicitly bound Mesh occurrence.

use crate::invalid_artifact;
use eqiora_core::Diagnostic;
use num_rational::BigRational;

pub(super) type Scalar = BigRational;
pub(super) type Point = [Scalar; 3];
pub(super) struct Budget(usize);
impl Budget {
    pub(super) fn new() -> Self {
        Self(262_144)
    }
    pub(super) fn charge(&mut self, n: usize) -> Result<(), Diagnostic> {
        self.0 = self.0.checked_sub(n).ok_or_else(|| {
            invalid_artifact("polyhedral correspondence exact predicate budget exceeded")
        })?;
        Ok(())
    }
}
pub(super) fn point(p: &[f64]) -> Point {
    std::array::from_fn(|i| Scalar::from_float(p[i]).expect("finite validated coordinate"))
}
pub(super) fn sub(a: &Point, b: &Point) -> Point {
    std::array::from_fn(|i| &a[i] - &b[i])
}
pub(super) fn add(a: &mut Point, b: &Point) {
    for (a, b) in a.iter_mut().zip(b) {
        *a += b;
    }
}
pub(super) fn cross(a: &Point, b: &Point) -> Point {
    [
        &a[1] * &b[2] - &a[2] * &b[1],
        &a[2] * &b[0] - &a[0] * &b[2],
        &a[0] * &b[1] - &a[1] * &b[0],
    ]
}
pub(super) fn dot(a: &Point, b: &Point) -> Scalar {
    &a[0] * &b[0] + &a[1] * &b[1] + &a[2] * &b[2]
}
pub(super) fn triangle(a: &Point, b: &Point, c: &Point) -> Point {
    cross(&sub(b, a), &sub(c, a))
}
pub(super) fn determinant(points: &[&Point]) -> Scalar {
    dot(
        &sub(points[1], points[0]),
        &cross(&sub(points[2], points[0]), &sub(points[3], points[0])),
    )
}
pub(super) fn polygon_area(points: &[Point]) -> Point {
    let mut result = Point::default();
    for pair in points[1..].windows(2) {
        add(&mut result, &triangle(&points[0], &pair[0], &pair[1]));
    }
    result
}
pub(super) fn polygon_volume6(points: &[Point]) -> Scalar {
    points[1..]
        .windows(2)
        .map(|pair| dot(&points[0], &cross(&pair[0], &pair[1])))
        .sum()
}

pub(super) struct Facet {
    pub(super) id: usize,
    pub(super) points: Vec<Point>,
    pub(super) normal: Point,
}
impl Facet {
    pub(super) fn inside(&self, p: &Point, budget: &mut Budget) -> Result<bool, Diagnostic> {
        budget.charge(1)?;
        Ok(dot(&self.normal, &sub(p, &self.points[0])) <= Scalar::default())
    }
    pub(super) fn contains_triangle(
        &self,
        points: &[&Point],
        budget: &mut Budget,
    ) -> Result<bool, Diagnostic> {
        let zero = Scalar::default();
        for &p in points {
            budget.charge(1)?;
            if dot(&self.normal, &sub(p, &self.points[0])) != zero {
                return Ok(false);
            }
            for (a, b) in self
                .points
                .iter()
                .zip(self.points.iter().cycle().skip(1))
                .take(self.points.len())
            {
                budget.charge(1)?;
                if dot(&cross(&self.normal, &sub(b, a)), &sub(p, a)) < zero {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

pub(super) fn separated(
    a: &[&Point],
    b: &[&Point],
    budget: &mut Budget,
) -> Result<bool, Diagnostic> {
    for tetra in [a, b] {
        for face in [[0, 1, 2], [0, 1, 3], [0, 2, 3], [1, 2, 3]] {
            if axis_separates(
                a,
                b,
                &triangle(tetra[face[0]], tetra[face[1]], tetra[face[2]]),
                budget,
            )? {
                return Ok(true);
            }
        }
    }
    let edges = [[0, 1], [0, 2], [0, 3], [1, 2], [1, 3], [2, 3]];
    for ea in edges {
        for eb in edges {
            let axis = cross(&sub(a[ea[1]], a[ea[0]]), &sub(b[eb[1]], b[eb[0]]));
            if axis_separates(a, b, &axis, budget)? {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn axis_separates(
    a: &[&Point],
    b: &[&Point],
    axis: &Point,
    budget: &mut Budget,
) -> Result<bool, Diagnostic> {
    budget.charge(a.len() + b.len())?;
    if axis.iter().all(|x| x == &Scalar::default()) {
        return Ok(false);
    }
    let project = |points: &[&Point]| {
        let first = dot(axis, points[0]);
        points[1..]
            .iter()
            .map(|p| dot(axis, p))
            .fold((first.clone(), first), |(low, high), x| {
                (low.min(x.clone()), high.max(x))
            })
    };
    let (al, ah) = project(a);
    let (bl, bh) = project(b);
    Ok(ah <= bl || bh <= al)
}

pub(super) fn segment_parameter(
    p: &Point,
    a: &Point,
    b: &Point,
    budget: &mut Budget,
) -> Result<Option<Scalar>, Diagnostic> {
    budget.charge(1)?;
    let delta = sub(b, a);
    let axis = delta
        .iter()
        .position(|x| x != &Scalar::default())
        .expect("nondegenerate Geometry edge");
    let t = (&p[axis] - &a[axis]) / &delta[axis];
    if t < Scalar::default()
        || t > Scalar::from_integer(1.into())
        || (0..3).any(|i| &a[i] + &t * &delta[i] != p[i])
    {
        return Ok(None);
    }
    Ok(Some(t))
}
