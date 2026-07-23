use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::{GeometryMap, ReferenceCell};

/// Runtime-dimensional affine map `x(xi) = origin + J xi`.
///
/// The Jacobian may be rectangular: a `d`-dimensional reference entity may be
/// embedded in any physical dimension `g >= d`. Physical measure is scaled by
/// `sqrt(det(J^T J))`, which reduces to the absolute determinant for square
/// maps without losing embedded curves or surfaces.
#[derive(Debug, Clone, PartialEq)]
pub struct AffineGeometryMap {
    reference_cell: ReferenceCell,
    physical_dimension: usize,
    origin: Vec<f64>,
    jacobian: Vec<f64>,
    measure_scale: f64,
}

/// One directional derivative of an affine reference-to-physical map.
///
/// The primal map and its tangent share one fixed reference cell and physical
/// coordinate space. This is the geometry-level action consumed by local
/// operator differentiation; it does not assign design meaning to the
/// direction that produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct AffineGeometryLinearization {
    map: AffineGeometryMap,
    origin_tangent: Vec<f64>,
    jacobian_tangent: Vec<f64>,
    measure_scale_tangent: f64,
}

/// Orientation and scale-invariant conditioning of one square affine map.
///
/// The mean-ratio value is `d |det(J)|^(2/d) / ||J||_F^2`. It lies in
/// `(0, 1]` for a full-rank map and equals one for an orthogonally scaled
/// reference map. Orientation remains separate because unsigned measure and
/// conditioning cannot detect an inverted cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AffineMapQuality {
    signed_measure_scale: f64,
    mean_ratio: f64,
}

impl AffineMapQuality {
    /// Signed physical measure divided by reference measure.
    #[must_use]
    pub const fn signed_measure_scale(self) -> f64 {
        self.signed_measure_scale
    }

    /// Scale-invariant mean-ratio quality in `(0, 1]`.
    #[must_use]
    pub const fn mean_ratio(self) -> f64 {
        self.mean_ratio
    }
}

impl AffineGeometryLinearization {
    /// Lift a primal map into a zero geometry direction.
    ///
    /// # Errors
    /// Preserves affine tangent validation.
    pub fn stationary(map: AffineGeometryMap) -> Result<Self, Diagnostic> {
        let origin_tangent = vec![0.0; map.origin.len()];
        let jacobian_tangent = vec![0.0; map.jacobian.len()];
        Self::new(map, origin_tangent, jacobian_tangent)
    }

    /// Construct a finite tangent over one accepted affine map.
    ///
    /// # Errors
    /// Returns `EQ0803` for incompatible tangent shapes or non-finite data.
    pub fn new(
        map: AffineGeometryMap,
        origin_tangent: Vec<f64>,
        jacobian_tangent: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        if origin_tangent.len() != map.origin.len()
            || jacobian_tangent.len() != map.jacobian.len()
            || origin_tangent
                .iter()
                .chain(&jacobian_tangent)
                .any(|value| !value.is_finite())
        {
            return Err(invalid_geometry(
                "affine geometry tangent requires finite origin/Jacobian data matching the primal map",
            ));
        }
        let measure_scale_tangent = gram_measure_scale_jvp(
            &map.jacobian,
            &jacobian_tangent,
            map.physical_dimension,
            map.reference_cell.dimension(),
            map.measure_scale,
        )?;
        Ok(Self {
            map,
            origin_tangent,
            jacobian_tangent,
            measure_scale_tangent,
        })
    }

    /// Accepted primal affine map.
    #[must_use]
    pub const fn map(&self) -> &AffineGeometryMap {
        &self.map
    }

    /// Tangent of the physical origin `x(0)`.
    #[must_use]
    pub fn origin_tangent(&self) -> &[f64] {
        &self.origin_tangent
    }

    /// Row-major tangent of the geometry Jacobian.
    #[must_use]
    pub fn jacobian_tangent(&self) -> &[f64] {
        &self.jacobian_tangent
    }

    /// JVP of physical measure divided by reference measure.
    #[must_use]
    pub const fn measure_scale_tangent(&self) -> f64 {
        self.measure_scale_tangent
    }

