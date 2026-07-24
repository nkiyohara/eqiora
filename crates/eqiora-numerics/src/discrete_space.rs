use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{ReferenceCell, VertexPermutation};

const MAX_LOCAL_DOFS: usize = 1_000_000;
const MAX_BASIS_ENTRIES: usize = 8_000_000;

/// Topological support of one scalar element-local degree of freedom.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LocalDof {
    entity_dimension: usize,
    entity_ordinal: usize,
    slot: usize,
}

impl LocalDof {
    /// Dimension of the supporting reference entity.
    #[must_use]
    pub const fn entity_dimension(self) -> usize {
        self.entity_dimension
    }

    /// Supporting entity's ordinal in the reference-cell stratum.
    #[must_use]
    pub const fn entity_ordinal(self) -> usize {
        self.entity_ordinal
    }

    /// DOF slot on the supporting entity.
    #[must_use]
    pub const fn slot(self) -> usize {
        self.slot
    }
}

/// Values and reference gradients of every local basis function at one point.
#[derive(Debug, Clone, PartialEq)]
pub struct BasisTabulation {
    reference_dimension: usize,
    values: Vec<f64>,
    reference_gradients: Vec<f64>,
}

impl BasisTabulation {
    /// Reference-cell dimension.
    #[must_use]
    pub const fn reference_dimension(&self) -> usize {
        self.reference_dimension
    }

    /// One value per local basis function.
    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }

    /// Row-major gradients with shape `local_dof_count x reference_dimension`.
    #[must_use]
    pub fn reference_gradients(&self) -> &[f64] {
        &self.reference_gradients
    }

    /// Gradient of one local basis function.
    #[must_use]
    pub fn gradient(&self, local_dof: usize) -> Option<&[f64]> {
        if local_dof >= self.values.len() {
            return None;
        }
        let start = local_dof.checked_mul(self.reference_dimension)?;
        let end = start.checked_add(self.reference_dimension)?;
        self.reference_gradients.get(start..end)
    }
}

/// Scalar finite-dimensional function space on one reference cell.
///
/// This contract stops before global numbering, constraints, and assembly.
/// Orientation enters through a reference-vertex permutation and returns an
/// element-local DOF order; future higher-order spaces can extend the same
/// seam without putting sparse indices into basis tabulation.
pub trait DiscreteSpace {
    /// Reference cell carrying the basis.
    fn reference_cell(&self) -> ReferenceCell;

    /// Topological descriptors in canonical local DOF order.
    fn local_dofs(&self) -> &[LocalDof];

    /// Tabulate values and reference gradients at a valid reference point.
    ///
    /// # Errors
    /// Returns `EQ0801` when the point dimension, finiteness, or reference-cell
    /// containment is invalid.
    fn tabulate(&self, reference: &[f64]) -> Result<BasisTabulation, Diagnostic>;

    /// Map canonical local DOFs through a reference-cell vertex permutation.
    ///
    /// # Errors
    /// Returns `EQ0801` unless the validated topological permutation has the
    /// vertex arity required by this reference cell.
    fn oriented_dof_order(
        &self,
        vertex_permutation: &VertexPermutation,
    ) -> Result<Vec<usize>, Diagnostic>;
}

/// Discontinuous scalar constants: one cell-interior DOF on any reference cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellConstantSpace {
    cell: ReferenceCell,
    local_dofs: [LocalDof; 1],
}

impl CellConstantSpace {
    /// Construct a P0 space on a reference cell.
    #[must_use]
    pub fn new(cell: ReferenceCell) -> Self {
        Self {
            cell,
            local_dofs: [LocalDof {
                entity_dimension: cell.dimension(),
                entity_ordinal: 0,
                slot: 0,
            }],
        }
    }
}

impl DiscreteSpace for CellConstantSpace {
    fn reference_cell(&self) -> ReferenceCell {
        self.cell
    }

