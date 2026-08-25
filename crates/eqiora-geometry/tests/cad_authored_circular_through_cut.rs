use std::collections::BTreeMap;

use eqiora_geometry::{
    CadAuthoredFaceHandle, CadAuthoredGraph, CadRepairDispositionV1, ConstrainedRectangleV1,
};

const WIRE: &str = r#"{"schema":"eqiora.cad-authored-operation-graph-envelope/v2","encoding":"eqiora.canonical-json/v1","length_unit":"metre","requested_modeling_tolerance_m":1e-10,"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.0},"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle","sketch_plane":"sketch-plane","constraint":"closed-by-construction","x_bounds_m":[-0.04,0.04],"y_bounds_m":[-0.025,0.025]},"face":{"id":"profile-face","kind":"one-closed-loop-face","profile":"rectangle-profile","region_count":1},"extrusion":{"id":"positive-z-extrusion","kind":"positive-z","face":"profile-face","depth_m":0.02,"repair":"none"},"cut_sketch_plane":{"id":"cut-sketch-plane","kind":"on-face","face":"end-cap"},"cut_profile":{"id":"circle-profile","kind":"circle","sketch_plane":"cut-sketch-plane","constraint":"closed-by-construction","center_m":[0.02,0.0],"radius_m":0.008},"cut_face":{"id":"cut-profile-face","kind":"one-closed-loop-face","profile":"circle-profile","region_count":1},"cut":{"id":"circular-through-cut","kind":"difference-through-all-negative-z","target":"positive-z-extrusion","tool_face":"cut-profile-face","requested_tolerance_m":1e-9,"repair":"none"},"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper","profile-y-lower","profile-y-upper","cut-wall"]}"#;

const DIGEST: [u8; 32] = [
    0x00, 0xac, 0xb9, 0x49, 0x4f, 0xc7, 0xde, 0xa8, 0xf1, 0xf2, 0x50, 0x0d, 0x13, 0x16, 0xcb, 0x33,
    0x15, 0x13, 0x0a, 0x96, 0x5a, 0x24, 0x17, 0x9b, 0x3e, 0xb1, 0xb1, 0x03, 0x45, 0x05, 0x8b, 0x47,
];

const DFG_SECTION_WIRE: &str = r#"{"schema":"eqiora.planar-circular-hole-envelope/v1","encoding":"eqiora.canonical-json/v1","kind":"axis-aligned-rectangle-with-circular-hole-v1","length_unit":"metre","tolerance_m":1e-12,"bounds":[[0.0,2.2],[0.0,0.41]],"circle":{"center":[0.2,0.2],"radius_m":0.05},"entity_sets":[{"name":"cylinder","dimension":1,"members":[4]},{"name":"inlet","dimension":1,"members":[0]},{"name":"outlet","dimension":1,"members":[1]},{"name":"walls","dimension":1,"members":[2,3]},{"name":"fluid","dimension":2,"members":[0]}]}"#;

const DFG_SECTION_DIGEST: [u8; 32] = [
    0xb0, 0x01, 0x23, 0x47, 0x2a, 0x59, 0x6e, 0x82, 0x89, 0x82, 0x0c, 0xab, 0xae, 0xe2, 0x0d, 0x52,
    0xcd, 0xf8, 0x1b, 0x55, 0x72, 0xfa, 0x9c, 0xe5, 0x8f, 0xf1, 0x7c, 0xda, 0xa0, 0x00, 0x46, 0xd9,
];

const RELATIVE_TOLERANCE: f64 = 4.0e-15;

fn base(modeling_tolerance_m: f64) -> CadAuthoredGraph {
    CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((-0.04, 0.04), (-0.025, 0.025), 0.0).unwrap(),
        0.02,
        modeling_tolerance_m,
    )
    .unwrap()
}

fn witness() -> CadAuthoredGraph {
    base(1.0e-10)
        .circular_through_cut([0.02, 0.0], 0.008, 1.0e-9)
        .unwrap()
}

fn dfg_graph() -> CadAuthoredGraph {
    CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap(),
        1.0,
        1.0e-10,
    )
    .unwrap()
    .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
    .unwrap()
}

