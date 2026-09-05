use std::collections::{BTreeMap, BTreeSet};

use eqiora::api::ModelDocument;
use eqiora::artifact::{
    GeometryIdentityEnvelopeV1, GeometryMeshCorrespondenceEnvelopeV1,
    GeometryRevisionAssociationEnvelopeV1, ModelDecoderLimits, ModelEnvelope,
    ModelTransactionEnvelope, SimplicialMeshEnvelopeV1,
};
use eqiora::geometry::BodyAssociationCandidate;
use eqiora::graph::{EdgeKind, Op, Precondition};
use eqiora::kernel::{AxisBounds, BoundarySide, DomainKind, KernelNode};
use eqiora::meshing::{MeshQualityGate, SimplicialMesh};
use eqiora::{Id, kinds};
use serde::Deserialize;

const BASE: &str =
    include_str!("../../../verify/geometry/parameter-cartesian-regeneration/models/base.eqi");
const TARGET: &str =
    include_str!("../../../verify/geometry/parameter-cartesian-regeneration/models/target.eqi");
const PARTIAL_X_ONLY: &str = include_str!(
    "../../../verify/geometry/parameter-cartesian-regeneration/models/partial-x-only.eqi"
);
const ORACLE: &[u8] = include_bytes!(
    "../../../verify/geometry/parameter-cartesian-regeneration/expected/oracle.json"
);
const TOLERANCE_M: f64 = 1.0e-12;
const INVALID_TARGETS: &str = r"
model invalid_targets {
  parameter scalar: 1 = 1;
  parameter ordinary_length: m = 1;
  domain body = box(0, 1, 0, 1, 0, 1);
  relation retain continuous on body {
    coordinate(0) - coordinate(0) = 0;
  }
}
";
const TWO_DIMENSIONAL: &str = r"
model planar_parameter_box {
  parameter extent: m = 2;
  domain body = box(-1, extent, extent, 6);
  relation retain continuous on body {
    coordinate(0) - coordinate(0) = 0;
  }
}
";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Oracle {
    schema: String,
    before: OracleState,
    after: OracleState,
    partial_x_only: OracleState,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OracleState {
    #[serde(default)]
    parameter_m: Option<f64>,
    bounds_m: [[f64; 2]; 3],
    #[serde(rename = "widthSumXYM")]
    width_sum_xym: f64,
    #[serde(rename = "widthDifferenceXYM")]
    width_difference_xym: f64,
    volume_m3: f64,
}

