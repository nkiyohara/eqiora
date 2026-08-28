use std::fmt::Debug;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::{
    CadAuthoredBuild, CadAuthoredFaceHandle, CadAuthoredGraph, CadAuthoredSketch,
    ConstrainedRectangleV1,
};

const V1_WIRE: &str = r#"{"schema":"eqiora.cad-authored-operation-graph-envelope/v1","encoding":"eqiora.canonical-json/v1","length_unit":"metre","requested_modeling_tolerance_m":1e-9,"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.5},"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle","sketch_plane":"sketch-plane","constraint":"closed-by-construction","x_bounds_m":[-2.0,3.0],"y_bounds_m":[-1.0,2.0]},"face":{"id":"profile-face","kind":"one-closed-loop-face","profile":"rectangle-profile","region_count":1},"extrusion":{"id":"positive-z-extrusion","kind":"positive-z","face":"profile-face","depth_m":4.0,"repair":"none"},"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper","profile-y-lower","profile-y-upper"]}"#;

const V1_DIGEST: [u8; 32] = [
    0x91, 0x95, 0x45, 0xf7, 0x01, 0x18, 0x84, 0x0c, 0x04, 0xda, 0x97, 0x15, 0x82, 0x9d, 0xeb, 0x2d,
    0xa9, 0x47, 0x46, 0x0a, 0x51, 0x31, 0x1e, 0xba, 0xbe, 0xc6, 0xa3, 0x40, 0x38, 0xc6, 0x6f, 0x36,
];

const V2_WIRE: &str = r#"{"schema":"eqiora.cad-authored-operation-graph-envelope/v2","encoding":"eqiora.canonical-json/v1","length_unit":"metre","requested_modeling_tolerance_m":1e-10,"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.0},"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle","sketch_plane":"sketch-plane","constraint":"closed-by-construction","x_bounds_m":[-0.04,0.04],"y_bounds_m":[-0.025,0.025]},"face":{"id":"profile-face","kind":"one-closed-loop-face","profile":"rectangle-profile","region_count":1},"extrusion":{"id":"positive-z-extrusion","kind":"positive-z","face":"profile-face","depth_m":0.02,"repair":"none"},"cut_sketch_plane":{"id":"cut-sketch-plane","kind":"on-face","face":"end-cap"},"cut_profile":{"id":"circle-profile","kind":"circle","sketch_plane":"cut-sketch-plane","constraint":"closed-by-construction","center_m":[0.02,0.0],"radius_m":0.008},"cut_face":{"id":"cut-profile-face","kind":"one-closed-loop-face","profile":"circle-profile","region_count":1},"cut":{"id":"circular-through-cut","kind":"difference-through-all-negative-z","target":"positive-z-extrusion","tool_face":"cut-profile-face","requested_tolerance_m":1e-9,"repair":"none"},"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper","profile-y-lower","profile-y-upper","cut-wall"]}"#;

const V2_DIGEST: [u8; 32] = [
    0x00, 0xac, 0xb9, 0x49, 0x4f, 0xc7, 0xde, 0xa8, 0xf1, 0xf2, 0x50, 0x0d, 0x13, 0x16, 0xcb, 0x33,
    0x15, 0x13, 0x0a, 0x96, 0x5a, 0x24, 0x17, 0x9b, 0x3e, 0xb1, 0xb1, 0x03, 0x45, 0x05, 0x8b, 0x47,
];

const V1_FACE_ORDER: [&str; 6] = [
    "start-cap",
    "end-cap",
    "profile-x-lower",
    "profile-x-upper",
    "profile-y-lower",
    "profile-y-upper",
];

const V2_FACE_ORDER: [&str; 7] = [
    "start-cap",
    "end-cap",
    "profile-x-lower",
    "profile-x-upper",
    "profile-y-lower",
    "profile-y-upper",
    "cut-wall",
];

fn rectangle_authority() -> ConstrainedRectangleV1 {
    ConstrainedRectangleV1::new((-2.0, 3.0), (-1.0, 2.0), 0.5).unwrap()
}