fn dfg_named_topology(graph: &CadAuthoredGraph) -> BTreeMap<String, Vec<CadAuthoredFaceHandle>> {
    BTreeMap::from([
        (
            "fluid".to_owned(),
            vec![graph.face_handle("end-cap").unwrap()],
        ),
        (
            "inlet".to_owned(),
            vec![graph.face_handle("profile-x-lower").unwrap()],
        ),
        (
            "outlet".to_owned(),
            vec![graph.face_handle("profile-x-upper").unwrap()],
        ),
        (
            "walls".to_owned(),
            vec![
                graph.face_handle("profile-y-lower").unwrap(),
                graph.face_handle("profile-y-upper").unwrap(),
            ],
        ),
        (
            "cylinder".to_owned(),
            vec![graph.face_handle("cut-wall").unwrap()],
        ),
    ])
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= RELATIVE_TOLERANCE * actual.abs().max(expected.abs()),
        "actual {actual:e}, expected {expected:e}"
    );
}

fn keys(handles: &[CadAuthoredFaceHandle]) -> Vec<&'static str> {
    handles
        .iter()
        .map(CadAuthoredFaceHandle::provenance_key)
        .collect()
}

#[test]
fn independent_oracles_freeze_wire_geometry_and_face_observations() {
    let graph = witness();
    assert_eq!(graph.canonical_bytes(), WIRE.as_bytes());
    assert_eq!(graph.canonical_bytes().len(), 1292);
    assert_eq!(graph.digest_bytes(), DIGEST);
    assert_eq!(
        graph.output().bounds_m(),
        [(-0.04, 0.04), (-0.025, 0.025), (0.0, 0.02)]
    );
    assert_eq!(graph.vertex_count(), None);
    assert_eq!(graph.edge_count(), None);
    assert_eq!(graph.face_count(), 7);
    assert_eq!(graph.closed_shell_count(), 1);
    assert_eq!(graph.body_count(), 1);
    assert_eq!(graph.genus(), 1);
    assert_eq!(
        keys(&graph.face_handles().unwrap()),
        [
            "start-cap",
            "end-cap",
            "profile-x-lower",
            "profile-x-upper",
            "profile-y-lower",
            "profile-y-upper",
            "cut-wall",
        ]
    );

    close(graph.volume_m3(), 7.597_876_140_340_507e-5);
    close(graph.surface_area_m2(), 0.013_803_185_789_489_24);

    for (provenance_key, area, loops) in [
        ("start-cap", 0.003_798_938_070_170_253_3, 2),
        ("end-cap", 0.003_798_938_070_170_253_3, 2),
        ("profile-x-lower", 0.001, 1),
        ("profile-x-upper", 0.001, 1),
        ("profile-y-lower", 0.0016, 1),
        ("profile-y-upper", 0.0016, 1),
        ("cut-wall", 0.001_005_309_649_148_734, 2),
    ] {
        let handle = graph.face_handle(provenance_key).unwrap();
        let replayed = CadAuthoredFaceHandle::decode_canonical(handle.canonical_bytes()).unwrap();
        assert_eq!(handle.provenance_key(), provenance_key);
        assert_eq!(graph.resolve_face(&replayed).unwrap(), provenance_key);
        close(graph.face_area_m2(&replayed).unwrap(), area);
        assert_eq!(graph.face_boundary_loop_count(&replayed).unwrap(), loops);
    }

    let cut_wall = graph.face_handle("cut-wall").unwrap();
    assert_eq!(graph.rectangular_face_vertices_m(&cut_wall).unwrap(), None);
    assert_eq!(graph.planar_face_outward_normal(&cut_wall).unwrap(), None);
}

