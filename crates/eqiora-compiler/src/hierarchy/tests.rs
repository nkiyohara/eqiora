use eqiora_graph::{GraphStore, InMemoryGraphStore, Op};
use eqiora_schema::kernel::KernelNode;

use eqiora_core::{DimExponents, DynQuantity, EntityKind};

use crate::identity::{DeclarationPath, ElaborationKey, InstancePath};
use crate::projection::PhysicalExposureContract;
use crate::source_identity::LocalSourceIdentity;

const EXTERNAL_SPATIAL_COMPONENT: &str = r#"
public component BoundaryLaw {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public parameter value: 1;
  representation space = continuum;
  field state on body as space: 1 = 0;
  relation volume_law continuous on body { state - value = 0; }
  relation wall_law continuous on wall { trace(state) = 0; }
}
"#;

fn external_binding() -> crate::external::ExternalComponentBinding {
    let digest = eqiora_schema::kernel::GeometryDigest::new([0x11; 32]);
    crate::external::ExternalComponentBinding::new(
        "BoundModel",
        "BoundaryLaw",
        vec![
            crate::external::ExternalGeometrySupportBinding::region("body", digest, "fluid", 2),
            crate::external::ExternalGeometrySupportBinding::boundary(
                "wall", digest, "walls", "body",
            ),
        ],
        vec![crate::external::ExternalParameterBinding::new(
            "value",
            DynQuantity::new(2.0, DimExponents::DIMENSIONLESS),
        )],
    )
}

#[test]
fn external_geometry_supports_enter_the_ordinary_component_lowerer() {
    use eqiora_schema::kernel::{DomainKind, ExprNode, SymbolRef};

    let compiled = super::compile_external_component(
        "boundary-law.eqi",
        EXTERNAL_SPATIAL_COMPONENT,
        &external_binding(),
    )
    .expect("external supports close one Component occurrence");
    assert!(compiled.symbols().get("body").is_some());
    assert!(compiled.symbols().get("wall").is_some());
    assert!(compiled.symbols().get("definition.state").is_some());
    let parameter = compiled
        .symbols()
        .get("value")
        .expect("external value is retained as a root Parameter");
    assert_eq!(compiled.symbols().get("definition.value"), Some(parameter));

    let mut region = 0;
    let mut boundary = 0;
    let mut parameters = 0;
    let mut parameter_references = Vec::new();
    for operation in compiled.transaction().ops() {
        let Op::DefineKernelNode { node } = operation else {
            continue;
        };
        match node {
            KernelNode::Domain(domain) => match domain.kind() {
                DomainKind::GeometryRegion {
                    geometry,
                    entity_set,
                } => {
                    region += 1;
                    assert_eq!(geometry.bytes(), [0x11; 32]);
                    assert_eq!(entity_set, "fluid");
                }
                DomainKind::GeometryBoundary { entity_set } => {
                    boundary += 1;
                    assert_eq!(entity_set, "walls");
                }
                _ => {}
            },
            KernelNode::Parameter(definition) => {
                parameters += 1;
                assert_eq!(definition.id().erase(), parameter);
                assert_eq!(definition.value().value(), 2.0);
            }
            KernelNode::Relation(relation) => {
                parameter_references.extend(relation.residuals().nodes().iter().filter_map(
                    |node| match node {
                        ExprNode::Symbol(SymbolRef::Parameter(parameter)) => {
                            Some(parameter.erase())
                        }
                        _ => None,
                    },
                ));
            }
            _ => {}
        }
    }
    assert_eq!((region, boundary), (1, 1));
    assert_eq!(parameters, 1);
    assert_eq!(parameter_references, [parameter]);
    let (transaction, _, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store
        .commit(transaction)
        .expect("external occurrence transaction commits atomically");
}

#[test]
fn omitted_external_parameter_default_remains_an_expression_constant() {
    use eqiora_schema::kernel::{ExprNode, SymbolRef};

    let source = EXTERNAL_SPATIAL_COMPONENT.replace(
        "public parameter value: 1;",
        "public parameter value: 1 = 3;",
    );
    let complete = external_binding();
    let binding = crate::external::ExternalComponentBinding::new(
        complete.model(),
        complete.component(),
        complete.supports().to_vec(),
        Vec::new(),
    );
    let compiled = super::compile_external_component("default.eqi", &source, &binding)
        .expect("omitted public default closes the occurrence");
    assert!(compiled.symbols().get("value").is_none());
    assert!(compiled.symbols().get("definition.value").is_none());
    assert!(compiled.transaction().ops().iter().all(|operation| {
        !matches!(
            operation,
            Op::DefineKernelNode {
                node: KernelNode::Parameter(_)
            }
        )
    }));
    assert!(compiled.transaction().ops().iter().all(|operation| {
        let Op::DefineKernelNode {
            node: KernelNode::Relation(relation),
        } = operation
        else {
            return true;
        };
        relation
            .residuals()
            .nodes()
            .iter()
            .all(|node| !matches!(node, ExprNode::Symbol(SymbolRef::Parameter(_))))
    }));
}

#[test]
fn external_geometry_binding_inventory_fails_before_a_transaction_exists() {
    let digest = eqiora_schema::kernel::GeometryDigest::new([0x11; 32]);
    let region =
        || crate::external::ExternalGeometrySupportBinding::region("body", digest, "fluid", 2);
    let boundary = || {
        crate::external::ExternalGeometrySupportBinding::boundary("wall", digest, "walls", "body")
    };
    let parameter = || {
        crate::external::ExternalParameterBinding::new(
            "value",
            DynQuantity::new(2.0, DimExponents::DIMENSIONLESS),
        )
    };
    let mut foreign_boundary = boundary();
    let crate::external::ExternalGeometrySupportBinding::Boundary {
        geometry: digest, ..
    } = &mut foreign_boundary
    else {
        unreachable!()
    };
    *digest = eqiora_schema::kernel::GeometryDigest::new([0x22; 32]);
    let mut duplicate_boundary = boundary();
    let crate::external::ExternalGeometrySupportBinding::Boundary { entity_set, .. } =
        &mut duplicate_boundary
    else {
        unreachable!()
    };
    *entity_set = "fluid".to_owned();
    let cases = [
        (
            crate::external::ExternalComponentBinding::new(
                "Missing",
                "BoundaryLaw",
                vec![region()],
                vec![parameter()],
            ),
            "no binding for required support slot `wall`",
        ),
        (
            crate::external::ExternalComponentBinding::new(
                "Foreign",
                "BoundaryLaw",
                vec![region(), foreign_boundary],
                vec![parameter()],
            ),
            "one exact Geometry identity",
        ),
        (
            crate::external::ExternalComponentBinding::new(
                "Duplicate",
                "BoundaryLaw",
                vec![region(), duplicate_boundary],
                vec![parameter()],
            ),
            "bound to more than one support slot",
        ),
    ];
    for (binding, expected) in cases {
        let diagnostics = super::compile_external_component(
            "boundary-law.eqi",
            EXTERNAL_SPATIAL_COMPONENT,
            &binding,
        )
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected)),
            "missing `{expected}` in {diagnostics:#?}"
        );
    }
}

