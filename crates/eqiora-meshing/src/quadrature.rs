use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

/// Family of a reference integration cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReferenceCellFamily {
    /// Unique zero-dimensional cell.
    Point,
    /// Unit simplex in an arbitrary positive dimension.
    Simplex,
    /// Tensor-product cell `[-1, 1]^d` in an arbitrary positive dimension.
    Hypercube,
}

/// Validated reference-cell topology with an explicit runtime dimension.
///
/// Runtime dimension is intentional: imported meshes and mixed-cell artifacts
/// must be inspectable before backend lowering specializes a kernel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceCell {
    family: ReferenceCellFamily,
    dimension: usize,
}

impl ReferenceCell {
    /// The unique reference point.
    #[must_use]
    pub const fn point() -> Self {
        Self {
            family: ReferenceCellFamily::Point,
            dimension: 0,
        }
    }

    /// Unit simplex of any positive dimension.
    ///
    /// # Errors
    /// Returns `EQ0804` for dimension zero.
    pub fn simplex(dimension: usize) -> Result<Self, Diagnostic> {
        positive_dimension(ReferenceCellFamily::Simplex, dimension)
    }

    /// Tensor-product reference cell `[-1, 1]^d` of any positive dimension.
    ///
    /// # Errors
    /// Returns `EQ0804` for dimension zero.
    pub fn hypercube(dimension: usize) -> Result<Self, Diagnostic> {
        positive_dimension(ReferenceCellFamily::Hypercube, dimension)
    }

    /// Canonical reference segment `[-1, 1]`.
    #[must_use]
    pub const fn segment() -> Self {
        Self {
            family: ReferenceCellFamily::Hypercube,
            dimension: 1,
        }
    }

    /// Cell family.
    #[must_use]
    pub const fn family(self) -> ReferenceCellFamily {
        self.family
    }

    /// Topological dimension.
    #[must_use]
    pub const fn dimension(self) -> usize {
        self.dimension
    }

    /// Whether finite coordinates lie in this reference cell.
    #[must_use]
    pub fn contains(self, coordinates: &[f64]) -> bool {
        if coordinates.len() != self.dimension
            || coordinates.iter().any(|coordinate| !coordinate.is_finite())
        {
            return false;
        }
        match self.family {
            ReferenceCellFamily::Point => coordinates.is_empty(),
            ReferenceCellFamily::Simplex => {
                let tolerance = 64.0 * f64::EPSILON * self.dimension as f64;
                coordinates.iter().all(|coordinate| *coordinate >= 0.0)
                    && coordinates.iter().sum::<f64>() <= 1.0 + tolerance
            }
            ReferenceCellFamily::Hypercube => coordinates
                .iter()
                .all(|coordinate| *coordinate >= -1.0 && *coordinate <= 1.0),
        }
    }
}

/// One weighted sample on a reference cell.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadraturePoint {
    /// Runtime-dimensional reference coordinates.
    pub coordinates: Vec<f64>,
    /// Reference-cell weight.
    pub weight: f64,
}

/// Validated quadrature rule tied to one reference-cell topology.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadratureRule {
    reference_cell: ReferenceCell,
    polynomial_exactness: Option<usize>,
    points: Vec<QuadraturePoint>,
}

impl QuadratureRule {
    /// Construct a custom rule in any dimension.
    ///
    /// `polynomial_exactness` is declarative evidence, not inferred. Use
    /// `None` for rules without a polynomial exactness contract.
    ///
    /// # Errors
    /// Returns `EQ0804` for an empty rule, non-finite data, inconsistent point
    /// dimensions, points outside the reference cell, or non-positive total
    /// measure.
    pub fn new(
        reference_cell: ReferenceCell,
        polynomial_exactness: Option<usize>,
        points: Vec<QuadraturePoint>,
    ) -> Result<Self, Diagnostic> {
        if points.is_empty() {
            return Err(invalid_quadrature(
                "quadrature rule requires at least one point",
            ));
        }
        for point in &points {
            if !point.weight.is_finite() || !reference_cell.contains(&point.coordinates) {
                return Err(invalid_quadrature(
                    "quadrature point is non-finite or outside its reference cell",
                ));
            }
        }
        let weight_sum = points.iter().map(|point| point.weight).sum::<f64>();
        if !weight_sum.is_finite() || weight_sum <= 0.0 {
            return Err(invalid_quadrature(
                "quadrature weights must have finite positive total measure",
            ));
        }
        Ok(Self {
            reference_cell,
            polynomial_exactness,
            points,
        })
    }

    /// Reference-cell topology and dimension.
    #[must_use]
    pub const fn reference_cell(&self) -> ReferenceCell {
        self.reference_cell
    }

    /// Declared maximum total polynomial degree integrated exactly.
    #[must_use]
    pub const fn polynomial_exactness(&self) -> Option<usize> {
        self.polynomial_exactness
    }