    /// JVP of the inverse Jacobian of a square affine map.
    ///
    /// # Errors
    /// Returns `EQ0803` unless the primal map is square and the computed
    /// inverse action is finite.
    pub fn inverse_jacobian_tangent(&self) -> Result<Vec<f64>, Diagnostic> {
        let inverse = self.map.inverse_jacobian()?;
        let dimension = self.map.reference_cell.dimension();
        let mut tangent = vec![0.0; inverse.len()];
        for row in 0..dimension {
            for column in 0..dimension {
                let mut action = 0.0;
                for left in 0..dimension {
                    for right in 0..dimension {
                        action += inverse[row * dimension + left]
                            * self.jacobian_tangent[left * dimension + right]
                            * inverse[right * dimension + column];
                    }
                }
                tangent[row * dimension + column] = -action;
            }
        }
        if tangent.iter().any(|value| !value.is_finite()) {
            return Err(invalid_geometry(
                "affine inverse-Jacobian JVP produced a non-finite value",
            ));
        }
        Ok(tangent)
    }

    /// Evaluate physical coordinates and their geometry JVP at one reference point.
    ///
    /// # Errors
    /// Preserves the primal map validation and returns `EQ0803` for an
    /// incompatible tangent output or non-finite tangent.
    pub fn map_point_jvp(
        &self,
        reference: &[f64],
        physical: &mut [f64],
        physical_tangent: &mut [f64],
    ) -> Result<(), Diagnostic> {
        self.map.map_point(reference, physical)?;
        if physical_tangent.len() != self.map.physical_dimension {
            return Err(invalid_geometry(format!(
                "affine point JVP expected physical tangent length {}, received {}",
                self.map.physical_dimension,
                physical_tangent.len()
            )));
        }
        let reference_dimension = self.map.reference_cell.dimension();
        for (row, tangent) in physical_tangent.iter_mut().enumerate() {
            *tangent = self.origin_tangent[row]
                + reference
                    .iter()
                    .enumerate()
                    .map(|(column, coordinate)| {
                        self.jacobian_tangent[row * reference_dimension + column] * coordinate
                    })
                    .sum::<f64>();
        }
        if physical_tangent.iter().any(|value| !value.is_finite()) {
            return Err(invalid_geometry(
                "affine point JVP produced a non-finite tangent",
            ));
        }
        Ok(())
    }
}

impl AffineGeometryMap {
    /// Construct a full-rank affine geometry map.
    ///
    /// `jacobian` is row-major with shape
    /// `physical_dimension x reference_cell.dimension()`.
    ///
    /// # Errors
    /// Returns `EQ0803` for invalid dimensions, shapes, non-finite data, or a
    /// rank-deficient Jacobian.
    pub fn new(
        reference_cell: ReferenceCell,
        physical_dimension: usize,
        origin: Vec<f64>,
        jacobian: Vec<f64>,
    ) -> Result<Self, Diagnostic> {
        let reference_dimension = reference_cell.dimension();
        if physical_dimension == 0 || physical_dimension < reference_dimension {
            return Err(invalid_geometry(
                "affine geometry requires a positive physical dimension not smaller than its reference dimension",
            ));
        }
        let jacobian_length = physical_dimension
            .checked_mul(reference_dimension)
            .ok_or_else(|| invalid_geometry("affine Jacobian shape overflows usize"))?;
        if origin.len() != physical_dimension || jacobian.len() != jacobian_length {
            return Err(invalid_geometry(format!(
                "affine geometry expected origin/Jacobian lengths {physical_dimension}/{jacobian_length}, received {}/{}",
                origin.len(),
                jacobian.len()
            )));
        }
        if origin
            .iter()
            .chain(jacobian.iter())
            .any(|value| !value.is_finite())
        {
            return Err(invalid_geometry(
                "affine geometry coordinates and Jacobian must be finite",
            ));
        }
        let measure_scale = gram_measure_scale(&jacobian, physical_dimension, reference_dimension)?;
        Ok(Self {
            reference_cell,
            physical_dimension,
            origin,
            jacobian,
            measure_scale,
        })
    }

