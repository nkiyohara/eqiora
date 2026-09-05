use super::*;
use crate::draft_spatial::DraftBoundarySide;

fn voltage_dimension() -> DimExponents {
    DimExponents {
        mass: 1,
        length: 2,
        time: -3,
        current: -1,
        ..DimExponents::DIMENSIONLESS
    }
}

fn current_dimension() -> DimExponents {
    DimExponents {
        current: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

#[test]
fn native_draft_rejects_foreign_symbol_even_when_name_matches() {
    let included = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let foreign = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let relation = DraftRelation::continuous("flow", [foreign.expression()]);

    let diagnostic = ModelDraft::new("decay", [included.into(), relation.into()]).unwrap_err();
    assert_eq!(diagnostic[0].code(), codes::LANGUAGE_TYPE_ERROR);
    assert_eq!(
        diagnostic[0].graph_path().unwrap().to_string(),
        "decay.flow"
    );
}

#[test]
fn typed_dimensions_and_expression_references_become_source_ast() {
    let state = DraftField::new("x", DimExponents::DIMENSIONLESS, 1.0);
    let rate = DraftParameter::new(
        "rate",
        DimExponents {
            time: -1,
            ..DimExponents::DIMENSIONLESS
        },
        1.0,
    );
    let residual = DraftExpression::derivative(&state) + rate.expression() * state.expression();
    let draft = ModelDraft::new(
        "decay",
        [
            state.into(),
            rate.into(),
            DraftRelation::continuous("flow", [residual]).into(),
        ],
    )
    .unwrap();

    let native = draft.native_ast();
    assert_eq!(native.model().name(), "decay");
    assert_eq!(native.model().items().len(), 3);
    assert!(native.graph_path(native.model().range()).is_some());
}

#[test]
fn native_draft_rejects_names_and_numbers_source_could_not_express() {
    let field = DraftField::new("not valid", DimExponents::DIMENSIONLESS, f64::INFINITY);
    let relation = DraftRelation::continuous(
        "flow",
        [field.expression() + DraftExpression::constant(f64::NAN)],
    );

    let diagnostics = ModelDraft::new("", [field.into(), relation.into()]).unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("model name"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("declaration name"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("must be finite"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("numeric literal"))
    );
}

#[test]
fn physical_vocabulary_projects_only_to_existing_source_ast_forms() {
    let electrical =
        DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
    let positive = DraftConservingPort::new("positive", &electrical);
    let negative = DraftConservingPort::new("negative", &electrical);
    let resistance = DraftParameter::new(
        "resistance",
        DimExponents {
            mass: 1,
            length: 2,
            time: -3,
            current: -2,
            ..DimExponents::DIMENSIONLESS
        },
        2.0,
    );
    let relation = DraftRelation::continuous(
        "resistor",
        [
            DraftExpression::across(&positive)
                - DraftExpression::across(&negative)
                - resistance.expression() * DraftExpression::through(&positive),
            DraftExpression::through(&positive) + DraftExpression::through(&negative),
        ],
    );
    let connection = DraftConservingConnection::new([&positive, &negative]);
    let draft = ModelDraft::new(
        "resistor",
        [
            electrical.into(),
            positive.into(),
            negative.into(),
            resistance.into(),
            relation.into(),
            connection.into(),
        ],
    )
    .unwrap();

    let native = draft.native_ast();
    let items = native.model().items();
    assert!(matches!(
        items[0],
        Item::Domain(DomainDecl {
            syntax: DomainSyntax::ScalarPhysical { .. },
            ..
        })
    ));
    assert!(matches!(
        items[1],
        Item::Port(PortDecl {
            syntax: PortSyntax::ScalarPhysical { .. },
            ..
        })
    ));
    let Item::Relation(relation) = &items[4] else {
        panic!("fifth item must be a Relation");
    };
    assert_eq!(relation.residuals().len(), 2);
    assert!(relation.residuals().iter().any(|residual| {
        expression_contains_call(residual, "across")
            && expression_contains_call(residual, "through")
    }));
    let Item::Connection(connection) = &items[5] else {
        panic!("sixth item must be a Connection");
    };
    assert_eq!(connection.syntax(), ConnectionSyntax::Conserving);
    assert_eq!(connection.port_paths().len(), 2);
    assert_eq!(connection.port_paths()[0].as_str(), "positive");
    assert_eq!(connection.port_paths()[1].as_str(), "negative");
    assert!(native.graph_path(connection.range()).is_some());
}

#[test]
fn draft_closure_rejects_foreign_domain_and_port_identity_before_rebinding_names() {
    let declared_domain =
        DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
    let foreign_domain =
        DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
    let declared_port = DraftConservingPort::new("terminal", &declared_domain);
    let foreign_domain_port = DraftConservingPort::new("foreign_domain", &foreign_domain);
    let foreign_port = DraftConservingPort::new("terminal", &declared_domain);
    let relation = DraftRelation::continuous("owner", [DraftExpression::across(&foreign_port)]);

    let diagnostics = ModelDraft::new(
        "identity",
        [
            declared_domain.into(),
            declared_port.into(),
            foreign_domain_port.into(),
            relation.into(),
        ],
    )
    .unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("foreign or omitted scalar physical Domain")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("foreign or omitted conserving Port `terminal`")
    }));
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.source_span().is_none())
    );
}

