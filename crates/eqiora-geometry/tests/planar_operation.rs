use std::collections::BTreeMap;

use eqiora_geometry::{
    CanonicalGeometryRef, CanonicalGeometryV1, EDGE_DIMENSION, FACE_DIMENSION,
    PlanarOperationGraph, PlanarTopologyHandle,
};

fn named(
    entries: impl IntoIterator<Item = (&'static str, Vec<PlanarTopologyHandle>)>,
) -> BTreeMap<String, Vec<PlanarTopologyHandle>> {
    entries
        .into_iter()
        .map(|(name, handles)| (name.to_owned(), handles))
        .collect()
}

#[test]
fn rectangle_publishes_direct_source_owned_topology() {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 2.2], [0.0, 0.41]).unwrap();
    let boundaries = rectangle.boundaries();
    assert_eq!(boundaries.len(), 4);

    let geometry = graph
        .build(
            &rectangle,
            &named([
                ("domain", vec![rectangle.region().into()]),
                ("left", vec![boundaries[0].into()]),
                ("right", vec![boundaries[1].into()]),
                ("bottom", vec![boundaries[2].into()]),
                ("top", vec![boundaries[3].into()]),
            ]),
        )
        .unwrap();

    assert_eq!(
        geometry.planar_rectangle_bounds(),
        Some(&[[0.0, 2.2], [0.0, 0.41]])
    );
    assert_eq!(geometry.classification_tolerance_m(), None);
    assert_eq!(
        geometry.entity_set_dimension("domain"),
        Some(FACE_DIMENSION)
    );
    assert_eq!(geometry.entity_set_dimension("left"), Some(EDGE_DIMENSION));
    let reference = CanonicalGeometryRef::from(&geometry);
    assert_eq!(
        ["left", "right", "bottom", "top"].map(|name| {
            reference
                .constant_parent_outward_normal(name)
                .expect("direct boundary handle names one canonical rectangle side")
        }),
        [[-1.0, 0.0], [1.0, 0.0], [0.0, -1.0], [0.0, 1.0]]
    );
    assert_eq!(reference.constant_parent_outward_normal("domain"), None);
    assert_eq!(
        CanonicalGeometryV1::decode_planar_rectangle_v2_canonical(
            geometry.canonical_bytes(),
            Default::default(),
        )
        .unwrap(),
        geometry
    );

    let canonical = std::str::from_utf8(geometry.canonical_bytes()).unwrap();
    let out_of_range = canonical.replacen(
        r#""name":"left","dimension":1,"members":[0]"#,
        r#""name":"left","dimension":1,"members":[4]"#,
        1,
    );
    assert_ne!(out_of_range, canonical);
    assert!(
        CanonicalGeometryV1::decode_planar_rectangle_v2_canonical(
            out_of_range.as_bytes(),
            Default::default(),
        )
        .is_err()
    );
}

#[test]
fn subtract_projects_predecessor_handles_without_names_or_coordinates() {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 2.2], [0.0, 0.41]).unwrap();
    let circle = graph.circle([0.2, 0.2], 0.05).unwrap();
    let result = graph.subtract(&rectangle, &circle).unwrap();
    let outer = rectangle.boundaries();
    let cut = circle.boundaries();

    let geometry = graph
        .build(
            &result,
            &named([
                ("fluid", vec![result.region().into()]),
                ("inlet", vec![outer[0].into()]),
                ("outlet", vec![outer[1].into()]),
                ("walls", vec![outer[2].into(), outer[3].into()]),
                ("cylinder", vec![cut[0].into()]),
            ]),
        )
        .unwrap();
    assert_eq!(geometry.circular_hole_center(), Some([0.2, 0.2]));
    assert_eq!(geometry.circular_hole_radius_m(), Some(0.05));

    let result_boundaries = result.boundaries();
    let direct = graph
        .build(
            &result,
            &named([
                ("fluid", vec![result.region().into()]),
                ("inlet", vec![result_boundaries[0].into()]),
                ("outlet", vec![result_boundaries[1].into()]),
                (
                    "walls",
                    vec![result_boundaries[2].into(), result_boundaries[3].into()],
                ),
                ("cylinder", vec![result_boundaries[4].into()]),
            ]),
        )
        .unwrap();
    assert_eq!(direct, geometry);
}

#[test]
fn foreign_deleted_stale_incomplete_and_mixed_handles_fail_closed() {
    let graph = PlanarOperationGraph::new();
    let rectangle = graph.rectangle([0.0, 2.2], [0.0, 0.41]).unwrap();
    let circle = graph.circle([0.2, 0.2], 0.05).unwrap();
    let result = graph.subtract(&rectangle, &circle).unwrap();
    let outer = rectangle.boundaries();
    let cut = circle.boundaries();

    let foreign_graph = PlanarOperationGraph::new();
    let foreign = foreign_graph.rectangle([0.0, 2.2], [0.0, 0.41]).unwrap();
    let stale = graph.rectangle([0.0, 1.0], [0.0, 1.0]).unwrap();

    for invalid_mapping in [
        named([
            ("fluid", vec![result.region().into()]),
            ("inlet", vec![foreign.boundaries()[0].into()]),
            ("outlet", vec![outer[1].into()]),
            ("walls", vec![outer[2].into(), outer[3].into()]),
            ("cylinder", vec![cut[0].into()]),
        ]),
        named([
            ("fluid", vec![rectangle.region().into()]),
            ("inlet", vec![outer[0].into()]),
            ("outlet", vec![outer[1].into()]),
            ("walls", vec![outer[2].into(), outer[3].into()]),
            ("cylinder", vec![cut[0].into()]),
        ]),
        named([
            ("fluid", vec![result.region().into()]),
            ("inlet", vec![stale.boundaries()[0].into()]),
            ("outlet", vec![outer[1].into()]),
            ("walls", vec![outer[2].into(), outer[3].into()]),
            ("cylinder", vec![cut[0].into()]),
        ]),
        named([
            ("fluid", vec![result.region().into()]),
            ("outer", vec![outer[0].into(), outer[1].into()]),
            ("cylinder", vec![cut[0].into()]),
        ]),
        named([
            ("mixed", vec![result.region().into(), outer[0].into()]),
            ("outlet", vec![outer[1].into()]),
            ("walls", vec![outer[2].into(), outer[3].into()]),
            ("cylinder", vec![cut[0].into()]),
        ]),
    ] {
        assert!(graph.build(&result, &invalid_mapping).is_err());
    }

    assert!(graph.circle([f64::NAN, 0.0], 1.0).is_err());
    let tangent = graph.circle([0.05, 0.2], 0.05).unwrap();
    assert!(graph.subtract(&rectangle, &tangent).is_err());
}