#[test]
fn external_dimensioned_parameter_failures_are_typed() {
    for (value, expected) in [
        (
            DynQuantity::new(
                2.0,
                DimExponents {
                    length: 1,
                    ..DimExponents::DIMENSIONLESS
                },
            ),
            "has dimension",
        ),
        (
            DynQuantity::new(f64::NAN, DimExponents::DIMENSIONLESS),
            "must have a finite",
        ),
    ] {
        let mut binding = external_binding();
        binding = crate::external::ExternalComponentBinding::new(
            "Rejected",
            "BoundaryLaw",
            binding.supports().to_vec(),
            vec![crate::external::ExternalParameterBinding::new(
                "value", value,
            )],
        );
        let diagnostics = super::compile_external_component(
            "boundary-law.eqi",
            EXTERNAL_SPATIAL_COMPONENT,
            &binding,
        )
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected)),
            "missing `{expected}` in {diagnostics:#?}",
        );
    }
}

#[test]
fn external_occurrence_rejects_closed_binding_and_source_failures() {
    let assert_rejected = |source: &str,
                           binding: crate::external::ExternalComponentBinding,
                           expected: &str| {
        let diagnostics =
            super::compile_external_component("boundary-law.eqi", source, &binding).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected)),
            "missing `{expected}` in {diagnostics:#?}",
        );
    };

    let valid = external_binding();
    assert_rejected("not valid eqiora source", valid.clone(), "expected");
    assert_rejected("model Root {}", valid.clone(), "definitions-only source");
    assert_rejected(
        &EXTERNAL_SPATIAL_COMPONENT.replacen("public component", "component", 1),
        valid.clone(),
        "must be declared public",
    );

    let no_parameters = crate::external::ExternalComponentBinding::new(
        "MissingParameter",
        "BoundaryLaw",
        valid.supports().to_vec(),
        Vec::new(),
    );
    assert_rejected(
        EXTERNAL_SPATIAL_COMPONENT,
        no_parameters,
        "required Parameter `value` has no instance binding",
    );

    let parameter = valid.parameters()[0].clone();
    for (parameters, expected) in [
        (
            vec![parameter.clone(), parameter.clone()],
            "duplicate binding for Parameter `value`",
        ),
        (
            vec![
                parameter.clone(),
                crate::external::ExternalParameterBinding::new(
                    "extra",
                    DynQuantity::new(1.0, DimExponents::DIMENSIONLESS),
                ),
            ],
            "unknown public Parameter `extra`",
        ),
    ] {
        assert_rejected(
            EXTERNAL_SPATIAL_COMPONENT,
            crate::external::ExternalComponentBinding::new(
                "RejectedParameter",
                "BoundaryLaw",
                valid.supports().to_vec(),
                parameters,
            ),
            expected,
        );
    }

    let digest = eqiora_schema::kernel::GeometryDigest::new([0x11; 32]);
    let extra =
        crate::external::ExternalGeometrySupportBinding::boundary("extra", digest, "inlet", "body");
    let mut supports = valid.supports().to_vec();
    supports.push(extra);
    assert_rejected(
        EXTERNAL_SPATIAL_COMPONENT,
        crate::external::ExternalComponentBinding::new(
            "ExtraSupport",
            "BoundaryLaw",
            supports,
            valid.parameters().to_vec(),
        ),
        "unknown support slot `extra`",
    );

    let mut wrong_parent = valid.supports().to_vec();
    let crate::external::ExternalGeometrySupportBinding::Boundary { parent_slot, .. } =
        &mut wrong_parent[1]
    else {
        unreachable!()
    };
    *parent_slot = "absent".to_owned();
    assert_rejected(
        EXTERNAL_SPATIAL_COMPONENT,
        crate::external::ExternalComponentBinding::new(
            "WrongParent",
            "BoundaryLaw",
            wrong_parent,
            valid.parameters().to_vec(),
        ),
        "has no region parent binding `absent`",
    );

    let excessive = " ".repeat(16 * 1_024 * 1_024 + 1);
    assert_rejected(
        &excessive,
        valid,
        "exceeding the 16777216 byte hierarchy limit",
    );
}

const RESISTOR_SOURCE: &str = r#"
connector Pin = scalar_physical(
  across = kg * m ^ 2 / (s ^ 3 * A),
  through = A
);

component Resistor {
  public parameter resistance: kg * m ^ 2 / (s ^ 3 * A ^ 2);
  public port positive: conserving on Pin;
  public port negative: conserving on Pin;

  relation voltage continuous {
    across(positive) - across(negative) - resistance * through(positive) = 0;
    through(positive) + through(negative) = 0;
  }
}

model parallel {
  instance r2: Resistor(resistance = 2);
  instance r4: Resistor(resistance = 4);
  connect conserving r2.positive, r4.positive;
  connect conserving r2.negative, r4.negative;
}
"#;

