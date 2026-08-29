use eqiora_geometry::{
    CadRepairDispositionV1, GeometryFaceHandle, GeometryGraph, GeometrySolidOperation,
};

fn graph(depth_m: f64, tolerance_m: f64) -> GeometrySolidOperation {
    GeometryGraph::new()
        .rectangle_extrusion((-2.0, 3.0), (-1.0, 2.0), 0.5, depth_m, tolerance_m)
        .unwrap()
}

const WITNESS_A_CANONICAL: &str = r#"{"schema":"eqiora.cad-authored-operation-graph-envelope/v1","encoding":"eqiora.canonical-json/v1","length_unit":"metre","requested_modeling_tolerance_m":1e-9,"sketch_plane":{"id":"sketch-plane","kind":"xy","z_m":0.5},"profile":{"id":"rectangle-profile","kind":"axis-aligned-rectangle","sketch_plane":"sketch-plane","constraint":"closed-by-construction","x_bounds_m":[-2.0,3.0],"y_bounds_m":[-1.0,2.0]},"face":{"id":"profile-face","kind":"one-closed-loop-face","profile":"rectangle-profile","region_count":1},"extrusion":{"id":"positive-z-extrusion","kind":"positive-z","face":"profile-face","depth_m":4.0,"repair":"none"},"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper","profile-y-lower","profile-y-upper"]}"#;

const WITNESS_A_DIGEST: [u8; 32] = [
    0x91, 0x95, 0x45, 0xf7, 0x01, 0x18, 0x84, 0x0c, 0x04, 0xda, 0x97, 0x15, 0x82, 0x9d, 0xeb, 0x2d,
    0xa9, 0x47, 0x46, 0x0a, 0x51, 0x31, 0x1e, 0xba, 0xbe, 0xc6, 0xa3, 0x40, 0x38, 0xc6, 0x6f, 0x36,
];

#[test]
fn dual_oracle_witness_closes_exact_polyhedron_and_provenance_faces() {
    let owner = GeometryGraph::new();
    let graph = owner
        .rectangle_extrusion((-2.0, 3.0), (-1.0, 2.0), 0.5, 4.0, 1.0e-9)
        .unwrap();

    assert_eq!(graph.canonical_bytes(), WITNESS_A_CANONICAL.as_bytes());
    assert_eq!(graph.canonical_bytes().len(), 731);
    assert_eq!(graph.digest_bytes(), WITNESS_A_DIGEST);

    assert_eq!(
        graph.output().bounds_m(),
        [(-2.0, 3.0), (-1.0, 2.0), (0.5, 4.5)]
    );
    assert_eq!(graph.vertex_count(), Some(8));
    assert_eq!(graph.edge_count(), Some(12));
    assert_eq!(graph.face_count(), 6);
    assert_eq!(graph.closed_shell_count(), 1);
    assert_eq!(graph.body_count(), 1);
    assert_eq!(
        graph.vertex_count().unwrap() as isize - graph.edge_count().unwrap() as isize
            + graph.face_count() as isize,
        2
    );
    assert_eq!(graph.volume_m3(), 60.0);
    assert_eq!(graph.surface_area_m2(), 94.0);
    assert_eq!(graph.repair_disposition(), CadRepairDispositionV1::None);

    let expected = [
        (
            "start-cap",
            [0.5, 0.5, 0.5],
            15.0,
            [0.0, 0.0, -1.0],
            [
                [-2.0, -1.0, 0.5],
                [-2.0, 2.0, 0.5],
                [3.0, 2.0, 0.5],
                [3.0, -1.0, 0.5],
            ],
        ),
        (
            "end-cap",
            [0.5, 0.5, 4.5],
            15.0,
            [0.0, 0.0, 1.0],
            [
                [-2.0, -1.0, 4.5],
                [3.0, -1.0, 4.5],
                [3.0, 2.0, 4.5],
                [-2.0, 2.0, 4.5],
            ],
        ),
        (
            "profile-x-lower",
            [-2.0, 0.5, 2.5],
            12.0,
            [-1.0, 0.0, 0.0],
            [
                [-2.0, -1.0, 0.5],
                [-2.0, -1.0, 4.5],
                [-2.0, 2.0, 4.5],
                [-2.0, 2.0, 0.5],
            ],
        ),
        (
            "profile-x-upper",
            [3.0, 0.5, 2.5],
            12.0,
            [1.0, 0.0, 0.0],
            [
                [3.0, -1.0, 0.5],
                [3.0, 2.0, 0.5],
                [3.0, 2.0, 4.5],
                [3.0, -1.0, 4.5],
            ],
        ),
        (
            "profile-y-lower",
            [0.5, -1.0, 2.5],
            20.0,
            [0.0, -1.0, 0.0],
            [
                [-2.0, -1.0, 0.5],
                [3.0, -1.0, 0.5],
                [3.0, -1.0, 4.5],
                [-2.0, -1.0, 4.5],
            ],
        ),
        (
            "profile-y-upper",
            [0.5, 2.0, 2.5],
            20.0,
            [0.0, 1.0, 0.0],
            [
                [-2.0, 2.0, 0.5],
                [-2.0, 2.0, 4.5],
                [3.0, 2.0, 4.5],
                [3.0, 2.0, 0.5],
            ],
        ),
    ];

    assert_eq!(
        graph
            .face_handles()
            .unwrap()
            .iter()
            .map(GeometryFaceHandle::provenance_key)
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(provenance_key, ..)| *provenance_key)
            .collect::<Vec<_>>()
    );
    assert!(graph.face_handle("cut-wall").is_err());
    assert!(graph.face_handle("unknown").is_err());

    for (provenance_key, centroid, area, normal, vertices) in expected {
        let handle = graph.face_handle(provenance_key).unwrap();
        let replayed_handle = owner
            .decode_face_handle(&graph, handle.canonical_bytes())
            .unwrap();
        assert_eq!(replayed_handle, handle);
        assert_eq!(handle.provenance_key(), provenance_key);
        assert_eq!(
            graph.resolve_face(&replayed_handle).unwrap(),
            provenance_key
        );
        assert_eq!(
            graph.rectangular_face_centroid_m(&replayed_handle).unwrap(),
            Some(centroid)
        );
        assert_eq!(graph.face_area_m2(&replayed_handle).unwrap(), area);
        assert_eq!(
            graph.planar_face_outward_normal(&replayed_handle).unwrap(),
            Some(normal)
        );
        assert_eq!(
            graph.rectangular_face_vertices_m(&replayed_handle).unwrap(),
            Some(vertices)
        );
    }

    let replayed = GeometryGraph::new()
        .decode_solid(graph.canonical_bytes())
        .unwrap();
    assert_eq!(replayed, graph);
}