fn symmetric_rectangle() -> ConstrainedRectangleV1 {
    ConstrainedRectangleV1::new((-0.04, 0.04), (-0.025, 0.025), 0.0).unwrap()
}

fn dfg_rectangle() -> ConstrainedRectangleV1 {
    ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap()
}

fn explicit_v1_authority() -> CadAuthoredGraph {
    CadAuthoredSketch::rectangle_xy(rectangle_authority(), 1.0e-9)
        .unwrap()
        .extrude_positive_z(4.0)
        .unwrap()
}

fn compatibility_v1_authority() -> CadAuthoredGraph {
    CadAuthoredGraph::new(rectangle_authority(), 4.0, 1.0e-9).unwrap()
}

fn explicit_symmetric_base() -> CadAuthoredGraph {
    CadAuthoredSketch::rectangle_xy(symmetric_rectangle(), 1.0e-10)
        .unwrap()
        .extrude_positive_z(0.02)
        .unwrap()
}

fn compatibility_symmetric_base() -> CadAuthoredGraph {
    CadAuthoredGraph::new(symmetric_rectangle(), 0.02, 1.0e-10).unwrap()
}

fn explicit_v2_authority(center_m: [f64; 2]) -> CadAuthoredGraph {
    let base = explicit_symmetric_base();
    let sketch =
        CadAuthoredSketch::circle_on_face(base.face_handle("end-cap").unwrap(), center_m, 0.008)
            .unwrap();
    base.through_cut(&sketch, 1.0e-9).unwrap()
}

fn compatibility_v2_authority() -> CadAuthoredGraph {
    compatibility_symmetric_base()
        .circular_through_cut([0.02, 0.0], 0.008, 1.0e-9)
        .unwrap()
}

fn explicit_dfg_graph() -> CadAuthoredGraph {
    let base = CadAuthoredSketch::rectangle_xy(dfg_rectangle(), 1.0e-10)
        .unwrap()
        .extrude_positive_z(1.0)
        .unwrap();
    let sketch =
        CadAuthoredSketch::circle_on_face(base.face_handle("end-cap").unwrap(), [0.2, 0.2], 0.05)
            .unwrap();
    base.through_cut(&sketch, 1.0e-10).unwrap()
}

fn compatibility_dfg_graph() -> CadAuthoredGraph {
    CadAuthoredGraph::new(dfg_rectangle(), 1.0, 1.0e-10)
        .unwrap()
        .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
        .unwrap()
}

fn assert_invalid<T: Debug>(result: Result<T, Diagnostic>) {
    let error = result.expect_err("the closed authored-sketch contract must reject this input");
    assert_eq!(error.code(), codes::INVALID_ARTIFACT);
}

fn keys(handles: &[CadAuthoredFaceHandle]) -> Vec<&'static str> {
    handles
        .iter()
        .map(CadAuthoredFaceHandle::provenance_key)
        .collect()
}

fn assert_membership(handles: &[CadAuthoredFaceHandle], expected: &[&str]) {
    let mut actual = keys(handles);
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected);
}

fn assert_lineage_partition(graph: &CadAuthoredGraph, build: &CadAuthoredBuild) {
    let mut lineage = Vec::new();
    for handles in [
        build.retained_unchanged(),
        build.retained_modified(),
        build.created(),
        build.deleted(),
        build.split(),
        build.merged(),
    ] {
        lineage.extend(keys(handles));
    }
    lineage.sort_unstable();

    let mut inventory = keys(&graph.face_handles().unwrap());
    inventory.sort_unstable();
    assert_eq!(lineage, inventory);
}

fn assert_lineage_route_equality(explicit: &CadAuthoredBuild, compatibility: &CadAuthoredBuild) {
    assert_eq!(
        explicit.retained_unchanged(),
        compatibility.retained_unchanged()
    );
    assert_eq!(
        explicit.retained_modified(),
        compatibility.retained_modified()
    );
    assert_eq!(explicit.created(), compatibility.created());
    assert_eq!(explicit.deleted(), compatibility.deleted());
    assert_eq!(explicit.split(), compatibility.split());
    assert_eq!(explicit.merged(), compatibility.merged());
}