#[test]
fn elaborates_repeated_component_instances_to_distinct_flat_entities() {
    let mut compiled = crate::compile("parallel.eqi", RESISTOR_SOURCE)
        .expect("bounded component hierarchy compiles");
    assert_eq!(compiled.len(), 1);
    let compiled = compiled.pop().expect("one Model");

    for name in [
        "connector::Pin",
        "r2.positive",
        "r2.negative",
        "r2.voltage",
        "r4.positive",
        "r4.negative",
        "r4.voltage",
    ] {
        assert!(compiled.symbols().get(name).is_some(), "missing `{name}`");
    }
    assert!(
        compiled.symbols().get("r2.resistance").is_none()
            && compiled.symbols().get("r4.resistance").is_none(),
        "literal component arguments are lexical terms, not fabricated Kernel Parameters"
    );
    assert_ne!(
        compiled.symbols().get("r2.positive"),
        compiled.symbols().get("r4.positive")
    );
    let provenance = compiled.provenance().expect("elaboration provenance");
    let r2_positive = compiled.symbols().get("r2.positive").unwrap();
    let r4_positive = compiled.symbols().get("r4.positive").unwrap();
    let r2_source = provenance.get_by_graph_id(r2_positive).unwrap();
    let r4_source = provenance.get_by_graph_id(r4_positive).unwrap();
    assert_eq!(r2_source.definition_span(), r4_source.definition_span());
    assert_ne!(r2_source.instance_span(), r4_source.instance_span());
    assert_eq!(r2_source.binding_spans().len(), 1);
    assert_eq!(r4_source.binding_spans().len(), 1);
    assert_ne!(
        provenance.identity_for_graph_id(r2_positive),
        provenance.identity_for_graph_id(r4_positive)
    );

    let (transaction, _, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store
        .commit(transaction)
        .expect("the complete elaboration commits atomically");
}

#[test]
fn semantic_declaration_order_and_source_location_do_not_change_ids() {
    let reordered = r#"
connector Pin = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
component Resistor {
  public port negative: conserving on Pin;
  relation voltage continuous {
    across(positive) - across(negative) - resistance * through(positive) = 0;
    through(positive) + through(negative) = 0;
  }
  public port positive: conserving on Pin;
  public parameter resistance: kg * m ^ 2 / (s ^ 3 * A ^ 2);
}
model parallel {
  connect conserving r2.negative, r4.negative;
  instance r4: Resistor(resistance = 4);
  connect conserving r4.positive, r2.positive;
  instance r2: Resistor(resistance = 2);
}
"#;
    let mut first = crate::compile("first/location.eqi", RESISTOR_SOURCE).unwrap();
    let mut second = crate::compile("elsewhere.eqi", reordered).unwrap();
    let first = first.pop().unwrap();
    let second = second.pop().unwrap();

    assert_eq!(first.model(), second.model());
    assert_eq!(
        first.symbols().iter().collect::<Vec<_>>(),
        second.symbols().iter().collect::<Vec<_>>()
    );
    assert_eq!(first.transaction().ops(), second.transaction().ops());
    assert_ne!(first.provenance(), second.provenance());
}

#[test]
fn nested_instances_are_elaborated_recursively() {
    let source = r#"
connector Pin = scalar_physical(across = 1, through = 1);
component Leaf {
  public port left: conserving on Pin;
  public port right: conserving on Pin;
  relation law continuous {
    across(left) - across(right) = 0;
    through(left) + through(right) = 0;
  }
}

component Pair {
  instance first: Leaf;
  instance second: Leaf;
  connect conserving first.left, second.left;
  connect conserving first.right, second.right;
}
model nested { instance pair: Pair; }
"#;
    let mut compiled = crate::compile("nested.eqi", source).unwrap();
    let compiled = compiled.pop().unwrap();
    for name in ["pair.first.left", "pair.first.law", "pair.second.right"] {
        assert!(compiled.symbols().get(name).is_some(), "missing `{name}`");
    }
    let (transaction, _, _) = compiled.into_parts();
    InMemoryGraphStore::new().commit(transaction).unwrap();
}

#[test]
fn occurrence_bound_field_is_an_exact_non_materialized_alias() {
    use eqiora_schema::kernel::{ExprNode, SymbolRef};

    let source = r#"
component Law {
  public support body: volume(ambient_dimension = 2);
  public field slot state on body as continuum: K;
  relation balance continuous on body { state = 0; }
}
model Coupled {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field state on body as space: K = 0;
  instance law: Law(support body = body, field state = state);
}
"#;
    let mut compiled = crate::compile("field-alias.eqi", source).expect("exact Field binding");
    let compiled = compiled.pop().expect("one Model");
    let target = compiled.symbols().get("state").expect("one owned Field");
    assert!(compiled.symbols().get("law.state").is_none());

    let mut fields = 0;
    let mut bound_target = None;
    for operation in compiled.transaction().ops() {
        let Op::DefineKernelNode { node } = operation else {
            continue;
        };
        match node {
            KernelNode::Field(_) => fields += 1,
            KernelNode::Relation(relation) => {
                bound_target = relation
                    .residuals()
                    .nodes()
                    .iter()
                    .find_map(|node| match node {
                        ExprNode::Symbol(SymbolRef::Field(field)) => Some(field.erase()),
                        _ => None,
                    });
            }
            _ => {}
        }
    }
    assert_eq!(fields, 1, "the Field slot allocates no second Field");
    assert_eq!(bound_target, Some(target));

    let relation = compiled
        .symbols()
        .get("law.balance")
        .expect("flattened law");
    let provenance = compiled.provenance().expect("source provenance");
    assert_eq!(
        provenance
            .get_by_graph_id(relation)
            .expect("Relation provenance")
            .binding_spans()
            .len(),
        2,
        "support and Field bindings explain the flattened Relation"
    );
}

#[test]
fn occurrence_bound_parameter_is_one_exact_non_materialized_identity() {
    use eqiora_schema::kernel::{ExprNode, SymbolRef};

    let source = r#"
component Law {
  public parameter coefficient: 1;
  relation balance continuous { coefficient - 1 = 0; }
}
model Coupled {
  parameter material: 1 = 2;
  instance first: Law(coefficient = material);
  instance second: Law(coefficient = material);
}
"#;
    let mut compiled =
        crate::compile("parameter-alias.eqi", source).expect("exact Parameter binding");
    let compiled = compiled.pop().expect("one Model");
    let target = compiled
        .symbols()
        .get("material")
        .expect("one owned Parameter");
    assert_eq!(compiled.symbols().get("first.coefficient"), Some(target));
    assert_eq!(compiled.symbols().get("second.coefficient"), Some(target));

    let mut parameters = 0;
    let mut bound_targets = Vec::new();
    for operation in compiled.transaction().ops() {
        let Op::DefineKernelNode { node } = operation else {
            continue;
        };
        match node {
            KernelNode::Parameter(_) => parameters += 1,
            KernelNode::Relation(relation) => {
                bound_targets.extend(relation.residuals().nodes().iter().filter_map(|node| {
                    match node {
                        ExprNode::Symbol(SymbolRef::Parameter(parameter)) => {
                            Some(parameter.erase())
                        }
                        _ => None,
                    }
                }));
            }
            _ => {}
        }
    }
    assert_eq!(parameters, 1, "a component slot allocates no Parameter");
    assert_eq!(bound_targets, [target, target]);
}

#[test]
fn equal_valued_parameters_remain_distinct_through_arithmetic_bindings() {
    use eqiora_schema::kernel::{ExprNode, SymbolRef};

    let source = r#"
component Law {
  public parameter coefficient: 1;
  relation balance continuous { coefficient - 1 = 0; }
}
model Coupled {
  parameter first_material: 1 = 2;
  parameter second_material: 1 = 2;
  instance first: Law(coefficient = 3 * first_material);
  instance second: Law(coefficient = 3 * second_material);
}
"#;
    let mut compiled =
        crate::compile("parameter-arithmetic.eqi", source).expect("typed arithmetic binding");
    let compiled = compiled.pop().expect("one Model");
    let first = compiled.symbols().get("first_material").unwrap();
    let second = compiled.symbols().get("second_material").unwrap();
    assert_ne!(first, second, "identity is not inferred from equal values");
    assert!(compiled.symbols().get("first.coefficient").is_none());
    assert!(compiled.symbols().get("second.coefficient").is_none());

    let relation_parameters = compiled
        .transaction()
        .ops()
        .iter()
        .filter_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => Some(
                relation
                    .residuals()
                    .nodes()
                    .iter()
                    .filter_map(|node| match node {
                        ExprNode::Symbol(SymbolRef::Parameter(parameter)) => {
                            Some(parameter.erase())
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(relation_parameters, [vec![first], vec![second]]);
}

#[test]
fn literal_parameter_bindings_normalize_negative_zero_without_a_direction() {
    let source = |literal: &str| {
        format!(
            "component Law {{ public parameter coefficient: 1; relation balance continuous {{ coefficient = 0; }} }} model Coupled {{ instance law: Law(coefficient = {literal}); }}"
        )
    };
    let mut positive = crate::compile("zero.eqi", &source("0.0")).unwrap();
    let mut negative = crate::compile("zero.eqi", &source("-0.0")).unwrap();
    let positive = positive.remove(0);
    let negative = negative.remove(0);

    assert_eq!(positive.model(), negative.model());
    assert_eq!(positive.transaction().ops(), negative.transaction().ops());
    assert!(positive.symbols().get("law.coefficient").is_none());
    assert!(positive.transaction().ops().iter().all(|operation| {
        !matches!(
            operation,
            Op::DefineKernelNode {
                node: KernelNode::Parameter(_)
            }
        )
    }));
}

#[test]
fn nested_field_forwarding_preserves_target_identity_and_occurrence_chain() {
    use eqiora_schema::kernel::{ExprNode, SymbolRef};

    let source = r#"
component Inner {
  public support body: volume(ambient_dimension = 2);
  public field slot state on body as continuum: K;
  relation balance continuous on body { state = 0; }
}
component Outer {
  public support body: volume(ambient_dimension = 2);
  public field slot state on body as continuum: K;
  instance inner: Inner(support body = body, field state = state);
}
model Coupled {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field state on body as space: K = 0;
  instance outer: Outer(support body = body, field state = state);
}
"#;
    let mut compiled =
        crate::compile("nested-field-alias.eqi", source).expect("nested exact forwarding");
    let compiled = compiled.pop().expect("one Model");
    let target = compiled.symbols().get("state").expect("one owned Field");
    assert!(compiled.symbols().get("outer.state").is_none());
    assert!(compiled.symbols().get("outer.inner.state").is_none());
    let bound_target = compiled
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => relation
                .residuals()
                .nodes()
                .iter()
                .find_map(|node| match node {
                    ExprNode::Symbol(SymbolRef::Field(field)) => Some(field.erase()),
                    _ => None,
                }),
            _ => None,
        });
    assert_eq!(bound_target, Some(target));

    let relation = compiled
        .symbols()
        .get("outer.inner.balance")
        .expect("nested flattened Relation");
    assert_eq!(
        compiled
            .provenance()
            .expect("source provenance")
            .get_by_graph_id(relation)
            .expect("nested Relation provenance")
            .binding_spans()
            .len(),
        4,
        "both occurrence binding sets remain explainable"
    );
}

#[test]
fn field_binding_requires_complete_exact_contract() {
    let base = r#"
component Law {
  public support body: volume(ambient_dimension = 2);
  public field slot state on body as continuum: K shape spatial_vector;
  relation balance continuous on body { state = 0; }
}
model Coupled {
  domain left = box(0, 1, 0, 1);
  domain right = box(1, 2, 0, 1);
  representation space = continuum;
  field state on right as space: K shape spatial_vector;
  instance law: Law(support body = left, field state = state);
}
"#;
    let diagnostics = crate::compile("field-support-mismatch.eqi", base)
        .expect_err("equal ambient dimension cannot replace exact support identity");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("disagree in exact spatial support")
    }));

    let wrong_shape = base
        .replace(
            "field state on right as space: K shape spatial_vector;",
            "field state on right as space: K shape [2];",
        )
        .replace("support body = left", "support body = right");
    let diagnostics = crate::compile("field-shape-mismatch.eqi", &wrong_shape)
        .expect_err("same extents with a different frame are not interchangeable");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("disagree in coordinate frame")
    }));

    let missing = base.replace(", field state = state", "");
    let diagnostics = crate::compile("field-missing.eqi", &missing)
        .expect_err("required Field slot cannot default");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("has no binding for required Field slot `state`")
    }));
}