    fn local_dofs(&self) -> &[LocalDof] {
        &self.local_dofs
    }

    fn tabulate(&self, reference: &[f64]) -> Result<BasisTabulation, Diagnostic> {
        validate_reference_point(self.cell, reference)?;
        Ok(BasisTabulation {
            reference_dimension: self.cell.dimension(),
            values: vec![1.0],
            reference_gradients: vec![0.0; self.cell.dimension()],
        })
    }

    fn oriented_dof_order(
        &self,
        vertex_permutation: &VertexPermutation,
    ) -> Result<Vec<usize>, Diagnostic> {
        validate_permutation_arity(vertex_permutation, reference_vertex_count(self.cell)?)?;
        Ok(vec![0])
    }
}

/// Continuous nodal P1 basis on a unit simplex of runtime dimension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SimplexP1Space {
    cell: ReferenceCell,
    local_dofs: Vec<LocalDof>,
}

impl SimplexP1Space {
    /// Construct a nodal P1 simplex space.
    ///
    /// # Errors
    /// Returns `EQ0801` for dimension zero or local-DOF count overflow.
    pub fn new(dimension: usize) -> Result<Self, Diagnostic> {
        let dof_count = dimension
            .checked_add(1)
            .ok_or_else(|| invalid_space("simplex P1 local-DOF count overflows usize"))?;
        let basis_entries = dof_count
            .checked_mul(dimension)
            .ok_or_else(|| invalid_space("simplex P1 basis-tabulation shape overflows usize"))?;
        if dimension == 0 || dof_count > MAX_LOCAL_DOFS || basis_entries > MAX_BASIS_ENTRIES {
            return Err(invalid_space(
                "simplex P1 requires a positive dimension within local resource limits",
            ));
        }
        let cell = ReferenceCell::simplex(dimension)
            .map_err(|_| invalid_space("invalid simplex P1 reference dimension"))?;
        let local_dofs = (0..dof_count)
            .map(|entity_ordinal| LocalDof {
                entity_dimension: 0,
                entity_ordinal,
                slot: 0,
            })
            .collect();
        Ok(Self { cell, local_dofs })
    }
}

impl DiscreteSpace for SimplexP1Space {
    fn reference_cell(&self) -> ReferenceCell {
        self.cell
    }

    fn local_dofs(&self) -> &[LocalDof] {
        &self.local_dofs
    }

    fn tabulate(&self, reference: &[f64]) -> Result<BasisTabulation, Diagnostic> {
        validate_reference_point(self.cell, reference)?;
        let dimension = self.cell.dimension();
        let mut values = Vec::with_capacity(dimension + 1);
        values.push(1.0 - reference.iter().sum::<f64>());
        values.extend_from_slice(reference);

        let mut gradients = vec![0.0; (dimension + 1) * dimension];
        gradients[..dimension].fill(-1.0);
        for axis in 0..dimension {
            gradients[(axis + 1) * dimension + axis] = 1.0;
        }
        Ok(BasisTabulation {
            reference_dimension: dimension,
            values,
            reference_gradients: gradients,
        })
    }

    fn oriented_dof_order(
        &self,
        vertex_permutation: &VertexPermutation,
    ) -> Result<Vec<usize>, Diagnostic> {
        validate_permutation_arity(vertex_permutation, self.local_dofs.len())?;
        Ok(vertex_permutation.images().to_vec())
    }
}

/// Hierarchical simplex P1 basis enriched by one normalized cell bubble.
///
/// The first `dimension + 1` coefficients multiply the nodal P1 basis. The
/// final coefficient multiplies a cell-interior bubble that is one at the
/// barycenter and zero on the complete boundary. It is therefore a
/// coefficient, not a point-evaluation degree of freedom.
#[derive(Debug, Clone, PartialEq)]
pub struct SimplexP1BubbleSpace {
    cell: ReferenceCell,
    local_dofs: Vec<LocalDof>,
    bubble_normalization: f64,
}

