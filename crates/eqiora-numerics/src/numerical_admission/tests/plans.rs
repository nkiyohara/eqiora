use super::*;

const POISSON_INTERVAL: &str = r#"
public component PoissonInterval {
  public support body: volume(ambient_dimension = 1);
  public support left: boundary(parent = body);
  public support right: boundary(parent = body);
  public parameter source_scale: 1 / m ^ 2;
  representation space = continuum;
  field potential on body as space: 1 = 0;
  relation balance continuous on body {
    -div(grad(potential)) - source_scale = 0;
  }
  relation left_value continuous on left { trace(potential) = 0; }
  relation right_value continuous on right { trace(potential) = 0; }
}
"#;

const POISSON_BOX: &str = r#"
public component PoissonBox {
  public support body: volume(ambient_dimension = 3);
  public support x_lower: boundary(parent = body);
  public support x_upper: boundary(parent = body);
  public support y_lower: boundary(parent = body);
  public support y_upper: boundary(parent = body);
  public support z_lower: boundary(parent = body);
  public support z_upper: boundary(parent = body);
  public parameter source_scale: 1 / m ^ 2;
  representation space = continuum;
  field potential on body as space: 1 = 0;
  relation balance continuous on body {
    -div(grad(potential)) - source_scale = 0;
  }
  relation x_lower_value continuous on x_lower { trace(potential) = 0; }
  relation x_upper_value continuous on x_upper { trace(potential) = 0; }
  relation y_lower_value continuous on y_lower { trace(potential) = 0; }
  relation y_upper_value continuous on y_upper { trace(potential) = 0; }
  relation z_lower_value continuous on z_lower { trace(potential) = 0; }
  relation z_upper_value continuous on z_upper { trace(potential) = 0; }
}
"#;

fn cartesian_box_3d() -> CanonicalGeometryV1 {
    CanonicalGeometryV1::decode_cartesian_box_v1_canonical(
        br#"{"schema":"eqiora.cartesian-box-envelope/v1","encoding":"eqiora.canonical-json/v1","length_unit":"metre","bounds":[[0.0,1.0],[0.0,1.0],[0.0,1.0]],"entity_sets":[{"name":"x_lower","dimension":2,"members":[0]},{"name":"x_upper","dimension":2,"members":[1]},{"name":"y_lower","dimension":2,"members":[2]},{"name":"y_upper","dimension":2,"members":[3]},{"name":"z_lower","dimension":2,"members":[4]},{"name":"z_upper","dimension":2,"members":[5]},{"name":"body","dimension":3,"members":[0]}]}"#,
        eqiora_geometry::CanonicalGeometryLimits::default(),
    )
    .unwrap()
}

fn cartesian_interval() -> CanonicalGeometryV1 {
    let graph = GeometryGraph::new();
    let interval = graph.interval([0.0, 1.0]).unwrap();
    let [left, right]: [_; 2] = interval.boundaries().try_into().unwrap();
    graph
        .build(
            &interval,
            &BTreeMap::from([
                ("body".to_owned(), vec![interval.region().into()]),
                ("left".to_owned(), vec![left.into()]),
                ("right".to_owned(), vec![right.into()]),
            ]),
        )
        .unwrap()
}

