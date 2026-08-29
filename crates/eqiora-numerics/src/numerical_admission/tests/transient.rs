use super::*;

fn newton_policy(linear: CommonLinearControls, nonlinear: NonlinearSolvePlan) -> CommonSolvePolicy {
    CommonSolvePolicy::Newton { nonlinear, linear }
}

#[test]
pub(super) fn transient_common_plan_resolves_exact_mini_and_supplied_cartesian_resources() {
    let geometry = rectangle();
    let model = transient_model();
    let replayed = ModelEnvelope::from_json(
        &model.canonical_json().unwrap(),
        ModelDecoderLimits::default(),
    )
    .unwrap();
    let linear =
        CommonLinearControls::new(1.0e-10, 1.0e-12, NonZeroUsize::new(2_000).unwrap()).unwrap();
    let temporal = CommonBackwardEuler::from_seconds(0.01).unwrap();
    let nonlinear =
        NonlinearSolvePlan::new(1.0e-9, 1.0e-11, NonZeroUsize::new(16).unwrap(), 12).unwrap();
    let scaling = IncompressibleScalingRequest2d::from_si(Some(1.0), Some(2.0), Some(3.0)).unwrap();
    let resolve = |model: &ModelEnvelope, owner, spatial| {
        resolve_common_plan(
            model,
            owner,
            spatial,
            newton_policy(linear, nonlinear),
            Some(scaling),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("spatial Model resolved as no-Mesh ODE"),
            |_| panic!("transient Model resolved as scalar"),
            |_| panic!("transient Model resolved as elasticity"),
            |_| panic!("transient Model resolved as steady Stokes"),
            |plan| plan,
            |_| panic!("transient Model resolved as FSI"),
        )
    };
    let mini = resolve(
        &model,
        affine_resources(&geometry),
        CommonSpatialPolicy::MiniP1,
    );
    let mini_replay = resolve(
        &replayed,
        affine_resources(&geometry),
        CommonSpatialPolicy::MiniP1,
    );
    let fvm = resolve(
        &model,
        resources(&geometry),
        CommonSpatialPolicy::CellCentered,
    );
    let custom_nonlinear =
        NonlinearSolvePlan::new(2.0e-9, 3.0e-11, NonZeroUsize::new(19).unwrap(), 7).unwrap();
    let custom = resolve_common_plan(
        &model,
        affine_resources(&geometry),
        CommonSpatialPolicy::MiniP1,
        newton_policy(linear, custom_nonlinear),
        Some(scaling),
        Some(temporal),
        &ResolveOnlyBackend,
    )
    .unwrap()
    .project(
        |_| panic!("spatial Model resolved as no-Mesh ODE"),
        |_| panic!("transient Model resolved as scalar"),
        |_| panic!("transient Model resolved as elasticity"),
        |_| panic!("transient Model resolved as steady Stokes"),
        |plan| plan,
        |_| panic!("transient Model resolved as FSI"),
    );
    let alternate_scaling =
        IncompressibleScalingRequest2d::from_si(Some(4.0), Some(5.0), Some(6.0)).unwrap();
    let fvm_alternate_scaling = resolve_common_plan(
        &model,
        resources(&geometry),
        CommonSpatialPolicy::CellCentered,
        newton_policy(linear, nonlinear),
        Some(alternate_scaling),
        Some(temporal),
        &ResolveOnlyBackend,
    )
    .unwrap()
    .project(
        |_| panic!("spatial Model resolved as no-Mesh ODE"),
        |_| panic!("transient Model resolved as scalar"),
        |_| panic!("transient Model resolved as elasticity"),
        |_| panic!("transient Model resolved as steady Stokes"),
        |plan| plan,
        |_| panic!("transient Model resolved as FSI"),
    );

    assert_eq!(mini.identity(), mini_replay.identity());
    assert_ne!(mini.identity(), fvm.identity());
    assert_ne!(mini.identity(), custom.identity());
    assert_eq!(custom.nonlinear(), custom_nonlinear);
    assert_eq!(mini.model_digest(), model.digest().unwrap().to_string());
    assert_eq!(mini.velocity_field_id(), fvm.velocity_field_id());
    assert_eq!(mini.pressure_field_id(), fvm.pressure_field_id());
    assert_eq!(mini.velocity_space().family(), SpaceFamily::SimplexP1Bubble);
    assert!(
        matches!(mini.pressure_space().family(), SpaceFamily::ContinuousLagrange { order } if order.get() == 1)
    );
    assert_eq!(fvm.velocity_space().family(), SpaceFamily::CellConstant);
    assert_eq!(fvm.pressure_space().family(), SpaceFamily::CellConstant);
    assert_eq!(mini.temporal().step().value().to_bits(), 0.01_f64.to_bits());
    assert_eq!(mini.scales().length().value().to_bits(), 1.0_f64.to_bits());
    assert_eq!(
        mini.scales().velocity().value().to_bits(),
        2.0_f64.to_bits()
    );
    assert_eq!(
        mini.scales().pressure().value().to_bits(),
        3.0_f64.to_bits()
    );
    assert_eq!(mini.linear().algorithm(), LinearSolver::SparseLu);
    assert_eq!(mini.linear().reduction(), ReductionPolicy::Fast);
    assert_eq!(fvm.linear().reduction(), ReductionPolicy::Reproducible);

    let mini_zero = mini.zero_state(0.0).unwrap();
    let fvm_zero = fvm.zero_state(0.0).unwrap();
    assert_eq!(mini_zero.velocity_vertex_values().unwrap().len(), 12);
    assert_eq!(mini_zero.velocity_cell_values().len(), 12);
    assert_eq!(mini_zero.pressure_vertex_values().unwrap().len(), 12);
    assert!(mini_zero.method_history_values().is_empty());
    assert_eq!(fvm_zero.velocity_cell_values().len(), 6);
    assert_eq!(fvm_zero.pressure_cell_values().unwrap().len(), 6);
    assert!(!fvm_zero.method_history_values().is_empty());
    let curl = mini.cell_average_velocity_curl_2d(&mini_zero).unwrap();
    assert_eq!(curl.as_ref(), &[0.0; 12]);
    assert_eq!(
        curl,
        custom.cell_average_velocity_curl_2d(&mini_zero).unwrap(),
        "derived values exclude solve policy when the exact State and field are unchanged",
    );
    assert!(fvm.cell_average_velocity_curl_2d(&fvm_zero).is_err());
    assert_eq!(custom.state_space_identity(), mini.state_space_identity());
    assert_eq!(
        fvm_alternate_scaling.state_space_identity(),
        fvm.state_space_identity(),
        "coherent-SI State compatibility excludes numerical scaling",
    );
    assert!(
        CommonTransientRunRequest::from_steps(custom.clone(), mini_zero.clone(), 2, vec![1, 2],)
            .is_ok()
    );
    assert!(CommonTransientRunRequest::from_steps(mini.clone(), fvm_zero, 1, vec![1],).is_err());
    let by_steps =
        CommonTransientRunRequest::from_steps(mini.clone(), mini_zero.clone(), 2, vec![1, 2])
            .unwrap();
    let by_times =
        CommonTransientRunRequest::from_times(mini.clone(), mini_zero, 0.02, vec![0.01, 0.02])
            .unwrap();
    assert_eq!(by_steps.identity(), by_times.identity());
    assert!(
        CommonTransientRunRequest::from_steps(mini, by_times.state().clone(), 2, vec![2, 1],)
            .is_err()
    );

    assert!(
        resolve_common_plan(
            &model,
            affine_resources(&geometry),
            CommonSpatialPolicy::MiniP1,
            CommonSolvePolicy::Linear(linear),
            Some(scaling),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .is_err()
    );
    assert!(
        resolve_common_plan(
            &model,
            affine_resources(&geometry),
            CommonSpatialPolicy::MiniP1,
            newton_policy(linear, nonlinear),
            Some(IncompressibleScalingRequest2d::from_si(Some(1.0), None, Some(3.0)).unwrap()),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .is_err()
    );
    assert!(
        resolve_common_plan(
            &model,
            affine_resources(&geometry),
            CommonSpatialPolicy::MiniP1,
            newton_policy(linear, nonlinear),
            Some(scaling),
            None,
            &ResolveOnlyBackend,
        )
        .is_err()
    );
    assert!(
        resolve_common_plan(
            &model,
            resources(&geometry),
            CommonSpatialPolicy::MiniP1,
            newton_policy(linear, nonlinear),
            Some(scaling),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .is_err()
    );
}