    /// Construct the affine map of a simplex from its physical vertices.
    ///
    /// Vertices are ordered as the origin vertex followed by the endpoints of
    /// the reference coordinate axes. A single vertex constructs a point map.
    ///
    /// # Errors
    /// Returns `EQ0803` for missing, inconsistent, non-finite, or affinely
    /// dependent vertices.
    pub fn from_simplex_vertices(vertices: Vec<Vec<f64>>) -> Result<Self, Diagnostic> {
        let Some(origin) = vertices.first() else {
            return Err(invalid_geometry(
                "simplex geometry requires at least one physical vertex",
            ));
        };
        let physical_dimension = origin.len();
        if physical_dimension == 0
            || vertices
                .iter()
                .any(|vertex| vertex.len() != physical_dimension)
        {
            return Err(invalid_geometry(
                "simplex geometry vertices require one common positive physical dimension",
            ));
        }
        let reference_dimension = vertices.len() - 1;
        let reference_cell = if reference_dimension == 0 {
            ReferenceCell::point()
        } else {
            ReferenceCell::simplex(reference_dimension)
                .map_err(|_| invalid_geometry("invalid simplex reference dimension"))?
        };
        let mut jacobian = vec![0.0; physical_dimension * reference_dimension];
        for column in 0..reference_dimension {
            for row in 0..physical_dimension {
                jacobian[row * reference_dimension + column] =
                    vertices[column + 1][row] - origin[row];
            }
        }
        Self::new(reference_cell, physical_dimension, origin.clone(), jacobian)
    }

    /// Physical origin `x(0)`.
    #[must_use]
    pub fn origin(&self) -> &[f64] {
        &self.origin
    }

    /// Constant row-major Jacobian.
    #[must_use]
    pub fn jacobian(&self) -> &[f64] {
        &self.jacobian
    }

    /// Physical measure divided by reference measure.
    #[must_use]
    pub const fn measure_scale(&self) -> f64 {
        self.measure_scale
    }

    /// Inverse of a square affine Jacobian, in row-major order.
    ///
    /// # Errors
    /// Returns `EQ0803` for an embedded/rectangular map or a numerical inverse
    /// failure. Full-rank square maps accepted by the constructor are expected
    /// to succeed.
    pub fn inverse_jacobian(&self) -> Result<Vec<f64>, Diagnostic> {
        let dimension = self.reference_cell.dimension();
        if dimension == 0 || self.physical_dimension != dimension {
            return Err(invalid_geometry(
                "inverse Jacobian requires one positive-dimensional square affine map",
            ));
        }
        invert_dense(&self.jacobian, dimension)
    }

    /// Evaluate orientation and mean-ratio quality of a square affine map.
    ///
    /// This operation deliberately does not reject negative orientation;
    /// mesh-level injectivity policy decides whether an orientation is valid.
    ///
    /// # Errors
    /// Returns `EQ0803` for a point/embedded map or non-finite quality data.
    pub fn square_quality(&self) -> Result<AffineMapQuality, Diagnostic> {
        let dimension = self.reference_cell.dimension();
        if dimension == 0 || self.physical_dimension != dimension {
            return Err(invalid_geometry(
                "affine map quality requires one positive-dimensional square map",
            ));
        }
        let determinant = determinant_dense(&self.jacobian, dimension)?;
        let frobenius_squared = self.jacobian.iter().map(|value| value * value).sum::<f64>();
        let mean_ratio =
            dimension as f64 * determinant.abs().powf(2.0 / dimension as f64) / frobenius_squared;
        if !determinant.is_finite()
            || !frobenius_squared.is_finite()
            || frobenius_squared <= 0.0
            || !mean_ratio.is_finite()
            || mean_ratio <= 0.0
        {
            return Err(invalid_geometry(
                "affine map orientation or mean-ratio quality is non-finite",
            ));
        }
        Ok(AffineMapQuality {
            signed_measure_scale: determinant,
            mean_ratio: mean_ratio.min(1.0),
        })
    }
}

impl GeometryMap for AffineGeometryMap {
    fn reference_cell(&self) -> ReferenceCell {
        self.reference_cell
    }

    fn physical_dimension(&self) -> usize {
        self.physical_dimension
    }

