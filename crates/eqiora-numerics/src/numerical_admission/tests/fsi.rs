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
    let solid_domain = Id::<Domain>::from_ulid(canonical.solid().continuum().domain().ulid());
    let field_ids = [
        Id::<Field>::from_ulid(canonical.fluid().velocity().ulid()),
        Id::<Field>::from_ulid(canonical.fluid().pressure().ulid()),
        Id::<Field>::from_ulid(canonical.solid().velocity().ulid()),
        Id::<Field>::from_ulid(canonical.solid().continuum().displacement().ulid()),
    ];
    let digest = model.digest().unwrap();
    let scoped = CommonMethodRequest::Scoped(vec![
        CommonScopedSpatialPolicy::new(digest.clone(), fluid_domain, CommonSpatialPolicy::MiniP1),
        CommonScopedSpatialPolicy::new(digest.clone(), solid_domain, CommonSpatialPolicy::P1),
    ]);
    let requested =
        CommonLinearRequest::new(1.0e-11, 1.0e-13, NonZeroUsize::new(20_000).unwrap()).unwrap();
    let temporal = CommonBackwardEuler::from_seconds(0.05).unwrap();
    let resolve = |scaling| {
        let resolved = resolve_common_plan(
            &model,
            resources.clone(),
            scoped.clone(),
            CommonSolvePolicy::Linear(requested),
            scaling,
            Some(temporal),
            &ResolveOnlyBackend,
            None,
        )
        .unwrap();
        replay_plan(resolved, &ResolveOnlyBackend).project(
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
    assert_eq!(
        automatic.realization_digest(),
        manual.realization_digest(),
        "equal effective realization choices have one portable graph identity"
    );
    assert_eq!(
        automatic.realization_digest(),
        hex_bytes(&automatic.portable_realization().digest().unwrap())
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
    let initial_bytes = initial.to_bytes().unwrap();
    assert_eq!(
        CommonState::from_bytes(
            &initial_bytes,
            &ResolvedCommonPlan::Fsi(Box::new(automatic.clone())),
        )
        .unwrap(),
        initial
    );
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
    let ten_step =
        CommonFsiRunRequest::from_steps(automatic.clone(), initial.clone(), 10, vec![10]).unwrap();
    let std::ops::ControlFlow::Continue(prepared_outputs) = ten_step
        .advance_accepted_actions(&REFERENCE_LINEAR_SOLVER, |_, _| false)
        .unwrap()
    else {
        panic!("uncancelled prepared FSI Run must finish")
    };
    let mut independently_accepted = initial.clone();
    let mut independently_accepted_at_four = None;
    for accepted_steps in 1..=10 {
        independently_accepted = automatic
            .advance(&independently_accepted, &REFERENCE_LINEAR_SOLVER)
            .unwrap();
        if accepted_steps == 4 {
            independently_accepted_at_four = Some(independently_accepted.clone());
        }
    }
    assert_eq!(prepared_outputs.as_slice(), &[(10, independently_accepted)]);
    assert_eq!(
        ten_step
            .advance_accepted_actions(&REFERENCE_LINEAR_SOLVER, |accepted_steps, _| {
                accepted_steps == 4
            })
            .unwrap(),
        std::ops::ControlFlow::Break((4, independently_accepted_at_four.unwrap())),
    );
    let request =
        CommonFsiRunRequest::from_steps(automatic.clone(), initial.clone(), 1, vec![1]).unwrap();
    let trajectory =
        crate::CommonTrajectory::accept_fsi(request, vec![(1, accepted.clone())]).unwrap();
    let result = crate::CommonResult::accept_trajectory(0.5, trajectory.clone()).unwrap();
    let result_bytes = result.to_bytes().unwrap();
    let replayed_result = crate::CommonResult::from_bytes(
        &result_bytes,
        &ResolvedCommonPlan::Fsi(Box::new(automatic.clone())),
    )
    .unwrap();
    assert_eq!(replayed_result.identity(), result.identity());
    assert_eq!(replayed_result.to_bytes().unwrap(), result_bytes);
    assert_eq!(replayed_result.fsi_state_count(), 1);
    assert_eq!(
        replayed_result.fsi_state_identity(0),
        Some(accepted.identity())
    );
    let mut forged_result: serde_json::Value = serde_json::from_slice(&result_bytes).unwrap();
    forged_result["content"]["payload"]["fsi"]["states"][0]["metrics"][8] =
        serde_json::Value::from(-1.0);
    assert!(
        crate::CommonResult::from_bytes(
            &serde_json::to_vec(&forged_result).unwrap(),
            &ResolvedCommonPlan::Fsi(Box::new(automatic.clone())),
        )
        .is_err()
    );
    let trajectory_bytes = trajectory.to_bytes().unwrap();
    let replayed_trajectory = crate::CommonTrajectory::from_bytes(
        &trajectory_bytes,
        &ResolvedCommonPlan::Fsi(Box::new(automatic.clone())),
    )
    .unwrap();
    assert_eq!(replayed_trajectory.identity(), trajectory.identity());
    assert_eq!(replayed_trajectory.to_bytes().unwrap(), trajectory_bytes);
    let replayed_states = replayed_trajectory.spatial_states().unwrap();
    assert_eq!(replayed_states[0].1.identity(), accepted.identity());
    assert!(replayed_states[0].1.fsi_accepted_solution().is_none());
    let accepted_bytes = accepted.to_bytes().unwrap();
    let replayed_accepted = CommonState::from_bytes(
        &accepted_bytes,
        &ResolvedCommonPlan::Fsi(Box::new(automatic.clone())),
    )
    .unwrap();
    assert_eq!(replayed_accepted.identity(), accepted.identity());
    assert_eq!(
        replayed_accepted.velocity_vertex_values(),
        accepted.velocity_vertex_values()
    );
    assert_eq!(
        replayed_accepted.pressure_vertex_values(),
        accepted.pressure_vertex_values()
    );
    assert!(replayed_accepted.fsi_accepted_solution().is_none());
    assert!(
        automatic
            .advance(&replayed_accepted, &REFERENCE_LINEAR_SOLVER)
            .is_ok()
    );

    let foreign = eqiora_artifact::ArtifactDigest::from_sha256([7; 32]);
    let foreign_scoped = CommonMethodRequest::Scoped(vec![
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
            None,
        )
        .is_err()
    );
}

pub(super) fn exercise_model_driven_common_mesh_admission_evidence() {
    scalar_q1_and_tpfa_consume_one_exact_anisotropic_common_mesh();
    admission_rejects_policy_and_resource_cross_wires();
    transient_common_plan_resolves_exact_mini_and_supplied_cartesian_resources();
}
