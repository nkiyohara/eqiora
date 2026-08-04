use eqiora::api::ModelDocument;
use eqiora::artifact::{
    GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1, ModelEnvelope,
    ReplayableCanonicalModelArtifact, SimplicialMeshEnvelopeV1,
};
use eqiora::assembly::{
    AssemblyBackend, AssemblyPlan, AssemblyResult, AssemblyWork, CsrMatrix,
    REFERENCE_ASSEMBLY_BACKEND,
};
use eqiora::kernel::BoundarySide;
use eqiora::meshing::{MeshQualityGate, SimplicialMesh, VertexId};
use eqiora::solver::{
    CanonicalCsrSystemView, ExecutionReport, LinearOperatorProperties, LinearProblem,
    LinearSolution, LinearSolver, LinearSolverBackend, PreconditionerPolicy,
    REFERENCE_LINEAR_SOLVER, ReductionPolicy, ReplicatedLinearExecution, SolverCapabilities,
    SolverPlan, SolverProvider,
};
use eqiora::{Diagnostic, DimExponents, DynQuantity, Id, kinds};
use eqiora_numerics::{
    common::PhysicalBoundaryDisposition,
    solid::{
        AcceptedPrescribedDynamicSolidStep3d, PrescribedDynamicSolidReference3d,
        lower_isotropic_elastodynamics_cartesian_3d,
    },
};
use serde::Deserialize;

const DIRECT_SOURCE: &str =
    include_str!("../../../verify/solid/prescribed-dynamic-solid-step-3d/models/direct.eqi");
