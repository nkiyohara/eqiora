use eqiora_assembly::LocalContribution;
use eqiora_core::Diagnostic;
use eqiora_meshing::{
    AffineGeometryMap, MeshGeometry, MeshTopology, QuadratureRule, ReferenceCellFamily,
};

use super::{
    REQUIRED_CONVECTIVE_FACET_QUADRATURE_EXACTNESS, REQUIRED_CONVECTIVE_QUADRATURE_EXACTNESS,
    invalid,
};
use crate::operator::LocalOperator;
use crate::simplicial_mini_transient::{MiniTransientCell, MiniTransport};
use crate::simplicial_stokes::SimplicialMiniVelocityField2d;
use crate::simplicial_stokes::element::{MiniSpaces, physical_gradients};
use crate::simplicial_stokes::{
    CELL_LOCAL_DOF_COUNT, COMPONENTS, DIMENSION, LOCAL_PRESSURE_OFFSET, P1_BASIS_COUNT,
    VELOCITY_BASIS_COUNT,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FixedDomainViscousForm {
    SymmetricNewtonian,
    DfgNonsymmetric,
}

#[cfg(test)]
type DfgViscousPairProbe<'a> = dyn Fn([usize; 2], [usize; 2], [[f64; 2]; 2], f64, f64) -> f64 + 'a;

#[cfg(test)]
#[derive(Clone, Copy)]
struct DfgViscousPairObservation {
    basis: [usize; 2],
    component: [usize; 2],
    gradient: [[f64; 2]; 2],
    viscosity: f64,
    actual: f64,
}

#[cfg(test)]
thread_local! {
    static DFG_VISCOUS_PAIR_OBSERVATIONS: std::cell::RefCell<Option<Vec<DfgViscousPairObservation>>> =
        const { std::cell::RefCell::new(None) };
}

/// Scope one crate-test-only observation of the actual private DFG pair.
#[cfg(test)]
pub(crate) fn with_dfg_viscous_pair_probe<R>(
    probe: &DfgViscousPairProbe<'_>,
    action: impl FnOnce() -> Result<R, Diagnostic>,
) -> Result<R, Diagnostic> {
    let previous = DFG_VISCOUS_PAIR_OBSERVATIONS.with(|slot| slot.replace(Some(Vec::new())));
    struct Restore(Option<Vec<DfgViscousPairObservation>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            DFG_VISCOUS_PAIR_OBSERVATIONS.with(|slot| slot.replace(self.0.take()));
        }
    }
    let restore = Restore(previous);
    let result = action();
    let observations = DFG_VISCOUS_PAIR_OBSERVATIONS
        .with(|slot| slot.borrow_mut().take())
        .expect("DFG observation scope remains installed through its action");
    drop(restore);
    if observations.len() != 2 {
        return Err(invalid(
            "DFG execution did not expose both P1-crossed and MINI-bubble production pairs",
        ));
    }
    for observation in observations {
        if !probe(
            observation.basis,
            observation.component,
            observation.gradient,
            observation.viscosity,
            observation.actual,
        )
        .is_finite()
        {
            return Err(invalid("DFG viscous-pair observation was rejected"));
        }
    }
    result
}

#[cfg(test)]
fn observe_dfg_viscous_pair(
    basis: [usize; 2],
    component: [usize; 2],
    gradient: [[f64; 2]; 2],
    viscosity: f64,
    actual: f64,
) -> f64 {
    DFG_VISCOUS_PAIR_OBSERVATIONS.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(observations) = slot.as_mut() else {
            return actual;
        };
        let crossed = viscosity * gradient[0][component[1]] * gradient[1][component[0]];
        let role = if basis.iter().all(|index| *index < P1_BASIS_COUNT) && crossed != 0.0 {
            0
        } else if basis == [3, 3] && actual != 0.0 {
            1
        } else {
            return actual;
        };
        let already_recorded = observations.iter().any(|observation| {
            (role == 0
                && observation
                    .basis
                    .iter()
                    .all(|index| *index < P1_BASIS_COUNT))
                || (role == 1 && observation.basis == [3, 3])
        });
        if !already_recorded {
            observations.push(DfgViscousPairObservation {
                basis,
                component,
                gradient,
                viscosity,
                actual,
            });
        }
        actual
    })
}

#[cfg(not(test))]
fn observe_dfg_viscous_pair(
    _basis: [usize; 2],
    _component: [usize; 2],
    _gradient: [[f64; 2]; 2],
    _viscosity: f64,
    actual: f64,
) -> f64 {
    actual
}