#[test]
fn transitive_physical_fragments_emit_one_canonical_connection() {
    let nary = r#"
connector Pin = scalar_physical(across = 1, through = 1);
component Terminal {
  public port p: conserving on Pin;
  relation owner continuous { across(p) = 0; }
}
model Network {
  instance a: Terminal;
  instance b: Terminal;
  instance c: Terminal;
  connect conserving a.p, b.p, c.p;
}
"#;
    let chain = nary.replace(
        "connect conserving a.p, b.p, c.p;",
        "connect conserving a.p, b.p; connect conserving b.p, c.p;",
    );
    let mut nary = crate::compile("nary.eqi", nary).unwrap();
    let mut chain = crate::compile("chain.eqi", &chain).unwrap();
    let nary = nary.pop().unwrap();
    let chain = chain.pop().unwrap();

    assert_eq!(nary.model(), chain.model());
    assert_eq!(nary.transaction().ops(), chain.transaction().ops());
    let connection_count = chain
        .transaction()
        .ops()
        .iter()
        .filter(|operation| {
            matches!(
                operation,
                Op::DefineKernelNode {
                    node: KernelNode::Connection(_)
                }
            )
        })
        .count();
    assert_eq!(connection_count, 1);
}

#[test]
fn redundant_ancestor_fragment_owns_one_connection_and_preserves_both_origins() {
    let source = r#"
connector Pin = scalar_physical(across = 1, through = 1);
component ClosedLeaf {
  public port a: conserving on Pin;
  public port b: conserving on Pin;
  relation law continuous {
    across(a) + across(b) = 0;
  }
  connect conserving a, b;
}
component Parent {
  instance leaf: ClosedLeaf;
  connect conserving leaf.a, leaf.b;
}
model Network { instance parent: Parent; }
"#;
    let document = eqiora_lang::parse("ancestor-reconnection.eqi", source)
        .into_document()
        .unwrap();
    let namespace = LocalSourceIdentity::from_document(&document)
        .unwrap()
        .namespace()
        .unwrap();
    let mut compiled = crate::compile("ancestor-reconnection.eqi", source).unwrap();
    let compiled = compiled.pop().unwrap();
    let a = compiled.symbols().get("parent.leaf.a").unwrap();
    let b = compiled.symbols().get("parent.leaf.b").unwrap();
    let provenance = compiled.provenance().unwrap();
    let a_identity = provenance.identity_for_graph_id(a).unwrap();
    let b_identity = provenance.identity_for_graph_id(b).unwrap();
    let connection = compiled
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Connection(connection),
            } => Some(connection.id().erase()),
            _ => None,
        })
        .expect("one canonical physical Connection");

    let expected = ElaborationKey::anonymous_connection(
        namespace,
        InstancePath::new(["Network", "parent"]).unwrap(),
        DeclarationPath::new(["component", "Parent", "net"]).unwrap(),
        [a_identity, b_identity],
    )
    .unwrap()
    .full_identity()
    .unwrap();
    assert_eq!(provenance.identity_for_graph_id(connection), Some(expected));
    assert_eq!(
        provenance
            .get_by_graph_id(connection)
            .unwrap()
            .origins()
            .len(),
        2,
        "the redundant ancestor fragment is idempotent topology but retained provenance"
    );
}