const EXPECTED_SOURCE: &str = include_str!(
    "../../../verify/solid/prescribed-dynamic-solid-step-3d/expected/accepted-step.json"
);
const TIME: DimExponents = DimExponents {
    time: 1,
    ..DimExponents::DIMENSIONLESS
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Oracle {
    schema: String,
    geometry: GeometryOracle,
    material: MaterialOracle,
    time_step_s: f64,
    prior_displacement_m: Vec<(usize, [f64; 3])>,
    prior_velocity_m_per_s: Vec<(usize, [f64; 3])>,
    driven_vertices: Vec<usize>,
    driven_total_displacement_m: Vec<(usize, [f64; 3])>,
    accepted: AcceptedOracle,
    tolerances: Tolerances,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeometryOracle {
    vertices: Vec<[f64; 3]>,
    tetrahedra: Vec<[usize; 4]>,
    signed_determinant: f64,
    tetrahedron_volume: f64,
    total_volume: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialOracle {
    density_kg_per_m3: f64,
    shear_modulus_pa: f64,
    first_lame_parameter_pa: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AcceptedOracle {
    generation: u64,
    displacement_m: Vec<(usize, [f64; 3])>,
    velocity_m_per_s: Vec<(usize, [f64; 3])>,
    acceleration_m_per_s2: Vec<(usize, [f64; 3])>,
    strain: [[f64; 3]; 3],
    stress_pa: [[f64; 3]; 3],
    constraint_on_body_reaction_n: Vec<(usize, [f64; 3])>,
    fixed_face_reaction_n: [f64; 3],
    driven_face_reaction_n: [f64; 3],
    center_mass_block: [[f64; 3]; 3],
    center_stiffness_block: [[f64; 3]; 3],
    center_backward_euler_block: [[f64; 3]; 3],
    free_momentum_residual_norm_n: f64,
    kinematic_residual_norm_m_per_s: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Tolerances {
    determinant_and_volume: f64,
    mass_and_stiffness: f64,
    displacement_and_velocity: f64,
    acceleration_stress_reaction_and_face_total: f64,
    kinematic_residual: f64,
    free_momentum_residual_n: f64,
}

struct Fixture {
    document: ModelDocument,
    model: ModelEnvelope,
    geometry: GeometryIdentityEnvelopeV1,
    mesh: SimplicialMeshEnvelopeV1,
    correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    driven_boundary: Id<kinds::Domain>,
    prior_displacement: Vec<(VertexId, [f64; 3])>,
    prior_velocity: Vec<(VertexId, [f64; 3])>,
}

impl Fixture {
    fn new(oracle: &Oracle) -> Self {
        let document =
            ModelDocument::compile("prescribed-dynamic-solid-step-3d.eqi", DIRECT_SOURCE)
                .expect("the direct standalone dynamic-solid Model compiles");
        let canonical = lower_isotropic_elastodynamics_cartesian_3d(document.program())
            .expect("the direct Model lowers as standalone 3D elastodynamics");
        assert_eq!(canonical.bounds(), &[[0.0, 1.0]; 3]);
        assert_eq!(canonical.mass_density(), oracle.material.density_kg_per_m3);
        assert_eq!(canonical.shear_modulus(), oracle.material.shear_modulus_pa);
        assert_eq!(
            canonical.first_lame_parameter(),
            oracle.material.first_lame_parameter_pa
        );
        assert_eq!(
            canonical
                .boundary_inventory()
                .boundary(0, BoundarySide::Lower)
                .expect("x-lower boundary")
                .disposition(),
            PhysicalBoundaryDisposition::TraceZero
        );
        let PhysicalBoundaryDisposition::PortBinding { port, .. } = canonical
            .boundary_inventory()
            .boundary(0, BoundarySide::Upper)
            .expect("x-upper boundary")
            .disposition()
        else {
            panic!("the driven x-upper side must remain a live velocity/traction PortBinding");
        };
        assert_eq!(
            port,
            document.aliases()["boundary.mechanical[axis=0,side=upper]"],
            "the driven disposition retains the exact solid-side live Port"
        );
        for (axis, side) in [
            (1, BoundarySide::Lower),
            (1, BoundarySide::Upper),
            (2, BoundarySide::Lower),
            (2, BoundarySide::Upper),
        ] {
            assert_eq!(
                canonical
                    .boundary_inventory()
                    .boundary(axis, side)
                    .expect("complete natural boundary inventory")
                    .disposition(),
                PhysicalBoundaryDisposition::FluxZero
            );
        }

        let model =
            ModelEnvelope::from_program(document.program()).expect("canonical Model artifact");
        let body = domain(&document, "body");
        let geometry = GeometryIdentityEnvelopeV1::new(&model, [body], 1.0e-12)
            .expect("unit-cube geometry identity");
        let mesh = SimplicialMeshEnvelopeV1::from_mesh(&oracle_mesh(oracle))
            .expect("exact tetrahedral mesh envelope");
        let correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(&geometry, &model, &mesh)
            .expect("complete unit-cube correspondence");

        Self {
            driven_boundary: domain(&document, "x_upper"),
            prior_displacement: coefficients(&oracle.prior_displacement_m),
            prior_velocity: coefficients(&oracle.prior_velocity_m_per_s),
            document,
            model,
            geometry,
            mesh,
            correspondence,
        }
    }

    fn reference(
        &self,
        time_step: DynQuantity,
    ) -> Result<PrescribedDynamicSolidReference3d, Diagnostic> {
        PrescribedDynamicSolidReference3d::new(
            &self.model,
            &self.geometry,
            &self.mesh,
            &self.correspondence,
            time_step,
            &self.prior_displacement,
            &self.prior_velocity,
            self.driven_boundary,
        )
    }
}

#[allow(dead_code, clippy::too_many_arguments)]
fn compile_contract_exact_issue_signatures(
    model: &impl ReplayableCanonicalModelArtifact,
    geometry: &GeometryIdentityEnvelopeV1,
    mesh: &SimplicialMeshEnvelopeV1,
    correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    time_step: DynQuantity,
    prior_displacement: &[(VertexId, [f64; 3])],
    prior_velocity: &[(VertexId, [f64; 3])],
    driven_boundary: Id<kinds::Domain>,
    driven_total_displacement: &[(VertexId, [f64; 3])],
    assembly: &dyn AssemblyBackend,
    solver: &dyn LinearSolverBackend,
) -> Result<AcceptedPrescribedDynamicSolidStep3d, Diagnostic> {
    let mut reference: PrescribedDynamicSolidReference3d = PrescribedDynamicSolidReference3d::new(
        model,
        geometry,
        mesh,
        correspondence,
        time_step,
        prior_displacement,
        prior_velocity,
        driven_boundary,
    )?;
    let immutable_reference: &PrescribedDynamicSolidReference3d = &reference;
    let generation: u64 = immutable_reference.accepted_generation();
    let _: &[VertexId] = immutable_reference.driven_vertices();
    let _: (u64, Vec<(VertexId, [f64; 3])>, Vec<(VertexId, [f64; 3])>) =
        immutable_reference.project_driven_surface();

    let accepted: AcceptedPrescribedDynamicSolidStep3d =
        reference.accept_candidate(generation, driven_total_displacement, assembly, solver)?;
    let _: u64 = accepted.generation();
    let _: &[(VertexId, [f64; 3])] = accepted.displacement();
    let _: &[(VertexId, [f64; 3])] = accepted.velocity();
    let _: &[(VertexId, [f64; 3])] = accepted.acceleration();
    let _: &[(VertexId, [f64; 3])] = accepted.constraint_reactions();
    let _: &CsrMatrix = accepted.mass_operator();
    let _: &CsrMatrix = accepted.stiffness_operator();
    let _: &CanonicalCsrSystemView = accepted.reduced_system();
    let _: f64 = accepted.free_momentum_residual_norm();
    let _: f64 = accepted.kinematic_residual_norm();
    let _: &eqiora::assembly::AssemblyReport = accepted.assembly_report();
    let _: &eqiora::solver::SolveReport = accepted.solve_report();
    Ok(accepted)
}

#[test]
fn fixture_is_self_consistent_before_topology_dependent_reactions_are_used() {
    let oracle = oracle();
    assert_eq!(
        oracle.schema,
        "eqiora.verify.prescribed-dynamic-solid-step-3d-oracle/v1"
    );
    assert_eq!(oracle.geometry.vertices.len(), 9);
    assert_eq!(oracle.geometry.tetrahedra.len(), 12);
    assert_eq!(
        oracle
            .geometry
            .tetrahedra
            .iter()
            .map(|cell| cell[0])
            .collect::<Vec<_>>(),
        vec![8; 12]
    );

    let mut volume = 0.0;
    for cell in &oracle.geometry.tetrahedra {
        let determinant = signed_tetrahedron_determinant(&oracle.geometry.vertices, cell);
        assert_close(
            determinant,
            oracle.geometry.signed_determinant,
            oracle.tolerances.determinant_and_volume,
            "signed tetrahedron determinant",
        );
        assert_close(
            determinant / 6.0,
            oracle.geometry.tetrahedron_volume,
            oracle.tolerances.determinant_and_volume,
            "tetrahedron volume",
        );
        volume += determinant / 6.0;
    }
    assert_close(
        volume,
        oracle.geometry.total_volume,
        oracle.tolerances.determinant_and_volume,
        "mesh volume",
    );

    for (vertex, coordinates) in oracle.geometry.vertices.iter().enumerate() {
        let prior_d = oracle.prior_displacement_m[vertex];
        let prior_v = oracle.prior_velocity_m_per_s[vertex];
        let next_d = oracle.accepted.displacement_m[vertex];
        let next_v = oracle.accepted.velocity_m_per_s[vertex];
        let next_a = oracle.accepted.acceleration_m_per_s2[vertex];
        assert_eq!(prior_d.0, vertex);
        assert_eq!(prior_v.0, vertex);
        assert_eq!(next_d.0, vertex);
        assert_eq!(next_v.0, vertex);
        assert_eq!(next_a.0, vertex);
        assert_vector_close(
            prior_d.1,
            [coordinates[0] / 100.0, 0.0, 0.0],
            oracle.tolerances.displacement_and_velocity,
            "prior affine displacement",
        );
        assert_vector_close(
            prior_v.1,
            [coordinates[0] / 50.0, 0.0, 0.0],
            oracle.tolerances.displacement_and_velocity,
            "prior affine velocity",
        );
        assert_vector_close(
            next_d.1,
            [3.0 * coordinates[0] / 200.0, 0.0, 0.0],
            oracle.tolerances.displacement_and_velocity,
            "next affine displacement",
        );
        assert_vector_close(
            next_v.1,
            [coordinates[0] / 50.0, 0.0, 0.0],
            oracle.tolerances.displacement_and_velocity,
            "next affine velocity",
        );
        let backward_difference = std::array::from_fn(|component| {
            (next_d.1[component] - prior_d.1[component]) / oracle.time_step_s
        });
        assert_vector_close(
            next_v.1,
            backward_difference,
            oracle.tolerances.displacement_and_velocity,
            "backward-Euler kinematics",
        );
        let acceleration = std::array::from_fn(|component| {
            (next_v.1[component] - prior_v.1[component]) / oracle.time_step_s
        });
        assert_vector_close(
            next_a.1,
            acceleration,
            oracle
                .tolerances
                .acceleration_stress_reaction_and_face_total,
            "backward-Euler acceleration",
        );
    }

    let expected_strain = [[3.0 / 200.0, 0.0, 0.0], [0.0; 3], [0.0; 3]];
    assert_matrix_close(
        oracle.accepted.strain,
        expected_strain,
        oracle
            .tolerances
            .acceleration_stress_reaction_and_face_total,
        "affine strain",
    );
    let trace =
        oracle.accepted.strain[0][0] + oracle.accepted.strain[1][1] + oracle.accepted.strain[2][2];
    let stress = std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            2.0 * oracle.material.shear_modulus_pa * oracle.accepted.strain[row][column]
                + if row == column {
                    oracle.material.first_lame_parameter_pa * trace
                } else {
                    0.0
                }
        })
    });
    assert_matrix_close(
        oracle.accepted.stress_pa,
        stress,
        oracle
            .tolerances
            .acceleration_stress_reaction_and_face_total,
        "affine stress",
    );

    let (mass, stiffness) = independently_assemble_center_blocks(&oracle);
    assert_matrix_close(
        oracle.accepted.center_mass_block,
        mass,
        oracle.tolerances.mass_and_stiffness,
        "independent density-inclusive center mass block",
    );
    assert_matrix_close(
        oracle.accepted.center_stiffness_block,
        stiffness,
        oracle.tolerances.mass_and_stiffness,
        "independent center stiffness block",
    );
    let backward_euler = std::array::from_fn(|row| {
        std::array::from_fn(|column| {
            stiffness[row][column] + mass[row][column] / oracle.time_step_s.powi(2)
        })
    });
    assert_matrix_close(
        oracle.accepted.center_backward_euler_block,
        backward_euler,
        oracle.tolerances.mass_and_stiffness,
        "independent center backward-Euler block",
    );

    assert_eq!(oracle.driven_vertices, vec![1, 3, 5, 7]);
    let fixed_total = reaction_total(
        &oracle.accepted.constraint_on_body_reaction_n,
        &[0, 2, 4, 6],
    );
    let driven_total = reaction_total(
        &oracle.accepted.constraint_on_body_reaction_n,
        &oracle.driven_vertices,
    );
    assert_vector_close(
        fixed_total,
        oracle.accepted.fixed_face_reaction_n,
        oracle
            .tolerances
            .acceleration_stress_reaction_and_face_total,
        "fixed-face reaction total",
    );
    assert_vector_close(
        driven_total,
        oracle.accepted.driven_face_reaction_n,
        oracle
            .tolerances
            .acceleration_stress_reaction_and_face_total,
        "driven-face reaction total",
    );
}

#[test]
fn prescribed_affine_step_matches_the_precommitted_oracle() {
    let oracle = oracle();
    let fixture = Fixture::new(&oracle);
    assert_eq!(
        fixture.mesh.mesh().vertices(),
        oracle
            .geometry
            .vertices
            .iter()
            .map(|coordinates| coordinates.to_vec())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.mesh.mesh().cells(),
        oracle
            .geometry
            .tetrahedra
            .iter()
            .map(|cell| cell.to_vec())
            .collect::<Vec<_>>()
    );

    let mut reference = fixture
        .reference(DynQuantity::new(oracle.time_step_s, TIME))
        .expect("the exact reference context is admitted");
    assert_eq!(reference.accepted_generation(), 0);
    assert_eq!(
        reference.driven_vertices(),
        oracle
            .driven_vertices
            .iter()
            .copied()
            .map(VertexId::new)
            .collect::<Vec<_>>()
    );
    let (generation, projected_displacement, projected_velocity) =
        reference.project_driven_surface();
    assert_eq!(generation, 0);
    assert_coefficients_close(
        &projected_displacement,
        &select_coefficients(&oracle.prior_displacement_m, &oracle.driven_vertices),
        oracle.tolerances.displacement_and_velocity,
        "projected prior driven displacement",
    );
    assert_coefficients_close(
        &projected_velocity,
        &select_coefficients(&oracle.prior_velocity_m_per_s, &oracle.driven_vertices),
        oracle.tolerances.displacement_and_velocity,
        "projected prior driven velocity",
    );

    let accepted: AcceptedPrescribedDynamicSolidStep3d = reference
        .accept_candidate(
            generation,
            &coefficients(&oracle.driven_total_displacement_m),
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("the precommitted total-displacement candidate is accepted");
    assert_eq!(reference.accepted_generation(), oracle.accepted.generation);
    assert_accepted_step(&accepted, &oracle);
}

#[test]
fn constructor_rejects_invalid_time_state_lineage_boundary_and_topology() {
    let oracle = oracle();
    let fixture = Fixture::new(&oracle);

    for invalid_time in [
        DynQuantity::new(oracle.time_step_s, DimExponents::DIMENSIONLESS),
        DynQuantity::new(0.0, TIME),
        DynQuantity::new(-oracle.time_step_s, TIME),
        DynQuantity::new(f64::NAN, TIME),
    ] {
        assert!(fixture.reference(invalid_time).is_err());
    }

    let mut missing_displacement = fixture.prior_displacement.clone();
    missing_displacement.pop();
    assert_reference_rejected(
        &fixture,
        &missing_displacement,
        &fixture.prior_velocity,
        fixture.driven_boundary,
    );
    let mut duplicate_velocity = fixture.prior_velocity.clone();
    duplicate_velocity[8].0 = duplicate_velocity[7].0;
    assert_reference_rejected(
        &fixture,
        &fixture.prior_displacement,
        &duplicate_velocity,
        fixture.driven_boundary,
    );
    let mut reordered_displacement = fixture.prior_displacement.clone();
    reordered_displacement.swap(0, 1);
    assert_reference_rejected(
        &fixture,
        &reordered_displacement,
        &fixture.prior_velocity,
        fixture.driven_boundary,
    );
    let mut foreign_vertex = fixture.prior_displacement.clone();
    foreign_vertex[8].0 = VertexId::new(99);
    assert_reference_rejected(
        &fixture,
        &foreign_vertex,
        &fixture.prior_velocity,
        fixture.driven_boundary,
    );
    let mut nonfinite_velocity = fixture.prior_velocity.clone();
    nonfinite_velocity[8].1[2] = f64::INFINITY;
    assert_reference_rejected(
        &fixture,
        &fixture.prior_displacement,
        &nonfinite_velocity,
        fixture.driven_boundary,
    );
    assert_reference_rejected(
        &fixture,
        &fixture.prior_displacement,
        &fixture.prior_velocity,
        domain(&fixture.document, "x_lower"),
    );
    assert!(
        PrescribedDynamicSolidReference3d::new(
            &fixture.model,
            &fixture.geometry,
            &fixture.mesh,
            &fixture.correspondence,
            DynQuantity::new(oracle.time_step_s, TIME),
            &fixture.prior_displacement,
            &fixture.prior_velocity,
            domain(&fixture.document, "y_lower"),
        )
        .is_err(),
        "an actual FluxZero y-lower boundary cannot be selected as the driven live PortBinding"
    );
    assert_reference_rejected(
        &fixture,
        &fixture.prior_displacement,
        &fixture.prior_velocity,
        Id::<kinds::Domain>::new(),
    );

    let foreign_source = DIRECT_SOURCE.replacen(
        "parameter density: kg / m ^ 3 = 2;",
        "parameter density: kg / m ^ 3 = 4;",
        1,
    );
    assert_ne!(foreign_source, DIRECT_SOURCE);
    let foreign_document = ModelDocument::compile("foreign-solid.eqi", &foreign_source).unwrap();
    let foreign_model = ModelEnvelope::from_program(foreign_document.program()).unwrap();
    let foreign_geometry = GeometryIdentityEnvelopeV1::new(
        &foreign_model,
        [domain(&foreign_document, "body")],
        1.0e-12,
    )
    .unwrap();
    let foreign_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&foreign_geometry, &foreign_model, &fixture.mesh)
            .unwrap();
    assert!(
        PrescribedDynamicSolidReference3d::new(
            &foreign_model,
            &foreign_geometry,
            &fixture.mesh,
            &foreign_correspondence,
            DynQuantity::new(oracle.time_step_s, TIME),
            &fixture.prior_displacement,
            &fixture.prior_velocity,
            domain(&foreign_document, "x_upper"),
        )
        .is_err(),
        "a coherent but foreign material Model and lineage must be rejected"
    );
    assert!(
        PrescribedDynamicSolidReference3d::new(
            &fixture.model,
            &fixture.geometry,
            &fixture.mesh,
            &foreign_correspondence,
            DynQuantity::new(oracle.time_step_s, TIME),
            &fixture.prior_displacement,
            &fixture.prior_velocity,
            fixture.driven_boundary,
        )
        .is_err(),
        "cross-wired correspondence lineage must be rejected"
    );
    let foreign_geometry = GeometryIdentityEnvelopeV1::new(
        &fixture.model,
        [domain(&fixture.document, "body")],
        2.0e-12,
    )
    .expect("the foreign geometry identity is independently valid");
    assert_ne!(
        foreign_geometry.digest().unwrap(),
        fixture.geometry.digest().unwrap()
    );
    assert!(
        PrescribedDynamicSolidReference3d::new(
            &fixture.model,
            &foreign_geometry,
            &fixture.mesh,
            &fixture.correspondence,
            DynQuantity::new(oracle.time_step_s, TIME),
            &fixture.prior_displacement,
            &fixture.prior_velocity,
            fixture.driven_boundary,
        )
        .is_err(),
        "a foreign geometry identity cannot be paired with the accepted correspondence"
    );

    let changed_mesh = SimplicialMeshEnvelopeV1::from_mesh(&changed_face_diagonal_mesh(&oracle))
        .expect("the face-diagonal mutant remains a valid positive mesh");
    let changed_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&fixture.geometry, &fixture.model, &changed_mesh)
            .expect("the changed face diagonal remains geometrically correspondent");
    assert!(
        PrescribedDynamicSolidReference3d::new(
            &fixture.model,
            &fixture.geometry,
            &changed_mesh,
            &changed_correspondence,
            DynQuantity::new(oracle.time_step_s, TIME),
            &fixture.prior_displacement,
            &fixture.prior_velocity,
            fixture.driven_boundary,
        )
        .is_err(),
        "a changed face diagonal cannot reuse the frozen node-indexed reaction oracle"
    );
}

