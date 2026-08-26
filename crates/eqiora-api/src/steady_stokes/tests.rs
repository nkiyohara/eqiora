use eqiora_artifact::ModelDecoderLimits;
use eqiora_geometry::{EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet};
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};
use eqiora_solver::{
    BackendId, LinearProblem, LinearSolution, PreconditionerPolicy, ReplicatedLinearExecution,
};

use super::*;
use crate::ModelDocument;

const PACKAGED_MODEL: &[u8] =
    include_bytes!("../../../../examples/steady-flow-past-cylinder.model.json");
const PACKAGED_MODEL_DIGEST: &str =
    "8bc5155bc1b64ed37f7a2ac010a966e1619091a118e6cf7806dbdf9621977146";
pub(crate) const ACCEPTED_COMPONENT_SOURCE: &str = include_str!("accepted_component.eqi");
const DYNAMIC_VISCOSITY: DimExponents = DimExponents {
    mass: 1,
    length: -1,
    time: -1,
    ..DimExponents::DIMENSIONLESS
};

#[derive(Debug)]
pub(crate) struct ResolveOnlyBackend;

impl LinearSolverBackend for ResolveOnlyBackend {
    fn provider(&self) -> SolverProvider {
        SolverProvider::new(BackendId::new("eqiora.test-resolve-only"), "1", &[])
    }

    fn capabilities(&self) -> SolverCapabilities {
        SolverCapabilities::exact([SolverCapability {
            algorithm: LinearSolver::SparseLu,
            operator_properties: LinearOperatorProperties::SymmetricIndefinite,
            preconditioner: PreconditionerPolicy::Identity,
            reduction: ReductionPolicy::Fast,
            scalar_type: ScalarType::F64,
        }])
        .expect("one valid resolve-only capability")
    }

    fn solve_with_execution(
        &self,
        _problem: &LinearProblem<'_>,
        _plan: SolverPlan,
        _execution: &dyn ReplicatedLinearExecution,
    ) -> Result<LinearSolution, Diagnostic> {
        unreachable!("Plan resolution must not execute the backend")
    }
}

pub(crate) fn accepted_source() -> CanonicalGeometryV1 {
    CanonicalGeometryV1::from_circular_hole(
        [[0.0, 2.2], [0.0, 0.41]],
        [0.2, 0.2],
        0.05,
        vec![
            NamedEntitySet::new("inlet", EDGE_DIMENSION, vec![0]),
            NamedEntitySet::new("outlet", EDGE_DIMENSION, vec![1]),
            NamedEntitySet::new("walls", EDGE_DIMENSION, vec![2, 3]),
            NamedEntitySet::new("cylinder", EDGE_DIMENSION, vec![4]),
            NamedEntitySet::new("fluid", FACE_DIMENSION, vec![0]),
        ],
        1.0e-12,
    )
    .expect("accepted exact-cylinder source")
}

pub(crate) fn accepted_realization() -> AcceptedCircularHoleChordalRealizationV1 {
    let source = accepted_source();
    AcceptedCircularHoleChordalRealizationV1::from_reference(
        &source,
        ACCEPTED_MAX_BOUNDARY_ERROR_M,
        50,
        MeshQualityGate::new(ACCEPTED_MINIMUM_MEAN_RATIO)
            .expect("accepted exact-cylinder quality policy"),
    )
    .expect("accepted exact-cylinder realization")
}

