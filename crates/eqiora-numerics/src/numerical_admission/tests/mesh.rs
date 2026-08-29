use super::*;

#[test]
fn authenticated_common_mesh_round_trips_canonically_and_rejects_aliases() {
    let rectangle = rectangle();
    let partition = fsi_geometry();
    for owner in [
        resources(&rectangle),
        affine_resources(&rectangle),
        fsi_resources(&partition),
    ] {
        let bytes = owner.to_bytes().unwrap();
        let replayed = AuthenticatedCommonMesh::from_bytes(&bytes).unwrap();
        assert_eq!(replayed, owner);
        assert_eq!(replayed.to_bytes().unwrap(), bytes);
        assert_eq!(replayed.digest().unwrap(), owner.digest().unwrap());

        let mut noncanonical = bytes.clone();
        noncanonical.push(b'\n');
        assert!(AuthenticatedCommonMesh::from_bytes(&noncanonical).is_err());
    }

    let bytes = resources(&rectangle).to_bytes().unwrap();
    let mut wire: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wire["kind"] = serde_json::Value::String("gmsh4152".to_owned());
    assert!(
        AuthenticatedCommonMesh::from_bytes(&serde_json::to_vec(&wire).unwrap()).is_err(),
        "a resource-family cross-wire must fail before publication"
    );
}

