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
use eqiora::{DimExponents, DynQuantity, Id, kinds};
use serde_json::Value;

const BASE: &str =
    include_str!("../../../verify/geometry/cartesian-domain-edit-3d/models/base.eqi");
const TARGET: &str =
    include_str!("../../../verify/geometry/cartesian-domain-edit-3d/models/target.eqi");
const TOLERANCE_M: f64 = 1.0e-12;
const TWO_DIMENSIONAL: &str = r"
model Plane {
  domain body = box(-0.5, 0.5, -0.5, 0.5);
  representation scalar_space = continuum;
  field witness on body as scalar_space: 1 = 0;
  relation retain_body continuous on body { witness = 0; }
}
";
const MULTI_BODY: &str = r"
model Pair {
  domain body = box(-0.5, 0.5, -0.5, 0.5, -0.5, 0.5);
  domain peer = box(1.0, 2.0, -0.5, 0.5, -0.5, 0.5);
  representation scalar_space = continuum;
  field witness on body as scalar_space: 1 = 0;
  relation retain_body continuous on body { witness = 0; }
}
";

#[test]
fn exact_cartesian_edit_matches_an_independent_model_and_retains_geometry_associations() {
    let base = ModelDocument::compile("base.eqi", BASE).unwrap();
    let independent_target = ModelDocument::compile("target.eqi", TARGET).unwrap();
    let body = domain(&base, "body");
    let base_digest = base.digest().unwrap();
    let base_nodes = node_ids(&base);
    let base_incidence = incident_edges(&base, body);
    let base_roles = boundary_roles(&base, body);
    let base_geometry_model = model_artifact(&base);
    let base_geometry =
        GeometryIdentityEnvelopeV1::new(&base_geometry_model, [body], TOLERANCE_M).unwrap();

    let requested = [(2, axis_bounds(-0.75, 0.75)), (0, axis_bounds(-0.6, 0.6))];
    let canonical_permutation = [(0, axis_bounds(-0.6, 0.6)), (2, axis_bounds(-0.75, 0.75))];
    let plan = base.preview_cartesian_domain_edit(body, requested).unwrap();
    let permuted = base
        .preview_cartesian_domain_edit(body, canonical_permutation)
        .unwrap();
    let distinct = base
        .preview_cartesian_domain_edit(
            body,
            [(0, axis_bounds(-0.7, 0.7)), (2, axis_bounds(-0.75, 0.75))],
        )
        .unwrap();
    let partial_mutant = base
        .preview_cartesian_domain_edit(body, [(0, axis_bounds(-0.6, 0.6))])
        .unwrap();
    assert_eq!(plan.base_digest(), base_digest);
    assert_eq!(plan.target(), body);
    assert_eq!(
        plan.edits(),
        &[
            (0, axis_bounds(-0.5, 0.5), axis_bounds(-0.6, 0.6)),
            (2, axis_bounds(-0.5, 0.5), axis_bounds(-0.75, 0.75)),
        ]
    );
    assert_ne!(plan.expected_child_digest(), base_digest);
    assert_eq!(plan, permuted);
    assert_eq!(plan.key(), permuted.key());
    assert_eq!(plan.transaction_digest(), permuted.transaction_digest());
    assert_eq!(
        plan.transaction_json().unwrap(),
        permuted.transaction_json().unwrap()
    );
    assert_ne!(plan.key(), distinct.key());
    assert_ne!(plan.transaction_digest(), distinct.transaction_digest());
    assert_ne!(
        plan.expected_child_digest(),
        distinct.expected_child_digest()
    );
    let ordinary_transaction = ModelTransactionEnvelope::from_json(
        &plan.transaction_json().unwrap(),
        ModelDecoderLimits::default(),
    )
    .unwrap()
    .to_transaction()
    .unwrap();
    assert_eq!(
        ordinary_transaction.preconditions(),
        &[Precondition::RevisionIs(base.program().revision())]
    );
    assert_eq!(
        ordinary_transaction
            .ops()
            .iter()
            .filter(|op| matches!(op, Op::RemoveNode { id } if *id == body.erase()))
            .count(),
        1
    );
    assert_eq!(
        ordinary_transaction
            .ops()
            .iter()
            .filter(|op| matches!(
                op,
                Op::DefineKernelNode { node } if node.id() == body.erase()
            ))
            .count(),
        1
    );
    let reconnects = ordinary_transaction
        .ops()
        .iter()
        .filter_map(|op| match op {
            Op::Connect { from, to, edge } => Some((*from, *to, *edge)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut expected_reconnects = base_incidence
        .iter()
        .map(|edge| (edge.from(), edge.to(), edge.kind()))
        .collect::<Vec<_>>();
    expected_reconnects.sort_unstable();
    assert_eq!(reconnects, expected_reconnects);
    assert_eq!(ordinary_transaction.ops().len(), 2 + base_incidence.len());
    let permuted_child = base
        .commit_cartesian_domain_edit(permuted)
        .unwrap()
        .into_document();
    let committed = base.commit_cartesian_domain_edit(plan).unwrap();
    let child = committed.document();

    assert_eq!(
        committed.result_revision().0,
        base.program().revision().0 + 1
    );
    assert!(child.structurally_equivalent(&independent_target).unwrap());
    assert_eq!(
        child.canonical_json().unwrap(),
        permuted_child.canonical_json().unwrap()
    );
    assert_eq!(child.digest().unwrap(), permuted_child.digest().unwrap());
    assert_ne!(child.digest().unwrap(), base_digest);
    assert_eq!(committed.result_digest(), child.digest().unwrap());
    assert_eq!(node_ids(child), base_nodes);
    assert_eq!(incident_edges(child, body), base_incidence);
    assert_eq!(boundary_roles(child, body), base_roles);
    assert_eq!(base_roles.len(), 6);
    assert_eq!(
        cartesian_bounds(child),
        vec![(-0.6, 0.6), (-0.5, 0.5), (-0.75, 0.75)]
    );
    assert!((box_volume(&independent_target) - 1.8).abs() < TOLERANCE_M);
    assert!((box_volume(child) - box_volume(&independent_target)).abs() < TOLERANCE_M);

    let partial_child = base
        .commit_cartesian_domain_edit(partial_mutant)
        .unwrap()
        .into_document();
    assert!(
        !partial_child
            .structurally_equivalent(&independent_target)
            .unwrap()
    );
    assert_ne!(partial_child.digest().unwrap(), committed.result_digest());
    assert_eq!(
        cartesian_bounds(&partial_child),
        vec![(-0.6, 0.6), (-0.5, 0.5), (-0.5, 0.5)]
    );
    assert!((box_volume(&partial_child) - 1.2).abs() < TOLERANCE_M);

    let child_geometry_model = model_artifact(child);
    let child_geometry =
        GeometryIdentityEnvelopeV1::new(&child_geometry_model, [body], TOLERANCE_M).unwrap();
    assert_ne!(
        child_geometry.digest().unwrap(),
        base_geometry.digest().unwrap()
    );

    let source_mesh = mesh_artifact([(-0.5, 0.5), (-0.5, 0.5), (-0.5, 0.5)]);
    let target_mesh = mesh_artifact([(-0.6, 0.6), (-0.5, 0.5), (-0.75, 0.75)]);
    let source_correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(
        &base_geometry,
        &base_geometry_model,
        &source_mesh,
    )
    .unwrap();
    let target_correspondence = GeometryMeshCorrespondenceEnvelopeV1::new(
        &child_geometry,
        &child_geometry_model,
        &target_mesh,
    )
    .unwrap();
    let association = GeometryRevisionAssociationEnvelopeV1::new(
        &base_geometry_model,
        &base_geometry,
        &source_correspondence,
        &source_mesh,
        &child_geometry_model,
        &child_geometry,
        &target_correspondence,
        &target_mesh,
        vec![BodyAssociationCandidate::new(body, body)],
    )
    .unwrap();
    association
        .validate_against(
            &base_geometry_model,
            &base_geometry,
            &source_correspondence,
            &source_mesh,
            &child_geometry_model,
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
    let selected_z_upper = base_roles[&(2, BoundarySide::Upper)];
    assert_eq!(
        association.retained_boundary_target(selected_z_upper),
        Some(selected_z_upper)
    );

    assert_eq!(base.digest().unwrap(), base_digest);
    assert_eq!(node_ids(&base), base_nodes);
}

#[test]
fn one_axis_edit_remains_a_cardinality_one_plan() {
    let base = ModelDocument::compile("base.eqi", BASE).unwrap();
    let body = domain(&base, "body");
    let plan = base
        .preview_cartesian_domain_edit(body, [(0, axis_bounds(-0.6, 0.6))])
        .unwrap();

    assert_eq!(
        plan.edits(),
        &[(0, axis_bounds(-0.5, 0.5), axis_bounds(-0.6, 0.6))]
    );
    let child = base
        .commit_cartesian_domain_edit(plan)
        .unwrap()
        .into_document();
    assert_eq!(
        cartesian_bounds(&child),
        vec![(-0.6, 0.6), (-0.5, 0.5), (-0.5, 0.5)]
    );
    assert_eq!(
        child.program().revision().0,
        base.program().revision().0 + 1
    );
}

#[test]
fn invalid_stale_and_foreign_edits_fail_before_mutation() {
    let base = ModelDocument::compile("base.eqi", BASE).unwrap();
    let body = domain(&base, "body");
    let boundary = domain(&base, "x_lower");
    let base_bytes = base.canonical_json().unwrap();
    let base_digest = base.digest().unwrap();
    let accepted = base
        .preview_cartesian_domain_edit(
            body,
            [(0, axis_bounds(-0.6, 0.6)), (2, axis_bounds(-0.75, 0.75))],
        )
        .unwrap();

    assert!(
        base.preview_cartesian_domain_edit(body, std::iter::empty())
            .is_err()
    );
    assert!(
        base.preview_cartesian_domain_edit(
            body,
            [(0, axis_bounds(-0.6, 0.6)), (0, axis_bounds(-0.7, 0.7)),],
        )
        .is_err()
    );
    assert!(
        base.preview_cartesian_domain_edit(
            body,
            [(0, axis_bounds(-0.6, 0.6)), (3, axis_bounds(-0.7, 0.7)),],
        )
        .is_err()
    );
    assert!(
        base.preview_cartesian_domain_edit(
            body,
            [(0, axis_bounds(-0.6, 0.6)), (2, axis_bounds(-0.5, 0.5)),],
        )
        .is_err()
    );
    assert!(
        base.preview_cartesian_domain_edit(boundary, [(0, axis_bounds(-0.6, 0.6))])
            .is_err()
    );

    let foreign_source = BASE.replacen("model Main", "model ForeignSibling", 1);
    let foreign = ModelDocument::compile("foreign-sibling.eqi", &foreign_source).unwrap();
    assert_eq!(foreign.program().revision(), base.program().revision());
    assert!(foreign.structurally_equivalent(&base).unwrap());
    assert_ne!(foreign.digest().unwrap(), base_digest);
    assert!(
        foreign
            .commit_cartesian_domain_edit(accepted.clone())
            .is_err()
    );

    let child = base
        .commit_cartesian_domain_edit(accepted.clone())
        .unwrap()
        .into_document();
    assert!(child.commit_cartesian_domain_edit(accepted).is_err());

    let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension");
    assert!(
        AxisBounds::new(
            DynQuantity::new(f64::NAN, length),
            DynQuantity::new(1.0, length),
        )
        .is_err()
    );
    assert!(
        AxisBounds::new(DynQuantity::new(2.0, length), DynQuantity::new(1.0, length),).is_err()
    );

    assert_eq!(base.canonical_json().unwrap(), base_bytes);
    assert_eq!(base.digest().unwrap(), base_digest);
}

#[test]
fn unsupported_profile_dimension_and_body_multiplicity_fail_closed() {
    let plane = ModelDocument::compile("plane.eqi", TWO_DIMENSIONAL).unwrap();
    let plane_body = domain(&plane, "body");
    assert!(
        plane
            .preview_cartesian_domain_edit(plane_body, [(0, axis_bounds(-0.6, 0.6))])
            .is_err()
    );

    let pair = ModelDocument::compile("pair.eqi", MULTI_BODY).unwrap();
    let pair_body = domain(&pair, "body");
    assert!(
        pair.preview_cartesian_domain_edit(pair_body, [(0, axis_bounds(-0.6, 0.6))])
            .is_err()
    );
}

#[test]
fn geometry_identity_rejects_a_missing_boundary_of_mutant() {
    let base = ModelDocument::compile("base.eqi", BASE).unwrap();
    let body = domain(&base, "body");
    let boundary = domain(&base, "x_lower");
    let child = base
        .commit_cartesian_domain_edit(
            base.preview_cartesian_domain_edit(
                body,
                [(0, axis_bounds(-0.6, 0.6)), (2, axis_bounds(-0.75, 0.75))],
            )
            .unwrap(),
        )
        .unwrap()
        .into_document();
    let accepted = model_artifact(&child);
    let accepted_geometry =
        GeometryIdentityEnvelopeV1::new(&accepted, [body], TOLERANCE_M).unwrap();

    let child_wire = child.canonical_json().unwrap();
    let mut edge_only_wire: Value = serde_json::from_slice(&child_wire).unwrap();
    let edges = edge_only_wire["edges"].as_array_mut().unwrap();
    let original_edge_count = edges.len();
    edges.retain(|edge| {
        !(edge["kind"] == "boundary-of"
            && edge["from"]["ulid"] == boundary.ulid().to_string()
            && edge["to"]["ulid"] == body.ulid().to_string())
    });
    assert_eq!(edges.len() + 1, original_edge_count);

    let edge_only_mutant = ModelEnvelope::from_json(
        &serde_json::to_vec(&edge_only_wire).unwrap(),
        ModelDecoderLimits::default(),
    )
    .unwrap();
    assert!(edge_only_mutant.to_program().is_err());
    assert!(
        GeometryIdentityEnvelopeV1::new(&edge_only_mutant, [body], TOLERANCE_M).is_err(),
        "Geometry Identity construction must never admit an invalid Model replay"
    );

    let mut missing_role_wire: Value = serde_json::from_slice(&child_wire).unwrap();
    missing_role_wire["edges"]
        .as_array_mut()
        .unwrap()
        .retain(|edge| {
            !(edge["kind"] == "boundary-of"
                && edge["from"]["ulid"] == boundary.ulid().to_string()
                && edge["to"]["ulid"] == body.ulid().to_string())
        });
    missing_role_wire["nodes"]
        .as_array_mut()
        .unwrap()
        .retain(|node| node["id"]["ulid"] != boundary.ulid().to_string());
    let missing_role_mutant = ModelEnvelope::from_json(
        &serde_json::to_vec(&missing_role_wire).unwrap(),
        ModelDecoderLimits::default(),
    )
    .unwrap();
    assert!(missing_role_mutant.to_program().is_ok());
    assert!(
        GeometryIdentityEnvelopeV1::new(&missing_role_mutant, [body], TOLERANCE_M).is_err(),
        "Geometry Identity must reject a replayable Model with an incomplete exterior"
    );
    assert!(
        accepted_geometry
            .validate_against(&missing_role_mutant)
            .is_err()
    );
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

fn incident_edges(document: &ModelDocument, body: Id<kinds::Domain>) -> Vec<eqiora::graph::Edge> {
    document
        .program()
        .edges()
        .iter()
        .filter(|edge| edge.from() == body.erase() || edge.to() == body.erase())
        .copied()
        .collect()
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

fn box_volume(document: &ModelDocument) -> f64 {
    cartesian_bounds(document)
        .into_iter()
        .map(|(lower, upper)| upper - lower)
        .product()
}

fn cartesian_bounds(document: &ModelDocument) -> Vec<(f64, f64)> {
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
        .map(|axis| (axis.lower().value(), axis.upper().value()))
        .collect()
}

fn axis_bounds(lower: f64, upper: f64) -> AxisBounds {
    let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension");
    AxisBounds::new(
        DynQuantity::new(lower, length),
        DynQuantity::new(upper, length),
    )
    .unwrap()
}

fn mesh_artifact(bounds: [(f64, f64); 3]) -> SimplicialMeshEnvelopeV1 {
    let [x, y, z] = bounds;
    let mesh = SimplicialMesh::new(
        3,
        vec![
            vec![x.0, y.0, z.0],
            vec![x.1, y.0, z.0],
            vec![x.0, y.1, z.0],
            vec![x.1, y.1, z.0],
            vec![x.0, y.0, z.1],
            vec![x.1, y.0, z.1],
            vec![x.0, y.1, z.1],
            vec![x.1, y.1, z.1],
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