#[test]
fn candidate_shape_identity_finiteness_and_generation_are_fail_closed() {
    let oracle = oracle();
    let fixture = Fixture::new(&oracle);
    let mut reference = fixture
        .reference(DynQuantity::new(oracle.time_step_s, TIME))
        .unwrap();
    let valid = coefficients(&oracle.driven_total_displacement_m);

    assert!(
        reference
            .accept_candidate(1, &valid, &PanicIfAssemblyReached, &REFERENCE_LINEAR_SOLVER,)
            .is_err(),
        "a future/stale generation must fail"
    );
    assert_eq!(reference.accepted_generation(), 0);

    let mut missing = valid.clone();
    missing.pop();
    let mut reordered = valid.clone();
    reordered.swap(0, 1);
    let mut duplicate = valid.clone();
    duplicate[1].0 = duplicate[0].0;
    let mut foreign = valid.clone();
    foreign[0].0 = VertexId::new(0);
    let mut nonfinite = valid.clone();
    nonfinite[2].1[1] = f64::NAN;
    for invalid in [missing, reordered, duplicate, foreign, nonfinite] {
        assert!(
            reference
                .accept_candidate(
                    0,
                    &invalid,
                    &PanicIfAssemblyReached,
                    &REFERENCE_LINEAR_SOLVER,
                )
                .is_err()
        );
        assert_eq!(reference.accepted_generation(), 0);
    }

    let accepted = reference
        .accept_candidate(
            0,
            &valid,
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
        )
        .unwrap();
    assert_eq!(accepted.generation(), 1);
    assert_eq!(reference.accepted_generation(), 1);
    assert!(
        reference
            .accept_candidate(0, &valid, &PanicIfAssemblyReached, &REFERENCE_LINEAR_SOLVER,)
            .is_err(),
        "an already-consumed generation must be stale"
    );
    assert_eq!(reference.accepted_generation(), 1);
}