#[test]
fn ownerless_public_port_is_eliminated_without_fabricating_an_entity_alias() {
    let source = r#"
connector Pin = scalar_physical(across = 1, through = 1);
component Leaf {
  public port p: conserving on Pin;
  relation owner continuous { across(p) = 0; }
}
component Wrapper {
  public port p: conserving on Pin;
  instance leaf: Leaf;
  connect conserving p, leaf.p;
}
model Network {
  instance left: Wrapper;
  instance right: Leaf;
  connect conserving left.p, right.p;
}
"#;
    let document = eqiora_lang::parse("exposure.eqi", source)
        .into_document()
        .unwrap();
    let namespace = LocalSourceIdentity::from_document(&document)
        .unwrap()
        .namespace()
        .unwrap();
    let mut compiled = crate::compile("exposure.eqi", source).unwrap();
    let compiled = compiled.pop().unwrap();
    assert_eq!(compiled.symbols().get("left.p"), None);
    let left_leaf = compiled.symbols().get("left.leaf.p").unwrap();
    let right = compiled.symbols().get("right.p").unwrap();
    let provenance = compiled.provenance().unwrap();
    let canonical_connection = compiled
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Connection(connection),
            } => Some(connection.id().erase()),
            _ => None,
        })
        .unwrap();
    let exposure = ElaborationKey::entity(
        namespace,
        InstancePath::new(["Network", "left"]).unwrap(),
        DeclarationPath::new(["component", "Wrapper", "p"]).unwrap(),
        EntityKind::Port,
    )
    .unwrap()
    .full_identity()
    .unwrap();
    let projection = compiled.physical_exposures().get("left.p").unwrap();
    assert_eq!(compiled.physical_exposures().len(), 1);
    assert_eq!(projection.exposure(), exposure);
    assert_eq!(projection.connection().id().erase(), canonical_connection);
    assert_eq!(projection.interior().len(), 1);
    assert_eq!(projection.interior()[0].id().erase(), left_leaf);
    assert_ne!(projection.interior()[0].id().erase(), right);
    let PhysicalExposureContract::ScalarPhysical { connector } = projection.contract() else {
        panic!("scalar exposure retains a scalar nominal contract");
    };
    assert_eq!(
        connector.id().erase(),
        compiled.symbols().get("connector::Pin").unwrap()
    );
    assert!(provenance.get(exposure).is_some());
    assert_eq!(
        provenance.identity_for_graph_id(projection.interior()[0].id().erase()),
        Some(projection.interior()[0].full_identity())
    );
    let connection_source = provenance.get_by_graph_id(canonical_connection).unwrap();
    assert_eq!(connection_source.origins().len(), 2);
    assert_eq!(
        compiled
            .transaction()
            .ops()
            .iter()
            .filter(|operation| matches!(
                operation,
                Op::DefineKernelNode {
                    node: KernelNode::Connection(_)
                }
            ))
            .count(),
        1
    );
}

