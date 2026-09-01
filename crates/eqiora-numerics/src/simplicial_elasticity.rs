//! P1 isotropic linear elasticity on affine triangle meshes of box domains.

use std::collections::BTreeMap;

use eqiora_assembly::{
    AssemblyBackend, AssemblyMap, AssemblyPacket, AssemblyPlan, AssemblyReport, AssemblyTarget,
    DofId, IndexedAssemblyWork, LocalContribution, LocalUnknown, REFERENCE_ASSEMBLY_BACKEND,
    TargetAssemblyMap,
};
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{
    AffineGeometryMap, GeometryMap, MeshEntity, MeshGeometry, MeshTopology, QuadratureRule,
    SimplicialMesh,
};
use eqiora_solver::{LinearOperatorProperties, LinearProblem, LinearSolveRequest, SolveReport};

use crate::affine_fem::physical_gradient;
use crate::discrete_space::{DiscreteSpace, SimplexP1Space};
use crate::linear_elasticity::{is_coercive_isotropic_material, isotropic_stiffness_entry};
use crate::operator::LocalOperator;
use crate::simplicial_boundary::validate_named_reaction_surfaces;
use crate::simplicial_elliptic::SimplicialP1Field;

const DIMENSION: usize = 2;
const COMPONENTS: usize = 2;
const SCALAR_DOFS: usize = 3;
const LOCAL_DOFS: usize = SCALAR_DOFS * COMPONENTS;

impl SimplicialP1Field {
    /// Solve intrinsic two-dimensional isotropic elasticity on an affine
    /// triangle mesh whose boundary conforms to the supplied Cartesian box.
    ///
    /// Every boundary vertex receives `essential_displacement`; only interior
    /// P1 coefficients are solved. The result tuple contains, in order:
    /// `[u_x, u_y]`, named constrained-boundary reactions, integrated constant
    /// body force, assembly report, and solve report. Named surface facets use
    /// the physics-neutral vertex-disjoint simplicial boundary partition.
    ///
    /// # Errors
    /// Returns a structured discretization, boundary-data, assembly, or solve
    /// diagnostic for an incompatible box, mesh, material, quadrature, field,
    /// or named reaction surface.
    // A named result would widen the frozen crate surface for one private
    // realization; the tuple's exact order is documented above.
    #[allow(clippy::too_many_arguments, clippy::type_complexity)]
    pub fn solve_linear_elasticity_simplicial_2d<B>(
        bounds: &[[f64; 2]; DIMENSION],
        mesh: &SimplicialMesh,
        shear_modulus: f64,
        first_lame_parameter: f64,
        body_force: [f64; COMPONENTS],
        named_reaction_surfaces: &BTreeMap<String, Vec<MeshEntity>>,
        essential_displacement: &B,
        quadrature: &QuadratureRule,
        solver: LinearSolveRequest<'_>,
    ) -> Result<
        (
            [Self; COMPONENTS],
            BTreeMap<String, [f64; COMPONENTS]>,
            [f64; COMPONENTS],
            AssemblyReport,
            SolveReport,
        ),
        Diagnostic,
    >
    where
        B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
    {
        solve(
            bounds,
            mesh,
            shear_modulus,
            first_lame_parameter,
            body_force,
            named_reaction_surfaces,
            essential_displacement,
            quadrature,
            solver,
        )
    }