fn assert_route_equivalence(explicit: &CadAuthoredGraph, compatibility: &CadAuthoredGraph) {
    assert_eq!(explicit, compatibility);
    assert_eq!(explicit.canonical_bytes(), compatibility.canonical_bytes());
    assert_eq!(explicit.digest_bytes(), compatibility.digest_bytes());
    assert_eq!(explicit.sketch(), compatibility.sketch());
    assert_eq!(
        explicit.extrusion_depth_m(),
        compatibility.extrusion_depth_m()
    );
    assert_eq!(
        explicit.requested_modeling_tolerance_m(),
        compatibility.requested_modeling_tolerance_m()
    );
    assert_eq!(
        explicit.requested_boolean_tolerance_m(),
        compatibility.requested_boolean_tolerance_m()
    );
    assert_eq!(explicit.cut_center_m(), compatibility.cut_center_m());
    assert_eq!(explicit.cut_radius_m(), compatibility.cut_radius_m());
    assert_eq!(explicit.output(), compatibility.output());
    assert_eq!(
        explicit.repair_disposition(),
        compatibility.repair_disposition()
    );
    assert_eq!(explicit.vertices_m(), compatibility.vertices_m());
    assert_eq!(explicit.vertex_count(), compatibility.vertex_count());
    assert_eq!(explicit.edge_count(), compatibility.edge_count());
    assert_eq!(explicit.face_count(), compatibility.face_count());
    assert_eq!(
        explicit.closed_shell_count(),
        compatibility.closed_shell_count()
    );
    assert_eq!(explicit.body_count(), compatibility.body_count());
    assert_eq!(explicit.genus(), compatibility.genus());
    assert_eq!(
        explicit.volume_m3().to_bits(),
        compatibility.volume_m3().to_bits()
    );
    assert_eq!(
        explicit.surface_area_m2().to_bits(),
        compatibility.surface_area_m2().to_bits()
    );

    let explicit_handles = explicit.face_handles().unwrap();
    let compatibility_handles = compatibility.face_handles().unwrap();
    assert_eq!(explicit_handles, compatibility_handles);
    for (explicit_handle, compatibility_handle) in
        explicit_handles.iter().zip(&compatibility_handles)
    {
        assert_eq!(
            explicit_handle.canonical_bytes(),
            compatibility_handle.canonical_bytes()
        );
        assert_eq!(
            explicit_handle.graph_digest_bytes(),
            explicit.digest_bytes()
        );
        let decoded_handle =
            CadAuthoredFaceHandle::decode_canonical(explicit_handle.canonical_bytes()).unwrap();
        assert_eq!(&decoded_handle, explicit_handle);
        assert_eq!(
            explicit.resolve_face(explicit_handle).unwrap(),
            compatibility.resolve_face(compatibility_handle).unwrap()
        );
        assert_eq!(
            explicit.face_area_m2(explicit_handle).unwrap().to_bits(),
            compatibility
                .face_area_m2(compatibility_handle)
                .unwrap()
                .to_bits()
        );
        assert_eq!(
            explicit.face_boundary_loop_count(explicit_handle).unwrap(),
            compatibility
                .face_boundary_loop_count(compatibility_handle)
                .unwrap()
        );
        assert_eq!(
            explicit
                .rectangular_face_vertices_m(explicit_handle)
                .unwrap(),
            compatibility
                .rectangular_face_vertices_m(compatibility_handle)
                .unwrap()
        );
        assert_eq!(
            explicit
                .rectangular_face_centroid_m(explicit_handle)
                .unwrap(),
            compatibility
                .rectangular_face_centroid_m(compatibility_handle)
                .unwrap()
        );
        assert_eq!(
            explicit
                .planar_face_outward_normal(explicit_handle)
                .unwrap(),
            compatibility
                .planar_face_outward_normal(compatibility_handle)
                .unwrap()
        );
    }

    let explicit_build = explicit.build_analytic().unwrap();
    let compatibility_build = compatibility.build_analytic().unwrap();
    assert_eq!(
        explicit_build.graph_digest_bytes(),
        compatibility_build.graph_digest_bytes()
    );
    assert_eq!(
        explicit_build.provider_profile(),
        compatibility_build.provider_profile()
    );
    assert_eq!(
        explicit_build.requested_modeling_tolerance_m(),
        compatibility_build.requested_modeling_tolerance_m()
    );
    assert_eq!(
        explicit_build.requested_boolean_tolerance_m(),
        compatibility_build.requested_boolean_tolerance_m()
    );
    assert_eq!(
        explicit_build.effective_boolean_tolerance_m(),
        compatibility_build.effective_boolean_tolerance_m()
    );
    assert_eq!(
        explicit_build.maximum_position_discrepancy_m().to_bits(),
        compatibility_build
            .maximum_position_discrepancy_m()
            .to_bits()
    );
    assert_eq!(
        explicit_build.maximum_area_discrepancy_m2().to_bits(),
        compatibility_build.maximum_area_discrepancy_m2().to_bits()
    );
    assert_eq!(
        explicit_build.maximum_volume_discrepancy_m3().to_bits(),
        compatibility_build
            .maximum_volume_discrepancy_m3()
            .to_bits()
    );
    assert_eq!(
        explicit_build.repair_disposition(),
        compatibility_build.repair_disposition()
    );
    assert_lineage_route_equality(&explicit_build, &compatibility_build);

    let decoded = CadAuthoredGraph::decode_canonical(explicit.canonical_bytes()).unwrap();
    assert_eq!(&decoded, explicit);
    assert_eq!(decoded.canonical_bytes(), explicit.canonical_bytes());
    assert_eq!(decoded.digest_bytes(), explicit.digest_bytes());
}

