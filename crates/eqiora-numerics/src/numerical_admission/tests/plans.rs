use super::*;

#[test]
pub(super) fn scalar_q1_and_tpfa_consume_one_exact_anisotropic_common_mesh() {
    let geometry = rectangle();
    let model = model(&geometry);
    let exact_owner = resources(&geometry);
    let caller_resources = exact_owner.resources.clone();
    let q1 = NativeNumericalAdmission::admit(
        &model,
        exact_owner.clone(),
        NativeSpatialPolicy::ScalarQ1,
        linear(),
    )
    .unwrap();
    let q1_repeat = NativeNumericalAdmission::admit(
        &model,
        resources(&geometry),
        NativeSpatialPolicy::ScalarQ1,
        linear(),
    )
    .unwrap();
    let tpfa = NativeNumericalAdmission::admit(
        &model,
        exact_owner,
        NativeSpatialPolicy::ScalarTpfa,
        linear(),
    )
    .unwrap();
    let alternate_provider = NativeNumericalAdmission::admit(
        &model,
        resources(&geometry),
        NativeSpatialPolicy::ScalarQ1,
        NativeLinearPolicy::exact(
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                1.0e-10,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap(),
            &AlternateScalarBackend,
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(q1.model(), &model);
    assert_eq!(q1.model_digest(), model.digest().unwrap().to_string());
    assert_ne!(q1.policy_identity(), tpfa.policy_identity());
    assert_eq!(q1.policy_identity(), q1_repeat.policy_identity());
    assert_ne!(q1.policy_identity(), alternate_provider.policy_identity());
    assert_eq!(q1.resources(), &caller_resources);
    assert_eq!(tpfa.resources(), &caller_resources);
    assert_eq!(q1.resources(), q1_repeat.resources());
    assert!(q1.execute_scalar(&AlternateScalarBackend).is_err());
    assert_eq!(
        q1.execute_scalar(&REFERENCE_LINEAR_SOLVER)
            .unwrap()
            .into_primary_field_values()
            .len(),
        12
    );
    assert_eq!(
        tpfa.execute_scalar(&REFERENCE_LINEAR_SOLVER)
            .unwrap()
            .into_primary_field_values()
            .len(),
        6
    );
}

#[test]
pub(super) fn common_scalar_plan_owns_exact_lineage_and_executes_without_repeated_inputs() {
    let geometry = rectangle();
    let model = model(&geometry);
    let linear =
        CommonLinearRequest::new(1.0e-10, 1.0e-12, NonZeroUsize::new(10_000).unwrap()).unwrap();
    let resolve_scalar = |spatial, solve| {
        resolve_common_plan(
            &model,
            resources(&geometry),
            spatial,
            CommonSolvePolicy::Linear(solve),
            None,
            None,
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("spatial Model resolved as no-Mesh ODE"),
            |plan| plan,
            |_| panic!("scalar Model resolved as elasticity"),
            |_| panic!("scalar Model resolved as another capability"),
            |_| panic!("scalar Model resolved as transient capability"),
            |_| panic!("scalar Model resolved as FSI"),
        )
    };
    let q1 = resolve_scalar(CommonSpatialPolicy::Q1, linear);
    let repeat = resolve_scalar(CommonSpatialPolicy::Q1, linear);
    let tpfa = resolve_scalar(CommonSpatialPolicy::CellCenteredTpfa, linear);
    let alternate_tolerance = resolve_scalar(
        CommonSpatialPolicy::Q1,
        CommonLinearRequest::new(1.0e-9, 1.0e-12, NonZeroUsize::new(10_000).unwrap()).unwrap(),
    );

    assert_eq!(q1.identity(), repeat.identity());
    assert_ne!(q1.identity(), tpfa.identity());
    assert_ne!(q1.identity(), alternate_tolerance.identity());
    assert_eq!(q1.realization_digest(), repeat.realization_digest());
    assert_ne!(q1.realization_digest(), tpfa.realization_digest());
    assert_eq!(
        q1.realization_digest(),
        hex_bytes(&q1.portable_realization().digest().unwrap())
    );
    assert_eq!(q1.model_digest(), model.digest().unwrap().to_string());
    assert_eq!(q1.cells(), [2, 3]);
    let mut crossed_realization = q1.clone();
    crossed_realization.portable = tpfa.portable_realization().clone();
    assert!(crossed_realization.run().is_err());
    assert_eq!(q1.run().unwrap().into_primary_field_values().len(), 12);
    assert_eq!(tpfa.run().unwrap().into_primary_field_values().len(), 6);
    assert!(
        resolve_common_plan(
            &model,
            resources(&geometry),
            CommonSpatialPolicy::MiniP1,
            CommonSolvePolicy::Linear(linear),
            None,
            None,
            &ResolveOnlyBackend,
        )
        .is_err()
    );
}

#[test]
pub(super) fn common_elasticity_plan_consumes_exact_mesh_and_model_meaning() {
    let geometry = rectangle();
    let model = elasticity_model(&geometry, 3.0);
    let alternate_material = elasticity_model(&geometry, 4.0);
    let solve =
        CommonLinearRequest::new(1.0e-10, 1.0e-12, NonZeroUsize::new(10_000).unwrap()).unwrap();
    let resolve_elasticity = |model: &ModelEnvelope| {
        resolve_common_plan(
            model,
            resources(&geometry),
            CommonSpatialPolicy::Q1,
            CommonSolvePolicy::Linear(solve),
            None,
            None,
            &ResolveOnlyBackend,
        )
        .unwrap()
        .project(
            |_| panic!("spatial Model resolved as no-Mesh ODE"),
            |_| panic!("elasticity Model resolved as scalar"),
            |plan| plan,
            |_| panic!("elasticity Model resolved as Stokes"),
            |_| panic!("elasticity Model resolved as transient flow"),
            |_| panic!("elasticity Model resolved as FSI"),
        )
    };
    let plan = resolve_elasticity(&model);
    let repeat = resolve_elasticity(&model);
    let alternate = resolve_elasticity(&alternate_material);
    assert_eq!(plan.identity(), repeat.identity());
    assert_ne!(plan.identity(), alternate.identity());
    assert_eq!(plan.realization_digest(), repeat.realization_digest());
    assert_eq!(
        plan.realization_digest(),
        alternate.realization_digest(),
        "material coefficients belong to Model identity, not numerical realization identity"
    );
    assert_eq!(
        plan.realization_digest(),
        hex_bytes(&plan.portable_realization().digest().unwrap())
    );
    assert_eq!(plan.model_digest(), model.digest().unwrap().to_string());
    assert_eq!(plan.cells(), [2, 3]);
    let result = plan.run().unwrap();
    assert_eq!(result.displacement().mesh().axis_cell_count(0), Some(2));
    assert_eq!(result.displacement().mesh().axis_cell_count(1), Some(3));
    assert_eq!(result.displacement().values().len(), 24);
    assert!(
        resolve_common_plan(
            &model,
            resources(&geometry),
            CommonSpatialPolicy::CellCenteredTpfa,
            CommonSolvePolicy::Linear(solve),
            None,
            None,
            &ResolveOnlyBackend,
        )
        .is_err()
    );
}

#[test]
pub(super) fn admission_rejects_policy_and_resource_cross_wires() {
    let geometry = rectangle();
    let model = model(&geometry);
    assert!(
        NativeLinearPolicy::exact(
            SolverPlan::new(
                LinearSolver::ConjugateGradient,
                -0.0,
                1.0e-13,
                NonZeroUsize::new(1000).unwrap(),
            )
            .unwrap(),
            &REFERENCE_LINEAR_SOLVER,
        )
        .is_err()
    );
    for solver in [
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-13,
            NonZeroUsize::new(1000).unwrap(),
        )
        .unwrap()
        .with_preconditioner(PreconditionerPolicy::Jacobi),
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-13,
            NonZeroUsize::new(1000).unwrap(),
        )
        .unwrap()
        .with_reduction(ReductionPolicy::Fast),
        SolverPlan::new(
            LinearSolver::BiConjugateGradientStabilized,
            1.0e-10,
            1.0e-13,
            NonZeroUsize::new(1000).unwrap(),
        )
        .unwrap(),
    ] {
        assert!(
            NativeNumericalAdmission::admit(
                &model,
                resources(&geometry),
                NativeSpatialPolicy::ScalarQ1,
                NativeLinearPolicy::exact(solver, &REFERENCE_LINEAR_SOLVER).unwrap(),
            )
            .is_err()
        );
    }
    assert!(
        NativeNumericalAdmission::admit(
            &model,
            resources(&geometry),
            NativeSpatialPolicy::StokesMiniP1(
                IncompressibleFlowScaleProfile2d::new(
                    DynQuantity::new(
                        1.0,
                        DimExponents {
                            length: 1,
                            ..DimExponents::DIMENSIONLESS
                        }
                    ),
                    DynQuantity::new(
                        1.0,
                        DimExponents {
                            length: 1,
                            time: -1,
                            ..DimExponents::DIMENSIONLESS
                        }
                    ),
                    DynQuantity::new(
                        1.0,
                        DimExponents {
                            mass: 1,
                            length: -1,
                            time: -2,
                            ..DimExponents::DIMENSIONLESS
                        }
                    ),
                )
                .unwrap(),
            ),
            linear(),
        )
        .is_err()
    );

    let owner = resources(&geometry);
    let NativeMeshResources::Cartesian {
        geometry: exact_geometry,
        mesh,
        correspondence,
        production,
    } = owner.resources
    else {
        unreachable!()
    };
    let substituted_mesh = CartesianMeshEnvelopeV1::from_mesh(
        &CartesianMesh::from_axes(vec![vec![0.0, 0.25, 1.0], vec![0.0, 0.2, 0.8, 1.0]]).unwrap(),
    )
    .unwrap();
    assert!(
        AuthenticatedCommonMesh::structured_cartesian(
            exact_geometry.clone(),
            substituted_mesh,
            correspondence.clone(),
            production.clone(),
        )
        .is_err()
    );
    let mut correspondence_value: serde_json::Value =
        serde_json::from_slice(&correspondence.canonical_json().unwrap()).unwrap();
    let frontiers = correspondence_value["frontiers"].as_array_mut().unwrap();
    let left = frontiers[0]["facet_indices"].clone();
    let right = frontiers[1]["facet_indices"].clone();
    frontiers[0]["facet_indices"] = right;
    frontiers[1]["facet_indices"] = left;
    let relabelled = GeometryMeshCorrespondenceEnvelopeV1::from_json(
        &serde_json::to_vec(&correspondence_value).unwrap(),
        GeometryDecoderLimits::default(),
    )
    .unwrap();
    assert!(
        AuthenticatedCommonMesh::structured_cartesian(
            exact_geometry.clone(),
            mesh.clone(),
            relabelled,
            production.clone(),
        )
        .is_err()
    );
    let production_json = String::from_utf8(production.canonical_json().unwrap()).unwrap();
    let provider_mutation = production_json.replace(
        "\"identity\":\"eqiora.structured-cartesian\",\"version\":\"1\"",
        "\"identity\":\"eqiora.gmsh-cli\",\"version\":\"4.15.2\"",
    );
    assert!(MeshProductionLineageEnvelopeV1::from_json(provider_mutation.as_bytes()).is_err());
    let foreign_production_json = production_json.replace(
        &correspondence.digest().unwrap().to_string(),
        &"00".repeat(32),
    );
    let foreign_production =
        MeshProductionLineageEnvelopeV1::from_json(foreign_production_json.as_bytes()).unwrap();
    assert!(
        AuthenticatedCommonMesh::structured_cartesian(
            exact_geometry,
            mesh,
            correspondence,
            foreign_production,
        )
        .is_err()
    );

    let reaction_source = COMPONENT.replace(
            "-div(grad(potential))\n      - source_scale * sin(wave_number * coordinate(0))\n        * sin(wave_number * coordinate(1)) = 0;",
            "potential - 1 = 0;",
        );
    let reaction = scalar_model_from_source(&geometry, &reaction_source);
    let reaction_program = replay_program(&reaction, &geometry).unwrap();
    let reaction_transient =
        lower_transient_incompressible_navier_stokes_cartesian_2d(&reaction_program);
    assert!(
        recognize_capability(
            &reaction_program,
            &reaction_transient,
            &Err(invalid("not Geometry transient")),
            &Err(invalid("not FSI"))
        )
        .is_err()
    );

    let stokes_geometry = stokes_geometry();
    let non_stokes_source =
        STOKES_COMPONENT.replace("div(velocity) = 0;", "pressure - zero_pressure = 0;");
    let non_stokes = stokes_model_from_source(&stokes_geometry, &non_stokes_source);
    let non_stokes_program = replay_program(&non_stokes, &stokes_geometry).unwrap();
    let non_stokes_transient =
        lower_transient_incompressible_navier_stokes_cartesian_2d(&non_stokes_program);
    assert!(
        recognize_capability(
            &non_stokes_program,
            &non_stokes_transient,
            &Err(invalid("not Geometry transient")),
            &Err(invalid("not FSI"))
        )
        .is_err()
    );

    let foreign = rectangle();
    let mut foreign_resources = resources(&foreign);
    if let NativeMeshResources::Cartesian { geometry, .. } = &mut foreign_resources.resources {
        *geometry = {
            let graph = GeometryGraph::new();
            let rectangle = graph.rectangle([0.0, 2.0], [0.0, 1.0]).unwrap();
            let edges = rectangle.boundaries();
            graph
                .build(
                    &rectangle,
                    &BTreeMap::from([
                        ("region".to_owned(), vec![rectangle.region().into()]),
                        ("left".to_owned(), vec![edges[0].into()]),
                        ("right".to_owned(), vec![edges[1].into()]),
                        ("bottom".to_owned(), vec![edges[2].into()]),
                        ("top".to_owned(), vec![edges[3].into()]),
                    ]),
                )
                .unwrap()
        };
    }
    assert!(
        NativeNumericalAdmission::admit(
            &model,
            foreign_resources,
            NativeSpatialPolicy::ScalarQ1,
            linear(),
        )
        .is_err()
    );
}