impl SimplexP1BubbleSpace {
    /// Construct a hierarchical P1-plus-bubble simplex space.
    ///
    /// # Errors
    /// Returns `EQ0801` for dimension zero, resource overflow, or a dimension
    /// whose barycentric bubble normalization is not representable in `f64`.
    pub fn new(dimension: usize) -> Result<Self, Diagnostic> {
        let vertex_count = dimension
            .checked_add(1)
            .ok_or_else(|| invalid_space("simplex P1-bubble vertex count overflows usize"))?;
        let dof_count = vertex_count
            .checked_add(1)
            .ok_or_else(|| invalid_space("simplex P1-bubble local-DOF count overflows usize"))?;
        let basis_entries = dof_count.checked_mul(dimension).ok_or_else(|| {
            invalid_space("simplex P1-bubble basis-tabulation shape overflows usize")
        })?;
        if dimension == 0 || dof_count > MAX_LOCAL_DOFS || basis_entries > MAX_BASIS_ENTRIES {
            return Err(invalid_space(
                "simplex P1-bubble requires a positive dimension within local resource limits",
            ));
        }
        let bubble_normalization = (0..vertex_count).try_fold(1.0_f64, |value, _| {
            let next = value * vertex_count as f64;
            next.is_finite().then_some(next).ok_or_else(|| {
                invalid_space("simplex P1-bubble normalization exceeds the scalar range")
            })
        })?;
        let cell = ReferenceCell::simplex(dimension)
            .map_err(|_| invalid_space("invalid simplex P1-bubble reference dimension"))?;
        let mut local_dofs = (0..vertex_count)
            .map(|entity_ordinal| LocalDof {
                entity_dimension: 0,
                entity_ordinal,
                slot: 0,
            })
            .collect::<Vec<_>>();
        local_dofs.push(LocalDof {
            entity_dimension: dimension,
            entity_ordinal: 0,
            slot: 0,
        });
        Ok(Self {
            cell,
            local_dofs,
            bubble_normalization,
        })
    }
}

impl DiscreteSpace for SimplexP1BubbleSpace {
    fn reference_cell(&self) -> ReferenceCell {
        self.cell
    }

    fn local_dofs(&self) -> &[LocalDof] {
        &self.local_dofs
    }

    fn tabulate(&self, reference: &[f64]) -> Result<BasisTabulation, Diagnostic> {
        validate_reference_point(self.cell, reference)?;
        let dimension = self.cell.dimension();
        let vertex_count = dimension + 1;
        let mut values = Vec::with_capacity(vertex_count + 1);
        values.push(1.0 - reference.iter().sum::<f64>());
        values.extend_from_slice(reference);

        let mut gradients = vec![0.0; (vertex_count + 1) * dimension];
        gradients[..dimension].fill(-1.0);
        for axis in 0..dimension {
            gradients[(axis + 1) * dimension + axis] = 1.0;
        }

        let mut prefix = vec![1.0; dimension + 1];
        for axis in 0..dimension {
            prefix[axis + 1] = prefix[axis] * reference[axis];
        }
        let mut suffix = vec![1.0; dimension + 1];
        for axis in (0..dimension).rev() {
            suffix[axis] = suffix[axis + 1] * reference[axis];
        }
        let lambda_zero = values[0];
        values.push(self.bubble_normalization * lambda_zero * prefix[dimension]);
        for axis in 0..dimension {
            let product_without_axis = prefix[axis] * suffix[axis + 1];
            gradients[vertex_count * dimension + axis] = self.bubble_normalization
                * (-prefix[dimension] + lambda_zero * product_without_axis);
        }

        Ok(BasisTabulation {
            reference_dimension: dimension,
            values,
            reference_gradients: gradients,
        })
    }

