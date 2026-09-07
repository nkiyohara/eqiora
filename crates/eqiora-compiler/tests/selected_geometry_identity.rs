use std::collections::BTreeMap;

use eqiora_compiler::{CompiledModel, StaticBindingValue};
use eqiora_geometry::{CanonicalGeometryV1, GeometryGraph};
use eqiora_graph::{EdgeKind, Op};
use eqiora_schema::kernel::KernelNode;

fn rectangle(extra_name: bool) -> CanonicalGeometryV1 {
    let graph = GeometryGraph::new();
    let region = graph.rectangle([0., 2.], [0., 1.]).unwrap();
    let names = BTreeMap::from([
        ("body".to_owned(), vec![region.region().into()]),
        ("left".to_owned(), vec![region.boundaries()[0].into()]),
        ("right".to_owned(), vec![region.boundaries()[1].into()]),
        (
            (if extra_name { "other_bottom" } else { "bottom" }).to_owned(),
            vec![region.boundaries()[2].into()],
        ),
        ("top".to_owned(), vec![region.boundaries()[3].into()]),
    ]);
    graph.build(&region, &names).unwrap()
}

fn selection<'a>(
    geometry: &'a CanonicalGeometryV1,
    name: &str,
    parent: Option<&str>,
) -> StaticBindingValue<'a> {
    StaticBindingValue::GeometrySupport {
        geometry,
        selection: geometry.entity_set(name).unwrap(),
        parent: parent.map(|name| geometry.entity_set(name).unwrap()),
    }
}

#[test]
fn repeated_geometry_supports_share_domains_and_retain_both_names() {
    let geometry = rectangle(false);
    for owner in ["model", "public component"] {
        let source = format!(
            "{owner} M(support a:volume(ambient_dimension=2),support b:volume(ambient_dimension=2),support left_a:boundary(parent=a),support left_b:boundary(parent=b)) {{variable x:1 on a;variable y:1 on b;relation first on a {{x=0;}} relation second on b {{y=0;}} relation boundary on left_b {{trace(y)=0;}}}}"
        );
        let compiled = CompiledModel::compile_selected(
            "aliases.eqi",
            &source,
            "M",
            &[
                ("a", selection(&geometry, "body", None)),
                ("b", selection(&geometry, "body", None)),
                ("left_a", selection(&geometry, "left", Some("body"))),
                ("left_b", selection(&geometry, "left", Some("body"))),
            ],
        )
        .unwrap_or_else(|errors| panic!("{errors:?}"));
        let symbols = compiled.symbols();
        let body = symbols.get("a").unwrap();
        let boundary = symbols.get("left_a").unwrap();
        assert_eq!(symbols.get("b"), Some(body));
        assert_eq!(symbols.get("left_b"), Some(boundary));
        assert_ne!(body, boundary);
        assert_eq!(
            compiled
                .transaction()
                .ops()
                .iter()
                .filter(|op| matches!(
                    op,
                    Op::DefineKernelNode {
                        node: KernelNode::Domain(_)
                    }
                ))
                .count(),
            2
        );
        assert!(compiled.transaction().ops().iter().any(|op| matches!(op, Op::Connect { from, to, edge: EdgeKind::BoundaryOf } if *from == boundary && *to == body)));
    }
}

#[test]
fn equal_bounds_and_names_in_distinct_geometries_remain_distinct() {
    let first = rectangle(false);
    let second = rectangle(true);
    assert_ne!(first.digest_bytes(), second.digest_bytes());
    let source = "model M(support a:volume(ambient_dimension=2),support b:volume(ambient_dimension=2)) {variable x:1 on a;variable y:1 on b;relation first on a {x=0;} relation second on b {y=0;}}";
    let compiled = CompiledModel::compile_selected(
        "distinct.eqi",
        source,
        "M",
        &[
            ("a", selection(&first, "body", None)),
            ("b", selection(&second, "body", None)),
        ],
    )
    .unwrap();
    assert_ne!(compiled.symbols().get("a"), compiled.symbols().get("b"));
}

#[test]
fn a_boundary_cannot_borrow_a_parent_from_another_geometry_revision() {
    let first = rectangle(false);
    let second = rectangle(true);
    let source = "model M(support a:volume(ambient_dimension=2),support left:boundary(parent=a)) {variable x:1 on a;relation r on left {trace(x)=0;}}";
    assert!(
        CompiledModel::compile_selected(
            "foreign.eqi",
            source,
            "M",
            &[
                ("a", selection(&first, "body", None)),
                ("left", selection(&second, "left", Some("body"))),
            ]
        )
        .is_err()
    );
}