fn scalar_box_model(
    geometry: &CanonicalGeometryV1,
    source: &str,
    model: &str,
    component: &str,
    boundaries: &[&str],
) -> ModelEnvelope {
    let body = geometry.entity_set("body").unwrap();
    let mut supports = vec![("body", body, None)];
    supports.extend(boundaries.iter().map(|&name| {
        (
            name,
            geometry.entity_set(name).unwrap(),
            Some(("body", body)),
        )
    }));
    compile_model(
        "poisson-box.eqi",
        source,
        geometry,
        model,
        component,
        &supports,
        &[(
            "source_scale",
            DynQuantity::new(
                1.0,
                DimExponents {
                    length: -2,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        )],
    )
}

fn cartesian_box_resources(
    geometry: &CanonicalGeometryV1,
    cells: &[usize],
) -> AuthenticatedCommonMesh {
    let policy = CartesianMeshCellsV2::new(cells.to_vec()).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_cartesian_box_v1(geometry, policy.cells())
            .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v2_resources(
        &policy,
        geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    AuthenticatedCommonMesh::structured_cartesian(
        geometry.clone(),
        mesh,
        correspondence,
        production,
    )
    .unwrap()
}

fn resolve_scalar_box(
    model: &ModelEnvelope,
    resources: AuthenticatedCommonMesh,
    spatial: CommonSpatialPolicy,
) -> CommonScalarPlan {
    let linear =
        CommonLinearRequest::new(1.0e-10, 1.0e-12, NonZeroUsize::new(10_000).unwrap()).unwrap();
    resolve_common_plan(
        model,
        resources,
        spatial,
        CommonSolvePolicy::Linear(linear),
        None,
        None,
        &ResolveOnlyBackend,
        None,
    )
    .unwrap()
    .project(
        |_| panic!("spatial Model resolved as no-Mesh ODE"),
        |plan| plan,
        |_| panic!("scalar Model resolved as elasticity"),
        |_| panic!("scalar Model resolved as Stokes"),
        |_| panic!("scalar Model resolved as transient flow"),
        |_| panic!("scalar Model resolved as FSI"),
    )
}

#[test]
fn common_scalar_plan_executes_exact_one_and_three_dimensional_meshes() {
    let interval = cartesian_interval();
    let interval_model = scalar_box_model(
        &interval,
        POISSON_INTERVAL,
        "PoissonIntervalModel",
        "PoissonInterval",
        &["left", "right"],
    );
    exercise_scalar_box(&interval_model, &interval, &[3]);

    let box_3d = cartesian_box_3d();
    let box_model = scalar_box_model(
        &box_3d,
        POISSON_BOX,
        "PoissonBoxModel",
        "PoissonBox",
        &[
            "x_lower", "x_upper", "y_lower", "y_upper", "z_lower", "z_upper",
        ],
    );
    exercise_scalar_box(&box_model, &box_3d, &[2, 2, 2]);
}

fn exercise_scalar_box(model: &ModelEnvelope, geometry: &CanonicalGeometryV1, cells: &[usize]) {
    for spatial in [
        CommonSpatialPolicy::Q1,
        CommonSpatialPolicy::CellCenteredTpfa,
    ] {
        let plan = resolve_scalar_box(model, cartesian_box_resources(geometry, cells), spatial);
        assert_eq!(plan.cells(), cells);
        match (
            cells.len(),
            plan.portable_realization().domains()[0]
                .discretization()
                .mesh(),
        ) {
            (1, MeshPolicy::SuppliedCartesian1d { .. })
            | (2, MeshPolicy::SuppliedCartesian { .. })
            | (3, MeshPolicy::SuppliedCartesian3d { .. }) => {}
            _ => panic!("common scalar Plan lost its exact Cartesian dimension"),
        }
        let replayed = replay_plan(
            ResolvedCommonPlan::Scalar(Box::new(plan.clone())),
            &ResolveOnlyBackend,
        )
        .project(
            |_| panic!("scalar Plan replayed as ODE"),
            |plan| plan,
            |_| panic!("scalar Plan replayed as elasticity"),
            |_| panic!("scalar Plan replayed as Stokes"),
            |_| panic!("scalar Plan replayed as transient flow"),
            |_| panic!("scalar Plan replayed as FSI"),
        );
        assert_eq!(replayed.cells(), cells);
        let result = replayed.run_result().unwrap();
        let expected_shape = match spatial {
            CommonSpatialPolicy::Q1 => cells.iter().map(|count| count + 1).collect::<Vec<_>>(),
            CommonSpatialPolicy::CellCenteredTpfa => cells.to_vec(),
            _ => unreachable!("exercise admits scalar policies only"),
        };
        assert_eq!(result.field_block(0, 0).unwrap().2, expected_shape);
    }
}

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
    let resolve_scalar = |method, solve| {
        let resolved = resolve_common_plan(
            &model,
            resources(&geometry),
            method,
            CommonSolvePolicy::Linear(solve),
            None,
            None,
            &ResolveOnlyBackend,
            None,
        )
        .unwrap();
        replay_plan(resolved, &ResolveOnlyBackend).project(
            |_| panic!("spatial Model resolved as no-Mesh ODE"),
            |plan| plan,
            |_| panic!("scalar Model resolved as elasticity"),
            |_| panic!("scalar Model resolved as another capability"),
            |_| panic!("scalar Model resolved as transient capability"),
            |_| panic!("scalar Model resolved as FSI"),
        )
    };
    let q1 = resolve_scalar(
        CommonMethodRequest::Uniform(CommonSpatialPolicy::Q1),
        linear,
    );
    let repeat = resolve_scalar(
        CommonMethodRequest::Uniform(CommonSpatialPolicy::Q1),
        linear,
    );
    let exact = resolve_scalar(
        CommonMethodRequest::Exact {
            spatial: CommonSpatialPolicy::Q1,
            formulation: FormulationKind::PrimalGalerkin,
        },
        linear,
    );
    let tpfa = resolve_scalar(
        CommonMethodRequest::Uniform(CommonSpatialPolicy::CellCenteredTpfa),
        linear,
    );
    let alternate_tolerance = resolve_scalar(
        CommonMethodRequest::Uniform(CommonSpatialPolicy::Q1),
        CommonLinearRequest::new(1.0e-9, 1.0e-12, NonZeroUsize::new(10_000).unwrap()).unwrap(),
    );

    assert_eq!(q1.identity(), repeat.identity());
    assert_ne!(q1.identity(), tpfa.identity());
    assert_ne!(q1.identity(), alternate_tolerance.identity());
    assert_ne!(q1.identity(), exact.identity());
    assert_eq!(q1.realization_digest(), exact.realization_digest());
    let automatic_form = q1.formulation().unwrap();
    let exact_form = exact.formulation().unwrap();
    assert_eq!(automatic_form.effective(), FormulationKind::PrimalGalerkin);
    assert_eq!(
        automatic_form.requested(),
        FormulationSelectionMode::Automatic
    );
    assert_eq!(exact_form.requested(), FormulationSelectionMode::Exact);
    assert_eq!(
        automatic_form.boundary_treatment(),
        "complete-homogeneous-essential"
    );
    assert_eq!(automatic_form.rule_ids().len(), 4);
    assert_eq!(
        automatic_form.selection_reason_codes(),
        ["eqiora.formulation.auto.primal-galerkin-for-q1/v1"]
    );
    assert!(tpfa.formulation().is_none());
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
            None,
        )
        .is_err()
    );
    assert!(
        resolve_common_plan(
            &model,
            resources(&geometry),
            CommonMethodRequest::Exact {
                spatial: CommonSpatialPolicy::Q1,
                formulation: FormulationKind::MixedGalerkin,
            },
            CommonSolvePolicy::Linear(linear),
            None,
            None,
            &ResolveOnlyBackend,
            None,
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
        let resolved = resolve_common_plan(
            model,
            resources(&geometry),
            CommonSpatialPolicy::Q1,
            CommonSolvePolicy::Linear(solve),
            None,
            None,
            &ResolveOnlyBackend,
            None,
        )
        .unwrap();
        replay_plan(resolved, &ResolveOnlyBackend).project(
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
            None,
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
        "\"identity\":\"eqiora.structured-cartesian\",\"version\":\"2\"",
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
            "-div(grad(potential))\n      - source_scale * math.sin(wave_number * coordinate(0))\n        * math.sin(wave_number * coordinate(1)) = 0;",
            "potential - 1 = 0;",
        );
    let reaction = scalar_model_from_source(&geometry, &reaction_source);
    let reaction_program = replay_program(&reaction, &geometry).unwrap();
    let reaction_owner = resources(&geometry);
    let reaction_scalar = lower_scalar_candidate(&reaction_program, &reaction_owner.resources);
    let reaction_transient =
        lower_transient_incompressible_navier_stokes_cartesian_2d(&reaction_program);
    assert!(
        recognize_capability(
            &reaction_program,
            &reaction_scalar,
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
            &Err(invalid("not scalar")),
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