pub(crate) struct MiniNavierStokesCell<'a, F> {
    pub(crate) cell: usize,
    pub(crate) vertices: &'a [eqiora_meshing::MeshEntity],
    pub(crate) density: f64,
    pub(crate) viscosity: f64,
    pub(crate) time_step: f64,
    pub(crate) previous_velocity: &'a SimplicialMiniVelocityField2d,
    pub(crate) candidate_velocity: &'a SimplicialMiniVelocityField2d,
    pub(crate) candidate_pressure: &'a [f64],
    pub(crate) body_force: &'a F,
}

/// One local nonlinear relation evaluated at an exact candidate point.
///
/// The residual is retained directly from the weak-form evaluation.  The
/// linear contribution is derived from that residual only after both actions
/// are complete, so acceptance never has to recover `R(w)` by subtracting two
/// nearly equal `J(w) w` terms.
pub(crate) struct MiniNavierStokesLocalLinearization {
    jacobian: Vec<f64>,
    residual: Vec<f64>,
    point: [f64; CELL_LOCAL_DOF_COUNT],
}

impl MiniNavierStokesLocalLinearization {
    pub(crate) fn residual(&self) -> &[f64] {
        &self.residual
    }

    pub(crate) fn into_linear_contribution(self) -> Result<LocalContribution, Diagnostic> {
        let rhs = self
            .jacobian
            .as_chunks::<CELL_LOCAL_DOF_COUNT>()
            .0
            .iter()
            .zip(&self.residual)
            .map(|(row, residual)| {
                row.iter()
                    .zip(self.point)
                    .map(|(entry, point)| entry * point)
                    .sum::<f64>()
                    - residual
            })
            .collect();
        LocalContribution::new(
            CELL_LOCAL_DOF_COUNT,
            CELL_LOCAL_DOF_COUNT,
            self.jacobian,
            rhs,
        )
    }
}

