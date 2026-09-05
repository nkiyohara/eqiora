use super::*;

const FILE: &str = "boundary-family.eqi";
const RANGE: TextRange = TextRange::new(0, 1);

fn identity(value: u8) -> FullElaborationIdentity {
    FullElaborationIdentity::from_sha256([value; 32])
}

fn path(segments: &[&str]) -> NamePath {
    NamePath::from_segments(segments.iter().copied(), RANGE).expect("valid test path")
}

fn selector(member: &str, target: &str) -> BoundaryPortSelectorSyntax {
    SourceAstFactory::boundary_port_selector(member, target, RANGE).expect("valid test selector")
}

fn reference(
    segments: &[&str],
    selector: Option<BoundaryPortSelectorSyntax>,
) -> BoundaryPortReferenceSyntax {
    SourceAstFactory::boundary_port_reference(path(segments), selector)
        .expect("valid test reference")
}

fn port(name: &str, value: u8) -> FlatSymbol {
    FlatSymbol {
        internal_name: name.to_owned(),
        display_name: name.to_owned(),
        full_identity: identity(value),
        kind: SymbolKind::Port,
    }
}

fn insert_boundary(scope: &mut Scope, name: &str, value: u8) {
    scope.insert_spatial_support(
        name.to_owned(),
        SpatialSupport::Boundary {
            domain: identity(value),
            parent: identity(100),
            dimensions: 2,
        },
    );
}

#[test]
fn boundary_family_selection_is_keyed_by_exact_identity_not_insertion_order() {
    let mut scope = Scope::default();
    insert_boundary(&mut scope, "left", 1);
    insert_boundary(&mut scope, "right", 2);
    scope
        .insert_port_family_member(
            FILE,
            RANGE,
            "mechanical".to_owned(),
            "boundary",
            identity(2),
            port("right_port", 12),
        )
        .expect("right member");
    scope
        .insert_port_family_member(
            FILE,
            RANGE,
            "mechanical".to_owned(),
            "boundary",
            identity(1),
            port("left_port", 11),
        )
        .expect("left member");

    let left = resolve_boundary_port_reference(
        FILE,
        &reference(&["mechanical"], Some(selector("boundary", "left"))),
        &scope,
        None,
    )
    .expect("left selection");
    let right = resolve_boundary_port_reference(
        FILE,
        &reference(&["mechanical"], Some(selector("boundary", "right"))),
        &scope,
        None,
    )
    .expect("right selection");

    assert_eq!(left.internal_name, "left_port");
    assert_eq!(right.internal_name, "right_port");
}

#[test]
fn active_binder_rewrites_selected_expression_to_ordinary_port() {
    let mut scope = Scope::default();
    scope
        .insert_port_family_member(
            FILE,
            RANGE,
            "mechanical".to_owned(),
            "boundary",
            identity(2),
            port("right_port", 12),
        )
        .expect("right member");
    let expression = SourceAstFactory::expression(
        ExprKind::BoundaryPortSelection {
            port: Box::new(path(&["mechanical"])),
            selector: Box::new(selector("boundary", "boundary")),
        },
        RANGE,
    )
    .expect("selected expression");

    let rewritten = rewrite_expression_with_boundary_member(
        FILE,
        &expression,
        &scope,
        Some(ActiveBoundaryMember::new("boundary", identity(2))),
    )
    .expect("active member rewrite");

    assert_eq!(rewritten.name_value(), Some("right_port"));
}

#[test]
fn child_public_family_resolves_through_the_same_exact_identity_index() {
    let mut child_scope = Scope::default();
    child_scope
        .insert_port_family_member(
            FILE,
            RANGE,
            "mechanical".to_owned(),
            "boundary",
            identity(1),
            port("child_left_port", 21),
        )
        .expect("child member");
    let public_families = BTreeMap::from([(
        "mechanical".to_owned(),
        child_scope
            .port_family("mechanical")
            .expect("child family")
            .clone(),
    )]);
    let mut scope = Scope::default();
    insert_boundary(&mut scope, "left", 1);
    scope.insert_child(
        "solid".to_owned(),
        InstanceInterface::with_public_port_families(BTreeMap::new(), public_families),
    );

    let selected = resolve_boundary_port_reference(
        FILE,
        &reference(&["solid", "mechanical"], Some(selector("boundary", "left"))),
        &scope,
        None,
    )
    .expect("public child family selection");

    assert_eq!(selected.internal_name, "child_left_port");
}

#[test]
fn family_selection_fails_closed_for_ambiguous_or_wrong_targets() {
    let mut scope = Scope::default();
    scope
        .insert_port_family_member(
            FILE,
            RANGE,
            "mechanical".to_owned(),
            "boundary",
            identity(1),
            port("left_port", 11),
        )
        .expect("family member");
    scope.insert_spatial_support(
        "body".to_owned(),
        SpatialSupport::Volume {
            domain: identity(100),
            dimensions: 2,
        },
    );
    insert_boundary(&mut scope, "foreign", 3);

    let unselected =
        resolve_boundary_port_reference(FILE, &reference(&["mechanical"], None), &scope, None)
            .expect_err("family must not decay to an ordinary Port");
    assert!(
        unselected
            .message()
            .contains("requires an exact boundary selector")
    );

    let mismatched = resolve_boundary_port_reference(
        FILE,
        &reference(&["mechanical"], Some(selector("face", "foreign"))),
        &scope,
        None,
    )
    .expect_err("selector member mismatch");
    assert!(mismatched.message().contains("does not match"));

    let volume = resolve_boundary_port_reference(
        FILE,
        &reference(&["mechanical"], Some(selector("boundary", "body"))),
        &scope,
        None,
    )
    .expect_err("volume target");
    assert!(
        volume
            .message()
            .contains("is not an exact Boundary support")
    );

    let missing_member = resolve_boundary_port_reference(
        FILE,
        &reference(&["mechanical"], Some(selector("boundary", "foreign"))),
        &scope,
        None,
    )
    .expect_err("exact boundary outside family");
    assert!(
        missing_member
            .message()
            .contains("has no member on exact Boundary")
    );
}