#[test]
fn one_value_transaction_regenerates_every_endpoint_and_retains_selections() {
    let oracle = oracle();
    let base = ModelDocument::compile("base.eqi", BASE).unwrap();
    let independent_target = ModelDocument::compile("target.eqi", TARGET).unwrap();
    let body = domain(&base, "body");
    let parameter = base.aliases()["extent"];
    let base_digest = base.digest().unwrap();
    let base_nodes = node_ids(&base);
    let base_edges = base.program().edges().to_vec();
    let base_roles = boundary_roles(&base, body);
    let base_definition = base.program().node(body.erase()).cloned().unwrap();
    assert_oracle(&base, &oracle.before);

    let plan = base
        .preview_parameter_geometry_regeneration(parameter, 3.5)
        .unwrap();
    assert_eq!(plan.base_digest(), base_digest);
    assert_eq!(plan.base_revision(), base.program().revision());
    assert_eq!(plan.parameter().erase(), parameter);
    assert_eq!(plan.before().value(), oracle.before.parameter_m.unwrap());
    assert_eq!(plan.after().value(), oracle.after.parameter_m.unwrap());
    assert_eq!(plan.domain(), body);
    assert_eq!(
        plan.edits(),
        &[
            (
                0,
                axis_bounds(oracle.before.bounds_m[0]),
                axis_bounds(oracle.after.bounds_m[0]),
            ),
            (
                1,
                axis_bounds(oracle.before.bounds_m[1]),
                axis_bounds(oracle.after.bounds_m[1]),
            ),
        ]
    );
    assert_ne!(plan.expected_child_digest(), base_digest);

    let transaction = ModelTransactionEnvelope::from_json(
        &plan.transaction_json().unwrap(),
        ModelDecoderLimits::default(),
    )
    .unwrap()
    .to_transaction()
    .unwrap();
    assert_eq!(
        transaction.preconditions(),
        &[
            Precondition::RevisionIs(base.program().revision()),
            Precondition::ValueEquals {
                target: parameter,
                expected: plan.before(),
            },
        ]
    );
    assert_eq!(
        transaction.ops(),
        &[Op::SetValue {
            target: parameter,
            value: plan.after(),
        }]
    );

    let committed = base
        .commit_parameter_geometry_regeneration(plan.clone())
        .unwrap();
    let child = committed.document();
    assert_eq!(committed.plan(), &plan);
    assert_eq!(
        committed.result_revision().0,
        base.program().revision().0 + 1
    );
    assert_eq!(committed.result_digest(), child.digest().unwrap());
    assert_eq!(child.digest().unwrap(), plan.expected_child_digest());
    assert_ne!(child.digest().unwrap(), base_digest);
    assert_oracle(&independent_target, &oracle.after);
    assert_eq!(
        cartesian_bounds(child),
        cartesian_bounds(&independent_target)
    );
    assert_eq!(box_volume(child), box_volume(&independent_target));
    assert_eq!(node_ids(child), base_nodes);
    assert_eq!(child.program().edges(), base_edges);
    assert_eq!(
        child.program().node(body.erase()).cloned().unwrap(),
        base_definition
    );
    assert_eq!(boundary_roles(child, body), base_roles);
    assert_eq!(base_roles.len(), 6);
    assert_oracle(child, &oracle.after);

    let base_model = model_artifact(&base);
    let child_model = model_artifact(child);
    let base_geometry = GeometryIdentityEnvelopeV1::new(&base_model, [body], TOLERANCE_M).unwrap();
    let child_geometry =
        GeometryIdentityEnvelopeV1::new(&child_model, [body], TOLERANCE_M).unwrap();
    assert_ne!(
        base_geometry.digest().unwrap(),
        child_geometry.digest().unwrap()
    );
    let source_mesh = mesh_artifact(oracle.before.bounds_m);
    let target_mesh = mesh_artifact(oracle.after.bounds_m);
    let source_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&base_geometry, &base_model, &source_mesh)
            .unwrap();
    let target_correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::new(&child_geometry, &child_model, &target_mesh)
            .unwrap();
    let association = GeometryRevisionAssociationEnvelopeV1::new(
        &base_model,
        &base_geometry,
        &source_correspondence,
        &source_mesh,
        &child_model,
        &child_geometry,
        &target_correspondence,
        &target_mesh,
        vec![BodyAssociationCandidate::new(body, body)],
    )
    .unwrap();
    association
        .validate_against(
            &base_model,
            &base_geometry,
            &source_correspondence,
            &source_mesh,
            &child_model,
            &child_geometry,
            &target_correspondence,
            &target_mesh,
        )
        .unwrap();
    assert_eq!(association.retained_body_target(body), Some(body));
    for boundary in base_roles.values() {
        assert_eq!(
            association.retained_boundary_target(*boundary),
            Some(*boundary)
        );
    }
    let selected_y_lower = base_roles[&(1, BoundarySide::Lower)];
    assert_eq!(
        association.retained_boundary_target(selected_y_lower),
        Some(selected_y_lower)
    );

    assert_eq!(base.digest().unwrap(), base_digest);
    assert_oracle(&base, &oracle.before);
}