    /// Ordered quadrature samples.
    #[must_use]
    pub fn points(&self) -> &[QuadraturePoint] {
        &self.points
    }

    /// Canonical measure-one rule for a reference point.
    #[must_use]
    pub fn point() -> Self {
        Self {
            reference_cell: ReferenceCell::point(),
            polynomial_exactness: Some(0),
            points: vec![QuadraturePoint {
                coordinates: Vec::new(),
                weight: 1.0,
            }],
        }
    }

    /// Gauss–Legendre rule on `[-1, 1]` with one to seven points.
    ///
    /// # Errors
    /// Returns `EQ0804` for an unsupported point count.
    pub fn gauss_legendre(point_count: usize) -> Result<Self, Diagnostic> {
        let points = gauss_legendre_axis(point_count)?;
        Self::new(
            ReferenceCell::segment(),
            Some(2 * point_count - 1),
            points
                .into_iter()
                .map(|(coordinate, weight)| QuadraturePoint {
                    coordinates: vec![coordinate],
                    weight,
                })
                .collect(),
        )
    }

    /// Tensor-product Gauss–Legendre rule on `[-1, 1]^d`.
    ///
    /// # Errors
    /// Returns `EQ0804` for dimension zero, an unsupported one-dimensional
    /// point count, point-count overflow, or allocation failure.
    pub fn tensor_product_gauss_legendre(
        dimension: usize,
        points_per_axis: usize,
    ) -> Result<Self, Diagnostic> {
        let reference_cell = ReferenceCell::hypercube(dimension)?;
        let axis = gauss_legendre_axis(points_per_axis)?;
        let exponent = u32::try_from(dimension).map_err(|_| {
            invalid_quadrature("tensor-product quadrature dimension exceeds u32 capacity")
        })?;
        let point_count = points_per_axis.checked_pow(exponent).ok_or_else(|| {
            invalid_quadrature("tensor-product quadrature point count overflows usize")
        })?;
        let mut points = Vec::new();
        points.try_reserve_exact(point_count).map_err(|_| {
            invalid_quadrature("tensor-product quadrature allocation exceeds platform capacity")
        })?;
        for linear_index in 0..point_count {
            let mut remainder = linear_index;
            let mut coordinates = Vec::new();
            coordinates.try_reserve_exact(dimension).map_err(|_| {
                invalid_quadrature("tensor-product point dimension exceeds platform capacity")
            })?;
            let mut weight = 1.0;
            for _ in 0..dimension {
                let axis_index = remainder % points_per_axis;
                remainder /= points_per_axis;
                coordinates.push(axis[axis_index].0);
                weight *= axis[axis_index].1;
            }
            points.push(QuadraturePoint {
                coordinates,
                weight,
            });
        }
        debug_assert_eq!(points.len(), point_count);
        Self::new(reference_cell, Some(2 * points_per_axis - 1), points)
    }
}

fn positive_dimension(
    family: ReferenceCellFamily,
    dimension: usize,
) -> Result<ReferenceCell, Diagnostic> {
    if dimension == 0 {
        Err(invalid_quadrature(
            "simplex and hypercube reference cells require positive dimension",
        ))
    } else {
        Ok(ReferenceCell { family, dimension })
    }
}