#[test]
fn nested_physical_exposures_retain_distinct_occurrence_cuts() {
    let source = r#"
connector Pin = scalar_physical(across = 1, through = 1);
component Leaf {
  public port p: conserving on Pin;
  relation owner continuous { across(p) = 0; }
}
component Inner {
  public port p: conserving on Pin;
  instance leaf: Leaf;
  connect conserving p, leaf.p;
}
component Outer {
  public port p: conserving on Pin;
  instance inner: Inner;
  instance sibling: Leaf;
  connect conserving p, inner.p, sibling.p;
}
model Network {
  instance outer: Outer;
  instance right: Leaf;
  connect conserving outer.p, right.p;
}
"#;
    let mut compiled = crate::compile("nested-exposures.eqi", source).unwrap();
    let compiled = compiled.pop().unwrap();
    let projections = compiled.physical_exposures();
    assert_eq!(
        projections
            .iter()
            .map(|projection| projection.selector())
            .collect::<Vec<_>>(),
        ["outer.inner.p", "outer.p"]
    );

    let leaf = compiled.symbols().get("outer.inner.leaf.p").unwrap();
    let sibling = compiled.symbols().get("outer.sibling.p").unwrap();
    let right = compiled.symbols().get("right.p").unwrap();
    let inner = projections.get("outer.inner.p").unwrap();
    let outer = projections.get("outer.p").unwrap();
    assert_eq!(inner.connection(), outer.connection());
    assert_eq!(
        inner
            .interior()
            .iter()
            .map(|member| member.id().erase())
            .collect::<Vec<_>>(),
        [leaf]
    );
    let mut expected_outer = vec![leaf, sibling];
    expected_outer.sort_unstable();
    assert_eq!(
        outer
            .interior()
            .iter()
            .map(|member| member.id().erase())
            .collect::<Vec<_>>(),
        expected_outer
    );
    assert!(
        projections
            .iter()
            .flat_map(|projection| projection.interior())
            .all(|member| member.id().erase() != right)
    );

    let reordered = source
        .replace(
            "  instance inner: Inner;\n  instance sibling: Leaf;\n  connect conserving p, inner.p, sibling.p;",
            "  instance sibling: Leaf;\n  connect conserving sibling.p, inner.p, p;\n  instance inner: Inner;",
        )
        .replace(
            "  instance outer: Outer;\n  instance right: Leaf;\n  connect conserving outer.p, right.p;",
            "  connect conserving right.p, outer.p;\n  instance right: Leaf;\n  instance outer: Outer;",
        );
    let mut reordered = crate::compile("nested-reordered.eqi", &reordered).unwrap();
    let reordered = reordered.pop().unwrap();
    assert_eq!(compiled.model(), reordered.model());
    assert_eq!(compiled.transaction().ops(), reordered.transaction().ops());
    assert_eq!(
        compiled.physical_exposures(),
        reordered.physical_exposures()
    );
}

const DISTINCT_EXPOSURE_CUTS: &str = r#"
connector Pin = scalar_physical(across = 1, through = 1);
component Leaf {
  public port p: conserving on Pin;
  relation owner continuous { across(p) = 0; }
}
component Pair {
  public port p: conserving on Pin;
  public port q: conserving on Pin;
  instance left: Leaf;
  instance right: Leaf;
  connect conserving p, left.p;
  connect conserving q, right.p;
}
model Network {
  instance pair: Pair;
  instance outside: Leaf;
  connect conserving pair.p, pair.q, outside.p;
}
"#;

#[test]
fn sibling_exposures_follow_their_internal_fragments_not_the_whole_occurrence() {
    let mut compiled = crate::compile("distinct-cuts.eqi", DISTINCT_EXPOSURE_CUTS).unwrap();
    let compiled = compiled.pop().unwrap();
    let projections = compiled.physical_exposures();
    assert_eq!(projections.len(), 2);
    let left = compiled.symbols().get("pair.left.p").unwrap();
    let right = compiled.symbols().get("pair.right.p").unwrap();
    let outside = compiled.symbols().get("outside.p").unwrap();
    assert_eq!(
        projections.get("pair.p").unwrap().interior()[0]
            .id()
            .erase(),
        left
    );
    assert_eq!(
        projections.get("pair.q").unwrap().interior()[0]
            .id()
            .erase(),
        right
    );
    assert!(projections.iter().all(|projection| {
        projection.interior().len() == 1 && projection.interior()[0].id().erase() != outside
    }));
}

#[test]
fn physical_exposure_projection_resources_fail_closed_independently() {
    let document = eqiora_lang::parse("projection-limits.eqi", DISTINCT_EXPOSURE_CUTS)
        .into_document()
        .unwrap();
    for (limits, expected) in [
        (
            super::PhysicalExposureProjectionLimits {
                max_projections: 1,
                ..super::PhysicalExposureProjectionLimits::default()
            },
            "physical exposure projections total 2",
        ),
        (
            super::PhysicalExposureProjectionLimits {
                max_members_per_cut: 0,
                ..super::PhysicalExposureProjectionLimits::default()
            },
            "cut has 1 members",
        ),
        (
            super::PhysicalExposureProjectionLimits {
                max_memberships: 1,
                ..super::PhysicalExposureProjectionLimits::default()
            },
            "cuts total 2 memberships",
        ),
        (
            super::PhysicalExposureProjectionLimits {
                max_traversal_memberships: 0,
                ..super::PhysicalExposureProjectionLimits::default()
            },
            "cut traversal totals 1 memberships",
        ),
    ] {
        let diagnostics = super::compile_hierarchy_with_limits(
            "projection-limits.eqi",
            DISTINCT_EXPOSURE_CUTS.len(),
            &document,
            super::HierarchyLimits {
                physical_exposures: limits,
                ..super::HierarchyLimits::default()
            },
        )
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected)),
            "expected `{expected}`, got {diagnostics:?}"
        );
    }
}