pub(crate) fn authored_model(
    component_source: &str,
    values: [f64; 4],
    reverse_bindings: bool,
) -> ModelEnvelope {
    let geometry = accepted_source();
    let fluid = geometry.entity_set("fluid").unwrap();
    let mut supports = vec![
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
    let mut parameters = vec![
        (
            "dynamic_viscosity",
            DynQuantity::new(values[0], DYNAMIC_VISCOSITY),
        ),
        ("zero_pressure", DynQuantity::new(values[1], PRESSURE)),
        ("inlet_speed", DynQuantity::new(values[2], VELOCITY)),
        ("channel_height", DynQuantity::new(values[3], LENGTH)),
    ];
    if reverse_bindings {
        supports.reverse();
        parameters.reverse();
    }
    let document = ModelDocument::compile_external_component(
        "authored-steady-flow-past-cylinder.eqi",
        component_source,
        &geometry,
        "AuthoredSteadyFlowPastCylinderModel",
        "SteadyFlowPastCylinder",
        &supports,
        &parameters,
    )
    .expect("valid authored exact-cylinder Model");
    ModelEnvelope::from_program(document.program()).expect("authored current Model envelope")
}

#[test]
fn alpha2_reference_mesh_remains_admitted_and_foreign_identity_rejects() {
    let model = ModelEnvelope::from_json(PACKAGED_MODEL, ModelDecoderLimits::default())
        .expect("accepted packaged Model");
    assert_eq!(model.digest().unwrap().to_string(), PACKAGED_MODEL_DIGEST);

    let source = accepted_source();
    assert_eq!(
        encode_digest(&source.digest_bytes()),
        ACCEPTED_SOURCE_DIGEST
    );
    let quality_gate = MeshQualityGate::new(ACCEPTED_MINIMUM_MEAN_RATIO)
        .expect("accepted exact-cylinder quality policy");
    let accepted = AcceptedCircularHoleChordalRealizationV1::from_reference(
        &source,
        ACCEPTED_MAX_BOUNDARY_ERROR_M,
        50,
        quality_gate,
    )
    .expect("accepted alpha2 reference mesh");
    assert_eq!(
        accepted.mesh().digest().unwrap().to_string(),
        ACCEPTED_REFERENCE_MESH_DIGEST
    );
    assert_eq!(accepted.circle_segments(), 50);
    assert_eq!(
        accepted.requested_max_boundary_error_m().to_bits(),
        ACCEPTED_MAX_BOUNDARY_ERROR_M.to_bits()
    );
    assert_eq!(
        accepted.envelope().required_minimum_mean_ratio().to_bits(),
        ACCEPTED_MINIMUM_MEAN_RATIO.to_bits()
    );
    require_accepted_inputs(&model, &accepted)
        .expect("private admission retains the alpha2 reference mesh");
    let plan = ResolvedSteadyStokesPlan2d::resolve(
        &model,
        reference_intent().expect("accepted exact-cylinder intent"),
        &accepted,
        &ResolveOnlyBackend,
    )
    .expect("public Plan resolution retains the alpha2 reference mesh");
    assert_eq!(plan.model(), &model);

    let reference_mesh = accepted.mesh().mesh();
    let mut reversed_cells = reference_mesh.cells().to_vec();
    reversed_cells.reverse();
    let foreign_mesh = SimplicialMesh::new(
        2,
        reference_mesh.vertices().to_vec(),
        reversed_cells,
        reference_mesh.quality_gate(),
    )
    .and_then(|mesh| SimplicialMeshEnvelopeV1::from_mesh(&mesh))
    .expect("cell-renumbered mesh remains locally valid");
    let foreign_correspondence = GeometryMeshCorrespondenceEnvelopeV1::from_region(
        accepted.realized_geometry(),
        &foreign_mesh,
    )
    .expect("cell-renumbered mesh has a matching correspondence");
    let foreign = accepted
        .bind_conforming_mesh(&foreign_mesh, &foreign_correspondence)
        .expect("foreign mesh is valid under its own source binding");
    let foreign_digest = foreign.mesh().digest().unwrap().to_string();
    assert_ne!(foreign_digest, ACCEPTED_REFERENCE_MESH_DIGEST);
    assert_ne!(foreign_digest, ACCEPTED_GMSH_MESH_DIGEST);

    for error in [
        require_accepted_inputs(&model, &foreign)
            .expect_err("private admission must reject an unrelated mesh identity"),
        ResolvedSteadyStokesPlan2d::resolve(
            &model,
            reference_intent().unwrap(),
            &foreign,
            &ResolveOnlyBackend,
        )
        .expect_err("public Plan resolution must reject an unrelated mesh identity"),
    ] {
        assert_eq!(error.code(), codes::INVALID_REALIZATION);
        assert_eq!(
            error.message(),
            "exact-cylinder reference operation requires an accepted exact mesh policy"
        );
    }
}

#[test]
fn replayed_and_fresh_models_share_semantic_admission_and_retain_exact_lineage() {
    let packaged = ModelEnvelope::from_json(PACKAGED_MODEL, ModelDecoderLimits::default())
        .expect("accepted packaged Model");
    let authored = authored_model(ACCEPTED_COMPONENT_SOURCE, [1.0e-3, 0.0, 0.3, 0.41], false);
    let reordered = authored_model(ACCEPTED_COMPONENT_SOURCE, [2.0e-3, 0.0, 0.2, 0.41], true);
    assert_ne!(authored.digest().unwrap(), packaged.digest().unwrap());
    assert_ne!(reordered.digest().unwrap(), packaged.digest().unwrap());
    assert_ne!(authored.digest().unwrap(), reordered.digest().unwrap());

    let accepted = accepted_realization();
    for model in [packaged, authored, reordered] {
        let plan = ResolvedSteadyStokesPlan2d::resolve(
            &model,
            reference_intent().unwrap(),
            &accepted,
            &ResolveOnlyBackend,
        )
        .expect("recognized Model reaches the existing Plan resolution");
        assert_eq!(plan.model(), &model);
        assert_eq!(plan.realization().model().unwrap(), model.model().unwrap());
    }
}

#[test]
fn unsupported_model_meaning_rejects_for_semantic_reasons_before_plan_publication() {
    let accepted = accepted_realization();
    let unsupported_law = ACCEPTED_COMPONENT_SOURCE.replace(
        "div(velocity) = 0;",
        "div(velocity) + zero_pressure / dynamic_viscosity = 0;",
    );
    let unsupported_field = ACCEPTED_COMPONENT_SOURCE.replace(
        "field inlet_profile on fluid as space: m / s = 0;",
        "field inlet_profile on fluid as space: m / s = 0;\n  field extra_velocity on fluid as space: m / s shape spatial_vector;",
    );
    let unsupported_dimension = ACCEPTED_COMPONENT_SOURCE.replace(
        "field inlet_profile on fluid as space: m / s = 0;",
        "field inlet_profile on fluid as space: m / s = 0;\n  field extra_state on fluid as space: 1 = 0;",
    );
    let unsupported_support = ACCEPTED_COMPONENT_SOURCE.replace(
        "relation cylinder_velocity continuous on cylinder { trace(velocity) = 0; }",
        "relation cylinder_velocity continuous on walls { trace(velocity) = 0; }",
    );
    let unsupported_boundary = ACCEPTED_COMPONENT_SOURCE.replace(
        "relation wall_velocity continuous on walls { trace(velocity) = 0; }",
        "",
    );
    let mutants = [
        (
            "governing law",
            authored_model(&unsupported_law, [1.0e-3, 0.0, 0.3, 0.41], false),
        ),
        (
            "field inventory",
            authored_model(&unsupported_field, [1.0e-3, 0.0, 0.3, 0.41], false),
        ),
        (
            "field dimension",
            authored_model(&unsupported_dimension, [1.0e-3, 0.0, 0.3, 0.41], false),
        ),
        (
            "support assignment",
            authored_model(&unsupported_support, [1.0e-3, 0.0, 0.3, 0.41], false),
        ),
        (
            "boundary disposition",
            authored_model(&unsupported_boundary, [1.0e-3, 0.0, 0.3, 0.41], false),
        ),
        (
            "coefficient domain",
            authored_model(ACCEPTED_COMPONENT_SOURCE, [0.0, 0.0, 0.3, 0.41], false),
        ),
    ];

    for (case, model) in mutants {
        let result = ResolvedSteadyStokesPlan2d::resolve(
            &model,
            reference_intent().unwrap(),
            &accepted,
            &ResolveOnlyBackend,
        );
        let Err(error) = result else {
            panic!("{case} unexpectedly reached Plan publication");
        };
        assert_eq!(error.code(), codes::INVALID_SPATIAL_LOWERING);
        assert!(
            error.message().contains("Stokes") || error.message().contains("velocity"),
            "{case} rejected for an unexpected reason: {}",
            error.message()
        );
    }
}

#[test]
fn unsupported_policy_rejects_after_model_admission_without_fallback() {
    let model = authored_model(ACCEPTED_COMPONENT_SOURCE, [1.0e-3, 0.0, 0.3, 0.41], false);
    let intent = SteadyStokesIntent2d::new(
        0.41,
        0.3,
        0.001 * 0.3 / 0.41,
        1.0e-5,
        1.0e-13,
        NonZeroUsize::new(10_000).unwrap(),
    )
    .unwrap();
    let error = ResolvedSteadyStokesPlan2d::resolve(
        &model,
        intent,
        &accepted_realization(),
        &ResolveOnlyBackend,
    )
    .expect_err("unsupported policy tuple must reject before Plan publication");
    assert_eq!(error.code(), codes::NOT_IMPLEMENTED);
    assert_eq!(
        error.message(),
        "steady Stokes admits only the existing scaling and SparseLU/Identity/Fast policy"
    );
}