pub(crate) fn gauss_legendre_axis(point_count: usize) -> Result<Vec<(f64, f64)>, Diagnostic> {
    match point_count {
        1 => Ok(vec![(0.0, 2.0)]),
        2 => {
            let coordinate = 1.0_f64 / 3.0_f64.sqrt();
            Ok(vec![(-coordinate, 1.0), (coordinate, 1.0)])
        }
        3 => {
            let coordinate = (3.0_f64 / 5.0).sqrt();
            Ok(vec![
                (-coordinate, 5.0 / 9.0),
                (0.0, 8.0 / 9.0),
                (coordinate, 5.0 / 9.0),
            ])
        }
        4 => {
            let inner = ((3.0 - 2.0 * (6.0_f64 / 5.0).sqrt()) / 7.0).sqrt();
            let outer = ((3.0 + 2.0 * (6.0_f64 / 5.0).sqrt()) / 7.0).sqrt();
            let inner_weight = (18.0 + 30.0_f64.sqrt()) / 36.0;
            let outer_weight = (18.0 - 30.0_f64.sqrt()) / 36.0;
            Ok(vec![
                (-outer, outer_weight),
                (-inner, inner_weight),
                (inner, inner_weight),
                (outer, outer_weight),
            ])
        }
        5 => {
            let inner = ((5.0 - 2.0 * (10.0_f64 / 7.0).sqrt()) / 9.0).sqrt();
            let outer = ((5.0 + 2.0 * (10.0_f64 / 7.0).sqrt()) / 9.0).sqrt();
            let center_weight = 128.0 / 225.0;
            let inner_weight = (322.0 + 13.0 * 70.0_f64.sqrt()) / 900.0;
            let outer_weight = (322.0 - 13.0 * 70.0_f64.sqrt()) / 900.0;
            Ok(vec![
                (-outer, outer_weight),
                (-inner, inner_weight),
                (0.0, center_weight),
                (inner, inner_weight),
                (outer, outer_weight),
            ])
        }
        6 => Ok(vec![
            (-0.932_469_514_203_152, 0.171_324_492_379_170_4),
            (-0.661_209_386_466_264_5, 0.360_761_573_048_138_6),
            (-0.238_619_186_083_196_9, 0.467_913_934_572_691),
            (0.238_619_186_083_196_9, 0.467_913_934_572_691),
            (0.661_209_386_466_264_5, 0.360_761_573_048_138_6),
            (0.932_469_514_203_152, 0.171_324_492_379_170_4),
        ]),
        7 => Ok(vec![
            (-0.949_107_912_342_758_5, 0.129_484_966_168_869_7),
            (-0.741_531_185_599_394_5, 0.279_705_391_489_276_7),
            (-0.405_845_151_377_397_2, 0.381_830_050_505_118_9),
            (0.0, 0.417_959_183_673_469_4),
            (0.405_845_151_377_397_2, 0.381_830_050_505_118_9),
            (0.741_531_185_599_394_5, 0.279_705_391_489_276_7),
            (0.949_107_912_342_758_5, 0.129_484_966_168_869_7),
        ]),
        _ => Err(invalid_quadrature(
            "Gauss-Legendre v0 supports one to seven points per axis",
        )),
    }
}

fn invalid_quadrature(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_QUADRATURE, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_point_gauss_rule_integrates_degree_seven() {
        let rule = QuadratureRule::gauss_legendre(4).unwrap();
        for degree in 0_i32..=7 {
            let actual = rule
                .points()
                .iter()
                .map(|point| point.weight * point.coordinates[0].powi(degree))
                .sum::<f64>();
            let exact = if degree % 2 == 0 {
                2.0 / f64::from(degree + 1)
            } else {
                0.0
            };
            assert!((actual - exact).abs() < 2.0e-15);
        }
    }

    #[test]
    fn five_point_gauss_rule_integrates_degree_nine() {
        let rule = QuadratureRule::gauss_legendre(5).unwrap();
        for degree in 0_i32..=9 {
            let actual = rule
                .points()
                .iter()
                .map(|point| point.weight * point.coordinates[0].powi(degree))
                .sum::<f64>();
            let exact = if degree % 2 == 0 {
                2.0 / f64::from(degree + 1)
            } else {
                0.0
            };
            assert!((actual - exact).abs() < 3.0e-15);
        }
    }

    #[test]
    fn six_point_gauss_rule_integrates_degree_eleven() {
        let rule = QuadratureRule::gauss_legendre(6).unwrap();
        for degree in 0_i32..=11 {
            let actual = rule
                .points()
                .iter()
                .map(|point| point.weight * point.coordinates[0].powi(degree))
                .sum::<f64>();
            let exact = if degree % 2 == 0 {
                2.0 / f64::from(degree + 1)
            } else {
                0.0
            };
            assert!((actual - exact).abs() < 4.0e-15);
        }
    }

    #[test]
    fn seven_point_gauss_rule_integrates_degree_thirteen() {
        let rule = QuadratureRule::gauss_legendre(7).unwrap();
        for degree in 0_i32..=13 {
            let actual = rule
                .points()
                .iter()
                .map(|point| point.weight * point.coordinates[0].powi(degree))
                .sum::<f64>();
            let exact = if degree % 2 == 0 {
                2.0 / f64::from(degree + 1)
            } else {
                0.0
            };
            assert!((actual - exact).abs() < 5.0e-15);
        }
    }

    #[test]
    fn tensor_product_rule_has_arbitrary_runtime_dimension() {
        let rule = QuadratureRule::tensor_product_gauss_legendre(5, 2).unwrap();
        assert_eq!(rule.reference_cell().dimension(), 5);
        assert_eq!(rule.points().len(), 32);
        assert!(
            rule.points()
                .iter()
                .all(|point| point.coordinates.len() == 5)
        );
        assert!(
            (rule.points().iter().map(|point| point.weight).sum::<f64>() - 32.0).abs() < 1.0e-13
        );
    }

    #[test]
    fn custom_rule_checks_reference_cell_and_coordinates() {
        assert_eq!(
            QuadratureRule::new(
                ReferenceCell::point(),
                None,
                vec![QuadraturePoint {
                    coordinates: vec![0.0],
                    weight: 1.0,
                }],
            )
            .unwrap_err()
            .code(),
            codes::INVALID_QUADRATURE
        );
    }
}