#[test]
fn build_receipt_keeps_tolerances_provenance_repair_and_lineage_distinct() {
    let graph = witness();
    let build = graph.build_analytic().unwrap();
    assert_eq!(build.graph_digest_bytes(), graph.digest_bytes());
    assert_eq!(
        build.provider_profile(),
        "eqiora.cad.analytic-circular-through-cut-v1"
    );
    assert_eq!(build.requested_modeling_tolerance_m(), 1.0e-10);
    assert_eq!(build.requested_boolean_tolerance_m(), Some(1.0e-9));
    assert_eq!(build.effective_boolean_tolerance_m(), Some(1.0e-9));
    assert_eq!(build.maximum_position_discrepancy_m(), 0.0);
    assert_eq!(build.maximum_area_discrepancy_m2(), 0.0);
    assert_eq!(build.maximum_volume_discrepancy_m3(), 0.0);
    assert_eq!(build.repair_disposition(), CadRepairDispositionV1::None);
    assert_eq!(
        keys(build.retained_unchanged()),
        [
            "profile-x-lower",
            "profile-x-upper",
            "profile-y-lower",
            "profile-y-upper"
        ]
    );
    assert_eq!(keys(build.retained_modified()), ["start-cap", "end-cap"]);
    assert_eq!(keys(build.created()), ["cut-wall"]);
    assert!(build.deleted().is_empty());
    assert!(build.split().is_empty());
    assert!(build.merged().is_empty());

    let receipt_discriminator = base(1.0e-10)
        .circular_through_cut([0.02, 0.0], 0.008, 1.0e-11)
        .unwrap()
        .build_analytic()
        .unwrap();
    assert_eq!(
        receipt_discriminator.requested_modeling_tolerance_m(),
        1.0e-10
    );
    assert_eq!(
        receipt_discriminator.requested_boolean_tolerance_m(),
        Some(1.0e-11)
    );
    assert_eq!(
        receipt_discriminator.effective_boolean_tolerance_m(),
        Some(1.0e-11)
    );
}

#[test]
fn signed_clearance_and_strict_boundary_reject_before_identity() {
    assert!(
        base(1.0e-10)
            .circular_through_cut([0.10, 0.0], 0.008, 1.0e-9)
            .is_err()
    );
    assert!(
        base(1.0e-10)
            .circular_through_cut([0.0335, 0.0], 0.008, 1.0e-9)
            .is_err()
    );

    let exact_boundary = CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((0.0, 4.0e-9), (0.0, 4.0e-9), 0.0).unwrap(),
        1.0,
        1.0e-10,
    )
    .unwrap();
    assert!(
        exact_boundary
            .circular_through_cut([2.0e-9, 2.0e-9], 1.0e-9, 1.0e-9)
            .is_err()
    );
    assert!(
        exact_boundary
            .circular_through_cut([2.0e-9, 2.0e-9], 0.5e-9, 1.0e-9)
            .is_ok()
    );

    for (radius, tolerance) in [
        (0.0, 1.0e-9),
        (-1.0, 1.0e-9),
        (f64::NAN, 1.0e-9),
        (0.008, 0.0),
        (0.008, f64::INFINITY),
    ] {
        assert!(
            base(1.0e-10)
                .circular_through_cut([0.02, 0.0], radius, tolerance)
                .is_err()
        );
    }
}

#[test]
fn wire_and_handles_fail_closed_without_rebinding() {
    let graph = witness();
    let value: serde_json::Value = serde_json::from_str(WIRE).unwrap();
    let reordered = serde_json::to_vec(&value).unwrap();
    let replayed = CadAuthoredGraph::decode_canonical(&reordered).unwrap();
    assert_eq!(replayed, graph);
    assert_eq!(replayed.canonical_bytes(), WIRE.as_bytes());

    for mutant in [
        WIRE.replace("envelope/v2", "envelope/v3"),
        WIRE.replace("\"length_unit\":", "\"unknown\":0,\"length_unit\":"),
        WIRE.replace(
            "\"length_unit\":",
            "\"length_unit\":\"metre\",\"length_unit\":",
        ),
        WIRE.replace(
            "\"target\":\"positive-z-extrusion\"",
            "\"target\":\"foreign\"",
        ),
        WIRE.replace("difference-through-all-negative-z", "blind-cut"),
        WIRE.replace("\"repair\":\"none\"", "\"repair\":\"healed\""),
        WIRE.replace(",\"cut-wall\"]", "]"),
    ] {
        assert!(
            CadAuthoredGraph::decode_canonical(mutant.as_bytes()).is_err(),
            "wire mutant must reject: {mutant}"
        );
    }

    let base_graph = base(1.0e-10);
    let old_handle = base_graph.face_handle("end-cap").unwrap();
    assert!(graph.resolve_face(&old_handle).is_err());

    let changed = base(1.0e-10)
        .circular_through_cut([0.02, 0.0], 0.008, 2.0e-9)
        .unwrap();
    let handle = graph.face_handle("cut-wall").unwrap();
    assert!(changed.resolve_face(&handle).is_err());

    let handle_wire = String::from_utf8(handle.canonical_bytes().to_vec()).unwrap();
    let foreign_wire = handle_wire.replacen(
        "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47",
        "10acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47",
        1,
    );
    let foreign = CadAuthoredFaceHandle::decode_canonical(foreign_wire.as_bytes()).unwrap();
    assert!(graph.resolve_face(&foreign).is_err());
}

