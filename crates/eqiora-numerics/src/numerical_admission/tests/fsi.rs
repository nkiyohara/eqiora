use super::*;

#[test]
pub(super) fn common_fsi_resolves_exact_scopes_initializes_and_restarts_without_pressure_gauge() {
    use eqiora_core::{
        Id,
        entity::kinds::{Domain, Field},
    };

    let geometry = fsi_geometry();
    let model = fsi_model(&geometry);
    let resources = fsi_resources(&geometry);
    let recognized = RecognizedNativeAdmission::recognize(&model, resources.clone()).unwrap();
    let RecognizedNativeModel::Fsi(canonical) = &recognized.recognized else {
        panic!("component FSI source was not recognized as FSI")
    };
    let fluid_domain = Id::<Domain>::from_ulid(canonical.fluid().domain().ulid());
    let solid_domain = Id::<Domain>::from_ulid(canonical.solid().domain().ulid());
    let field_ids = [
        Id::<Field>::from_ulid(canonical.fluid().velocity().ulid()),
        Id::<Field>::from_ulid(canonical.fluid().pressure().ulid()),
        Id::<Field>::from_ulid(canonical.solid().velocity().ulid()),
        Id::<Field>::from_ulid(canonical.solid().displacement().ulid()),
    ];
    let digest = model.digest().unwrap();
    let scoped = CommonSpatialRequest::Scoped(vec![
        CommonScopedSpatialPolicy::new(digest.clone(), fluid_domain, CommonSpatialPolicy::MiniP1),
        CommonScopedSpatialPolicy::new(digest.clone(), solid_domain, CommonSpatialPolicy::P1),
    ]);
    let requested = SolverPlan::new(
        LinearSolver::ConjugateGradient,
        1.0e-11,
        1.0e-13,
        NonZeroUsize::new(20_000).unwrap(),
    )
    .unwrap();
    let temporal = CommonBackwardEuler::from_seconds(0.05).unwrap();
    let resolve = |scaling| {
        resolve_common_plan(
            &model,
            resources.clone(),
            scoped.clone(),
            CommonSolvePolicy::Linear(requested),
            scaling,
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("FSI resolved as ODE"),
            |_| panic!("FSI resolved as scalar"),
            |_| panic!("FSI resolved as elasticity"),
            |_| panic!("FSI resolved as Stokes"),
            |_| panic!("FSI resolved as transient flow"),
            |plan| plan,
        )
    };
    let automatic = resolve(None);
    let manual = resolve(Some(
        IncompressibleScalingRequest2d::from_si(
            Some(2.0),
            Some((4.0_f64 / 3.0).sqrt()),
            Some(8.0 / 3.0),
        )
        .unwrap(),
    ));
    assert_ne!(
        automatic.identity(),
        manual.identity(),
        "scaling provenance belongs to Plan identity"
    );
    assert_eq!(
        automatic.state_space_identity(),
        manual.state_space_identity()
    );
    assert!(automatic.scaling_receipt().production().is_some());
    assert_eq!(
        automatic.linear().algorithm(),
        LinearSolver::MinimumResidual
    );
    assert_eq!(
        automatic.solver_provider(),
        REFERENCE_LINEAR_SOLVER.provider()
    );

    let fields = vec![
        CommonInitialField::new(
            digest.clone(),
            field_ids[0],
            Some(CommonInitialValues::Vector2(
                vec![[0.0; 2]; 6].into_boxed_slice(),
            )),
            Some(CommonInitialValues::Vector2(
                vec![[0.0; 2]; 4].into_boxed_slice(),
            )),
        )
        .unwrap(),
        CommonInitialField::new(
            digest.clone(),
            field_ids[1],
            Some(CommonInitialValues::Scalar(
                vec![0.25; 6].into_boxed_slice(),
            )),
            None,
        )
        .unwrap(),
        CommonInitialField::new(
            digest.clone(),
            field_ids[2],
            Some(CommonInitialValues::Vector2(
                vec![[0.0; 2]; 6].into_boxed_slice(),
            )),
            None,
        )
        .unwrap(),
        CommonInitialField::new(
            digest.clone(),
            field_ids[3],
            Some(CommonInitialValues::Vector2(
                vec![[0.02, 0.0]; 6].into_boxed_slice(),
            )),
            None,
        )
        .unwrap(),
    ];
    let initial = automatic.initial_state(0.0, fields).unwrap();
    assert!(
        initial
            .pressure_vertex_values()
            .unwrap()
            .iter()
            .all(|value| *value == 0.25)
    );
    assert!(CommonFsiRunRequest::from_steps(manual.clone(), initial.clone(), 1, vec![1]).is_ok());
    let accepted = automatic
        .advance(&initial, &REFERENCE_LINEAR_SOLVER)
        .unwrap();
    assert!(accepted.fsi_accepted_solution().is_some());

    let foreign = eqiora_artifact::ArtifactDigest::from_sha256([7; 32]);
    let foreign_scoped = CommonSpatialRequest::Scoped(vec![
        CommonScopedSpatialPolicy::new(foreign, fluid_domain, CommonSpatialPolicy::MiniP1),
        CommonScopedSpatialPolicy::new(digest, solid_domain, CommonSpatialPolicy::P1),
    ]);
    assert!(
        resolve_common_plan(
            &model,
            resources,
            foreign_scoped,
            CommonSolvePolicy::Linear(requested),
            None,
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .is_err()
    );
}

pub(super) fn exercise_model_driven_common_mesh_admission_evidence() {
    scalar_q1_and_tpfa_consume_one_exact_anisotropic_common_mesh();
    admission_rejects_policy_and_resource_cross_wires();
    stokes_resolution_consumes_exact_source_owned_common_mesh();
    transient_common_plan_resolves_exact_mini_and_supplied_cartesian_resources();
}