#[test]
fn changed_total_candidate_derives_new_boundary_velocity() {
    let oracle = oracle();
    let fixture = Fixture::new(&oracle);
    let mut reference = fixture
        .reference(DynQuantity::new(oracle.time_step_s, TIME))
        .unwrap();
    let changed_candidate = oracle
        .driven_vertices
        .iter()
        .copied()
        .map(|vertex| (VertexId::new(vertex), [1.0 / 50.0, 0.0, 0.0]))
        .collect::<Vec<_>>();
    let accepted = reference
        .accept_candidate(
            0,
            &changed_candidate,
            &REFERENCE_ASSEMBLY_BACKEND,
            &REFERENCE_LINEAR_SOLVER,
        )
        .expect("the changed total-displacement candidate is admissible");

    for vertex in &oracle.driven_vertices {
        assert_vector_close(
            coefficient(accepted.displacement(), *vertex),
            [1.0 / 50.0, 0.0, 0.0],
            oracle.tolerances.displacement_and_velocity,
            "changed total boundary displacement",
        );
        assert_vector_close(
            coefficient(accepted.velocity(), *vertex),
            [1.0 / 25.0, 0.0, 0.0],
            oracle.tolerances.displacement_and_velocity,
            "velocity derived from changed total candidate",
        );
        assert_vector_close(
            coefficient(accepted.acceleration(), *vertex),
            [2.0 / 25.0, 0.0, 0.0],
            oracle
                .tolerances
                .acceleration_stress_reaction_and_face_total,
            "changed boundary acceleration",
        );
    }
}