    fn map_point(&self, reference: &[f64], physical: &mut [f64]) -> Result<(), Diagnostic> {
        if !self.reference_cell.contains(reference) || physical.len() != self.physical_dimension {
            return Err(invalid_geometry(format!(
                "affine map expected a reference point in dimension {} and physical output length {}, received {}/{}",
                self.reference_cell.dimension(),
                self.physical_dimension,
                reference.len(),
                physical.len()
            )));
        }
        let reference_dimension = self.reference_cell.dimension();
        for (row, output) in physical.iter_mut().enumerate() {
            *output = self.origin[row]
                + reference
                    .iter()
                    .enumerate()
                    .map(|(column, coordinate)| {
                        self.jacobian[row * reference_dimension + column] * coordinate
                    })
                    .sum::<f64>();
        }
        if physical.iter().any(|coordinate| !coordinate.is_finite()) {
            return Err(invalid_geometry(
                "affine point mapping produced a non-finite coordinate",
            ));
        }
        Ok(())
    }

    fn jacobian_at(&self, reference: &[f64], jacobian: &mut [f64]) -> Result<(), Diagnostic> {
        if !self.reference_cell.contains(reference) || jacobian.len() != self.jacobian.len() {
            return Err(invalid_geometry(format!(
                "affine Jacobian expected a reference point in dimension {} and output length {}, received {}/{}",
                self.reference_cell.dimension(),
                self.jacobian.len(),
                reference.len(),
                jacobian.len()
            )));
        }
        jacobian.copy_from_slice(&self.jacobian);
        Ok(())
    }
}

fn gram_measure_scale(
    jacobian: &[f64],
    physical_dimension: usize,
    reference_dimension: usize,
) -> Result<f64, Diagnostic> {
    if reference_dimension == 0 {
        return Ok(1.0);
    }

    // Modified Gram-Schmidt computes the volume of the Jacobian column
    // parallelotope without explicitly forming and taking det(J^T J).
    let mut orthogonal_columns: Vec<Vec<f64>> = Vec::with_capacity(reference_dimension);
    let mut scale = 1.0;
    let maximum_column_norm = (0..reference_dimension)
        .map(|column| {
            (0..physical_dimension)
                .map(|row| jacobian[row * reference_dimension + column].powi(2))
                .sum::<f64>()
                .sqrt()
        })
        .fold(0.0, f64::max);
    let rank_tolerance = 128.0
        * f64::EPSILON
        * maximum_column_norm.max(f64::MIN_POSITIVE)
        * reference_dimension as f64;

    for column in 0..reference_dimension {
        let mut vector = (0..physical_dimension)
            .map(|row| jacobian[row * reference_dimension + column])
            .collect::<Vec<_>>();
        for basis in &orthogonal_columns {
            let projection = vector
                .iter()
                .zip(basis)
                .map(|(left, right)| left * right)
                .sum::<f64>();
            for (entry, basis_entry) in vector.iter_mut().zip(basis) {
                *entry -= projection * basis_entry;
            }
        }
        let norm = vector.iter().map(|value| value * value).sum::<f64>().sqrt();
        if !norm.is_finite() || norm <= rank_tolerance {
            return Err(invalid_geometry(
                "affine geometry Jacobian is rank deficient at numerical precision",
            ));
        }
        scale *= norm;
        if !scale.is_finite() {
            return Err(invalid_geometry(
                "affine geometry measure scale is not finite",
            ));
        }
        for entry in &mut vector {
            *entry /= norm;
        }
        orthogonal_columns.push(vector);
    }
    Ok(scale)
}

