//! Dimensionless MINI-fluid and P1-solid local operators.

use eqiora_core::Diagnostic;

use crate::affine_fem::physical_gradient;
use crate::simplicial_mini_transient::{MiniAffineScales, MiniScaledAffineCell};
use crate::{
    AffineGeometryMap, DiscreteSpace, LocalContribution, MeshEntity, QuadratureRule, SimplexP1Space,
};

use super::contract::{
    FixedReferenceFsiMaterial, FixedReferenceFsiState, FixedReferenceFsiStepConfig,
    require_local_geometry_dimension,
};
use super::{fluid_local_size, p1_count, solid_local_size};

pub(crate) fn fluid_local<const D: usize>(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
    config: FixedReferenceFsiStepConfig<D>,
    vertices: &[MeshEntity],
    previous: &FixedReferenceFsiState<D>,
    fluid_position: usize,
) -> Result<LocalContribution, Diagnostic> {
    require_geometry::<D>(geometry, quadrature)?;
    let p1_count = p1_count::<D>();
    let mut previous_velocity = vertices
        .iter()
        .take(p1_count)
        .map(|vertex| previous.vertex_velocity()[vertex.index()])
        .collect::<Vec<_>>();
    previous_velocity.push(previous.fluid_cell_bubble_velocity()[fluid_position]);
    let material = config.material();
    let (local_size, matrix, rhs) = MiniScaledAffineCell::<D> {
        geometry,
        density: material.fluid_density(),
        viscosity: material.fluid_dynamic_viscosity(),
        time_step: config.time_step(),
        previous_velocity: &previous_velocity,
        scales: MiniAffineScales::new(
            config.scale().velocity(),
            config.scale().pressure(),
            config.scale().power(),
        )?,
    }
    .project(quadrature)?
    .into_parts();
    debug_assert_eq!(local_size, fluid_local_size::<D>());
    LocalContribution::new(local_size, local_size, matrix, rhs)
}

pub(crate) fn solid_local<const D: usize>(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
    config: FixedReferenceFsiStepConfig<D>,
    vertices: &[MeshEntity],
    previous: &FixedReferenceFsiState<D>,
) -> Result<LocalContribution, Diagnostic> {
    require_geometry::<D>(geometry, quadrature)?;
    let p1_count = p1_count::<D>();
    let solid_local_size = solid_local_size::<D>();
    let inverse = geometry.inverse_jacobian()?;
    let space = SimplexP1Space::new(D)?;
    let material = config.material();
    let velocity_scale = config.scale().velocity();
    let power_scale = config.scale().power();
    let mut matrix = vec![0.0; solid_local_size * solid_local_size];
    let mut rhs = vec![0.0; solid_local_size];
    for point in quadrature.points() {
        let basis = space.tabulate(&point.coordinates)?;
        let gradients = (0..p1_count)
            .map(|index| {
                physical_gradient(
                    basis.gradient(index).expect("accepted P1 basis index"),
                    &inverse,
                    D,
                )
            })
            .collect::<Vec<_>>();
        let scale = point.weight * geometry.measure_scale();
        let mut previous_velocity = [0.0; D];
        for (local, vertex) in vertices.iter().take(p1_count).enumerate() {
            for (component, value) in previous_velocity.iter_mut().enumerate() {
                *value +=
                    basis.values()[local] * previous.vertex_velocity()[vertex.index()][component];
            }
        }
        for row_basis in 0..p1_count {
            for (row_component, previous_component) in previous_velocity.iter().enumerate() {
                let row = local_velocity_dimension::<D>(row_basis, row_component);
                rhs[row] += scale * material.solid_density() / config.time_step()
                    * basis.values()[row_basis]
                    * previous_component
                    * velocity_scale
                    / power_scale;
                for column_basis in 0..p1_count {
                    for column_component in 0..D {
                        let column = local_velocity_dimension::<D>(column_basis, column_component);
                        let mass = if row_component == column_component {
                            material.solid_density() / config.time_step()
                                * basis.values()[row_basis]
                                * basis.values()[column_basis]
                        } else {
                            0.0
                        };
                        let stiffness = elasticity_entry(
                            &gradients[row_basis],
                            row_component,
                            &gradients[column_basis],
                            column_component,
                            material,
                        );
                        matrix[row * solid_local_size + column] += scale
                            * (mass + config.time_step() * stiffness)
                            * velocity_scale
                            * velocity_scale
                            / power_scale;
                        rhs[row] -= scale
                            * stiffness
                            * previous.solid_displacement()[vertices[column_basis].index()]
                                [column_component]
                            * velocity_scale
                            / power_scale;
                    }
                }
            }
        }
    }
    LocalContribution::new(solid_local_size, solid_local_size, matrix, rhs)
}