#[test]
fn explicit_rectangle_route_reproduces_the_exact_v1_authority() {
    let explicit = explicit_v1_authority();
    let compatibility = compatibility_v1_authority();
    assert_route_equivalence(&explicit, &compatibility);
    assert_eq!(explicit.canonical_bytes(), V1_WIRE.as_bytes());
    assert_eq!(explicit.canonical_bytes().len(), 731);
    assert_eq!(explicit.digest_bytes(), V1_DIGEST);
    assert_eq!(
        keys(&explicit.face_handles().unwrap()).as_slice(),
        V1_FACE_ORDER
    );

    let build = explicit.build_analytic().unwrap();
    assert!(build.retained_unchanged().is_empty());
    assert!(build.retained_modified().is_empty());
    assert_eq!(build.created().len(), V1_FACE_ORDER.len());
    assert!(build.deleted().is_empty());
    assert!(build.split().is_empty());
    assert!(build.merged().is_empty());
    assert_lineage_partition(&explicit, &build);
}

#[test]
fn explicit_cut_route_reproduces_the_exact_v2_authority() {
    let explicit = explicit_v2_authority([0.02, 0.0]);
    let compatibility = compatibility_v2_authority();
    assert_route_equivalence(&explicit, &compatibility);
    assert_eq!(explicit.canonical_bytes(), V2_WIRE.as_bytes());
    assert_eq!(explicit.canonical_bytes().len(), 1292);
    assert_eq!(explicit.digest_bytes(), V2_DIGEST);
    assert_eq!(
        keys(&explicit.face_handles().unwrap()).as_slice(),
        V2_FACE_ORDER
    );

    let build = explicit.build_analytic().unwrap();
    assert_membership(
        build.retained_unchanged(),
        &[
            "profile-x-lower",
            "profile-x-upper",
            "profile-y-lower",
            "profile-y-upper",
        ],
    );
    assert_membership(build.retained_modified(), &["start-cap", "end-cap"]);
    assert_eq!(build.created().len(), 1);
    assert!(build.deleted().is_empty());
    assert!(build.split().is_empty());
    assert!(build.merged().is_empty());
    assert_lineage_partition(&explicit, &build);

    let discriminator_base = explicit_symmetric_base();
    let discriminator_sketch = CadAuthoredSketch::circle_on_face(
        discriminator_base.face_handle("end-cap").unwrap(),
        [0.02, 0.0],
        0.008,
    )
    .unwrap();
    let explicit_discriminator = discriminator_base
        .through_cut(&discriminator_sketch, 1.0e-11)
        .unwrap();
    let compatibility_discriminator = compatibility_symmetric_base()
        .circular_through_cut([0.02, 0.0], 0.008, 1.0e-11)
        .unwrap();
    assert_route_equivalence(&explicit_discriminator, &compatibility_discriminator);
}