fn gram_measure_scale_jvp(
    jacobian: &[f64],
    jacobian_tangent: &[f64],
    physical_dimension: usize,
    reference_dimension: usize,
    measure_scale: f64,
) -> Result<f64, Diagnostic> {
    if reference_dimension == 0 {
        return Ok(0.0);
    }
    let mut gram = vec![0.0; reference_dimension * reference_dimension];
    let mut gram_tangent = vec![0.0; gram.len()];
    for row in 0..reference_dimension {
        for column in 0..reference_dimension {
            for physical in 0..physical_dimension {
                let left = jacobian[physical * reference_dimension + row];
                let right = jacobian[physical * reference_dimension + column];
                let left_tangent = jacobian_tangent[physical * reference_dimension + row];
                let right_tangent = jacobian_tangent[physical * reference_dimension + column];
                gram[row * reference_dimension + column] += left * right;
                gram_tangent[row * reference_dimension + column] +=
                    left_tangent * right + left * right_tangent;
            }
        }
    }
    let inverse = invert_dense(&gram, reference_dimension)?;
    let trace = (0..reference_dimension)
        .map(|row| {
            (0..reference_dimension)
                .map(|column| {
                    inverse[row * reference_dimension + column]
                        * gram_tangent[column * reference_dimension + row]
                })
                .sum::<f64>()
        })
        .sum::<f64>();
    let tangent = 0.5 * measure_scale * trace;
    if !tangent.is_finite() {
        return Err(invalid_geometry(
            "affine geometry measure JVP is not finite",
        ));
    }
    Ok(tangent)
}

fn invert_dense(matrix: &[f64], dimension: usize) -> Result<Vec<f64>, Diagnostic> {
    let width = dimension
        .checked_mul(2)
        .ok_or_else(|| invalid_geometry("affine inverse workspace shape overflows usize"))?;
    if dimension == 0 || matrix.len() != dimension * dimension {
        return Err(invalid_geometry(
            "affine inverse requires one nonempty square matrix",
        ));
    }
    let mut augmented = vec![0.0; dimension * width];
    let scale = matrix.iter().map(|value| value.abs()).fold(0.0, f64::max);
    let tolerance = 128.0 * f64::EPSILON * scale.max(f64::MIN_POSITIVE) * dimension as f64;
    for row in 0..dimension {
        augmented[row * width..row * width + dimension]
            .copy_from_slice(&matrix[row * dimension..(row + 1) * dimension]);
        augmented[row * width + dimension + row] = 1.0;
    }
    for column in 0..dimension {
        let pivot = (column..dimension)
            .max_by(|&left, &right| {
                augmented[left * width + column]
                    .abs()
                    .total_cmp(&augmented[right * width + column].abs())
            })
            .expect("nonempty pivot range");
        if augmented[pivot * width + column].abs() <= tolerance {
            return Err(invalid_geometry(
                "affine geometry Gram matrix is singular at numerical precision",
            ));
        }
        if pivot != column {
            for entry in 0..width {
                augmented.swap(column * width + entry, pivot * width + entry);
            }
        }
        let diagonal = augmented[column * width + column];
        for entry in 0..width {
            augmented[column * width + entry] /= diagonal;
        }
        for row in 0..dimension {
            if row == column {
                continue;
            }
            let factor = augmented[row * width + column];
            for entry in 0..width {
                augmented[row * width + entry] -= factor * augmented[column * width + entry];
            }
        }
    }
    let inverse = (0..dimension)
        .flat_map(|row| {
            let augmented = &augmented;
            (0..dimension).map(move |column| augmented[row * width + dimension + column])
        })
        .collect::<Vec<_>>();
    if inverse.iter().any(|value| !value.is_finite()) {
        return Err(invalid_geometry(
            "affine geometry Gram inverse is not finite",
        ));
    }
    Ok(inverse)
}

fn determinant_dense(matrix: &[f64], dimension: usize) -> Result<f64, Diagnostic> {
    if dimension == 0 || matrix.len() != dimension * dimension {
        return Err(invalid_geometry(
            "affine determinant requires one nonempty square matrix",
        ));
    }
    let mut factor = matrix.to_vec();
    let scale = matrix.iter().map(|value| value.abs()).fold(0.0, f64::max);
    let tolerance = 128.0 * f64::EPSILON * scale.max(f64::MIN_POSITIVE) * dimension as f64;
    let mut sign = 1.0;
    let mut determinant = 1.0;
    for column in 0..dimension {
        let pivot = (column..dimension)
            .max_by(|&left, &right| {
                factor[left * dimension + column]
                    .abs()
                    .total_cmp(&factor[right * dimension + column].abs())
            })
            .expect("nonempty pivot range");
        let pivot_value = factor[pivot * dimension + column];
        if !pivot_value.is_finite() || pivot_value.abs() <= tolerance {
            return Err(invalid_geometry(
                "affine geometry Jacobian is singular at numerical precision",
            ));
        }
        if pivot != column {
            for entry in 0..dimension {
                factor.swap(column * dimension + entry, pivot * dimension + entry);
            }
            sign = -sign;
        }
        let diagonal = factor[column * dimension + column];
        determinant *= diagonal;
        for row in column + 1..dimension {
            let multiplier = factor[row * dimension + column] / diagonal;
            for entry in column + 1..dimension {
                factor[row * dimension + entry] -= multiplier * factor[column * dimension + entry];
            }
        }
    }
    determinant *= sign;
    if !determinant.is_finite() || determinant == 0.0 {
        return Err(invalid_geometry(
            "affine geometry determinant is zero or non-finite",
        ));
    }
    Ok(determinant)
}

