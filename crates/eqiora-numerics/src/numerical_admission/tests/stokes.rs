use super::*;

#[test]
pub(super) fn stokes_resolution_consumes_exact_source_owned_common_mesh() {
    let geometry = stokes_geometry();
    let model = stokes_model(&geometry);
    let policy = PlanarMeshQualityV1::new(1.0e-4, 1.0e-5, 50).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_reference(
            &geometry,
            policy.maximum_boundary_error_m(),
            policy.maximum_boundary_facets(),
            MeshQualityGate::new(policy.minimum_mean_ratio()).unwrap(),
        )
        .unwrap();
    let production =
        MeshProductionLineageEnvelopeV1::from_planar_circular_hole_reference_v1_resources(
            policy,
            &geometry,
            &mesh,
            &correspondence,
        )
        .unwrap();
    let correspondence_value: serde_json::Value =
        serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
    let frontiers = correspondence_value["frontiers"].as_array().unwrap();
    let assignment_proof: [Vec<usize>; 5] = std::array::from_fn(|edge| {
        frontiers[edge]["facet_indices"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| usize::try_from(value.as_u64().unwrap()).unwrap())
            .collect()
    });
    let provider_output = gmsh_provider_output(&mesh, &assignment_proof);
    let exact_gmsh =
        AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, provider_output.clone())
            .unwrap();
    let mut relabelled_assignments = assignment_proof.clone();
    relabelled_assignments.swap(0, 1);
    let relabelled_output = gmsh_provider_output(&mesh, &relabelled_assignments);
    let relabelled_gmsh =
        AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, relabelled_output).unwrap();
    assert_ne!(exact_gmsh, relabelled_gmsh);
    let NativeMeshResources::GmshSimplicial {
        correspondence: exact_correspondence,
        production: exact_production,
        ..
    } = &exact_gmsh.resources
    else {
        unreachable!("Gmsh factory returns Gmsh resources")
    };
    let NativeMeshResources::GmshSimplicial {
        correspondence: relabelled_correspondence,
        production: relabelled_production,
        ..
    } = &relabelled_gmsh.resources
    else {
        unreachable!("Gmsh factory returns Gmsh resources")
    };
    assert_ne!(exact_correspondence, relabelled_correspondence);
    assert_ne!(exact_production, relabelled_production);
    let malformed_output = provider_output
        .windows(b"1 5 1".len())
        .position(|window| window == b"1 5 1")
        .map(|offset| {
            let mut mutated = provider_output.clone();
            mutated[offset + 2] = b'9';
            mutated
        })
        .unwrap();
    assert!(
        AuthenticatedCommonMesh::gmsh_4152(geometry.clone(), policy, malformed_output).is_err()
    );
    let resources = AuthenticatedCommonMesh::planar_reference(
        geometry,
        mesh.clone(),
        correspondence,
        production,
    )
    .unwrap();
    let solver = SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-6,
        1.0e-13,
        NonZeroUsize::new(10_000).unwrap(),
    )
    .unwrap();
    let common = resolve_common_plan(
        &model,
        resources.clone(),
        CommonSpatialPolicy::MiniP1,
        CommonSolvePolicy::Linear(solver),
        None,
        None,
        &ResolveOnlyBackend,
    )
    .unwrap()
    .project(
        |_| panic!("spatial Model resolved as no-Mesh ODE"),
        |_| panic!("steady-Stokes Model resolved as another capability"),
        |_| panic!("steady-Stokes Model resolved as elasticity"),
        |plan| plan,
        |_| panic!("steady-Stokes Model resolved as transient capability"),
        |_| panic!("steady-Stokes Model resolved as FSI"),
    );
    let gmsh_common = resolve_common_plan(
        &model,
        exact_gmsh,
        CommonSpatialPolicy::MiniP1,
        CommonSolvePolicy::Linear(solver),
        None,
        None,
        &ResolveOnlyBackend,
    )
    .unwrap()
    .project(
        |_| panic!("spatial Model resolved as no-Mesh ODE"),
        |_| panic!("steady-Stokes Model resolved as another capability"),
        |_| panic!("steady-Stokes Model resolved as elasticity"),
        |plan| plan,
        |_| panic!("steady-Stokes Model resolved as transient capability"),
        |_| panic!("steady-Stokes Model resolved as FSI"),
    );
    let relabelled_common = resolve_common_plan(
        &model,
        relabelled_gmsh,
        CommonSpatialPolicy::MiniP1,
        CommonSolvePolicy::Linear(solver),
        None,
        None,
        &ResolveOnlyBackend,
    )
    .unwrap()
    .project(
        |_| panic!("spatial Model resolved as no-Mesh ODE"),
        |_| panic!("steady-Stokes Model resolved as another capability"),
        |_| panic!("steady-Stokes Model resolved as elasticity"),
        |plan| plan,
        |_| panic!("steady-Stokes Model resolved as transient capability"),
        |_| panic!("steady-Stokes Model resolved as FSI"),
    );
    assert_ne!(
        relabelled_common.identity(),
        gmsh_common.identity(),
        "provider role/support drift retained the same exact Plan identity",
    );
    assert_eq!(common.model_digest(), model.digest().unwrap().to_string());
    assert_eq!(common.mesh_digest(), mesh.digest().unwrap().to_string());
    assert_eq!(
        common.scales().length().value().to_bits(),
        0.41_f64.to_bits()
    );
    assert_eq!(
        common.scales().velocity().value().to_bits(),
        0.3_f64.to_bits()
    );
    assert_eq!(
        common.scales().pressure().value().to_bits(),
        (0.001_f64 * 0.3 / 0.41).to_bits()
    );
    assert_eq!(
        [
            common.scales().length().value().to_bits(),
            common.scales().velocity().value().to_bits(),
            common.scales().pressure().value().to_bits(),
            common.scales().gauge().value().to_bits(),
            common.scales().weak_functional().value().to_bits(),
        ],
        [
            gmsh_common.scales().length().value().to_bits(),
            gmsh_common.scales().velocity().value().to_bits(),
            gmsh_common.scales().pressure().value().to_bits(),
            gmsh_common.scales().gauge().value().to_bits(),
            gmsh_common.scales().weak_functional().value().to_bits(),
        ],
        "reference and Gmsh occurrences of one exact source must resolve bit-equal automatic scales",
    );
    assert_eq!(common.linear().algorithm(), LinearSolver::SparseLu);
    assert_eq!(common.linear().reduction(), ReductionPolicy::Fast);
    assert_eq!(
        common.linear().relative_tolerance(),
        solver.relative_tolerance()
    );
    assert_eq!(
        common.linear().absolute_tolerance(),
        solver.absolute_tolerance()
    );
    assert_eq!(
        common.linear().maximum_iterations(),
        solver.maximum_iterations()
    );
    let admission = NativeNumericalAdmission::admit(
        &model,
        resources,
        NativeSpatialPolicy::StokesMiniP1(common.scales()),
        NativeLinearPolicy::exact(common.linear(), &ResolveOnlyBackend).unwrap(),
    )
    .unwrap();
    assert_eq!(
        admission.capability(),
        NativeCapability::SteadyIncompressibleStokes
    );
    assert_eq!(admission.model(), &model);
    let binding = admission.stokes_binding().unwrap();
    let (_resolved, realization, velocity, pressure) = admission.resolve_stokes(&binding).unwrap();
    assert_eq!(
        realization.mesh_artifact().unwrap(),
        Some(mesh.digest().unwrap())
    );
    assert_eq!(velocity.family(), SpaceFamily::SimplexP1Bubble);
    assert!(matches!(
        pressure.family(),
        SpaceFamily::ContinuousLagrange { .. }
    ));
}