#[test]
fn separate_dfg_graph_routes_reproduce_the_exact_cad_authority() {
    let explicit = explicit_dfg_graph();
    let compatibility = compatibility_dfg_graph();
    assert_route_equivalence(&explicit, &compatibility);
}

#[test]
fn rectangle_and_circle_signed_zero_ownership_are_separate() {
    let positive_rectangle = CadAuthoredSketch::rectangle_xy(
        ConstrainedRectangleV1::new((0.0, 0.04), (0.0, 0.025), 0.0).unwrap(),
        1.0e-10,
    )
    .unwrap()
    .extrude_positive_z(0.02)
    .unwrap();
    let negative_rectangle = CadAuthoredSketch::rectangle_xy(
        ConstrainedRectangleV1::new((-0.0, 0.04), (-0.0, 0.025), 0.0).unwrap(),
        1.0e-10,
    )
    .unwrap()
    .extrude_positive_z(0.02)
    .unwrap();
    assert_eq!(positive_rectangle, negative_rectangle);
    assert_eq!(
        positive_rectangle.canonical_bytes(),
        negative_rectangle.canonical_bytes()
    );
    assert_eq!(
        positive_rectangle.digest_bytes(),
        negative_rectangle.digest_bytes()
    );

    let positive_center = explicit_v2_authority([0.02, 0.0]);
    let negative_center = explicit_v2_authority([0.02, -0.0]);
    assert_eq!(positive_center, negative_center);
    assert_eq!(positive_center.canonical_bytes(), V2_WIRE.as_bytes());
    assert_eq!(negative_center.canonical_bytes(), V2_WIRE.as_bytes());
    assert_eq!(positive_center.digest_bytes(), V2_DIGEST);
    assert_eq!(negative_center.digest_bytes(), V2_DIGEST);
    assert_route_equivalence(&positive_center, &compatibility_v2_authority());
    assert_route_equivalence(&negative_center, &compatibility_v2_authority());
}

#[test]
fn rectangle_modeling_tolerance_and_depth_admission_fail_closed() {
    for (x_bounds, y_bounds, plane_z) in [
        ((f64::NAN, 1.0), (0.0, 1.0), 0.0),
        ((0.0, f64::INFINITY), (0.0, 1.0), 0.0),
        ((0.0, 1.0), (f64::NEG_INFINITY, 1.0), 0.0),
        ((0.0, 1.0), (0.0, f64::NAN), 0.0),
        ((0.0, 1.0), (0.0, 1.0), f64::NAN),
        ((0.0, 1.0), (0.0, 1.0), f64::INFINITY),
        ((0.0, 1.0), (0.0, 1.0), f64::NEG_INFINITY),
        ((1.0, 1.0), (0.0, 1.0), 0.0),
        ((1.0, 0.0), (0.0, 1.0), 0.0),
    ] {
        assert_invalid(ConstrainedRectangleV1::new(x_bounds, y_bounds, plane_z));
    }

    for tolerance in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_invalid(CadAuthoredSketch::rectangle_xy(
            rectangle_authority(),
            tolerance,
        ));
    }

    let sketch = CadAuthoredSketch::rectangle_xy(rectangle_authority(), 1.0e-9).unwrap();
    for depth in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_invalid(sketch.extrude_positive_z(depth));
    }

    let overflow = CadAuthoredSketch::rectangle_xy(
        ConstrainedRectangleV1::new((0.0, 1.0), (0.0, 1.0), f64::MAX).unwrap(),
        1.0e-9,
    )
    .unwrap();
    assert_invalid(overflow.extrude_positive_z(f64::MAX));
}