fn invalid_geometry(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_MESH, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_an_embedded_line_and_surface() {
        let line = AffineGeometryMap::from_simplex_vertices(vec![vec![1.0, -1.0], vec![4.0, 3.0]])
            .unwrap();
        let mut physical = [0.0, 0.0];
        line.map_point(&[0.25], &mut physical).unwrap();
        assert_eq!(physical, [1.75, 0.0]);
        assert!((line.measure_scale() - 5.0).abs() < 1.0e-14);

        let triangle = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.0, 0.0, 1.0],
            vec![2.0, 0.0, 1.0],
            vec![0.0, 3.0, 1.0],
        ])
        .unwrap();
        triangle
            .map_point(&[0.25, 0.5], &mut physical[..0])
            .unwrap_err();
        let mut surface_point = [0.0; 3];
        triangle
            .map_point(&[0.25, 0.5], &mut surface_point)
            .unwrap();
        assert_eq!(surface_point, [0.5, 1.5, 1.0]);
        assert!((triangle.measure_scale() - 6.0).abs() < 1.0e-14);
    }

    #[test]
    fn unsigned_measure_is_invariant_under_simplex_orientation() {
        let positive = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.0, 0.0],
            vec![2.0, 0.0],
            vec![0.0, 3.0],
        ])
        .unwrap();
        let negative = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.0, 0.0],
            vec![0.0, 3.0],
            vec![2.0, 0.0],
        ])
        .unwrap();
        assert_eq!(positive.measure_scale(), negative.measure_scale());
        assert!(positive.square_quality().unwrap().signed_measure_scale() > 0.0);
        assert!(negative.square_quality().unwrap().signed_measure_scale() < 0.0);
        assert_eq!(
            positive.square_quality().unwrap().mean_ratio(),
            negative.square_quality().unwrap().mean_ratio()
        );
    }

    #[test]
    fn square_quality_is_scale_invariant_and_inverse_jvp_is_analytic() {
        let primal = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.0, 0.0],
            vec![2.0, 0.0],
            vec![0.0, 1.0],
        ])
        .unwrap();
        let scaled = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.0, 0.0],
            vec![6.0, 0.0],
            vec![0.0, 3.0],
        ])
        .unwrap();
        assert!((primal.square_quality().unwrap().mean_ratio() - 0.8).abs() < 1.0e-14);
        assert_eq!(
            primal.square_quality().unwrap().mean_ratio(),
            scaled.square_quality().unwrap().mean_ratio()
        );

        let linearized = AffineGeometryLinearization::new(
            primal.clone(),
            vec![0.0, 0.0],
            vec![0.3, -0.2, 0.1, 0.4],
        )
        .unwrap();
        let computed = linearized.inverse_jacobian_tangent().unwrap();
        let step = 1.0e-6;
        let perturbed_inverse = |sign: f64| {
            AffineGeometryMap::new(
                primal.reference_cell(),
                primal.physical_dimension(),
                primal.origin().to_vec(),
                primal
                    .jacobian()
                    .iter()
                    .zip(linearized.jacobian_tangent())
                    .map(|(value, tangent)| value + sign * step * tangent)
                    .collect(),
            )
            .unwrap()
            .inverse_jacobian()
            .unwrap()
        };
        let plus = perturbed_inverse(1.0);
        let minus = perturbed_inverse(-1.0);
        for ((computed, plus), minus) in computed.iter().zip(plus).zip(minus) {
            assert!((computed - (plus - minus) / (2.0 * step)).abs() < 2.0e-10);
        }
    }

    #[test]
    fn simplex_rule_and_map_integrate_physical_affine_data() {
        let triangle = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.0, 0.0],
            vec![2.0, 0.0],
            vec![0.0, 3.0],
        ])
        .unwrap();
        let rule = crate::simplex_centroid_rule(2).unwrap();
        let integral = rule
            .points()
            .iter()
            .map(|point| {
                let mut physical = [0.0; 2];
                triangle
                    .map_point(&point.coordinates, &mut physical)
                    .unwrap();
                point.weight * triangle.measure_scale() * (1.0 + physical[0] + physical[1])
            })
            .sum::<f64>();
        assert!((integral - 8.0).abs() < 2.0e-15);

        let tetrahedron = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.0, 0.0, 0.0],
            vec![2.0, 0.0, 0.0],
            vec![0.0, 3.0, 0.0],
            vec![0.0, 0.0, 4.0],
        ])
        .unwrap();
        let reference_volume = crate::simplex_centroid_rule(3).unwrap().points()[0].weight;
        assert!((tetrahedron.measure_scale() * reference_volume - 4.0).abs() < 1.0e-14);
    }

    #[test]
    fn rejects_degenerate_and_nonfinite_maps() {
        let error = AffineGeometryMap::from_simplex_vertices(vec![
            vec![0.0, 0.0],
            vec![1.0, 1.0],
            vec![2.0, 2.0],
        ])
        .unwrap_err();
        assert_eq!(error.code(), codes::INVALID_MESH);
        assert_eq!(
            AffineGeometryMap::new(ReferenceCell::segment(), 1, vec![f64::NAN], vec![1.0],)
                .unwrap_err()
                .code(),
            codes::INVALID_MESH
        );
    }

    #[test]
    fn affine_map_jvp_matches_an_independent_perturbation() {
        let primal = AffineGeometryMap::new(
            ReferenceCell::hypercube(2).unwrap(),
            3,
            vec![1.0, -2.0, 0.5],
            vec![2.0, 0.0, 0.5, 3.0, -1.0, 1.0],
        )
        .unwrap();
        let origin_tangent = vec![0.25, -0.5, 0.75];
        let jacobian_tangent = vec![0.2, -0.1, 0.3, 0.4, -0.2, 0.1];
        let linearized = AffineGeometryLinearization::new(
            primal.clone(),
            origin_tangent.clone(),
            jacobian_tangent.clone(),
        )
        .unwrap();
        let reference = [0.2, -0.4];
        let mut physical = [0.0; 3];
        let mut tangent = [0.0; 3];
        linearized
            .map_point_jvp(&reference, &mut physical, &mut tangent)
            .unwrap();

        let step = 1.0e-6;
        let perturbed = |sign: f64| {
            AffineGeometryMap::new(
                ReferenceCell::hypercube(2).unwrap(),
                3,
                primal
                    .origin()
                    .iter()
                    .zip(&origin_tangent)
                    .map(|(value, direction)| value + sign * step * direction)
                    .collect(),
                primal
                    .jacobian()
                    .iter()
                    .zip(&jacobian_tangent)
                    .map(|(value, direction)| value + sign * step * direction)
                    .collect(),
            )
            .unwrap()
        };
        let plus = perturbed(1.0);
        let minus = perturbed(-1.0);
        let mut plus_point = [0.0; 3];
        let mut minus_point = [0.0; 3];
        plus.map_point(&reference, &mut plus_point).unwrap();
        minus.map_point(&reference, &mut minus_point).unwrap();
        for ((actual, plus), minus) in tangent.iter().zip(plus_point).zip(minus_point) {
            let expected = (plus - minus) / (2.0 * step);
            assert!(
                (actual - expected).abs() < 1.0e-9,
                "actual={actual:e}, expected={expected:e}"
            );
        }
        let measure_difference = (plus.measure_scale() - minus.measure_scale()) / (2.0 * step);
        assert!((linearized.measure_scale_tangent() - measure_difference).abs() < 2.0e-9);
    }
}