#[test]
fn validation_assembly_and_solver_failures_publish_no_partial_generation() {
    let oracle = oracle();
    let fixture = Fixture::new(&oracle);
    let candidate = coefficients(&oracle.driven_total_displacement_m);

    let mut validation_reference = fixture
        .reference(DynQuantity::new(oracle.time_step_s, TIME))
        .unwrap();
    let mut invalid = candidate.clone();
    invalid[0].1[0] = f64::INFINITY;
    assert!(
        validation_reference
            .accept_candidate(
                0,
                &invalid,
                &PanicIfAssemblyReached,
                &REFERENCE_LINEAR_SOLVER,
            )
            .is_err()
    );
    assert_eq!(validation_reference.accepted_generation(), 0);

    let mut assembly_reference = fixture
        .reference(DynQuantity::new(oracle.time_step_s, TIME))
        .unwrap();
    assert!(
        assembly_reference
            .accept_candidate(0, &candidate, &RejectAssembly, &REFERENCE_LINEAR_SOLVER)
            .is_err()
    );
    assert_eq!(assembly_reference.accepted_generation(), 0);
    assert_eq!(assembly_reference.project_driven_surface().0, 0);
    assert!(
        assembly_reference
            .accept_candidate(
                0,
                &candidate,
                &REFERENCE_ASSEMBLY_BACKEND,
                &REFERENCE_LINEAR_SOLVER,
            )
            .is_ok(),
        "the same generation remains admissible after assembly failure"
    );

    let mut solver_reference = fixture
        .reference(DynQuantity::new(oracle.time_step_s, TIME))
        .unwrap();
    assert!(
        solver_reference
            .accept_candidate(0, &candidate, &REFERENCE_ASSEMBLY_BACKEND, &RejectSolver)
            .is_err()
    );
    assert_eq!(solver_reference.accepted_generation(), 0);
    assert_eq!(solver_reference.project_driven_surface().0, 0);
    assert!(
        solver_reference
            .accept_candidate(
                0,
                &candidate,
                &REFERENCE_ASSEMBLY_BACKEND,
                &REFERENCE_LINEAR_SOLVER,
            )
            .is_ok(),
        "the same generation remains admissible after solver failure"
    );
}