#[test]
fn plan_is_canonical_and_invalid_requests_fail_before_mutation() {
    let base = ModelDocument::compile("base.eqi", BASE).unwrap();
    let parameter = base.aliases()["extent"];
    let plan = base
        .preview_parameter_geometry_regeneration(parameter, 3.5)
        .unwrap();
    let repeated = base
        .preview_parameter_geometry_regeneration(parameter, 3.5)
        .unwrap();
    let distinct = base
        .preview_parameter_geometry_regeneration(parameter, 3.0)
        .unwrap();
    let traversal_permuted = replay_with_reversed_declaration_arrays(&base);
    let traversal_plan = traversal_permuted
        .preview_parameter_geometry_regeneration(parameter, 3.5)
        .unwrap();
    assert_eq!(base.digest().unwrap(), traversal_permuted.digest().unwrap());
    assert_eq!(plan, repeated);
    assert_eq!(plan, traversal_plan);
    assert_eq!(plan.key(), repeated.key());
    assert_eq!(plan.key(), traversal_plan.key());
    assert_ne!(plan.key(), distinct.key());
    assert_eq!(plan.transaction_digest(), repeated.transaction_digest());
    assert_eq!(
        plan.transaction_digest(),
        traversal_plan.transaction_digest()
    );
    assert_ne!(plan.transaction_digest(), distinct.transaction_digest());
    assert_ne!(
        plan.expected_child_digest(),
        distinct.expected_child_digest()
    );
    assert_eq!(
        plan.transaction_json().unwrap(),
        traversal_plan.transaction_json().unwrap()
    );
    let child = base
        .commit_parameter_geometry_regeneration(plan.clone())
        .unwrap()
        .into_document();
    let traversal_child = traversal_permuted
        .commit_parameter_geometry_regeneration(traversal_plan)
        .unwrap()
        .into_document();
    assert_eq!(
        child.canonical_json().unwrap(),
        traversal_child.canonical_json().unwrap()
    );
    assert_eq!(child.digest().unwrap(), traversal_child.digest().unwrap());

    let positive_zero = base
        .preview_parameter_geometry_regeneration(parameter, 0.0)
        .unwrap();
    let negative_zero = base
        .preview_parameter_geometry_regeneration(parameter, -0.0)
        .unwrap();
    assert_eq!(positive_zero, negative_zero);
    assert_eq!(
        positive_zero.transaction_json().unwrap(),
        negative_zero.transaction_json().unwrap()
    );

    let base_bytes = base.canonical_json().unwrap();
    let base_digest = base.digest().unwrap();
    assert!(
        base.preview_parameter_geometry_regeneration(parameter, f64::NAN)
            .is_err()
    );
    assert!(
        base.preview_parameter_geometry_regeneration(parameter, 2.0)
            .is_err()
    );
    assert!(
        base.preview_parameter_geometry_regeneration(parameter, 6.0)
            .is_err()
    );
    assert!(
        base.preview_parameter_geometry_regeneration(Id::<kinds::Parameter>::new().erase(), 3.5,)
            .is_err()
    );
    assert!(
        base.preview_parameter_geometry_regeneration(base.aliases()["retain"], 3.5)
            .is_err()
    );
    assert!(base.preview_value_edit(parameter, 3.5).is_err());

    let invalid = ModelDocument::compile("invalid-targets.eqi", INVALID_TARGETS).unwrap();
    assert!(
        invalid
            .preview_parameter_geometry_regeneration(invalid.aliases()["scalar"], 2.0)
            .is_err()
    );
    assert!(
        invalid
            .preview_parameter_geometry_regeneration(invalid.aliases()["ordinary_length"], 2.0)
            .is_err()
    );
    let legacy = eqiora::api::ModelDocument::compile("legacy-fixed.eqi", INVALID_TARGETS).unwrap();
    assert!(
        legacy
            .preview_parameter_geometry_regeneration(legacy.aliases()["ordinary_length"], 2.0)
            .is_err()
    );
    let planar = ModelDocument::compile("planar.eqi", TWO_DIMENSIONAL).unwrap();
    let planar_parameter = planar.aliases()["extent"];
    assert_eq!(
        planar
            .preview_value_edit(planar_parameter, 3.5)
            .unwrap_err()
            .message(),
        "value edit cannot target a Cartesian coordinate Parameter; the geometry regeneration owner currently accepts one 3D Domain"
    );
    assert!(
        planar
            .preview_parameter_geometry_regeneration(planar_parameter, 3.5)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic
                .message()
                .contains("requires three dimensions, found 2"))
    );
    assert_eq!(base.canonical_json().unwrap(), base_bytes);
    assert_eq!(base.digest().unwrap(), base_digest);
}

#[test]
fn stale_foreign_and_partial_successors_fail_the_exact_contract() {
    let oracle = oracle();
    let base = ModelDocument::compile("base.eqi", BASE).unwrap();
    let parameter = base.aliases()["extent"];
    let plan = base
        .preview_parameter_geometry_regeneration(parameter, 3.5)
        .unwrap();
    let child = base
        .commit_parameter_geometry_regeneration(plan.clone())
        .unwrap()
        .into_document();
    assert!(
        child
            .commit_parameter_geometry_regeneration(plan.clone())
            .is_err()
    );

    let foreign_source = BASE.replacen("model param_box_3d", "model foreign_box_3d", 1);
    let foreign = ModelDocument::compile("foreign.eqi", &foreign_source).unwrap();
    assert_eq!(foreign.program().revision(), base.program().revision());
    assert_ne!(foreign.digest().unwrap(), base.digest().unwrap());
    assert!(
        foreign
            .commit_parameter_geometry_regeneration(plan)
            .is_err()
    );

    let independent_target = ModelDocument::compile("target.eqi", TARGET).unwrap();
    let partial = ModelDocument::compile("partial.eqi", PARTIAL_X_ONLY).unwrap();
    assert_oracle(&partial, &oracle.partial_x_only);
    assert!(
        !partial
            .structurally_equivalent(&independent_target)
            .unwrap()
    );
    assert_ne!(
        partial.digest().unwrap(),
        independent_target.digest().unwrap()
    );
    assert_ne!(
        cartesian_bounds(&partial),
        cartesian_bounds(&independent_target)
    );
    assert_ne!(box_volume(&partial), oracle.after.volume_m3);
}

