use super::*;

fn newton_policy(linear: CommonLinearRequest, nonlinear: NonlinearSolvePlan) -> CommonSolvePolicy {
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
        CommonLinearRequest::new(1.0e-10, 1.0e-12, NonZeroUsize::new(2_000).unwrap()).unwrap();
    let temporal = CommonBackwardEuler::from_seconds(0.01).unwrap();
    let nonlinear =
        NonlinearSolvePlan::new(1.0e-9, 1.0e-11, NonZeroUsize::new(16).unwrap(), 12).unwrap();
    let scaling = IncompressibleScalingRequest2d::from_si(Some(1.0), Some(2.0), Some(3.0)).unwrap();
    let resolve = |model: &ModelEnvelope, owner, spatial, formulation| {
        let method = match formulation {
            None => CommonMethodRequest::Uniform(spatial),
            Some(formulation) => CommonMethodRequest::Exact {
                spatial,
                formulation,
            },
        };
        let resolved = resolve_common_plan(
            model,
            owner,
            method,
            newton_policy(linear, nonlinear),
            Some(scaling),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .unwrap();
        replay_plan(resolved, &ResolveOnlyBackend).project(
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
        None,
    );
    let mini_replay = resolve(
        &replayed,
        affine_resources(&geometry),
        CommonSpatialPolicy::MiniP1,
        None,
    );
    let fvm = resolve(
        &model,
        resources(&geometry),
        CommonSpatialPolicy::CellCentered,
        None,
    );
    let resolve_program_controlled = |objective| {
        let linear = CommonLinearRequest::program_controlled(
            1.0e-10,
            1.0e-12,
            NonZeroUsize::new(2_000).unwrap(),
            objective,
        )
        .unwrap();
        resolve_common_plan(
            &model,
            resources(&geometry),
            CommonSpatialPolicy::CellCentered,
            newton_policy(linear, nonlinear),
            Some(scaling),
            Some(temporal),
            &PlanningFaerBackend,
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
    let robust = resolve_program_controlled(SolverPlanningObjective::Robust);
    let fast = resolve_program_controlled(SolverPlanningObjective::Fast);
    let low_memory = resolve_program_controlled(SolverPlanningObjective::LowMemory);
    let mini_exact = resolve(
        &model,
        affine_resources(&geometry),
        CommonSpatialPolicy::MiniP1,
        Some(FormulationKind::MixedGalerkin),
    );
    let fvm_exact = resolve(
        &model,
        resources(&geometry),
        CommonSpatialPolicy::CellCentered,
        Some(FormulationKind::IntegralConservative),
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
    assert_eq!(mini.realization_digest(), mini_replay.realization_digest());
    assert_ne!(mini.realization_digest(), fvm.realization_digest());
    assert_eq!(
        mini.realization_digest(),
        hex_bytes(&mini.portable_realization().digest().unwrap())
    );
    assert_ne!(mini.identity(), mini_exact.identity());
    assert_ne!(fvm.identity(), fvm_exact.identity());
    assert_ne!(robust.identity(), fast.identity());
    assert_ne!(fast.identity(), low_memory.identity());
    assert_eq!(
        robust.selected_solver_candidate_id(),
        Some("eqiora.reference.bicgstab-general-jacobi-reproducible-f64")
    );
    assert_eq!(
        fast.selected_solver_candidate_id(),
        Some("eqiora.faer.sparse-lu-general-identity-fast-f64")
    );
    assert_eq!(
        low_memory.selected_solver_candidate_id(),
        Some("eqiora.faer.bicgstab-general-jacobi-fast-f64")
    );
    for plan in [&robust, &fast, &low_memory] {
        assert_eq!(
            plan.solver_planning_policy_id(),
            Some("eqiora.host-serial-solver-planning/v1")
        );
        assert_eq!(plan.solver_planning_reasons().len(), 6);
        assert!(plan.selected_solver_evidence_case().is_some());
    }
    let unsupported_mini = resolve_common_plan(
        &model,
        affine_resources(&geometry),
        CommonSpatialPolicy::MiniP1,
        newton_policy(
            CommonLinearRequest::program_controlled(
                1.0e-10,
                1.0e-12,
                NonZeroUsize::new(2_000).unwrap(),
                SolverPlanningObjective::Robust,
            )
            .unwrap(),
            nonlinear,
        ),
        Some(scaling),
        Some(temporal),
        &PlanningFaerBackend,
    )
    .unwrap_err();
    assert!(
        unsupported_mini
            .message()
            .contains("cell-centered General canonical-CSR")
    );
    assert_eq!(
        mini.formulation().effective(),
        mini_exact.formulation().effective()
    );
    assert_eq!(
        fvm.formulation().effective(),
        fvm_exact.formulation().effective()
    );
    assert_eq!(
        mini_exact.formulation().requested(),
        FormulationSelectionMode::Exact
    );
    assert_eq!(
        fvm_exact.formulation().requested(),
        FormulationSelectionMode::Exact
    );
    assert_eq!(
        mini.state_space_identity(),
        mini_exact.state_space_identity()
    );
    assert_eq!(fvm.state_space_identity(), fvm_exact.state_space_identity());
    assert_eq!(mini.realization_digest(), mini_exact.realization_digest());
    assert_eq!(fvm.realization_digest(), fvm_exact.realization_digest());
    assert_ne!(
        fvm.realization_digest(),
        fvm_alternate_scaling.realization_digest()
    );
    assert_eq!(
        mini_exact.formulation().selection_reason_codes(),
        &["eqiora.formulation.exact.mixed-galerkin-admitted/v1"]
    );
    assert_eq!(
        fvm_exact.formulation().selection_reason_codes(),
        &["eqiora.formulation.exact.integral-conservative-admitted/v1"]
    );
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
    let mini_formulation = mini.formulation();
    assert_eq!(
        mini_formulation.requested(),
        FormulationSelectionMode::Automatic
    );
    assert_eq!(mini_formulation.effective(), FormulationKind::MixedGalerkin);
    assert_eq!(
        mini_formulation.boundary_treatment(),
        "explicit-trace-flux-laws"
    );
    assert_eq!(mini_formulation.rule_ids().len(), 6);
    assert_eq!(mini_formulation.selection_reason_codes().len(), 1);
    let fvm_formulation = fvm.formulation();
    assert_eq!(
        fvm_formulation.effective(),
        FormulationKind::IntegralConservative
    );
    assert_eq!(fvm_formulation.rule_ids().len(), 7);
    assert_ne!(mini_formulation, fvm_formulation);

    let mini_zero = mini.zero_state(0.0).unwrap();
    let fvm_zero = fvm.zero_state(0.0).unwrap();
    let mini_bytes = mini_zero.to_bytes().unwrap();
    assert_eq!(
        CommonState::from_bytes(
            &mini_bytes,
            &ResolvedCommonPlan::TransientFlow(Box::new(mini.clone())),
        )
        .unwrap(),
        mini_zero
    );
    let fvm_bytes = fvm_zero.to_bytes().unwrap();
    assert_eq!(
        CommonState::from_bytes(
            &fvm_bytes,
            &ResolvedCommonPlan::TransientFlow(Box::new(fvm.clone())),
        )
        .unwrap(),
        fvm_zero
    );
    let mut noncanonical = mini_bytes;
    noncanonical.push(b'\n');
    assert!(
        CommonState::from_bytes(
            &noncanonical,
            &ResolvedCommonPlan::TransientFlow(Box::new(mini.clone())),
        )
        .is_err()
    );
    assert!(
        CommonState::from_bytes(
            &fvm_bytes,
            &ResolvedCommonPlan::TransientFlow(Box::new(mini.clone())),
        )
        .is_err()
    );
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
    assert!(
        resolve_common_plan(
            &model,
            affine_resources(&geometry),
            CommonMethodRequest::Exact {
                spatial: CommonSpatialPolicy::MiniP1,
                formulation: FormulationKind::IntegralConservative,
            },
            newton_policy(linear, nonlinear),
            Some(scaling),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .is_err()
    );
    assert!(
        resolve_common_plan(
            &model,
            resources(&geometry),
            CommonMethodRequest::Exact {
                spatial: CommonSpatialPolicy::CellCentered,
                formulation: FormulationKind::MixedGalerkin,
            },
            newton_policy(linear, nonlinear),
            Some(scaling),
            Some(temporal),
            &ResolveOnlyBackend,
        )
        .is_err()
    );
}