fn assert_accepted_step(accepted: &AcceptedPrescribedDynamicSolidStep3d, oracle: &Oracle) {
    let generation: u64 = accepted.generation();
    let displacement: &[(VertexId, [f64; 3])] = accepted.displacement();
    let velocity: &[(VertexId, [f64; 3])] = accepted.velocity();
    let acceleration: &[(VertexId, [f64; 3])] = accepted.acceleration();
    let reactions: &[(VertexId, [f64; 3])] = accepted.constraint_reactions();
    let mass: &CsrMatrix = accepted.mass_operator();
    let stiffness: &CsrMatrix = accepted.stiffness_operator();
    let reduced: &CanonicalCsrSystemView = accepted.reduced_system();
    let assembly: &eqiora::assembly::AssemblyReport = accepted.assembly_report();
    let solve: &eqiora::solver::SolveReport = accepted.solve_report();

    assert_eq!(generation, oracle.accepted.generation);
    assert_coefficients_close(
        displacement,
        &oracle.accepted.displacement_m,
        oracle.tolerances.displacement_and_velocity,
        "accepted displacement",
    );
    assert_coefficients_close(
        velocity,
        &oracle.accepted.velocity_m_per_s,
        oracle.tolerances.displacement_and_velocity,
        "accepted velocity",
    );
    assert_coefficients_close(
        acceleration,
        &oracle.accepted.acceleration_m_per_s2,
        oracle
            .tolerances
            .acceleration_stress_reaction_and_face_total,
        "accepted acceleration",
    );
    assert_coefficients_close(
        reactions,
        &oracle.accepted.constraint_on_body_reaction_n,
        oracle
            .tolerances
            .acceleration_stress_reaction_and_face_total,
        "constraint-on-body reactions",
    );
    assert_vector_close(
        reaction_total_from_coefficients(reactions, &[0, 2, 4, 6]),
        oracle.accepted.fixed_face_reaction_n,
        oracle
            .tolerances
            .acceleration_stress_reaction_and_face_total,
        "accepted fixed-face reaction total",
    );
    assert_vector_close(
        reaction_total_from_coefficients(reactions, &oracle.driven_vertices),
        oracle.accepted.driven_face_reaction_n,
        oracle
            .tolerances
            .acceleration_stress_reaction_and_face_total,
        "accepted driven-face reaction total",
    );

    assert_eq!((mass.rows(), mass.columns()), (27, 27));
    assert_eq!((stiffness.rows(), stiffness.columns()), (27, 27));
    assert_csr_block_close(
        mass,
        24,
        oracle.accepted.center_mass_block,
        oracle.tolerances.mass_and_stiffness,
        "M_88",
    );
    assert_csr_block_close(
        stiffness,
        24,
        oracle.accepted.center_stiffness_block,
        oracle.tolerances.mass_and_stiffness,
        "K_88",
    );
    assert_eq!((reduced.rows(), reduced.columns()), (3, 3));
    assert_eq!(
        reduced.properties(),
        LinearOperatorProperties::SymmetricPositiveDefinite
    );
    assert_system_block_close(
        reduced,
        oracle.accepted.center_backward_euler_block,
        oracle.tolerances.mass_and_stiffness,
        "reduced A_88",
    );

    assert_close(
        accepted.free_momentum_residual_norm(),
        oracle.accepted.free_momentum_residual_norm_n,
        oracle.tolerances.free_momentum_residual_n,
        "free momentum residual norm",
    );
    assert_close(
        accepted.kinematic_residual_norm(),
        oracle.accepted.kinematic_residual_norm_m_per_s,
        oracle.tolerances.kinematic_residual,
        "kinematic residual norm",
    );
    assert_eq!(assembly.execution(), ExecutionReport::host_serial());
    assert_eq!(assembly.packet_count(), 12);
    assert_eq!(assembly.target_count(), 2);
    assert_eq!(solve.execution(), ExecutionReport::host_serial());
    assert_eq!(solve.verification(), ExecutionReport::host_serial());
    assert_eq!(solve.algorithm(), LinearSolver::ConjugateGradient);
    assert_eq!(solve.preconditioner(), PreconditionerPolicy::Identity);
    assert_eq!(solve.reduction(), ReductionPolicy::Reproducible);
    assert_eq!(solve.solver_plan(), reference_solver_plan());
    assert!(solve.true_residual_norm() <= solve.residual_target());
}

fn reference_solver_plan() -> SolverPlan {
    SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-13,
        1.0e-15,
        std::num::NonZeroUsize::new(500).unwrap(),
    )
    .unwrap()
    .with_preconditioner(PreconditionerPolicy::Identity)
    .with_reduction(ReductionPolicy::Reproducible)
}