#[test]
fn circle_center_radius_and_boolean_tolerance_admission_fail_closed() {
    let base = explicit_symmetric_base();
    let end_cap = base.face_handle("end-cap").unwrap();
    for center in [
        [f64::NAN, 0.0],
        [f64::INFINITY, 0.0],
        [f64::NEG_INFINITY, 0.0],
        [0.0, f64::NAN],
        [0.0, f64::INFINITY],
        [0.0, f64::NEG_INFINITY],
    ] {
        assert_invalid(CadAuthoredSketch::circle_on_face(
            end_cap.clone(),
            center,
            0.008,
        ));
    }

    for radius in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_invalid(CadAuthoredSketch::circle_on_face(
            end_cap.clone(),
            [0.02, 0.0],
            radius,
        ));
    }

    let circle = CadAuthoredSketch::circle_on_face(end_cap, [0.02, 0.0], 0.008).unwrap();
    for tolerance in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_invalid(base.through_cut(&circle, tolerance));
    }
}

#[test]
fn face_version_provenance_and_graph_binding_never_rebind() {
    let base = explicit_symmetric_base();
    for provenance in [
        "start-cap",
        "profile-x-lower",
        "profile-x-upper",
        "profile-y-lower",
        "profile-y-upper",
    ] {
        assert_invalid(CadAuthoredSketch::circle_on_face(
            base.face_handle(provenance).unwrap(),
            [0.02, 0.0],
            0.008,
        ));
    }

    let cut = compatibility_v2_authority();
    assert_invalid(CadAuthoredSketch::circle_on_face(
        cut.face_handle("end-cap").unwrap(),
        [0.02, 0.0],
        0.008,
    ));

    let foreign = CadAuthoredGraph::new(
        ConstrainedRectangleV1::new((-0.05, 0.05), (-0.025, 0.025), 0.0).unwrap(),
        0.02,
        1.0e-10,
    )
    .unwrap();
    let foreign_circle = CadAuthoredSketch::circle_on_face(
        foreign.face_handle("end-cap").unwrap(),
        [0.02, 0.0],
        0.008,
    )
    .unwrap();
    assert_invalid(base.through_cut(&foreign_circle, 1.0e-9));

    let original_circle =
        CadAuthoredSketch::circle_on_face(base.face_handle("end-cap").unwrap(), [0.02, 0.0], 0.008)
            .unwrap();
    let changed_tolerance = CadAuthoredSketch::rectangle_xy(symmetric_rectangle(), 2.0e-10)
        .unwrap()
        .extrude_positive_z(0.02)
        .unwrap();
    assert_invalid(changed_tolerance.through_cut(&original_circle, 1.0e-9));

    let changed_depth = CadAuthoredSketch::rectangle_xy(symmetric_rectangle(), 1.0e-10)
        .unwrap()
        .extrude_positive_z(0.03)
        .unwrap();
    assert_invalid(changed_depth.through_cut(&original_circle, 1.0e-9));
}