#[test]
pub(super) fn affine_triangle_common_owner_reauthenticates_exact_resource_occurrence() {
    let geometry = rectangle();
    let policy = AffineTriangleMeshCellsV1::new([2, 3]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_affine_triangles(
            &geometry,
            policy.cells(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
        policy,
        &geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    let exact_owner = AuthenticatedCommonMesh::affine_triangle_rectangle(
        geometry.clone(),
        mesh.clone(),
        correspondence.clone(),
        production.clone(),
    )
    .unwrap();
    let stokes_scales = IncompressibleFlowScaleProfile2d::new(
        DynQuantity::new(
            1.0,
            DimExponents {
                length: 1,
                ..DimExponents::DIMENSIONLESS
            },
        ),
        DynQuantity::new(
            1.0,
            DimExponents {
                length: 1,
                time: -1,
                ..DimExponents::DIMENSIONLESS
            },
        ),
        DynQuantity::new(
            1.0,
            DimExponents {
                mass: 1,
                length: -1,
                time: -2,
                ..DimExponents::DIMENSIONLESS
            },
        ),
    )
    .unwrap();
    assert!(
        validate_resources(
            NativeCapability::SteadyIncompressibleStokes,
            NativeSpatialPolicy::StokesMiniP1(stokes_scales),
            &exact_owner.resources,
        )
        .is_err(),
        "#574 publishes physics-independent Mesh resources but does not admit Stokes"
    );

    let alternate_policy = AffineTriangleMeshCellsV1::new([3, 2]).unwrap();
    let (alternate_mesh, alternate_correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_affine_triangles(
            &geometry,
            alternate_policy.cells(),
        )
        .unwrap();
    let alternate_production =
        MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
            alternate_policy,
            &geometry,
            &alternate_mesh,
            &alternate_correspondence,
        )
        .unwrap();
    assert!(
        AuthenticatedCommonMesh::affine_triangle_rectangle(
            geometry.clone(),
            alternate_mesh,
            correspondence.clone(),
            production.clone(),
        )
        .is_err()
    );
    assert!(
        AuthenticatedCommonMesh::affine_triangle_rectangle(
            geometry.clone(),
            mesh.clone(),
            alternate_correspondence,
            production.clone(),
        )
        .is_err()
    );
    assert!(
        AuthenticatedCommonMesh::affine_triangle_rectangle(
            geometry.clone(),
            mesh.clone(),
            correspondence.clone(),
            alternate_production,
        )
        .is_err()
    );

    let graph = GeometryGraph::new();
    let foreign_rectangle = graph.rectangle([0.0, 2.0], [0.0, 1.0]).unwrap();
    let foreign_edges = foreign_rectangle.boundaries();
    let foreign_geometry = graph
        .build(
            &foreign_rectangle,
            &BTreeMap::from([
                ("region".to_owned(), vec![foreign_rectangle.region().into()]),
                ("left".to_owned(), vec![foreign_edges[0].into()]),
                ("right".to_owned(), vec![foreign_edges[1].into()]),
                ("bottom".to_owned(), vec![foreign_edges[2].into()]),
                ("top".to_owned(), vec![foreign_edges[3].into()]),
            ]),
        )
        .unwrap();
    assert!(
        AuthenticatedCommonMesh::affine_triangle_rectangle(
            foreign_geometry,
            mesh,
            correspondence,
            production,
        )
        .is_err()
    );
}

pub(super) fn model(geometry: &CanonicalGeometryV1) -> ModelEnvelope {
    scalar_model_from_source(geometry, COMPONENT)
}

pub(super) fn scalar_model_from_source(
    geometry: &CanonicalGeometryV1,
    source: &str,
) -> ModelEnvelope {
    let region = geometry.entity_set("region").unwrap();
    let supports = [
        ("region", region, None),
        (
            "left",
            geometry.entity_set("left").unwrap(),
            Some(("region", region)),
        ),
        (
            "right",
            geometry.entity_set("right").unwrap(),
            Some(("region", region)),
        ),
        (
            "bottom",
            geometry.entity_set("bottom").unwrap(),
            Some(("region", region)),
        ),
        (
            "top",
            geometry.entity_set("top").unwrap(),
            Some(("region", region)),
        ),
    ];
    let parameters = [
        (
            "wave_number",
            DynQuantity::new(
                std::f64::consts::PI,
                DimExponents {
                    length: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "source_scale",
            DynQuantity::new(
                2.0 * std::f64::consts::PI.powi(2),
                DimExponents {
                    length: -2,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
    ];
    compile_model(
        "poisson-rectangle.eqi",
        source,
        geometry,
        "PoissonRectangleModel",
        "PoissonRectangle",
        &supports,
        &parameters,
    )
}

pub(super) fn elasticity_model(geometry: &CanonicalGeometryV1, mu: f64) -> ModelEnvelope {
    let region = geometry.entity_set("region").unwrap();
    let supports = [
        ("region", region, None),
        (
            "left",
            geometry.entity_set("left").unwrap(),
            Some(("region", region)),
        ),
        (
            "right",
            geometry.entity_set("right").unwrap(),
            Some(("region", region)),
        ),
        (
            "bottom",
            geometry.entity_set("bottom").unwrap(),
            Some(("region", region)),
        ),
        (
            "top",
            geometry.entity_set("top").unwrap(),
            Some(("region", region)),
        ),
    ];
    let pressure = DimExponents {
        mass: 1,
        length: -1,
        time: -2,
        ..DimExponents::DIMENSIONLESS
    };
    let parameters = [
        ("mu", DynQuantity::new(mu, pressure)),
        ("lambda", DynQuantity::new(0.0, pressure)),
        (
            "length_scale",
            DynQuantity::new(
                1.0,
                DimExponents {
                    length: 1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
    ];
    compile_model(
        "mixed-boundary-elasticity.eqi",
        ELASTICITY_COMPONENT,
        geometry,
        "MixedBoundaryElasticityModel",
        "MixedBoundaryElasticity",
        &supports,
        &parameters,
    )
}

pub(super) fn resources(geometry: &CanonicalGeometryV1) -> AuthenticatedCommonMesh {
    let cells = CartesianMeshCellsV1::new([2, 3]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_cartesian(
            geometry,
            cells.cells(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_structured_cartesian_v1_resources(
        cells,
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

pub(super) fn affine_resources(geometry: &CanonicalGeometryV1) -> AuthenticatedCommonMesh {
    let cells = AffineTriangleMeshCellsV1::new([2, 3]).unwrap();
    let (mesh, correspondence) =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_rectangle_v2_affine_triangles(
            geometry,
            cells.cells(),
        )
        .unwrap();
    let production = MeshProductionLineageEnvelopeV1::from_affine_triangle_rectangle_v1_resources(
        cells,
        geometry,
        &mesh,
        &correspondence,
    )
    .unwrap();
    AuthenticatedCommonMesh::affine_triangle_rectangle(
        geometry.clone(),
        mesh,
        correspondence,
        production,
    )
    .unwrap()
}

pub(super) fn transient_model() -> ModelEnvelope {
    let compiled = eqiora_compiler::compile("transient-direct.eqi", TRANSIENT_SOURCE)
        .unwrap()
        .pop()
        .unwrap();
    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    ModelEnvelope::from_program(&program).unwrap()
}

pub(super) fn linear() -> NativeLinearPolicy {
    NativeLinearPolicy::exact(
        SolverPlan::new(
            LinearSolver::ConjugateGradient,
            1.0e-10,
            1.0e-13,
            NonZeroUsize::new(1000).unwrap(),
        )
        .unwrap(),
        &REFERENCE_LINEAR_SOLVER,
    )
    .unwrap()
}

pub(super) fn stokes_geometry() -> CanonicalGeometryV1 {
    let owner = GeometryGraph::new();
    let predecessor = owner
        .rectangle_extrusion((0.0, 2.2), (0.0, 0.41), 0.0, 1.0, 1.0e-10)
        .unwrap();
    let graph = owner
        .circular_through_cut(&predecessor, [0.2, 0.2], 0.05, 1.0e-10)
        .unwrap();
    let end_cap = graph.face_handle("end-cap").unwrap();
    let x_lower = graph.face_handle("profile-x-lower").unwrap();
    let x_upper = graph.face_handle("profile-x-upper").unwrap();
    let y_lower = graph.face_handle("profile-y-lower").unwrap();
    let y_upper = graph.face_handle("profile-y-upper").unwrap();
    let cut_wall = graph.face_handle("cut-wall").unwrap();
    owner
        .build_solid_geometry(
            &graph,
            &BTreeMap::from([
                ("fluid".to_owned(), vec![end_cap]),
                ("inlet".to_owned(), vec![x_lower]),
                ("outlet".to_owned(), vec![x_upper]),
                ("walls".to_owned(), vec![y_lower, y_upper]),
                ("cylinder".to_owned(), vec![cut_wall]),
            ]),
        )
        .unwrap()
}

pub(super) fn stokes_model_from_source(
    geometry: &CanonicalGeometryV1,
    source: &str,
) -> ModelEnvelope {
    let fluid = geometry.entity_set("fluid").unwrap();
    let supports = [
        ("fluid", fluid, None),
        (
            "inlet",
            geometry.entity_set("inlet").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "outlet",
            geometry.entity_set("outlet").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "walls",
            geometry.entity_set("walls").unwrap(),
            Some(("fluid", fluid)),
        ),
        (
            "cylinder",
            geometry.entity_set("cylinder").unwrap(),
            Some(("fluid", fluid)),
        ),
    ];
    let parameters = [
        (
            "dynamic_viscosity",
            DynQuantity::new(
                0.001,
                DimExponents {
                    mass: 1,
                    length: -1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "zero_pressure",
            DynQuantity::new(
                0.0,
                DimExponents {
                    mass: 1,
                    length: -1,
                    time: -2,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "inlet_speed",
            DynQuantity::new(
                0.3,
                DimExponents {
                    length: 1,
                    time: -1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
        (
            "channel_height",
            DynQuantity::new(
                0.41,
                DimExponents {
                    length: 1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
        ),
    ];
    compile_model(
        "steady-flow-past-cylinder.eqi",
        source,
        geometry,
        "SteadyFlowPastCylinderModel",
        "SteadyFlowPastCylinder",
        &supports,
        &parameters,
    )
}