    fn oriented_dof_order(
        &self,
        vertex_permutation: &VertexPermutation,
    ) -> Result<Vec<usize>, Diagnostic> {
        let vertex_count = self.local_dofs.len() - 1;
        validate_permutation_arity(vertex_permutation, vertex_count)?;
        let mut order = vertex_permutation.images().to_vec();
        order.push(vertex_count);
        Ok(order)
    }
}

/// Tensor-product nodal Q1 basis on `[-1, 1]^d`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HypercubeQ1Space {
    cell: ReferenceCell,
    local_dofs: Vec<LocalDof>,
}

impl HypercubeQ1Space {
    /// Construct a nodal Q1 hypercube space.
    ///
    /// # Errors
    /// Returns `EQ0801` for dimension zero, count overflow, or a local-DOF
    /// count exceeding the inspectable artifact limit.
    pub fn new(dimension: usize) -> Result<Self, Diagnostic> {
        if dimension == 0 {
            return Err(invalid_space("hypercube Q1 requires a positive dimension"));
        }
        let exponent = u32::try_from(dimension)
            .map_err(|_| invalid_space("hypercube Q1 dimension exceeds u32 capacity"))?;
        let dof_count = 2_usize
            .checked_pow(exponent)
            .ok_or_else(|| invalid_space("hypercube Q1 local-DOF count overflows usize"))?;
        let basis_entries = dof_count
            .checked_mul(dimension)
            .ok_or_else(|| invalid_space("hypercube Q1 basis-tabulation shape overflows usize"))?;
        if dof_count > MAX_LOCAL_DOFS || basis_entries > MAX_BASIS_ENTRIES {
            return Err(invalid_space(
                "hypercube Q1 local-DOF count exceeds artifact resource limits",
            ));
        }
        let cell = ReferenceCell::hypercube(dimension)
            .map_err(|_| invalid_space("invalid hypercube Q1 reference dimension"))?;
        let local_dofs = (0..dof_count)
            .map(|entity_ordinal| LocalDof {
                entity_dimension: 0,
                entity_ordinal,
                slot: 0,
            })
            .collect();
        Ok(Self { cell, local_dofs })
    }
}

impl DiscreteSpace for HypercubeQ1Space {
    fn reference_cell(&self) -> ReferenceCell {
        self.cell
    }

    fn local_dofs(&self) -> &[LocalDof] {
        &self.local_dofs
    }

    fn tabulate(&self, reference: &[f64]) -> Result<BasisTabulation, Diagnostic> {
        validate_reference_point(self.cell, reference)?;
        let dimension = self.cell.dimension();
        let dof_count = self.local_dofs.len();
        let mut values = vec![0.0; dof_count];
        let mut gradients = vec![0.0; dof_count * dimension];

        for vertex in 0..dof_count {
            let factors = (0..dimension)
                .map(|axis| {
                    let sign = if (vertex >> axis) & 1 == 0 { -1.0 } else { 1.0 };
                    (sign, 0.5 * (1.0 + sign * reference[axis]))
                })
                .collect::<Vec<_>>();
            values[vertex] = factors.iter().map(|(_, factor)| factor).product();
            for axis in 0..dimension {
                gradients[vertex * dimension + axis] = 0.5
                    * factors[axis].0
                    * factors
                        .iter()
                        .enumerate()
                        .filter(|(other_axis, _)| *other_axis != axis)
                        .map(|(_, (_, factor))| factor)
                        .product::<f64>();
            }
        }
        Ok(BasisTabulation {
            reference_dimension: dimension,
            values,
            reference_gradients: gradients,
        })
    }

    fn oriented_dof_order(
        &self,
        vertex_permutation: &VertexPermutation,
    ) -> Result<Vec<usize>, Diagnostic> {
        validate_permutation_arity(vertex_permutation, self.local_dofs.len())?;
        Ok(vertex_permutation.images().to_vec())
    }
}