#[test]
fn draft_closure_rejects_invalid_connection_membership_atomically() {
    let electrical =
        DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
    let other = DraftPhysicalDomain::new("other", voltage_dimension(), current_dimension());
    let a = DraftConservingPort::new("a", &electrical);
    let b = DraftConservingPort::new("b", &electrical);
    let incompatible = DraftConservingPort::new("incompatible", &other);
    let foreign = DraftConservingPort::new("a", &electrical);
    let diagnostics = ModelDraft::new(
        "invalid_connections",
        [
            electrical.into(),
            other.into(),
            a.clone().into(),
            b.clone().into(),
            incompatible.clone().into(),
            DraftConservingConnection::new([&a]).into(),
            DraftConservingConnection::new([&a, &b, &b]).into(),
            DraftConservingConnection::new([&incompatible, &b, &foreign]).into(),
        ],
    )
    .unwrap_err();

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("at least two Ports"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("repeats Port `b`"))
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("already belongs to another Connection")
    }));
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("foreign or omitted Port `a`"))
    );
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("exact same draft-local scalar physical Domain")
    }));
}

#[test]
fn duplicate_names_are_rejected_across_physical_and_scalar_declarations() {
    let domain = DraftPhysicalDomain::new("shared", voltage_dimension(), current_dimension());
    let field = DraftField::new("shared", DimExponents::DIMENSIONLESS, 0.0);
    let diagnostics = ModelDraft::new("duplicates", [domain.into(), field.into()]).unwrap_err();
    assert!(
        diagnostics[0]
            .message()
            .contains("duplicate declaration `shared`")
    );
}

#[test]
fn anonymous_connection_diagnostic_paths_follow_membership_not_declaration_position() {
    let domain = DraftPhysicalDomain::new("electrical", voltage_dimension(), current_dimension());
    let terminal = DraftConservingPort::new("terminal", &domain);
    let unrelated = DraftField::new("x", DimExponents::DIMENSIONLESS, 0.0);
    let connection = DraftConservingConnection::new([&terminal]);
    let forward = ModelDraft::new(
        "stable_path",
        [
            domain.clone().into(),
            terminal.clone().into(),
            connection.clone().into(),
            unrelated.clone().into(),
        ],
    )
    .unwrap_err();
    let reordered = ModelDraft::new(
        "stable_path",
        [
            unrelated.into(),
            connection.into(),
            terminal.into(),
            domain.into(),
        ],
    )
    .unwrap_err();

    assert_eq!(
        forward[0].graph_path().unwrap(),
        reordered[0].graph_path().unwrap()
    );
    assert_eq!(
        forward[0].graph_path().unwrap().to_string(),
        "stable_path.connection[terminal]"
    );
}

#[test]
fn spatial_draft_retains_exact_scope_identity_before_ast_projection() {
    let included = DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]);
    let foreign = DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]);
    let included_space = DraftRepresentation::continuum("space");
    let foreign_space = DraftRepresentation::continuum("space");
    let field = DraftField::spatial_scalar(
        "u",
        &foreign,
        &foreign_space,
        DimExponents::DIMENSIONLESS,
        0.0,
    );
    let diagnostics = ModelDraft::new(
        "foreign_scope",
        [included.into(), included_space.into(), field.into()],
    )
    .unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("foreign or omitted Domain `interval`")
    }));
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("foreign or omitted Representation `space`")
    }));
}

#[test]
fn spatial_draft_projects_only_to_existing_source_ast_forms() {
    let interval = DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]);
    let lower = DraftSpatialDomain::boundary("lower", &interval, 0, DraftBoundarySide::Lower);
    let space = DraftRepresentation::continuum("space");
    let field =
        DraftField::spatial_scalar("u", &interval, &space, DimExponents::DIMENSIONLESS, 0.0);
    let balance = DraftRelation::continuous_on(
        "balance",
        &interval,
        [-DraftExpression::divergence(DraftExpression::gradient(
            field.expression(),
        ))],
    );
    let boundary = DraftRelation::continuous_on(
        "lower_value",
        &lower,
        [DraftExpression::trace(field.expression())],
    );
    let draft = ModelDraft::new(
        "poisson",
        [
            interval.into(),
            lower.into(),
            space.into(),
            field.into(),
            balance.into(),
            boundary.into(),
        ],
    )
    .unwrap();

    let native = draft.native_ast();
    assert!(matches!(
        native.model().items()[0],
        Item::Domain(DomainDecl {
            syntax: DomainSyntax::CartesianBox(_),
            ..
        })
    ));
    assert!(matches!(
        native.model().items()[1],
        Item::Domain(DomainDecl {
            syntax: DomainSyntax::Boundary { .. },
            ..
        })
    ));
    assert!(matches!(
        native.model().items()[2],
        Item::Representation(RepresentationDecl {
            syntax: RepresentationSyntax::Continuum,
            ..
        })
    ));
    let Item::Relation(relation) = &native.model().items()[4] else {
        panic!("fifth item must be a Relation");
    };
    assert_eq!(relation.domain(), Some("interval"));
    assert!(expression_contains_call(&relation.residuals()[0], "grad"));
    assert!(expression_contains_call(&relation.residuals()[0], "div"));
}

fn expression_contains_call(expression: &Expr, expected: &str) -> bool {
    match expression.kind() {
        ExprKind::Call { callee, arguments } => {
            callee.as_str() == expected
                || arguments
                    .iter()
                    .any(|argument| expression_contains_call(argument, expected))
        }
        ExprKind::Unary { value, .. } => expression_contains_call(value, expected),
        ExprKind::Binary { left, right, .. } => {
            expression_contains_call(left, expected) || expression_contains_call(right, expected)
        }
        ExprKind::Number(_)
        | ExprKind::Name(_)
        | ExprKind::Path(_)
        | ExprKind::BoundaryPortSelection { .. } => false,
    }
}