#[test]
fn tolerance_changes_identity_not_geometry_and_handles_never_rebind() {
    let first = graph(4.0, 1.0e-9);
    let changed_tolerance = graph(4.0, 2.0e-9);
    let changed_depth = graph(5.0, 1.0e-9);
    let handle = first.face_handle("profile-x-lower").unwrap();

    assert_eq!(first.output(), changed_tolerance.output());
    assert_eq!(first.vertices_m(), changed_tolerance.vertices_m());
    assert_eq!(first.volume_m3(), changed_tolerance.volume_m3());
    assert_eq!(first.surface_area_m2(), changed_tolerance.surface_area_m2());
    assert_ne!(first.digest_bytes(), changed_tolerance.digest_bytes());
    assert!(changed_tolerance.resolve_face(&handle).is_err());

    assert_ne!(first.output(), changed_depth.output());
    assert_ne!(first.digest_bytes(), changed_depth.digest_bytes());
    assert!(changed_depth.resolve_face(&handle).is_err());
}

#[test]
fn member_order_is_nonsemantic_but_wire_vocabulary_is_closed() {
    let expected = graph(4.0, 1.0e-9);
    let permuted = br#"{"selections":["start-cap","end-cap","profile-x-lower","profile-x-upper","profile-y-lower","profile-y-upper"],"extrusion":{"repair":"none","depth_m":4.0,"face":"profile-face","kind":"positive-z","id":"positive-z-extrusion"},"face":{"region_count":1,"profile":"rectangle-profile","kind":"one-closed-loop-face","id":"profile-face"},"profile":{"y_bounds_m":[-1.0,2.0],"x_bounds_m":[-2.0,3.0],"constraint":"closed-by-construction","sketch_plane":"sketch-plane","kind":"axis-aligned-rectangle","id":"rectangle-profile"},"sketch_plane":{"z_m":0.5,"kind":"xy","id":"sketch-plane"},"requested_modeling_tolerance_m":1e-9,"length_unit":"metre","encoding":"eqiora.canonical-json/v1","schema":"eqiora.cad-authored-operation-graph-envelope/v1"}"#;
    let decoder = GeometryGraph::new();
    let decoded = decoder.decode_solid(permuted).unwrap();
    assert_eq!(decoded, expected);
    assert_eq!(decoded.canonical_bytes(), expected.canonical_bytes());

    let canonical = String::from_utf8(expected.canonical_bytes().to_vec()).unwrap();
    for mutant in [
        canonical.replace("\"length_unit\":", "\"unknown\":0,\"length_unit\":"),
        canonical.replace(
            "\"length_unit\":",
            "\"length_unit\":\"metre\",\"length_unit\":",
        ),
        canonical.replace("\"positive-z\"", "\"negative-z\""),
        canonical.replace(
            "\"profile-face\",\"depth_m\"",
            "\"foreign-face\",\"depth_m\"",
        ),
        canonical.replace(
            "\"profile-y-upper\"]",
            "\"profile-y-lower\",\"profile-y-upper\"]",
        ),
        canonical.replace("\"region_count\":1", "\"region_count\":2"),
        canonical.replace("\"repair\":\"none\"", "\"repair\":\"healed\""),
    ] {
        assert!(
            decoder.decode_solid(mutant.as_bytes()).is_err(),
            "wire mutant must reject: {mutant}"
        );
    }
}

#[test]
fn invalid_scalars_and_signed_zero_fail_or_canonicalize_as_contract_requires() {
    let graph = GeometryGraph::new();
    for (depth, tolerance) in [
        (0.0, 1.0e-9),
        (-1.0, 1.0e-9),
        (f64::NAN, 1.0e-9),
        (1.0, 0.0),
        (1.0, -1.0),
        (1.0, f64::INFINITY),
    ] {
        assert!(
            graph
                .rectangle_extrusion((0.0, 1.0), (0.0, 1.0), 0.0, depth, tolerance)
                .is_err()
        );
    }
    assert!(
        graph
            .rectangle_extrusion((1.0, 1.0), (0.0, 1.0), 0.0, 1.0, 1.0e-9)
            .is_err()
    );
    assert!(
        graph
            .rectangle_extrusion((0.0, 1.0), (2.0, 1.0), 0.0, 1.0, 1.0e-9)
            .is_err()
    );

    let positive = graph
        .rectangle_extrusion((0.0, 1.0), (0.0, 1.0), 0.0, 1.0, 1.0e-9)
        .unwrap();
    let negative = graph
        .rectangle_extrusion((-0.0, 1.0), (-0.0, 1.0), -0.0, 1.0, 1.0e-9)
        .unwrap();
    assert_eq!(negative.canonical_bytes(), positive.canonical_bytes());
    assert_eq!(negative.digest_bytes(), positive.digest_bytes());
}