#[test]
fn parameter_bindings_are_closed_typed_and_fail_before_lowering() {
    let cases = [
        (
            "component C { public parameter p: 1; } model m { instance c: C; }",
            "required Parameter `p` has no instance binding",
        ),
        (
            "component C { public parameter p: 1 = 1; } model m { instance c: C(q = 2); }",
            "unknown public Parameter `q`",
        ),
        (
            "component C { parameter p: 1 = 1; } model m { instance c: C(p = 2); }",
            "private Parameter `p` cannot be bound",
        ),
        (
            "component C { public parameter p: m = 1; } model m { parameter q: s = 2; instance c: C(p = q); }",
            "Parameter binding has dimension",
        ),
        (
            "component C { public parameter p: 1; } model m { parameter exponent: 1 = 2; instance c: C(p = 3 ^ exponent); }",
            "power exponent cannot depend on a live Parameter",
        ),
    ];
    for (source, expected) in cases {
        let diagnostics = crate::compile("invalid.eqi", source).unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected)),
            "expected `{expected}`, got {diagnostics:#?}"
        );
    }
}

#[test]
fn recursion_and_private_member_selection_fail_closed() {
    let recursive =
        "component A { instance b: B; } component B { instance a: A; } model m { instance a: A; }";
    let diagnostics = crate::compile("recursive.eqi", recursive).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("recursive component definition graph")
    }));

    let private = r#"
component C { port hidden: signal input 1; }
model m {
  port source: signal output 1;
  instance c: C;
  connect signal source -> c.hidden;
}
"#;
    let diagnostics = crate::compile("private.eqi", private).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("does not select a public Port")
    }));

    let invalid_member =
        "component C { relation bad continuous { missing = 0; } } model m { instance c: C; }";
    let diagnostics = crate::compile("instance-context.eqi", invalid_member).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message().contains("`missing`") && diagnostic.source_span().is_some()
    }));
}

#[test]
fn one_port_boundary_remains_valid_in_hierarchy_lowering() {
    let source = r#"
component Empty {}
model bounded {
  port input: signal input 1;
  relation law continuous { input = 0; }
  boundary input;
}
"#;
    let compiled = crate::compile("boundary.eqi", source).unwrap();
    assert_eq!(compiled.len(), 1);
    assert!(compiled[0].symbols().get("input").is_some());
}

#[test]
fn preflight_enforces_depth_staging_and_provenance_limits() {
    let nested = "component B {} component A { instance b: B; } model m { instance a: A; }";
    let document = eqiora_lang::parse("limits.eqi", nested)
        .into_document()
        .unwrap();
    let limits = super::HierarchyLimits {
        max_instance_depth: 2,
        ..super::HierarchyLimits::default()
    };
    let diagnostics =
        super::compile_hierarchy_with_limits("limits.eqi", nested.len(), &document, limits)
            .unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("requires 3 Model-relative instance depth, exceeding the 2 limit")
    }));

    let one_parameter = "component Empty {} model m { parameter p: 1 = 1; }";
    let document = eqiora_lang::parse("limits.eqi", one_parameter)
        .into_document()
        .unwrap();
    let mut staging_limits = super::HierarchyLimits::default();
    staging_limits.identity.max_staged_identities = 1;
    let diagnostics = super::compile_hierarchy_with_limits(
        "limits.eqi",
        one_parameter.len(),
        &document,
        staging_limits,
    )
    .unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("2 staged identities"))
    );

    let mut provenance_limits = super::HierarchyLimits::default();
    provenance_limits.provenance.max_entries = 1;
    let diagnostics = super::compile_hierarchy_with_limits(
        "limits.eqi",
        one_parameter.len(),
        &document,
        provenance_limits,
    )
    .unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("2 provenance entries"))
    );
}

#[test]
fn field_physical_ports_specialize_on_exact_occurrence_supports() {
    use eqiora_graph::Op;
    use eqiora_schema::kernel::{DomainKind, ExprNode, KernelNode, PortPayload, SymbolRef};

    let source = r#"
public connector MechanicalBoundary = field_physical(
  trace = velocity: m / s,
  flux = traction: kg / (m * s ^ 2),
  shape = spatial_vector,
  frame = spatial,
  pairing = euclidean_boundary_duality
);
public component Side {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public port interface: conserving MechanicalBoundary over wall;
  relation load continuous on wall { flux(interface) = 0; }
}

model Coupled {
  domain left_body = box(0, 1, 0, 1);
  domain left_wall = boundary(left_body, axis = 0, side = upper);
  domain right_body = box(1, 2, 0, 1);
  domain right_wall = boundary(right_body, axis = 0, side = lower);
  instance left: Side(support body = left_body, support wall = left_wall);
  instance right: Side(support body = right_body, support wall = right_wall);
  connect conserving left.interface, right.interface;
}
"#;
    let compiled = crate::compile("field-interface.eqi", source).unwrap();
    let nodes = compiled[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|operation| {
            let Op::DefineKernelNode { node } = operation else {
                return None;
            };
            Some(node)
        });
    let mut connector_count = 0;
    let mut port_count = 0;
    let mut flux_count = 0;
    for node in nodes {
        match node {
            KernelNode::Domain(domain) => {
                if let DomainKind::BoundaryPhysical { connector } = domain.kind() {
                    connector_count += 1;
                    assert_eq!(connector.shape().extents()[0].get(), 2);
                }
            }
            KernelNode::Port(port) => {
                if matches!(port.payload(), PortPayload::BoundaryPhysical { .. }) {
                    port_count += 1;
                }
            }
            KernelNode::Relation(relation) => {
                flux_count += relation
                    .residuals()
                    .nodes()
                    .iter()
                    .filter(|node| matches!(node, ExprNode::Symbol(SymbolRef::PortFlux(_))))
                    .count();
            }
            _ => {}
        }
    }
    assert_eq!(connector_count, 1);
    assert_eq!(port_count, 2);
    assert_eq!(flux_count, 2);
}