    /// Recover constant cell strains, constant cell stresses, and total
    /// small-strain energy from an accepted `[u_x, u_y]` field pair.
    ///
    /// # Errors
    /// Returns `EQ0801` unless both finite fields share one full-dimensional
    /// triangle mesh and the Lamé coefficients define a coercive intrinsic-2D
    /// material.
    #[allow(clippy::type_complexity)]
    pub fn linear_elasticity_cell_states_2d(
        displacement: &[Self; COMPONENTS],
        shear_modulus: f64,
        first_lame_parameter: f64,
    ) -> Result<
        (
            Vec<[[f64; COMPONENTS]; COMPONENTS]>,
            Vec<[[f64; COMPONENTS]; COMPONENTS]>,
            f64,
        ),
        Diagnostic,
    > {
        let mesh = displacement[0].mesh();
        if displacement[1].mesh() != mesh
            || mesh.topological_dimension() != DIMENSION
            || mesh.geometric_dimension() != DIMENSION
            || !valid_material(shear_modulus, first_lame_parameter)
        {
            return Err(invalid(
                "simplicial elasticity cell state requires two fields on one triangle mesh and a coercive material",
            ));
        }
        let vertex_values = (0..mesh.vertices().len())
            .map(|vertex| {
                [
                    displacement[0].vertex_values()[vertex],
                    displacement[1].vertex_values()[vertex],
                ]
            })
            .collect::<Vec<_>>();
        recover_cell_states(mesh, &vertex_values, shear_modulus, first_lame_parameter)
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn solve<B>(
    bounds: &[[f64; 2]; DIMENSION],
    mesh: &SimplicialMesh,
    shear_modulus: f64,
    first_lame_parameter: f64,
    body_force: [f64; COMPONENTS],
    named_reaction_surfaces: &BTreeMap<String, Vec<MeshEntity>>,
    essential_displacement: &B,
    quadrature: &QuadratureRule,
    solver: LinearSolveRequest<'_>,
) -> Result<
    (
        [SimplicialP1Field; COMPONENTS],
        BTreeMap<String, [f64; COMPONENTS]>,
        [f64; COMPONENTS],
        AssemblyReport,
        SolveReport,
    ),
    Diagnostic,
>
where
    B: Fn([f64; DIMENSION]) -> Result<[f64; COMPONENTS], Diagnostic> + Sync,
{
    validate_problem(
        bounds,
        mesh,
        shear_modulus,
        first_lame_parameter,
        body_force,
        quadrature,
    )?;
    let named_reaction_vertices = named_reaction_vertices(mesh, named_reaction_surfaces)?;
    let vertex_count = mesh.vertices().len();
    let global_width = vertex_count
        .checked_mul(COMPONENTS)
        .ok_or_else(|| invalid("simplicial elasticity global width overflows usize"))?;
    let mut fixed_values = vec![None; global_width];
    let mut free_indices = vec![None; global_width];
    let mut free_count = 0_usize;
    for vertex in 0..vertex_count {
        let entity = MeshEntity::new(0, vertex);
        if mesh
            .is_boundary_entity(entity)
            .expect("mesh owns every vertex boundary classification")
        {
            let coordinates = [mesh.vertices()[vertex][0], mesh.vertices()[vertex][1]];
            let value = essential_displacement(coordinates)?;
            if value.iter().any(|component| !component.is_finite()) {
                return Err(invalid(
                    "simplicial elasticity essential displacement is non-finite",
                ));
            }
            for (component, value) in value.into_iter().enumerate() {
                fixed_values[global_dof(vertex, component)?] = Some(value);
            }
        } else {
            for component in 0..COMPONENTS {
                let global = global_dof(vertex, component)?;
                free_indices[global] = Some(DofId::new(free_count));
                free_count = free_count
                    .checked_add(1)
                    .ok_or_else(|| invalid("simplicial elasticity free width overflows usize"))?;
            }
        }
    }
    if free_count == 0 {
        return Err(invalid(
            "simplicial P1 elasticity requires at least one unconstrained interior vertex",
        ));
    }

    let operator = SimplicialElasticityCell {
        shear_modulus,
        first_lame_parameter,
        body_force,
    };
    let cell_count = mesh
        .entity_count(DIMENSION)
        .expect("2D mesh owns its cell stratum");
    let plan = AssemblyPlan::new(vec![
        AssemblyTarget::new(free_count)?,
        AssemblyTarget::new(global_width)?,
    ])?;
    let reduced_target = plan
        .target_id(0)
        .expect("two-target elasticity plan owns its reduced target");
    let full_target = plan
        .target_id(1)
        .expect("two-target elasticity plan owns its full target");
    let work = IndexedAssemblyWork::new(cell_count, |cell_index| {
        let cell = MeshEntity::new(DIMENSION, cell_index);
        let geometry = mesh
            .geometry_map(cell)
            .expect("accepted triangle owns affine geometry");
        let local = operator.evaluate(&geometry, quadrature)?;
        let vertices = mesh
            .entity_vertices(cell)
            .expect("accepted triangle owns three vertices");
        let global_dofs = local_global_dofs(&vertices)?;
        let reduced = AssemblyMap::new(
            global_dofs
                .iter()
                .map(|global| free_indices[*global])
                .collect(),
            global_dofs
                .iter()
                .map(|global| {
                    fixed_values[*global].map_or_else(
                        || {
                            LocalUnknown::Free(
                                free_indices[*global]
                                    .expect("unfixed component owns a free equation"),
                            )
                        },
                        LocalUnknown::Fixed,
                    )
                })
                .collect(),
        )?;
        let full = AssemblyMap::new(
            global_dofs
                .iter()
                .map(|global| Some(DofId::new(*global)))
                .collect(),
            global_dofs
                .iter()
                .map(|global| LocalUnknown::Free(DofId::new(*global)))
                .collect(),
        )?;
        AssemblyPacket::new(
            local,
            vec![
                TargetAssemblyMap::new(reduced_target, reduced),
                TargetAssemblyMap::new(full_target, full),
            ],
        )
    });
    let (systems, assembly_report) = REFERENCE_ASSEMBLY_BACKEND
        .assemble(&plan, &work)?
        .into_parts();
    let mut systems = systems.into_iter();
    let reduced_system = systems
        .next()
        .expect("two-target elasticity assembly returns a reduced system");
    let full_system = systems
        .next()
        .expect("two-target elasticity assembly returns a full system");
    debug_assert!(systems.next().is_none());

    let solved = solver.solve(&LinearProblem::new(
        reduced_system.matrix(),
        reduced_system.rhs(),
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )?)?;
    let mut full_values = vec![0.0; global_width];
    for global in 0..global_width {
        full_values[global] = fixed_values[global].unwrap_or_else(|| {
            solved.values()[free_indices[global]
                .expect("unfixed component owns a free equation")
                .index()]
        });
    }
    let component_values = [0, 1].map(|component| {
        full_values
            .as_chunks::<COMPONENTS>()
            .0
            .iter()
            .map(|value| value[component])
            .collect::<Vec<_>>()
    });
    let displacement = [
        SimplicialP1Field::new(mesh.clone(), component_values[0].clone())?,
        SimplicialP1Field::new(mesh.clone(), component_values[1].clone())?,
    ];

    let mut residual = full_system.matrix().multiply(&full_values)?;
    for (value, rhs) in residual.iter_mut().zip(full_system.rhs()) {
        *value -= rhs;
    }
    let named_boundary_reactions = named_reaction_vertices
        .into_iter()
        .map(|(name, vertices)| {
            let mut reaction = [0.0; COMPONENTS];
            for vertex in vertices {
                for (component, result) in reaction.iter_mut().enumerate() {
                    *result += residual[global_dof(vertex, component)?];
                }
            }
            Ok((name, reaction))
        })
        .collect::<Result<BTreeMap<_, _>, Diagnostic>>()?;
    let mut integrated_body_force = [0.0; COMPONENTS];
    for values in full_system.rhs().as_chunks::<COMPONENTS>().0 {
        for component in 0..COMPONENTS {
            integrated_body_force[component] += values[component];
        }
    }
    if integrated_body_force
        .iter()
        .chain(named_boundary_reactions.values().flatten())
        .any(|value| !value.is_finite())
    {
        return Err(invalid("simplicial elasticity evidence is non-finite"));
    }
    Ok((
        displacement,
        named_boundary_reactions,
        integrated_body_force,
        assembly_report,
        solved.report().clone(),
    ))
}

fn named_reaction_vertices(
    mesh: &SimplicialMesh,
    surfaces: &BTreeMap<String, Vec<MeshEntity>>,
) -> Result<Vec<(String, Vec<usize>)>, Diagnostic> {
    let constrained_vertices = (0..mesh.vertices().len()).collect();
    validate_named_reaction_surfaces(
        mesh,
        surfaces
            .iter()
            .map(|(name, facets)| (name.as_str(), facets.as_slice())),
        &constrained_vertices,
        "simplicial elasticity",
    )
}

struct SimplicialElasticityCell {
    shear_modulus: f64,
    first_lame_parameter: f64,
    body_force: [f64; COMPONENTS],
}

impl LocalOperator<AffineGeometryMap> for SimplicialElasticityCell {
    fn evaluate(
        &self,
        geometry: &AffineGeometryMap,
        quadrature: &QuadratureRule,
    ) -> Result<LocalContribution, Diagnostic> {
        require_geometry_rule(geometry, quadrature)?;
        let gradients = triangle_gradients(geometry)?;
        let space = SimplexP1Space::new(DIMENSION)?;
        let mut matrix = vec![0.0; LOCAL_DOFS * LOCAL_DOFS];
        let mut rhs = vec![0.0; LOCAL_DOFS];
        for point in quadrature.points() {
            let basis = space.tabulate(&point.coordinates)?;
            let scale = point.weight * geometry.measure_scale();
            for local_row in 0..SCALAR_DOFS {
                let row_gradient = &gradients[local_row];
                for row_component in 0..COMPONENTS {
                    let row = local_dof(local_row, row_component)?;
                    rhs[row] += scale * basis.values()[local_row] * self.body_force[row_component];
                    for (local_column, column_gradient) in gradients.iter().enumerate() {
                        for column_component in 0..COMPONENTS {
                            let column = local_dof(local_column, column_component)?;
                            matrix[row * LOCAL_DOFS + column] += scale
                                * isotropic_stiffness_entry(
                                    row_gradient,
                                    row_component,
                                    column_gradient,
                                    column_component,
                                    self.shear_modulus,
                                    self.first_lame_parameter,
                                );
                        }
                    }
                }
            }
        }
        LocalContribution::new(LOCAL_DOFS, LOCAL_DOFS, matrix, rhs)
    }
}

#[allow(clippy::type_complexity)]
fn recover_cell_states(
    mesh: &SimplicialMesh,
    vertex_values: &[[f64; COMPONENTS]],
    shear_modulus: f64,
    first_lame_parameter: f64,
) -> Result<
    (
        Vec<[[f64; COMPONENTS]; COMPONENTS]>,
        Vec<[[f64; COMPONENTS]; COMPONENTS]>,
        f64,
    ),
    Diagnostic,
> {
    let cell_count = mesh
        .entity_count(DIMENSION)
        .expect("2D mesh owns its cell stratum");
    let mut strains = Vec::with_capacity(cell_count);
    let mut stresses = Vec::with_capacity(cell_count);
    let mut energy = 0.0;
    for cell_index in 0..cell_count {
        let cell = MeshEntity::new(DIMENSION, cell_index);
        let geometry = mesh
            .geometry_map(cell)
            .expect("accepted triangle owns affine geometry");
        let gradients = triangle_gradients(&geometry)?;
        let vertices = mesh
            .entity_vertices(cell)
            .expect("accepted triangle owns three vertices");
        let mut displacement_gradient = [[0.0; DIMENSION]; COMPONENTS];
        for (local, vertex) in vertices.iter().enumerate() {
            for component in 0..COMPONENTS {
                for axis in 0..DIMENSION {
                    displacement_gradient[component][axis] +=
                        vertex_values[vertex.index()][component] * gradients[local][axis];
                }
            }
        }
        let strain = [
            [
                displacement_gradient[0][0],
                0.5 * (displacement_gradient[0][1] + displacement_gradient[1][0]),
            ],
            [
                0.5 * (displacement_gradient[1][0] + displacement_gradient[0][1]),
                displacement_gradient[1][1],
            ],
        ];
        let trace = strain[0][0] + strain[1][1];
        let stress = [
            [
                2.0 * shear_modulus * strain[0][0] + first_lame_parameter * trace,
                2.0 * shear_modulus * strain[0][1],
            ],
            [
                2.0 * shear_modulus * strain[1][0],
                2.0 * shear_modulus * strain[1][1] + first_lame_parameter * trace,
            ],
        ];
        let strain_square = strain
            .iter()
            .flatten()
            .map(|value| value * value)
            .sum::<f64>();
        let density = shear_modulus * strain_square + 0.5 * first_lame_parameter * trace * trace;
        energy += 0.5 * geometry.measure_scale() * density;
        strains.push(strain);
        stresses.push(stress);
    }
    if strains
        .iter()
        .flatten()
        .flatten()
        .chain(stresses.iter().flatten().flatten())
        .chain([&energy])
        .any(|value| !value.is_finite())
    {
        return Err(invalid(
            "simplicial elasticity cell-state evidence is non-finite",
        ));
    }
    Ok((strains, stresses, energy))
}

fn triangle_gradients(geometry: &AffineGeometryMap) -> Result<Vec<Vec<f64>>, Diagnostic> {
    let inverse = geometry.inverse_jacobian()?;
    let basis = SimplexP1Space::new(DIMENSION)?.tabulate(&[1.0 / 3.0, 1.0 / 3.0])?;
    Ok(basis
        .reference_gradients()
        .as_chunks::<DIMENSION>()
        .0
        .iter()
        .map(|gradient| physical_gradient(gradient, &inverse, DIMENSION))
        .collect())
}

fn local_global_dofs(vertices: &[MeshEntity]) -> Result<Vec<usize>, Diagnostic> {
    vertices
        .iter()
        .flat_map(|vertex| {
            (0..COMPONENTS).map(move |component| global_dof(vertex.index(), component))
        })
        .collect()
}

fn global_dof(vertex: usize, component: usize) -> Result<usize, Diagnostic> {
    vertex
        .checked_mul(COMPONENTS)
        .and_then(|value| value.checked_add(component))
        .ok_or_else(|| invalid("simplicial elasticity global DOF index overflows usize"))
}

fn local_dof(vertex: usize, component: usize) -> Result<usize, Diagnostic> {
    vertex
        .checked_mul(COMPONENTS)
        .and_then(|value| value.checked_add(component))
        .ok_or_else(|| invalid("simplicial elasticity local DOF index overflows usize"))
}

fn valid_material(shear_modulus: f64, first_lame_parameter: f64) -> bool {
    is_coercive_isotropic_material::<DIMENSION>(shear_modulus, first_lame_parameter)
}

fn validate_problem(
    bounds: &[[f64; 2]; DIMENSION],
    mesh: &SimplicialMesh,
    shear_modulus: f64,
    first_lame_parameter: f64,
    body_force: [f64; COMPONENTS],
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    if mesh.topological_dimension() != DIMENSION || mesh.geometric_dimension() != DIMENSION {
        return Err(invalid(
            "simplicial P1 elasticity requires a full-dimensional triangle mesh",
        ));
    }
    if !valid_material(shear_modulus, first_lame_parameter)
        || body_force.iter().any(|value| !value.is_finite())
    {
        return Err(invalid(
            "simplicial isotropic elasticity requires finite mu > 0, lambda + mu > 0, and body force",
        ));
    }
    if bounds
        .iter()
        .any(|axis| axis.iter().any(|value| !value.is_finite()) || axis[0] >= axis[1])
    {
        return Err(invalid(
            "simplicial elasticity box requires finite increasing bounds",
        ));
    }
    let expected = eqiora_meshing::ReferenceCell::simplex(DIMENSION)?;
    if quadrature.reference_cell() != expected || quadrature.polynomial_exactness().unwrap_or(0) < 1
    {
        return Err(invalid(
            "simplicial elasticity requires triangle quadrature exact through degree one",
        ));
    }
    validate_box_conformity(bounds, mesh)
}

fn validate_box_conformity(
    bounds: &[[f64; 2]; DIMENSION],
    mesh: &SimplicialMesh,
) -> Result<(), Diagnostic> {
    for vertex in mesh.vertices() {
        if vertex.iter().zip(bounds).any(|(coordinate, axis)| {
            let tolerance = 256.0 * f64::EPSILON * axis[0].abs().max(axis[1].abs()).max(1.0);
            *coordinate < axis[0] - tolerance || *coordinate > axis[1] + tolerance
        }) {
            return Err(invalid(
                "simplicial elasticity mesh vertex lies outside the Cartesian box",
            ));
        }
    }
    let facet_count = mesh
        .entity_count(DIMENSION - 1)
        .expect("2D mesh owns edge entities");
    for facet_index in 0..facet_count {
        let facet = MeshEntity::new(DIMENSION - 1, facet_index);
        if !mesh
            .is_boundary_entity(facet)
            .expect("mesh owns every edge boundary classification")
        {
            continue;
        }
        let vertices = mesh
            .entity_vertices(facet)
            .expect("accepted boundary edge owns vertices");
        let lies_on_box_side = (0..DIMENSION).any(|axis| {
            bounds[axis].into_iter().any(|bound| {
                vertices.iter().all(|vertex| {
                    mesh.vertices()[vertex.index()][axis].to_bits() == bound.to_bits()
                })
            })
        });
        if !lies_on_box_side {
            return Err(invalid(
                "simplicial elasticity boundary facet does not lie on one box side",
            ));
        }
    }
    Ok(())
}

fn require_geometry_rule(
    geometry: &AffineGeometryMap,
    quadrature: &QuadratureRule,
) -> Result<(), Diagnostic> {
    if geometry.reference_cell() != quadrature.reference_cell()
        || geometry.reference_cell().dimension() != DIMENSION
        || geometry.physical_dimension() != DIMENSION
    {
        Err(invalid(
            "simplicial elasticity geometry and triangle quadrature differ",
        ))
    } else {
        Ok(())
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_DISCRETIZATION, message)
}
