use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::{ConstrainedRectangleV1, GeometryGraph};

fn rectangle() -> ConstrainedRectangleV1 {
    ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap()
}

fn assert_invalid<T: std::fmt::Debug>(result: Result<T, Diagnostic>) {
    assert_eq!(result.unwrap_err().code(), codes::INVALID_ARTIFACT);
}

#[test]
fn accepted_build_owns_atomic_classifier_free_planar_result_naming() {
    let owner = GeometryGraph::new();
    let rectangle_authority = rectangle();
    let predecessor = owner
        .rectangle_extrusion(
            rectangle_authority.x_bounds_m(),
            rectangle_authority.y_bounds_m(),
            rectangle_authority.plane_z_m(),
            1.0,
            1.0e-10,
        )
        .unwrap();
    let graph = owner
        .circular_through_cut(&predecessor, [0.2, 0.2], 0.05, 1.0e-10)
        .unwrap();
    let end_cap = graph.face_handle("end-cap").unwrap();
    let x_lower = graph.face_handle("profile-x-lower").unwrap();
    let x_upper = graph.face_handle("profile-x-upper").unwrap();
    let y_lower = graph.face_handle("profile-y-lower").unwrap();
    let y_upper = graph.face_handle("profile-y-upper").unwrap();
    let cylinder = graph.face_handle("cut-wall").unwrap();
    let named = BTreeMap::from([
        ("fluid".to_owned(), vec![end_cap.clone()]),
        ("inlet".to_owned(), vec![x_lower.clone()]),
        ("outlet".to_owned(), vec![x_upper]),
        ("walls".to_owned(), vec![y_lower.clone(), y_upper]),
        ("cylinder".to_owned(), vec![cylinder]),
    ]);

    let geometry = owner.build_solid_geometry(&graph, &named).unwrap();
    assert_eq!(geometry.classification_tolerance_m(), None);
    assert_eq!(geometry.entity_set_dimension("fluid"), Some(2));
    assert_eq!(geometry.entity_set_dimension("cylinder"), Some(1));

    let foreign_owner = GeometryGraph::new();
    let rectangle = rectangle();
    let foreign_predecessor = foreign_owner
        .rectangle_extrusion(
            rectangle.x_bounds_m(),
            rectangle.y_bounds_m(),
            rectangle.plane_z_m(),
            1.0,
            1.0e-10,
        )
        .unwrap();
    let foreign_graph = foreign_owner
        .circular_through_cut(&foreign_predecessor, [0.2, 0.2], 0.05, 1.0e-10)
        .unwrap();
    let mut foreign_named = named.clone();
    foreign_named.insert(
        "cylinder".to_owned(),
        vec![foreign_graph.face_handle("cut-wall").unwrap()],
    );
    assert_invalid(owner.build_solid_geometry(&graph, &foreign_named));
    assert_invalid(owner.build_solid_geometry(
        &graph,
        &BTreeMap::from([(
            "foreign".to_owned(),
            vec![foreign_predecessor.face_handle("profile-x-lower").unwrap()],
        )]),
    ));
    assert_invalid(owner.build_solid_geometry(
        &graph,
        &BTreeMap::from([(
            "stale".to_owned(),
            vec![predecessor.face_handle("profile-x-lower").unwrap()],
        )]),
    ));

    let mut incomplete = named.clone();
    incomplete.remove("cylinder");
    assert_invalid(owner.build_solid_geometry(&graph, &incomplete));

    let mut duplicate = named.clone();
    duplicate.get_mut("walls").unwrap().push(x_lower);
    assert_invalid(owner.build_solid_geometry(&graph, &duplicate));

    let mut mixed = named;
    mixed.get_mut("walls").unwrap().push(end_cap);
    assert_invalid(owner.build_solid_geometry(&graph, &mixed));

    assert_invalid(owner.build_solid_geometry(&predecessor, &BTreeMap::new()));
}
