use super::*;
use crate::draft_spatial::DraftBoundarySide;
use crate::{
    ConnectionSyntax, DomainDecl, DomainSyntax, FieldRoleSyntax, Item, PortDecl, PortSyntax,
};

fn voltage_dimension() -> DimExponents {
    DimExponents::from_integers([1, 2, -3, -1, 0, 0, 0]).expect("bounded dimension")
}

fn current_dimension() -> DimExponents {
    DimExponents::from_integers([0, 0, 0, 1, 0, 0, 0]).expect("bounded dimension")
}

#[test]
fn native_draft_rejects_foreign_symbol_even_when_name_matches() {
    let included = DraftField::new(
        "x",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::Variable,
    );
    let foreign = DraftField::new(
        "x",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::Variable,
    );
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
    let state = DraftField::new(
        "x",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::State,
    );
    let rate = DraftParameter::new(
        "rate",
        eqiora_core::ValueLiteral::from_real(
            eqiora_core::ValueType::scalar(
                eqiora_core::ScalarDomain::Real,
                DimExponents::from_integers([0, 0, -1, 0, 0, 0, 0]).expect("bounded dimension"),
            ),
            1.0,
        )
        .unwrap(),
    );
    let initial =
        DraftDeclaration::Initial(vec![state.expression() - DraftExpression::constant(1.0)]);
    let residual = DraftExpression::derivative(&state) + rate.expression() * state.expression();
    let draft = ModelDraft::new(
        "decay",
        [
            state.into(),
            rate.into(),
            initial,
            DraftRelation::continuous("flow", [residual]).into(),
        ],
    )
    .unwrap();

    let native = draft.native_ast();
    assert_eq!(native.model().name(), "decay");
    assert_eq!(native.model().items().len(), 4);
    assert!(native.graph_path(native.model().range()).is_some());
    let Item::Field(field) = &native.model().items()[0] else {
        panic!("state");
    };
    assert_eq!(field.role(), FieldRoleSyntax::State);
    let Item::Parameter(parameter) = &native.model().items()[1] else {
        panic!("parameter");
    };
    assert!(matches!(
        parameter.value().kind(),
        ExprKind::Quantity { value: 1.0, .. }
    ));
    assert!(matches!(native.model().items()[2], Item::Initial(_)));
}

#[test]
fn native_draft_rejects_names_and_numbers_source_could_not_express() {
    let field = DraftField::new(
        "not valid",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::Variable,
    );
    let relation = DraftRelation::continuous(
        "flow",
        [field.expression() + DraftExpression::constant(f64::NAN)],
    );

    let diagnostics = ModelDraft::new(
        "",
        [
            field.into(),
            relation.into(),
            DraftDeclaration::Initial(vec![DraftExpression::constant(f64::INFINITY)]),
        ],
    )
    .unwrap_err();
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
            .any(|diagnostic| diagnostic.message().contains("non-finite"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("numeric literal"))
    );
}

#[test]
fn physical_vocabulary_projects_only_to_existing_source_ast_forms() {
    let electrical = DraftPhysicalDomain::new(
        "electrical",
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, voltage_dimension()),
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, current_dimension()),
    );
    let positive = DraftConservingPort::new("positive", &electrical);
    let negative = DraftConservingPort::new("negative", &electrical);
    let resistance = DraftParameter::new(
        "resistance",
        eqiora_core::ValueLiteral::from_real(
            eqiora_core::ValueType::scalar(
                eqiora_core::ScalarDomain::Real,
                DimExponents::from_integers([1, 2, -3, -2, 0, 0, 0]).expect("bounded dimension"),
            ),
            2.0,
        )
        .unwrap(),
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
    assert_eq!(relation.equations().len(), 2);
    assert!(
        relation
            .equations()
            .iter()
            .map(|equation| equation.left())
            .any(|residual| {
                expression_contains_call(residual, "across")
                    && expression_contains_call(residual, "through")
            })
    );
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
    let declared_domain = DraftPhysicalDomain::new(
        "electrical",
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, voltage_dimension()),
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, current_dimension()),
    );
    let foreign_domain = DraftPhysicalDomain::new(
        "electrical",
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, voltage_dimension()),
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, current_dimension()),
    );
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
    let electrical = DraftPhysicalDomain::new(
        "electrical",
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, voltage_dimension()),
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, current_dimension()),
    );
    let other = DraftPhysicalDomain::new(
        "other",
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, voltage_dimension()),
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, current_dimension()),
    );
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
    let domain = DraftPhysicalDomain::new(
        "shared",
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, voltage_dimension()),
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, current_dimension()),
    );
    let field = DraftField::new(
        "shared",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::Variable,
    );
    let diagnostics = ModelDraft::new("duplicates", [domain.into(), field.into()]).unwrap_err();
    assert!(
        diagnostics[0]
            .message()
            .contains("duplicate declaration `shared`")
    );
}