fn validate_reference_point(cell: ReferenceCell, reference: &[f64]) -> Result<(), Diagnostic> {
    if !cell.contains(reference) {
        return Err(invalid_space(
            "basis tabulation point is non-finite, dimensionally invalid, or outside the reference cell",
        ));
    }
    Ok(())
}

fn reference_vertex_count(cell: ReferenceCell) -> Result<usize, Diagnostic> {
    match cell.family() {
        eqiora_meshing::ReferenceCellFamily::Point => Ok(1),
        eqiora_meshing::ReferenceCellFamily::Simplex => cell
            .dimension()
            .checked_add(1)
            .ok_or_else(|| invalid_space("simplex reference-vertex count overflows usize")),
        eqiora_meshing::ReferenceCellFamily::Hypercube => {
            let exponent = u32::try_from(cell.dimension()).map_err(|_| {
                invalid_space("hypercube reference-vertex dimension exceeds u32 capacity")
            })?;
            2_usize
                .checked_pow(exponent)
                .filter(|&count| count <= MAX_LOCAL_DOFS)
                .ok_or_else(|| {
                    invalid_space("hypercube reference-vertex count exceeds resource limits")
                })
        }
    }
}

fn validate_permutation_arity(
    permutation: &VertexPermutation,
    arity: usize,
) -> Result<(), Diagnostic> {
    if permutation.arity() != arity {
        return Err(invalid_space(format!(
            "orientation expected {arity} vertex images, received {}",
            permutation.arity()
        )));
    }
    Ok(())
}