fn oracle() -> Oracle {
    serde_json::from_str(EXPECTED_SOURCE).expect("the independent oracle fixture is valid JSON")
}

fn domain(document: &ModelDocument, name: &str) -> Id<kinds::Domain> {
    document.aliases()[name].downcast().unwrap()
}

fn coefficients(values: &[(usize, [f64; 3])]) -> Vec<(VertexId, [f64; 3])> {
    values
        .iter()
        .map(|(vertex, value)| (VertexId::new(*vertex), *value))
        .collect()
}

fn select_coefficients(values: &[(usize, [f64; 3])], vertices: &[usize]) -> Vec<(usize, [f64; 3])> {
    vertices
        .iter()
        .map(|vertex| {
            values
                .iter()
                .copied()
                .find(|(candidate, _)| candidate == vertex)
                .unwrap()
        })
        .collect()
}

fn oracle_mesh(oracle: &Oracle) -> SimplicialMesh {
    SimplicialMesh::new(
        3,
        oracle
            .geometry
            .vertices
            .iter()
            .map(|coordinates| coordinates.to_vec())
            .collect(),
        oracle
            .geometry
            .tetrahedra
            .iter()
            .map(|cell| cell.to_vec())
            .collect(),
        MeshQualityGate::new(0.1).unwrap(),
    )
    .expect("the exact ordered tetrahedral mesh is valid")
}

fn changed_face_diagonal_mesh(oracle: &Oracle) -> SimplicialMesh {
    let mut cells = oracle.geometry.tetrahedra.clone();
    cells[0] = [8, 0, 2, 4];
    cells[1] = [8, 2, 6, 4];
    for cell in &mut cells[..2] {
        if signed_tetrahedron_determinant(&oracle.geometry.vertices, cell) < 0.0 {
            cell.swap(1, 2);
        }
    }
    assert_ne!(cells, oracle.geometry.tetrahedra);
    SimplicialMesh::new(
        3,
        oracle
            .geometry
            .vertices
            .iter()
            .map(|coordinates| coordinates.to_vec())
            .collect(),
        cells.iter().map(|cell| cell.to_vec()).collect(),
        MeshQualityGate::new(0.1).unwrap(),
    )
    .expect("the changed face diagonal is a valid positive tetrahedralization")
}

fn assert_reference_rejected(
    fixture: &Fixture,
    prior_displacement: &[(VertexId, [f64; 3])],
    prior_velocity: &[(VertexId, [f64; 3])],
    driven_boundary: Id<kinds::Domain>,
) {
    assert!(
        PrescribedDynamicSolidReference3d::new(
            &fixture.model,
            &fixture.geometry,
            &fixture.mesh,
            &fixture.correspondence,
            DynQuantity::new(0.25, TIME),
            prior_displacement,
            prior_velocity,
            driven_boundary,
        )
        .is_err()
    );
}

fn signed_tetrahedron_determinant(vertices: &[[f64; 3]], cell: &[usize; 4]) -> f64 {
    let origin = vertices[cell[0]];
    let a = subtract(vertices[cell[1]], origin);
    let b = subtract(vertices[cell[2]], origin);
    let c = subtract(vertices[cell[3]], origin);
    determinant([a, b, c])
}

fn subtract(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|axis| left[axis] - right[axis])
}

fn determinant(columns: [[f64; 3]; 3]) -> f64 {
    let [a, b, c] = columns;
    a[0] * (b[1] * c[2] - b[2] * c[1]) - b[0] * (a[1] * c[2] - a[2] * c[1])
        + c[0] * (a[1] * b[2] - a[2] * b[1])
}

fn inverse(matrix_columns: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let [a, b, c] = matrix_columns;
    let matrix = [[a[0], b[0], c[0]], [a[1], b[1], c[1]], [a[2], b[2], c[2]]];
    let determinant = determinant(matrix_columns);
    [
        [
            (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1]) / determinant,
            (matrix[0][2] * matrix[2][1] - matrix[0][1] * matrix[2][2]) / determinant,
            (matrix[0][1] * matrix[1][2] - matrix[0][2] * matrix[1][1]) / determinant,
        ],
        [
            (matrix[1][2] * matrix[2][0] - matrix[1][0] * matrix[2][2]) / determinant,
            (matrix[0][0] * matrix[2][2] - matrix[0][2] * matrix[2][0]) / determinant,
            (matrix[0][2] * matrix[1][0] - matrix[0][0] * matrix[1][2]) / determinant,
        ],
        [
            (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]) / determinant,
            (matrix[0][1] * matrix[2][0] - matrix[0][0] * matrix[2][1]) / determinant,
            (matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0]) / determinant,
        ],
    ]
}

fn independently_assemble_center_blocks(oracle: &Oracle) -> ([[f64; 3]; 3], [[f64; 3]; 3]) {
    let mut mass = [[0.0; 3]; 3];
    let mut stiffness = [[0.0; 3]; 3];
    for cell in &oracle.geometry.tetrahedra {
        assert_eq!(cell[0], 8, "the center is local P1 node zero in every cell");
        let origin = oracle.geometry.vertices[cell[0]];
        let jacobian = [
            subtract(oracle.geometry.vertices[cell[1]], origin),
            subtract(oracle.geometry.vertices[cell[2]], origin),
            subtract(oracle.geometry.vertices[cell[3]], origin),
        ];
        let determinant = determinant(jacobian);
        let volume = determinant / 6.0;
        let inverse = inverse(jacobian);
        let center_gradient: [f64; 3] =
            std::array::from_fn(|axis| -(inverse[0][axis] + inverse[1][axis] + inverse[2][axis]));
        let gradient_norm_squared = center_gradient
            .iter()
            .map(|component| component * component)
            .sum::<f64>();
        for row in 0..3 {
            mass[row][row] += oracle.material.density_kg_per_m3 * volume / 10.0;
            for column in 0..3 {
                stiffness[row][column] += volume
                    * ((oracle.material.first_lame_parameter_pa
                        + oracle.material.shear_modulus_pa)
                        * center_gradient[row]
                        * center_gradient[column]
                        + if row == column {
                            oracle.material.shear_modulus_pa * gradient_norm_squared
                        } else {
                            0.0
                        });
            }
        }
    }
    (mass, stiffness)
}