#[test]
fn spatial_periodic_model_pair_lowers_without_connection_set_union() {
    use eqiora_schema::kernel::ConnectionSemantics;

    let source = r#"
public connector TransportBoundary = field_physical(
  trace = concentration: K,
  flux = transport_flux: K * m / s,
  shape = [],
  frame = invariant,
  pairing = euclidean_boundary_duality
);
public component Side {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public port interface: conserving TransportBoundary over wall;
  relation owner continuous on wall { flux(interface) = 0; }
}

model Periodic {
  domain body = box(0, 2, -1, 1);
  domain lower = boundary(body, axis = 0, side = lower);
  domain upper = boundary(body, axis = 0, side = upper);
  instance lower_side: Side(support body = body, support wall = lower);
  instance upper_side: Side(support body = body, support wall = upper);
  connect periodic upper_side.interface, lower_side.interface;
}
"#;
    let compiled = crate::compile("spatial-periodic.eqi", source).unwrap();
    let formatted = eqiora_lang::format(
        &eqiora_lang::parse("spatial-periodic.eqi", source)
            .into_document()
            .expect("scalar boundary Connector source parses"),
    );
    let reformatted = crate::compile("reformatted-periodic.eqi", &formatted).unwrap();
    assert_eq!(
        compiled[0].model(),
        reformatted[0].model(),
        "the explicit scalar specialization is stable across source presentation",
    );
    let connections = compiled[0]
        .transaction()
        .ops()
        .iter()
        .filter_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Connection(connection),
            } => Some(connection.semantics()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(connections, [ConnectionSemantics::SpatialPeriodic]);
}

#[test]
fn field_physical_exposure_retains_exact_boundary_cut_and_contract() {
    use eqiora_schema::kernel::DomainKind;

    let source = r#"
public connector MechanicalBoundary = field_physical(
  trace = velocity: m / s,
  flux = traction: kg / (m * s ^ 2),
  shape = spatial_vector,
  frame = spatial,
  pairing = euclidean_boundary_duality
);
public component Side {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public port interface: conserving MechanicalBoundary over wall;
  relation load continuous on wall { flux(interface) = 0; }
}
public component Wrapper {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public port interface: conserving MechanicalBoundary over wall;
  instance side: Side(support body = body, support wall = wall);
  connect conserving interface, side.interface;
}
model Coupled {
  domain left_body = box(0, 1, 0, 1);
  domain left_wall = boundary(left_body, axis = 0, side = upper);
  domain right_body = box(1, 2, 0, 1);
  domain right_wall = boundary(right_body, axis = 0, side = lower);
  instance left: Wrapper(support body = left_body, support wall = left_wall);
  instance right: Side(support body = right_body, support wall = right_wall);
  connect conserving left.interface, right.interface;
}
"#;
    let mut compiled = crate::compile("field-exposure.eqi", source).unwrap();
    let compiled = compiled.pop().unwrap();
    assert_eq!(compiled.symbols().get("left.interface"), None);
    let retained = compiled.symbols().get("left.side.interface").unwrap();
    let projection = compiled.physical_exposures().get("left.interface").unwrap();
    assert_eq!(compiled.physical_exposures().len(), 1);
    assert_eq!(projection.interior().len(), 1);
    assert_eq!(projection.interior()[0].id().erase(), retained);
    let PhysicalExposureContract::FieldBoundary {
        connector,
        boundary,
    } = projection.contract()
    else {
        panic!("field exposure retains its exact field-boundary contract");
    };
    assert_eq!(
        boundary.id().erase(),
        compiled.symbols().get("left_wall").unwrap()
    );
    let connector_node = compiled
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Domain(domain),
            } if domain.id() == connector.id() => Some(domain),
            _ => None,
        })
        .unwrap();
    let DomainKind::BoundaryPhysical { connector } = connector_node.kind() else {
        panic!("projection connector resolves to a boundary-physical Domain");
    };
    assert_eq!(connector.shape().extents()[0].get(), 2);
    assert!(
        compiled
            .provenance()
            .unwrap()
            .get(projection.exposure())
            .is_some()
    );
}

#[test]
fn noncoincident_field_boundaries_fail_before_connection_set_union() {
    let source = r#"
public connector B = field_physical(
  trace = u: 1, flux = f: 1, shape = spatial_vector,
  frame = spatial, pairing = euclidean_boundary_duality
);
public component Side {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public port p: conserving B over wall;
  relation owner continuous on wall { flux(p) = 0; }
}

model M {
  domain a = box(0, 1, 0, 1);
  domain aw = boundary(a, axis = 0, side = upper);
  domain b = box(2, 3, 0, 1);
  domain bw = boundary(b, axis = 0, side = lower);
  instance left: Side(support body = a, support wall = aw);
  instance right: Side(support body = b, support wall = bw);
  connect conserving left.p, right.p;
}
"#;
    let diagnostics = crate::compile("noncoincident.eqi", source).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("before topology normalization: NoncoincidentBoundaries")
    }));
}

#[test]
fn spatial_connector_checks_every_extent_against_ambient_dimension() {
    let source = r#"
public connector Bad = field_physical(
  trace = u: 1, flux = f: 1, shape = [2, 3],
  frame = spatial, pairing = euclidean_boundary_duality
);
public component Side {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public port p: conserving Bad over wall;
}
model Empty {}
"#;
    let diagnostics = crate::compile("shape-mismatch.eqi", source).unwrap_err();
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("spatial Connector shape must equal the exact support ambient dimension")
    }));
}
