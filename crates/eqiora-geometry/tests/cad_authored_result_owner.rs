use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::{CadAuthoredGraph, ConstrainedRectangleV1};

fn rectangle() -> ConstrainedRectangleV1 {
    ConstrainedRectangleV1::new((0.0, 2.2), (0.0, 0.41), 0.0).unwrap()
}

fn assert_invalid<T: std::fmt::Debug>(result: Result<T, Diagnostic>) {
    assert_eq!(result.unwrap_err().code(), codes::INVALID_ARTIFACT);
}

#[test]
fn accepted_build_owns_atomic_classifier_free_planar_result_naming() {
    let predecessor = CadAuthoredGraph::new(rectangle(), 1.0, 1.0e-10).unwrap();
    let end_cap = predecessor.face_handle("end-cap").unwrap();
    let x_lower = predecessor.face_handle("profile-x-lower").unwrap();
    let x_upper = predecessor.face_handle("profile-x-upper").unwrap();
    let y_lower = predecessor.face_handle("profile-y-lower").unwrap();
    let y_upper = predecessor.face_handle("profile-y-upper").unwrap();
    let graph = predecessor
        .circular_through_cut([0.2, 0.2], 0.05, 1.0e-10)
        .unwrap();
    let result = graph.planar_result().unwrap();
    let cylinder = graph.face_handle("cut-wall").unwrap();
    let named = BTreeMap::from([
        ("fluid".to_owned(), vec![end_cap.clone()]),
        ("inlet".to_owned(), vec![x_lower.clone()]),
        ("outlet".to_owned(), vec![x_upper]),
        ("walls".to_owned(), vec![y_lower.clone(), y_upper]),
        ("cylinder".to_owned(), vec![cylinder]),
    ]);

    let geometry = result.with_named_topology(&named).unwrap();
    assert_eq!(geometry.classification_tolerance_m(), None);
    assert_eq!(geometry.entity_set_dimension("fluid"), Some(2));
    assert_eq!(geometry.entity_set_dimension("cylinder"), Some(1));

    let foreign_predecessor = CadAuthoredGraph::new(rectangle(), 1.0, 2.0e-10).unwrap();
    assert_invalid(result.with_named_topology(&BTreeMap::from([(
        "foreign".to_owned(),
        vec![foreign_predecessor.face_handle("profile-x-lower").unwrap()],
    )])));
    assert_invalid(result.with_named_topology(&BTreeMap::from([(
        "stale".to_owned(),
        vec![graph.face_handle("profile-x-lower").unwrap()],
    )])));

    let mut incomplete = named.clone();
    incomplete.remove("cylinder");
    assert_invalid(result.with_named_topology(&incomplete));

    let mut duplicate = named.clone();
    duplicate.get_mut("walls").unwrap().push(x_lower);
    assert_invalid(result.with_named_topology(&duplicate));

    let mut mixed = named;
    mixed.get_mut("walls").unwrap().push(end_cap);
    assert_invalid(result.with_named_topology(&mixed));

    let rectangle_only = CadAuthoredGraph::new(rectangle(), 1.0, 1.0e-10).unwrap();
    assert_invalid(rectangle_only.planar_result());
}