#[test]
fn atomic_topology_naming_reproduces_the_frozen_exact_planar_section() {
    let graph = dfg_graph();
    let section = graph.planar_section(&dfg_named_topology(&graph)).unwrap();

    assert_eq!(section.canonical_bytes(), DFG_SECTION_WIRE.as_bytes());
    assert_eq!(section.digest_bytes(), DFG_SECTION_DIGEST);

    let mut arbitrary = dfg_named_topology(&graph);
    let inlet = arbitrary.remove("inlet").unwrap();
    arbitrary.insert("left boundary".to_owned(), inlet);
    let renamed = graph.planar_section(&arbitrary).unwrap();
    assert_eq!(renamed.entity_set("left boundary").unwrap().dimension(), 1);
    assert!(renamed.entity_set("inlet").is_none());
}

#[test]
fn atomic_topology_naming_rejects_foreign_incomplete_and_ambiguous_mappings() {
    let graph = dfg_graph();
    let foreign = CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((0.0, 2.3), (0.0, 0.41), 0.0).unwrap(),
        1.0,
        1.0e-10,
    )
    .unwrap()
    .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
    .unwrap();

    let mut named = dfg_named_topology(&graph);
    named.insert(
        "cylinder".to_owned(),
        vec![foreign.face_handle("cut-wall").unwrap()],
    );
    assert!(graph.planar_section(&named).is_err());

    let mut named = dfg_named_topology(&graph);
    named.remove("outlet");
    assert!(graph.planar_section(&named).is_err());

    let mut named = dfg_named_topology(&graph);
    named.insert("empty".to_owned(), Vec::new());
    assert!(graph.planar_section(&named).is_err());

    let mut named = dfg_named_topology(&graph);
    named
        .get_mut("walls")
        .unwrap()
        .push(graph.face_handle("profile-x-lower").unwrap());
    assert!(graph.planar_section(&named).is_err());

    let mut named = dfg_named_topology(&graph);
    named
        .get_mut("walls")
        .unwrap()
        .push(graph.face_handle("end-cap").unwrap());
    assert!(graph.planar_section(&named).is_err());

    let mut named = dfg_named_topology(&graph);
    named.insert(
        "fluid".to_owned(),
        vec![graph.face_handle("start-cap").unwrap()],
    );
    assert!(graph.planar_section(&named).is_err());
}

#[test]
fn planar_section_keeps_nonplanar_graph_facts_out_of_geometry_identity() {
    let derive = |plane_z, depth, modeling_tolerance| {
        let graph = CadAuthoredGraph::new(
            ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), plane_z).unwrap(),
            depth,
            modeling_tolerance,
        )
        .unwrap()
        .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
        .unwrap();
        graph.planar_section(&dfg_named_topology(&graph)).unwrap()
    };
    assert_eq!(derive(0.0, 1.0, 1.0e-10), derive(4.0, 3.0, 2.0e-10));
    let rectangle = CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap(),
        1.0,
        1.0e-10,
    )
    .unwrap();
    assert!(rectangle.planar_section(&BTreeMap::new()).is_err());
}