#[test]
fn anonymous_connection_diagnostic_paths_follow_membership_not_declaration_position() {
    let domain = DraftPhysicalDomain::new(
        "electrical",
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, voltage_dimension()),
        eqiora_core::ValueType::scalar(eqiora_core::ScalarDomain::Real, current_dimension()),
    );
    let terminal = DraftConservingPort::new("terminal", &domain);
    let unrelated = DraftField::new(
        "x",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::Variable,
    );
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
    let field = DraftField::spatial(
        "u",
        &foreign,
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::Variable,
    );
    let diagnostics =
        ModelDraft::new("foreign_scope", [included.into(), field.into()]).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("foreign or omitted Domain `interval`")
    }));
}

#[test]
fn spatial_draft_projects_only_to_existing_source_ast_forms() {
    let interval = DraftSpatialDomain::cartesian_box("interval", [(0.0, 1.0)]);
    let lower = DraftSpatialDomain::boundary("lower", &interval, 0, DraftBoundarySide::Lower);
    let field = DraftField::spatial(
        "u",
        &interval,
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::Variable,
    );
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
    let Item::Field(field) = &native.model().items()[2] else {
        panic!("third item must be a Field");
    };
    assert_eq!(field.domain(), Some("interval"));
    assert_eq!(field.role(), FieldRoleSyntax::Variable);
    let Item::Relation(relation) = &native.model().items()[3] else {
        panic!("fourth item must be a Relation");
    };
    assert_eq!(relation.domain(), Some("interval"));
    assert!(expression_contains_call(
        relation.equations()[0].left(),
        "grad"
    ));
    assert!(expression_contains_call(
        relation.equations()[0].left(),
        "div"
    ));
}

fn expression_contains_call(expression: &Expr, expected: &str) -> bool {
    match expression.kind() {
        ExprKind::Call { callee, arguments } => {
            callee.as_str() == expected
                || arguments
                    .iter()
                    .any(|argument| expression_contains_call(argument, expected))
        }
        ExprKind::Array(values) => values
            .iter()
            .any(|value| expression_contains_call(value, expected)),
        ExprKind::Index { value, index } => {
            expression_contains_call(value, expected) || expression_contains_call(index, expected)
        }
        ExprKind::Unary { value, .. } => expression_contains_call(value, expected),
        ExprKind::Binary { left, right, .. } => {
            expression_contains_call(left, expected) || expression_contains_call(right, expected)
        }
        ExprKind::Number(_)
        | ExprKind::Quantity { .. }
        | ExprKind::Name(_)
        | ExprKind::Path(_)
        | ExprKind::BoundaryPortSelection { .. } => false,
    }
}

#[test]
fn draft_channel_literals_cannot_hide_empty_arrays_or_foreign_symbols() {
    let omitted = DraftField::new(
        "x",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        FieldRoleSyntax::Variable,
    );
    let array = DraftExpression::array([omitted.expression()]).index(0);
    assert!(
        ModelDraft::new(
            "foreign",
            [DraftRelation::continuous("law", [array]).into()]
        )
        .is_err()
    );
    assert!(
        ModelDraft::new(
            "empty",
            [DraftRelation::continuous("law", [DraftExpression::array([])]).into()]
        )
        .is_err()
    );
}