fn oracle() -> Oracle {
    let oracle: Oracle = serde_json::from_slice(ORACLE).unwrap();
    assert_eq!(
        oracle.schema,
        "eqiora.verify.parameter-cartesian-regeneration-oracle/v1"
    );
    oracle
}

fn replay_with_reversed_declaration_arrays(document: &ModelDocument) -> ModelDocument {
    let mut wire: serde_json::Value =
        serde_json::from_slice(&document.canonical_json().unwrap()).unwrap();
    for field in ["nodes", "values", "edges", "boundary"] {
        wire[field].as_array_mut().unwrap().reverse();
    }
    eqiora::api::ModelDocument::replay(&serde_json::to_vec(&wire).unwrap()).unwrap()
}

fn assert_oracle(document: &ModelDocument, expected: &OracleState) {
    let bounds = cartesian_bounds(document);
    assert_eq!(bounds, expected.bounds_m);
    let widths = [bounds[0][1] - bounds[0][0], bounds[1][1] - bounds[1][0]];
    assert_eq!(widths[0] + widths[1], expected.width_sum_xym);
    assert_eq!(widths[0] - widths[1], expected.width_difference_xym);
    assert_eq!(box_volume(document), expected.volume_m3);
}

fn model_artifact(document: &ModelDocument) -> ModelEnvelope {
    ModelEnvelope::from_json(
        &document.canonical_json().unwrap(),
        ModelDecoderLimits::default(),
    )
    .unwrap()
}

fn domain(document: &ModelDocument, name: &str) -> Id<kinds::Domain> {
    document.aliases()[name].downcast().unwrap()
}

fn node_ids(document: &ModelDocument) -> BTreeSet<eqiora::RawId> {
    document.program().nodes().map(KernelNode::id).collect()
}

fn boundary_roles(
    document: &ModelDocument,
    body: Id<kinds::Domain>,
) -> BTreeMap<(usize, BoundarySide), Id<kinds::Domain>> {
    document
        .program()
        .edges()
        .iter()
        .filter(|edge| edge.kind() == EdgeKind::BoundaryOf && edge.to() == body.erase())
        .map(|edge| {
            let KernelNode::Domain(boundary) = document
                .program()
                .node(edge.from())
                .expect("boundary Domain")
            else {
                panic!("BoundaryOf source must remain a Domain");
            };
            let DomainKind::CartesianBoundary { axis, side } = boundary.kind() else {
                panic!("boundary role must remain Cartesian");
            };
            ((*axis, *side), boundary.id())
        })
        .collect()
}

fn cartesian_bounds(document: &ModelDocument) -> [[f64; 2]; 3] {
    let KernelNode::Domain(body) = document
        .program()
        .nodes()
        .find(|node| {
            matches!(
                node,
                KernelNode::Domain(domain)
                    if matches!(domain.kind(), DomainKind::CartesianBox { .. })
            )
        })
        .expect("one Cartesian body")
    else {
        unreachable!();
    };
    document
        .program()
        .resolved_cartesian_bounds(body.id())
        .unwrap()
        .iter()
        .map(|axis| [axis.lower().value(), axis.upper().value()])
        .collect::<Vec<_>>()
        .try_into()
        .unwrap()
}

fn box_volume(document: &ModelDocument) -> f64 {
    cartesian_bounds(document)
        .into_iter()
        .map(|axis| axis[1] - axis[0])
        .product()
}

fn axis_bounds(bounds: [f64; 2]) -> AxisBounds {
    AxisBounds::new(
        eqiora::DynQuantity::new(bounds[0], length_dimension()),
        eqiora::DynQuantity::new(bounds[1], length_dimension()),
    )
    .unwrap()
}

fn length_dimension() -> eqiora::DimExponents {
    eqiora::DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension")
}

fn mesh_artifact(bounds: [[f64; 2]; 3]) -> SimplicialMeshEnvelopeV1 {
    let [x, y, z] = bounds;
    let mesh = SimplicialMesh::new(
        3,
        vec![
            vec![x[0], y[0], z[0]],
            vec![x[1], y[0], z[0]],
            vec![x[0], y[1], z[0]],
            vec![x[1], y[1], z[0]],
            vec![x[0], y[0], z[1]],
            vec![x[1], y[0], z[1]],
            vec![x[0], y[1], z[1]],
            vec![x[1], y[1], z[1]],
        ],
        vec![
            vec![0, 1, 3, 7],
            vec![0, 3, 2, 7],
            vec![0, 2, 6, 7],
            vec![0, 6, 4, 7],
            vec![0, 4, 5, 7],
            vec![0, 5, 1, 7],
        ],
        MeshQualityGate::new(0.05).unwrap(),
    )
    .unwrap();
    SimplicialMeshEnvelopeV1::from_mesh(&mesh).unwrap()
}