fn reaction_total(values: &[(usize, [f64; 3])], vertices: &[usize]) -> [f64; 3] {
    vertices.iter().fold([0.0; 3], |mut total, vertex| {
        let value = values
            .iter()
            .find(|(candidate, _)| candidate == vertex)
            .unwrap()
            .1;
        for component in 0..3 {
            total[component] += value[component];
        }
        total
    })
}

fn reaction_total_from_coefficients(
    values: &[(VertexId, [f64; 3])],
    vertices: &[usize],
) -> [f64; 3] {
    vertices.iter().fold([0.0; 3], |mut total, vertex| {
        let value = coefficient(values, *vertex);
        for component in 0..3 {
            total[component] += value[component];
        }
        total
    })
}

fn coefficient(values: &[(VertexId, [f64; 3])], vertex: usize) -> [f64; 3] {
    values
        .iter()
        .find(|(candidate, _)| candidate.index() == vertex)
        .unwrap()
        .1
}

fn assert_coefficients_close(
    actual: &[(VertexId, [f64; 3])],
    expected: &[(usize, [f64; 3])],
    tolerance: f64,
    label: &str,
) {
    assert_eq!(actual.len(), expected.len(), "{label} coefficient count");
    for ((actual_vertex, actual_value), (expected_vertex, expected_value)) in
        actual.iter().zip(expected)
    {
        assert_eq!(
            actual_vertex.index(),
            *expected_vertex,
            "{label} vertex order"
        );
        assert_vector_close(*actual_value, *expected_value, tolerance, label);
    }
}

fn assert_csr_block_close(
    matrix: &CsrMatrix,
    first: usize,
    expected: [[f64; 3]; 3],
    tolerance: f64,
    label: &str,
) {
    for row in 0..3 {
        for column in 0..3 {
            assert_close(
                csr_entry(
                    matrix.row_offsets(),
                    matrix.column_indices(),
                    matrix.values(),
                    first + row,
                    first + column,
                ),
                expected[row][column],
                tolerance,
                label,
            );
        }
    }
}

fn assert_system_block_close(
    system: &CanonicalCsrSystemView,
    expected: [[f64; 3]; 3],
    tolerance: f64,
    label: &str,
) {
    for row in 0..3 {
        for column in 0..3 {
            assert_close(
                csr_entry(
                    system.row_offsets(),
                    system.column_indices(),
                    system.values(),
                    row,
                    column,
                ),
                expected[row][column],
                tolerance,
                label,
            );
        }
    }
}

fn csr_entry(
    row_offsets: &[usize],
    column_indices: &[usize],
    values: &[f64],
    row: usize,
    column: usize,
) -> f64 {
    let range = row_offsets[row]..row_offsets[row + 1];
    match column_indices[range.clone()].binary_search(&column) {
        Ok(local) => values[range.start + local],
        Err(_) => 0.0,
    }
}

fn assert_matrix_close(
    actual: [[f64; 3]; 3],
    expected: [[f64; 3]; 3],
    tolerance: f64,
    label: &str,
) {
    for row in 0..3 {
        assert_vector_close(actual[row], expected[row], tolerance, label);
    }
}

fn assert_vector_close(actual: [f64; 3], expected: [f64; 3], tolerance: f64, label: &str) {
    for component in 0..3 {
        assert_close(actual[component], expected[component], tolerance, label);
    }
}

fn assert_close(actual: f64, expected: f64, tolerance: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{label}: expected {expected:.17e} +/- {tolerance:.1e}, found {actual:.17e}"
    );
}

#[derive(Debug)]
struct RejectAssembly;

impl AssemblyBackend for RejectAssembly {
    fn assemble(
        &self,
        _plan: &AssemblyPlan,
        _work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic> {
        Err(Diagnostic::error(
            eqiora::diagnostic::codes::ASSEMBLY_FAILED,
            "oracle-injected assembly failure",
        ))
    }
}

#[derive(Debug)]
struct PanicIfAssemblyReached;

impl AssemblyBackend for PanicIfAssemblyReached {
    fn assemble(
        &self,
        _plan: &AssemblyPlan,
        _work: &dyn AssemblyWork,
    ) -> Result<AssemblyResult, Diagnostic> {
        panic!("candidate validation and generation checks must precede assembly")
    }
}

#[derive(Debug)]
struct RejectSolver;

impl LinearSolverBackend for RejectSolver {
    fn provider(&self) -> SolverProvider {
        REFERENCE_LINEAR_SOLVER.provider()
    }

    fn capabilities(&self) -> SolverCapabilities {
        REFERENCE_LINEAR_SOLVER.capabilities()
    }

    fn solve_with_execution(
        &self,
        _problem: &LinearProblem<'_>,
        _plan: SolverPlan,
        _execution: &dyn ReplicatedLinearExecution,
    ) -> Result<LinearSolution, Diagnostic> {
        Err(Diagnostic::error(
            eqiora::diagnostic::codes::NUMERICAL_SOLVE_FAILED,
            "oracle-injected solver failure",
        ))
    }
}