#[test]
fn signed_clearance_uses_the_strict_asymmetric_predicate() {
    let base = explicit_symmetric_base();
    for center in [[0.10, 0.0], [0.0335, 0.0]] {
        let circle =
            CadAuthoredSketch::circle_on_face(base.face_handle("end-cap").unwrap(), center, 0.008)
                .unwrap();
        assert_invalid(base.through_cut(&circle, 1.0e-9));
    }

    let tiny_rectangle = ConstrainedRectangleV1::new((0.0, 4.0e-9), (0.0, 4.0e-9), 0.0).unwrap();
    let exact_boundary = CadAuthoredSketch::rectangle_xy(tiny_rectangle, 1.0e-10)
        .unwrap()
        .extrude_positive_z(1.0)
        .unwrap();
    let equality = CadAuthoredSketch::circle_on_face(
        exact_boundary.face_handle("end-cap").unwrap(),
        [2.0e-9, 2.0e-9],
        1.0e-9,
    )
    .unwrap();
    assert_invalid(exact_boundary.through_cut(&equality, 1.0e-9));

    let admitted = CadAuthoredSketch::circle_on_face(
        exact_boundary.face_handle("end-cap").unwrap(),
        [2.0e-9, 2.0e-9],
        0.5e-9,
    )
    .unwrap();
    let explicit = exact_boundary.through_cut(&admitted, 1.0e-9).unwrap();
    let compatibility = CadAuthoredGraph::new(tiny_rectangle, 1.0, 1.0e-10)
        .unwrap()
        .circular_through_cut([2.0e-9, 2.0e-9], 0.5e-9, 1.0e-9)
        .unwrap();
    assert_route_equivalence(&explicit, &compatibility);
}

#[test]
fn closed_operation_order_rejects_atomically() {
    let base = explicit_symmetric_base();
    let base_before_rejection = base.clone();
    let rectangle = CadAuthoredSketch::rectangle_xy(symmetric_rectangle(), 1.0e-10).unwrap();
    assert_invalid(base.through_cut(&rectangle, 1.0e-9));
    assert_eq!(base, base_before_rejection);

    let circle =
        CadAuthoredSketch::circle_on_face(base.face_handle("end-cap").unwrap(), [0.02, 0.0], 0.008)
            .unwrap();
    let circle_before_rejection = circle.clone();
    assert_invalid(circle.extrude_positive_z(0.02));
    assert_eq!(circle, circle_before_rejection);

    let once_cut = base.through_cut(&circle, 1.0e-9).unwrap();
    let once_cut_before_rejection = once_cut.clone();
    assert_invalid(once_cut.through_cut(&circle, 1.0e-9));
    assert_eq!(once_cut, once_cut_before_rejection);
}

#[test]
fn clone_move_and_inline_argument_construction_preserve_identity() {
    let rectangle = CadAuthoredSketch::rectangle_xy(rectangle_authority(), 1.0e-9).unwrap();
    assert_eq!(rectangle.clone(), rectangle);
    let cloned_rectangle_route = rectangle.clone().extrude_positive_z(4.0).unwrap();
    let moved_rectangle_route = rectangle.extrude_positive_z(4.0).unwrap();
    let inline_rectangle_route = CadAuthoredSketch::rectangle_xy(rectangle_authority(), 1.0e-9)
        .unwrap()
        .extrude_positive_z(4.0)
        .unwrap();
    assert_eq!(cloned_rectangle_route, moved_rectangle_route);
    assert_eq!(moved_rectangle_route, inline_rectangle_route);
    assert_eq!(inline_rectangle_route.canonical_bytes(), V1_WIRE.as_bytes());
    assert_eq!(inline_rectangle_route.digest_bytes(), V1_DIGEST);

    let base = explicit_symmetric_base();
    let circle =
        CadAuthoredSketch::circle_on_face(base.face_handle("end-cap").unwrap(), [0.02, 0.0], 0.008)
            .unwrap();
    assert_eq!(circle.clone(), circle);
    let cloned_cut_route = base.clone().through_cut(&circle.clone(), 1.0e-9).unwrap();
    let moved_cut_route = base.through_cut(&circle, 1.0e-9).unwrap();

    let inline_base = explicit_symmetric_base();
    let inline_cut_route = inline_base
        .through_cut(
            &CadAuthoredSketch::circle_on_face(
                inline_base.face_handle("end-cap").unwrap(),
                [0.02, 0.0],
                0.008,
            )
            .unwrap(),
            1.0e-9,
        )
        .unwrap();
    assert_eq!(cloned_cut_route, moved_cut_route);
    assert_eq!(moved_cut_route, inline_cut_route);
    assert_eq!(inline_cut_route.canonical_bytes(), V2_WIRE.as_bytes());
    assert_eq!(inline_cut_route.digest_bytes(), V2_DIGEST);
}
