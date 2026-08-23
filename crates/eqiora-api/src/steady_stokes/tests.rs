use eqiora_artifact::ModelDecoderLimits;
use eqiora_geometry::{EDGE_DIMENSION, FACE_DIMENSION, NamedEntitySet};
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};
use eqiora_solver::{
    BackendId, LinearProblem, LinearSolution, PreconditionerPolicy, ReplicatedLinearExecution,
};

use super::*;

const PACKAGED_MODEL: &[u8] =
    include_bytes!("../../../../examples/steady-flow-past-cylinder.model.json");

#[derive(Debug)]
struct ResolveOnlyBackend;

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

fn accepted_source() -> CanonicalGeometryV1 {
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

#[test]
fn alpha2_reference_mesh_remains_admitted_and_foreign_identity_rejects() {
    let model = ModelEnvelope::from_json(PACKAGED_MODEL, ModelDecoderLimits::default())
        .expect("accepted packaged Model");
    assert_eq!(model.digest().unwrap().to_string(), ACCEPTED_MODEL_DIGEST);

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