fn elasticity_entry<const D: usize>(
    row_gradient: &[f64],
    row_component: usize,
    column_gradient: &[f64],
    column_component: usize,
    material: FixedReferenceFsiMaterial<D>,
) -> f64 {
    let diagonal = if row_component == column_component {
        dot(row_gradient, column_gradient)
    } else {
        0.0
    };
    let crossed = row_gradient[column_component] * column_gradient[row_component];
    material.solid_shear_modulus() * (diagonal + crossed)
        + material.solid_first_lame_parameter()
            * row_gradient[row_component]
            * column_gradient[column_component]
}

pub(crate) const fn local_velocity_dimension<const D: usize>(
    basis: usize,
    component: usize,
) -> usize {
    basis * D + component
}

pub(crate) fn dot(left: &[f64], right: &[f64]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

fn require_geometry<const D: usize>(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    require_local_geometry_dimension::<D>(geometry, quadrature)
}

#[cfg(test)]
mod tests {
    use crate::{
        CellId, FacetId, MeshEntity, MeshGeometry, MeshQualityGate, MeshTopology, SimplicialMesh,
        simplex_duffy_gauss_legendre,
    };

    use super::{fluid_local, solid_local};
    use crate::simplicial_fsi::contract::{
        FixedReferenceFsiBoundary3d, FixedReferenceFsiLoad3d, FixedReferenceFsiMaterial3d,
        FixedReferenceFsiScale3d, FixedReferenceFsiState3d, FixedReferenceFsiStepConfig3d,
    };
    use crate::simplicial_fsi::partition::FixedReferenceFsiPartition3d;

    #[test]
    fn tetrahedral_mini_and_p1_actions_share_one_finite_symmetric_kernel() {
        let fixture = fixture();
        let fluid = local_geometry(&fixture.mesh, 0);
        let solid = local_geometry(&fixture.mesh, 1);
        let fluid_local = fluid_local(
            &fluid.0,
            &fixture.quadrature,
            fixture.config,
            &fluid.1,
            &fixture.previous,
            0,
        )
        .unwrap();
        let solid_local = solid_local(
            &solid.0,
            &fixture.quadrature,
            fixture.config,
            &solid.1,
            &fixture.previous,
        )
        .unwrap();
        let layout = crate::simplicial_fsi::layout::FsiLayout::<3>::new(
            &fixture.mesh,
            &fixture.partition,
            &fixture.boundary,
        )
        .unwrap();
        let fluid_map = layout.fluid_map(0, &fluid.1, false).unwrap();
        let solid_map = layout.solid_map(&solid.1, false).unwrap();

        // tetrahedral MINI velocity: (P1 four vertices + one bubble) * 3,
        // followed by four P1 pressure coefficients.
        assert_eq!((fluid_local.rows(), fluid_local.columns()), (19, 19));
        assert_eq!((solid_local.rows(), solid_local.columns()), (12, 12));
        assert_eq!(
            (fluid_map.equations().len(), fluid_map.unknowns().len()),
            (19, 19)
        );
        assert_eq!(
            (solid_map.equations().len(), solid_map.unknowns().len()),
            (12, 12)
        );
        for local in [&fluid_local, &solid_local] {
            assert!(
                local
                    .matrix()
                    .iter()
                    .chain(local.rhs())
                    .all(|value| value.is_finite())
            );
            for row in 0..local.rows() {
                for column in 0..local.columns() {
                    assert!(
                        (local.matrix()[row * local.columns() + column]
                            - local.matrix()[column * local.columns() + row])
                            .abs()
                            < 2.0e-13
                    );
                }
            }
        }
    }

    #[test]
    fn tetrahedral_local_action_uses_area_scaled_power_normalization() {
        let fixture = fixture();
        let fluid = local_geometry(&fixture.mesh, 0);
        let solid = local_geometry(&fixture.mesh, 1);
        let wider_scale = FixedReferenceFsiScale3d::new(4.0, 5.0, 3.0).unwrap();
        let wider = FixedReferenceFsiStepConfig3d::new(
            fixture.config.time_step(),
            fixture.config.material(),
            wider_scale,
            FixedReferenceFsiLoad3d::Zero,
        )
        .unwrap();

        let fluid_reference = fluid_local(
            &fluid.0,
            &fixture.quadrature,
            fixture.config,
            &fluid.1,
            &fixture.previous,
            0,
        )
        .unwrap();
        let fluid_wider = fluid_local(
            &fluid.0,
            &fixture.quadrature,
            wider,
            &fluid.1,
            &fixture.previous,
            0,
        )
        .unwrap();
        let solid_reference = solid_local(
            &solid.0,
            &fixture.quadrature,
            fixture.config,
            &solid.1,
            &fixture.previous,
        )
        .unwrap();
        let solid_wider = solid_local(
            &solid.0,
            &fixture.quadrature,
            wider,
            &solid.1,
            &fixture.previous,
        )
        .unwrap();

        // With U and P fixed, doubling L in 3D multiplies the power scale by
        // four. Every nonzero dimensionless local coefficient therefore falls
        // by exactly four; a linear-length rule would fail this assertion.
        for (reference, wider) in [
            (fluid_reference.matrix()[0], fluid_wider.matrix()[0]),
            (solid_reference.matrix()[0], solid_wider.matrix()[0]),
        ] {
            assert!(reference != 0.0);
            assert!((reference - 4.0 * wider).abs() < 2.0e-13 * reference.abs());
        }
    }

    struct Fixture {
        mesh: SimplicialMesh,
        partition: FixedReferenceFsiPartition3d,
        boundary: FixedReferenceFsiBoundary3d,
        previous: FixedReferenceFsiState3d,
        config: FixedReferenceFsiStepConfig3d,
        quadrature: crate::QuadratureRule,
    }

    fn fixture() -> Fixture {
        let mesh = SimplicialMesh::new(
            3,
            vec![
                vec![0.0, 0.0, 0.0],
                vec![1.0, 0.0, 0.0],
                vec![0.0, 1.0, 0.0],
                vec![0.0, 0.0, 1.0],
                vec![0.0, 0.0, -1.0],
            ],
            vec![vec![0, 1, 2, 3], vec![0, 2, 1, 4]],
            MeshQualityGate::new(0.05).unwrap(),
        )
        .unwrap();
        let interface = (0..mesh.entity_count(2).unwrap())
            .find(|&facet| {
                mesh.entity_vertices(MeshEntity::new(2, facet))
                    .unwrap()
                    .iter()
                    .map(|vertex| vertex.index())
                    .collect::<Vec<_>>()
                    == [0, 1, 2]
            })
            .map(FacetId::new)
            .unwrap();
        let partition = FixedReferenceFsiPartition3d::new(
            &mesh,
            vec![CellId::new(0)],
            vec![CellId::new(1)],
            vec![interface],
        )
        .unwrap();
        let previous = FixedReferenceFsiState3d::new(
            &mesh,
            &partition,
            vec![[0.0; 3]; mesh.vertices().len()],
            vec![[0.0; 3]; partition.fluid_cells().len()],
            vec![[0.0; 3]; mesh.vertices().len()],
        )
        .unwrap();
        let boundary = FixedReferenceFsiBoundary3d::homogeneous_exterior(&mesh).unwrap();
        let material = FixedReferenceFsiMaterial3d::new(1.0, 0.1, 2.0, 3.0, 1.0).unwrap();
        let scale = FixedReferenceFsiScale3d::new(2.0, 5.0, 3.0).unwrap();
        let config = FixedReferenceFsiStepConfig3d::new(
            0.25,
            material,
            scale,
            FixedReferenceFsiLoad3d::Zero,
        )
        .unwrap();
        Fixture {
            mesh,
            partition,
            boundary,
            previous,
            config,
            quadrature: simplex_duffy_gauss_legendre(3, 6).unwrap(),
        }
    }

    fn local_geometry(
        mesh: &SimplicialMesh,
        cell: usize,
    ) -> (crate::AffineGeometryMap, Vec<MeshEntity>) {
        let entity = MeshEntity::new(3, cell);
        (
            mesh.geometry_map(entity).unwrap(),
            mesh.entity_vertices(entity).unwrap(),
        )
    }
}