fn invalid_space(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_partition_and_gradient_sum(space: &impl DiscreteSpace, point: &[f64]) {
        let tabulation = space.tabulate(point).unwrap();
        assert!((tabulation.values().iter().sum::<f64>() - 1.0).abs() < 1.0e-13);
        for axis in 0..tabulation.reference_dimension() {
            let sum = (0..tabulation.values().len())
                .map(|dof| tabulation.gradient(dof).unwrap()[axis])
                .sum::<f64>();
            assert!(sum.abs() < 1.0e-13);
        }
    }

    #[test]
    fn p0_is_the_same_contract_on_all_cell_families() {
        for cell in [
            ReferenceCell::point(),
            ReferenceCell::simplex(4).unwrap(),
            ReferenceCell::hypercube(5).unwrap(),
        ] {
            let space = CellConstantSpace::new(cell);
            let point = match cell.family() {
                eqiora_meshing::ReferenceCellFamily::Point => Vec::new(),
                eqiora_meshing::ReferenceCellFamily::Simplex => vec![0.0; cell.dimension()],
                eqiora_meshing::ReferenceCellFamily::Hypercube => vec![0.0; cell.dimension()],
            };
            assert_partition_and_gradient_sum(&space, &point);
            assert_eq!(space.local_dofs()[0].entity_dimension(), cell.dimension());
        }
    }

    #[test]
    fn simplex_p1_is_nodal_and_dimension_general() {
        for dimension in 1..=5 {
            let space = SimplexP1Space::new(dimension).unwrap();
            assert_partition_and_gradient_sum(&space, &vec![0.2 / dimension as f64; dimension]);
            for vertex in 0..=dimension {
                let mut point = vec![0.0; dimension];
                if vertex > 0 {
                    point[vertex - 1] = 1.0;
                }
                let values = space.tabulate(&point).unwrap().values;
                for (dof, value) in values.into_iter().enumerate() {
                    assert_eq!(value, if dof == vertex { 1.0 } else { 0.0 });
                }
            }
        }
    }

    #[test]
    fn simplex_p1_bubble_has_a_p1_trace_and_invariant_cell_coefficient() {
        for dimension in 1..=5 {
            let space = SimplexP1BubbleSpace::new(dimension).unwrap();
            let vertex_count = dimension + 1;
            let barycenter = vec![1.0 / vertex_count as f64; dimension];
            let centered = space.tabulate(&barycenter).unwrap();
            assert!((centered.values()[vertex_count] - 1.0).abs() < 2.0e-13);
            assert_eq!(
                space.local_dofs()[vertex_count].entity_dimension(),
                dimension
            );

            for vertex in 0..vertex_count {
                let mut point = vec![0.0; dimension];
                if vertex > 0 {
                    point[vertex - 1] = 1.0;
                }
                let values = space.tabulate(&point).unwrap().values;
                assert_eq!(values[vertex_count], 0.0);
                for (dof, value) in values[..vertex_count].iter().enumerate() {
                    assert_eq!(*value, if dof == vertex { 1.0 } else { 0.0 });
                }
            }

            let permutation = VertexPermutation::new((0..vertex_count).rev().collect()).unwrap();
            let mut expected = permutation.images().to_vec();
            expected.push(vertex_count);
            assert_eq!(space.oriented_dof_order(&permutation).unwrap(), expected);
        }
    }

    #[test]
    fn simplex_p1_bubble_gradient_matches_its_normalized_barycentric_definition() {
        let space = SimplexP1BubbleSpace::new(2).unwrap();
        let centered = space.tabulate(&[1.0 / 3.0, 1.0 / 3.0]).unwrap();
        for derivative in centered.gradient(3).unwrap() {
            assert!(derivative.abs() < 2.0e-15);
        }

        let boundary_midpoint = space.tabulate(&[0.5, 0.0]).unwrap();
        assert_eq!(boundary_midpoint.values()[3], 0.0);
        assert_eq!(boundary_midpoint.gradient(3).unwrap(), &[0.0, 6.75]);
    }

    #[test]
    fn hypercube_q1_is_nodal_and_dimension_general() {
        for dimension in 1..=5 {
            let space = HypercubeQ1Space::new(dimension).unwrap();
            assert_partition_and_gradient_sum(&space, &vec![0.125; dimension]);
            for vertex in 0..space.local_dofs().len() {
                let point = (0..dimension)
                    .map(|axis| if (vertex >> axis) & 1 == 0 { -1.0 } else { 1.0 })
                    .collect::<Vec<_>>();
                let values = space.tabulate(&point).unwrap().values;
                for (dof, value) in values.into_iter().enumerate() {
                    assert_eq!(value, if dof == vertex { 1.0 } else { 0.0 });
                }
            }
        }
    }

    #[test]
    fn orientation_is_explicit_and_validated() {
        let space = SimplexP1Space::new(2).unwrap();
        assert_eq!(
            space
                .oriented_dof_order(&VertexPermutation::new(vec![2, 0, 1]).unwrap())
                .unwrap(),
            vec![2, 0, 1]
        );
        assert_eq!(
            VertexPermutation::new(vec![0, 0, 2]).unwrap_err().code(),
            codes::INVALID_MESH
        );
        assert_eq!(
            space
                .oriented_dof_order(&VertexPermutation::identity(2))
                .unwrap_err()
                .code(),
            codes::INVALID_DISCRETIZATION
        );
    }

    #[test]
    fn rejects_unsupported_or_excessive_spaces_and_points() {
        assert_eq!(
            SimplexP1Space::new(0).unwrap_err().code(),
            codes::INVALID_DISCRETIZATION
        );
        assert_eq!(
            SimplexP1Space::new(usize::MAX).unwrap_err().code(),
            codes::INVALID_DISCRETIZATION
        );
        assert_eq!(
            SimplexP1BubbleSpace::new(usize::MAX).unwrap_err().code(),
            codes::INVALID_DISCRETIZATION
        );
        assert_eq!(
            HypercubeQ1Space::new(64).unwrap_err().code(),
            codes::INVALID_DISCRETIZATION
        );
        let space = HypercubeQ1Space::new(2).unwrap();
        assert_eq!(
            space.tabulate(&[0.0, f64::NAN]).unwrap_err().code(),
            codes::INVALID_DISCRETIZATION
        );
    }
}