impl<F> MiniNavierStokesCell<'_, F>
where
    F: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    pub(crate) fn residual(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<Vec<f64>, Diagnostic> {
        let (candidate, previous, pressure) = self.local_state();
        MiniTransientCell::<DIMENSION> {
            geometry,
            transport: MiniTransport::SkewStationary,
            density: self.density,
            viscosity: self.viscosity,
            time_step: self.time_step,
            previous_velocity: &previous,
            current_velocity: &candidate,
            current_pressure: &pressure,
        }
        .residual_fixed_geometry_state(self.body_force, quadrature)
    }

    pub(crate) fn linearize(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<MiniNavierStokesLocalLinearization, Diagnostic> {
        let (candidate, previous, pressure) = self.local_state();
        let (jacobian, residual) = MiniTransientCell::<DIMENSION> {
            geometry,
            transport: MiniTransport::SkewStationary,
            density: self.density,
            viscosity: self.viscosity,
            time_step: self.time_step,
            previous_velocity: &previous,
            current_velocity: &candidate,
            current_pressure: &pressure,
        }
        .linearize_fixed_geometry_state(self.body_force, quadrature)?
        .into_parts();

        Ok(MiniNavierStokesLocalLinearization {
            jacobian,
            residual,
            point: local_point(&candidate, &pressure),
        })
    }

    pub(crate) fn residual_dfg(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<Vec<f64>, Diagnostic> {
        let mut residual = self.residual(geometry, quadrature)?;
        self.apply_dfg_viscous_correction(geometry, quadrature, &mut residual, None)?;
        Ok(residual)
    }

    pub(crate) fn linearize_dfg(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<MiniNavierStokesLocalLinearization, Diagnostic> {
        let mut linearization = self.linearize(geometry, quadrature)?;
        self.apply_dfg_viscous_correction(
            geometry,
            quadrature,
            &mut linearization.residual,
            Some(&mut linearization.jacobian),
        )?;
        Ok(linearization)
    }

    fn apply_dfg_viscous_correction(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
        residual: &mut [f64],
        mut jacobian: Option<&mut [f64]>,
    ) -> Result<(), Diagnostic> {
        let inverse = geometry.inverse_jacobian()?;
        let spaces = MiniSpaces::new()?;
        let (candidate, _, _) = self.local_state();
        for point in quadrature.points() {
            let basis = spaces.tabulate(&point.coordinates)?;
            let gradients = physical_gradients(&basis, &inverse);
            let measure = point.weight * geometry.measure_scale();
            for row_basis in 0..VELOCITY_BASIS_COUNT {
                for row_component in 0..COMPONENTS {
                    let row = row_basis * COMPONENTS + row_component;
                    for column_basis in 0..VELOCITY_BASIS_COUNT {
                        for column_component in 0..COMPONENTS {
                            let column = column_basis * COMPONENTS + column_component;
                            let direct = if row_component == column_component {
                                self.viscosity
                                    * (gradients[row_basis][0] * gradients[column_basis][0]
                                        + gradients[row_basis][1] * gradients[column_basis][1])
                            } else {
                                0.0
                            };
                            let observed = observe_dfg_viscous_pair(
                                [row_basis, column_basis],
                                [row_component, column_component],
                                [gradients[row_basis], gradients[column_basis]],
                                self.viscosity,
                                direct,
                            );
                            let crossed = self.viscosity
                                * gradients[row_basis][column_component]
                                * gradients[column_basis][row_component];
                            let correction = -crossed + (observed - direct);
                            residual[row] +=
                                measure * correction * candidate[column_basis][column_component];
                            if let Some(jacobian) = jacobian.as_deref_mut() {
                                jacobian[row * CELL_LOCAL_DOF_COUNT + column] +=
                                    measure * correction;
                            }
                        }
                    }
                }
            }
        }
        if residual.iter().any(|value| !value.is_finite())
            || jacobian
                .as_deref()
                .is_some_and(|entries| entries.iter().any(|value| !value.is_finite()))
        {
            return Err(invalid(
                "DFG fixed-domain viscous correction produced a non-finite value",
            ));
        }
        Ok(())
    }

    fn local_state(
        &self,
    ) -> (
        [[f64; COMPONENTS]; VELOCITY_BASIS_COUNT],
        [[f64; COMPONENTS]; VELOCITY_BASIS_COUNT],
        [f64; P1_BASIS_COUNT],
    ) {
        (
            local_velocity_coefficients(self.candidate_velocity, self.cell, self.vertices),
            local_velocity_coefficients(self.previous_velocity, self.cell, self.vertices),
            std::array::from_fn(|local| self.candidate_pressure[self.vertices[local].index()]),
        )
    }
}

impl<F> LocalOperator<AffineGeometryMap> for MiniNavierStokesCell<'_, F>
where
    F: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    fn evaluate(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<LocalContribution, Diagnostic> {
        self.linearize(geometry, quadrature)?
            .into_linear_contribution()
    }
}

pub(super) struct ConvectiveRealizationEvidence {
    pub(super) skew_residual_norm: f64,
    pub(super) skew_power: f64,
    pub(super) conservative_defect_norm: f64,
    pub(super) defect_identity_error: f64,
}

pub(super) fn require_convective_evidence_quadrature(
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    if cell_quadrature.polynomial_exactness().unwrap_or(0)
        < REQUIRED_CONVECTIVE_QUADRATURE_EXACTNESS
    {
        return Err(invalid(
            "convective evidence requires degree-eight cell quadrature exactness",
        ));
    }
    let facet_cell = facet_quadrature.reference_cell();
    if facet_cell.family() != ReferenceCellFamily::Simplex
        || facet_cell.dimension() != DIMENSION - 1
        || facet_quadrature.polynomial_exactness().unwrap_or(0)
            < REQUIRED_CONVECTIVE_FACET_QUADRATURE_EXACTNESS
    {
        return Err(invalid(
            "convective boundary-flux evidence requires degree-three quadrature exactness on the one-dimensional unit simplex",
        ));
    }
    Ok(())
}

pub(super) fn integrate_convective_evidence(
    mesh: &eqiora_meshing::SimplicialMesh,
    velocity: &SimplicialMiniVelocityField2d,
    density: f64,
    cell_quadrature: &QuadratureRule,
    facet_quadrature: &QuadratureRule,
) -> Result<ConvectiveRealizationEvidence, Diagnostic> {
    require_convective_evidence_quadrature(cell_quadrature, facet_quadrature)?;
    let spaces = MiniSpaces::new()?;
    let cell_count = mesh
        .entity_count(DIMENSION)
        .expect("2D simplex mesh owns cells");
    let vertex_count = mesh.vertices().len();
    let width = COMPONENTS * (vertex_count + cell_count);
    let mut skew_global = vec![0.0; width];
    let mut conservative_global = vec![0.0; width];
    let mut expected_defect_global = vec![0.0; width];
    for cell in 0..cell_count {
        let entity = eqiora_meshing::MeshEntity::new(DIMENSION, cell);
        let geometry = mesh
            .geometry_map(entity)
            .expect("accepted simplex cell owns geometry");
        let vertices = mesh
            .entity_vertices(entity)
            .expect("accepted simplex cell owns vertices");
        let coefficients = local_velocity_coefficients(velocity, cell, &vertices);
        let inverse = geometry.inverse_jacobian()?;
        let mut local_residual = [0.0; crate::simplicial_stokes::LOCAL_VELOCITY_DOF_COUNT];
        let mut conservative = [0.0; crate::simplicial_stokes::LOCAL_VELOCITY_DOF_COUNT];
        let mut expected_defect = [0.0; crate::simplicial_stokes::LOCAL_VELOCITY_DOF_COUNT];
        for point in cell_quadrature.points() {
            let basis = spaces.tabulate(&point.coordinates)?;
            let gradients = physical_gradients(&basis, &inverse);
            let (value, gradient) = evaluate_velocity(&coefficients, &basis.values, &gradients);
            let divergence = (0..DIMENSION).map(|axis| gradient[axis][axis]).sum::<f64>();
            let scale = point.weight * geometry.measure_scale();
            for (row_basis, test_gradient) in gradients.iter().enumerate() {
                let velocity_dot_test_gradient = dot(&value, test_gradient);
                for component in 0..COMPONENTS {
                    let row = local_velocity(row_basis, component);
                    local_residual[row] += 0.5
                        * scale
                        * density
                        * (dot(&value, &gradient[component]) * basis.values[row_basis]
                            - velocity_dot_test_gradient * value[component]);
                    conservative[row] -=
                        scale * density * velocity_dot_test_gradient * value[component];
                    expected_defect[row] -= 0.5
                        * scale
                        * density
                        * divergence
                        * value[component]
                        * basis.values[row_basis];
                }
            }
        }
        for (basis, vertex) in vertices.iter().enumerate() {
            for component in 0..COMPONENTS {
                let local = local_velocity(basis, component);
                let global = COMPONENTS * vertex.index() + component;
                skew_global[global] += local_residual[local];
                conservative_global[global] += conservative[local];
                expected_defect_global[global] += expected_defect[local];
            }
        }
        for component in 0..COMPONENTS {
            let local = local_velocity(P1_BASIS_COUNT, component);
            let global = COMPONENTS * (vertex_count + cell) + component;
            skew_global[global] += local_residual[local];
            conservative_global[global] += conservative[local];
            expected_defect_global[global] += expected_defect[local];
        }
    }
    add_boundary_flux_defect(
        mesh,
        velocity,
        density,
        facet_quadrature,
        &mut expected_defect_global,
    )?;
    let coefficients = velocity
        .vertex_values()
        .iter()
        .chain(velocity.cell_bubble_values())
        .flatten()
        .copied()
        .collect::<Vec<_>>();
    let residual_squared = skew_global.iter().map(|value| value * value).sum::<f64>();
    let energy = skew_global
        .iter()
        .zip(coefficients)
        .map(|(residual, coefficient)| residual * coefficient)
        .sum::<f64>();
    let mut defect_squared = 0.0;
    let mut identity_error_squared = 0.0;
    for ((skew, conservative), expected) in skew_global
        .iter()
        .zip(conservative_global)
        .zip(expected_defect_global)
    {
        let defect = skew - conservative;
        defect_squared += defect * defect;
        identity_error_squared += (defect - expected).powi(2);
    }
    if !residual_squared.is_finite()
        || !energy.is_finite()
        || !defect_squared.is_finite()
        || !identity_error_squared.is_finite()
    {
        return Err(invalid("convective evidence is non-finite"));
    }
    Ok(ConvectiveRealizationEvidence {
        skew_residual_norm: residual_squared.sqrt(),
        skew_power: energy,
        conservative_defect_norm: defect_squared.sqrt(),
        defect_identity_error: identity_error_squared.sqrt(),
    })
}

fn add_boundary_flux_defect(
    mesh: &eqiora_meshing::SimplicialMesh,
    velocity: &SimplicialMiniVelocityField2d,
    density: f64,
    quadrature: &QuadratureRule,
    expected_defect: &mut [f64],
) -> Result<(), Diagnostic> {
    let facet_count = mesh
        .entity_count(DIMENSION - 1)
        .expect("2D simplex mesh owns edge entities");
    for facet_index in 0..facet_count {
        let facet = eqiora_meshing::MeshEntity::new(DIMENSION - 1, facet_index);
        if !mesh
            .is_boundary_entity(facet)
            .expect("accepted mesh classifies every edge")
        {
            continue;
        }
        let vertices = mesh
            .entity_vertices(facet)
            .expect("accepted boundary edge owns vertices");
        let adjacent = mesh
            .incidence(facet, DIMENSION)
            .expect("accepted boundary edge owns cell incidence");
        if vertices.len() != 2 || adjacent.len() != 1 {
            return Err(invalid(
                "convective boundary-flux evidence requires one segment and one incident parent cell",
            ));
        }
        let parent = mesh
            .cells()
            .get(adjacent[0].entity.index())
            .expect("accepted boundary incidence names a cell");
        let opposite = parent
            .iter()
            .copied()
            .find(|candidate| !vertices.iter().any(|vertex| vertex.index() == *candidate))
            .ok_or_else(|| invalid("boundary edge has no unique opposite parent vertex"))?;
        let first = &mesh.vertices()[vertices[0].index()];
        let second = &mesh.vertices()[vertices[1].index()];
        let opposite = &mesh.vertices()[opposite];
        let tangent = [second[0] - first[0], second[1] - first[1]];
        let parent_side =
            tangent[0] * (opposite[1] - first[1]) - tangent[1] * (opposite[0] - first[0]);
        if !parent_side.is_finite() || parent_side == 0.0 {
            return Err(invalid(
                "convective boundary-flux evidence found a degenerate parent orientation",
            ));
        }
        // This is the parent-outward unit normal multiplied by edge measure.
        // Keeping the product avoids a normalization and its cancelling
        // multiplication in the boundary integral.
        let outward_normal_measure = if parent_side > 0.0 {
            [tangent[1], -tangent[0]]
        } else {
            [-tangent[1], tangent[0]]
        };
        for point in quadrature.points() {
            let coordinate = point.coordinates[0];
            let basis = [1.0 - coordinate, coordinate];
            // The MINI bubble has exactly zero trace, so only the two P1
            // endpoint coefficients contribute on this edge.
            let value = std::array::from_fn::<_, COMPONENTS, _>(|component| {
                basis[0] * velocity.vertex_values()[vertices[0].index()][component]
                    + basis[1] * velocity.vertex_values()[vertices[1].index()][component]
            });
            let scale = 0.5 * point.weight * density * dot(&value, &outward_normal_measure);
            for local in 0..2 {
                for component in 0..COMPONENTS {
                    expected_defect[COMPONENTS * vertices[local].index() + component] +=
                        scale * value[component] * basis[local];
                }
            }
        }
    }
    Ok(())
}

pub(super) fn local_velocity_coefficients(
    velocity: &SimplicialMiniVelocityField2d,
    cell: usize,
    vertices: &[eqiora_meshing::MeshEntity],
) -> [[f64; COMPONENTS]; VELOCITY_BASIS_COUNT] {
    std::array::from_fn(|basis| {
        if basis < P1_BASIS_COUNT {
            velocity.vertex_values()[vertices[basis].index()]
        } else {
            velocity.cell_bubble_values()[cell]
        }
    })
}

fn local_point(
    velocity: &[[f64; COMPONENTS]; VELOCITY_BASIS_COUNT],
    pressure: &[f64; P1_BASIS_COUNT],
) -> [f64; CELL_LOCAL_DOF_COUNT] {
    let mut point = [0.0; CELL_LOCAL_DOF_COUNT];
    for basis in 0..VELOCITY_BASIS_COUNT {
        for component in 0..COMPONENTS {
            point[local_velocity(basis, component)] = velocity[basis][component];
        }
    }
    point[LOCAL_PRESSURE_OFFSET..].copy_from_slice(pressure);
    point
}

pub(super) fn evaluate_velocity(
    coefficients: &[[f64; COMPONENTS]; VELOCITY_BASIS_COUNT],
    basis: &[f64; VELOCITY_BASIS_COUNT],
    gradients: &[[f64; DIMENSION]; VELOCITY_BASIS_COUNT],
) -> ([f64; COMPONENTS], [[f64; DIMENSION]; COMPONENTS]) {
    let mut value = [0.0; COMPONENTS];
    let mut gradient = [[0.0; DIMENSION]; COMPONENTS];
    for local in 0..VELOCITY_BASIS_COUNT {
        for component in 0..COMPONENTS {
            value[component] += coefficients[local][component] * basis[local];
            for axis in 0..DIMENSION {
                gradient[component][axis] +=
                    coefficients[local][component] * gradients[local][axis];
            }
        }
    }
    (value, gradient)
}

fn dot(left: &[f64; DIMENSION], right: &[f64; DIMENSION]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

const fn local_velocity(basis: usize, component: usize) -> usize {
    basis * COMPONENTS + component
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use eqiora_meshing::{MeshQualityGate, SimplicialMesh, triangle_duffy_gauss_legendre};
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn fixed_geometry_local_projection_has_stable_bits() {
        let mesh = SimplicialMesh::new(
            DIMENSION,
            vec![vec![0.2, -0.3], vec![1.4, 0.1], vec![-0.15, 1.25]],
            vec![vec![0, 1, 2]],
            MeshQualityGate::new(0.01).unwrap(),
        )
        .unwrap();
        let previous = SimplicialMiniVelocityField2d::new(
            mesh.clone(),
            vec![[0.17, -0.08], [0.11, 0.06], [-0.04, 0.13]],
            vec![[0.025, -0.035]],
        )
        .unwrap();
        let candidate = SimplicialMiniVelocityField2d::new(
            mesh.clone(),
            vec![[0.21, -0.02], [0.09, 0.075], [-0.055, 0.16]],
            vec![[0.04, -0.015]],
        )
        .unwrap();
        let pressure = [0.14, -0.065, 0.035];
        let calls = AtomicUsize::new(0);
        let body_force = |[x, y]: [f64; DIMENSION]| {
            calls.fetch_add(1, Ordering::Relaxed);
            Ok([1.7 * x - 0.3 * y + 0.2, -0.4 * x + 0.9 * y * y - 0.1])
        };
        let cell = eqiora_meshing::MeshEntity::new(DIMENSION, 0);
        let vertices = mesh.entity_vertices(cell).unwrap();
        let geometry = mesh.geometry_map(cell).unwrap();
        let quadrature = triangle_duffy_gauss_legendre(5).unwrap();
        let operator = MiniNavierStokesCell {
            cell: 0,
            vertices: &vertices,
            density: 1.35,
            viscosity: 0.07,
            time_step: 0.18,
            previous_velocity: &previous,
            candidate_velocity: &candidate,
            candidate_pressure: &pressure,
            body_force: &body_force,
        };
        let residual_only = operator.residual(&geometry, &quadrature).unwrap();
        let linearization = operator.linearize(&geometry, &quadrature).unwrap();

        assert_eq!(
            bit_digest(&residual_only),
            bit_digest(&linearization.residual)
        );
        let residual = bit_digest(&linearization.residual);
        let jacobian = bit_digest(&linearization.jacobian);
        let point = bit_digest(&linearization.point);
        let contribution = linearization.into_linear_contribution().unwrap();
        let matrix = bit_digest(contribution.matrix());
        let rhs = bit_digest(contribution.rhs());
        assert_eq!(
            residual,
            [
                209, 196, 225, 173, 63, 172, 57, 101, 81, 197, 107, 128, 202, 54, 147, 101, 213,
                86, 132, 72, 90, 171, 199, 131, 145, 239, 33, 72, 147, 84, 245, 188,
            ]
        );
        assert_eq!(
            jacobian,
            [
                242, 68, 12, 192, 136, 45, 119, 67, 182, 20, 230, 152, 169, 36, 118, 103, 136, 180,
                1, 213, 219, 227, 137, 94, 183, 60, 186, 139, 198, 219, 120, 142,
            ]
        );
        assert_eq!(
            point,
            [
                37, 66, 63, 68, 153, 50, 14, 4, 45, 3, 75, 214, 218, 95, 13, 63, 22, 57, 94, 30,
                15, 223, 245, 180, 169, 58, 46, 12, 162, 13, 95, 195,
            ]
        );
        assert_eq!(
            rhs,
            [
                53, 81, 41, 209, 103, 144, 137, 140, 162, 52, 69, 197, 255, 100, 86, 204, 141, 199,
                130, 183, 241, 176, 79, 151, 176, 175, 72, 93, 89, 13, 117, 210,
            ]
        );
        assert_eq!(jacobian, matrix);
        assert_eq!(calls.load(Ordering::Relaxed), 2 * quadrature.points().len());
    }

    fn bit_digest(values: &[f64]) -> [u8; 32] {
        let mut digest = Sha256::new();
        for value in values {
            digest.update(value.to_bits().to_le_bytes());
        }
        digest.finalize().into()
    }
}
