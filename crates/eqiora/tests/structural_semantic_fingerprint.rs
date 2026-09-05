use eqiora::api::{ModelDocument, SemanticFingerprintGeneration, StructuralSemanticFingerprint};
use eqiora::graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora::kernel::{
    ActivationDef, ExprDagBuilder, FieldDef, PortDef, RelationDef, SignalDirection, SymbolRef,
};
use eqiora::language::{
    DraftConservingConnection, DraftConservingPort, DraftExpression, DraftField, DraftParameter,
    DraftPhysicalDomain, DraftRelation, ModelDraft,
};
use eqiora::ontology::{Model, ModelView, OntologyId};
use eqiora::sem::KernelProgram;
use eqiora::{DimExponents, DynQuantity, Id, kinds};

const DECAY: &str =
    include_str!("../../../verify/interfaces/structural-semantic-fingerprint/models/decay.eqi");

const PHYSICAL: &str =
    include_str!("../../../verify/interfaces/structural-semantic-fingerprint/models/resistor.eqi");

#[test]
fn current_generation_is_independent_of_coordinate_vocabulary() {
    let fixed = ModelDocument::compile(
        "fixed.eqi",
        "model m { parameter length: m = 1; domain body = box(0, 1); relation r continuous on body { coordinate(0) - coordinate(0) = 0; } }",
    )
    .unwrap();
    let referenced = ModelDocument::compile(
        "referenced.eqi",
        "model m { parameter length: m = 1; domain body = box(0, length); relation r continuous on body { coordinate(0) - coordinate(0) = 0; } }",
    )
    .unwrap();
    for model in [&fixed, &referenced] {
        assert_eq!(
            model.structural_fingerprint().unwrap().generation(),
            SemanticFingerprintGeneration::V3
        );
    }
    // Equal endpoint values do not erase the nominal Parameter dependency.
    assert!(!fixed.structurally_equivalent(&referenced).unwrap());
}

#[test]
fn source_native_codec_and_allocation_routes_share_only_structural_identity() {
    let source = ModelDocument::compile("decay.eqi", DECAY).unwrap();
    let independently_compiled = ModelDocument::compile(
        "renamed.eqi",
        "model renamed { parameter r: 1/s=1; field state: 1=1; relation balance continuous { derivative(state)+r*state=0; } }",
    )
    .unwrap();
    let native = ModelDocument::define(&native_decay(false)).unwrap();
    let reordered_native = ModelDocument::define(&native_decay(true)).unwrap();

    for equivalent in [&independently_compiled, &native, &reordered_native] {
        assert!(source.structurally_equivalent(equivalent).unwrap());
        assert_eq!(
            source.structural_fingerprint().unwrap(),
            equivalent.structural_fingerprint().unwrap()
        );
        assert_ne!(
            source.artifact_reference().unwrap(),
            equivalent.artifact_reference().unwrap()
        );
    }
    let fingerprint = source.structural_fingerprint().unwrap();
    assert_eq!(fingerprint.generation(), SemanticFingerprintGeneration::V3);
    assert_eq!(fingerprint.digest().len(), 64);

    let replay = eqiora::api::ModelDocument::replay(&source.canonical_json().unwrap()).unwrap();
    assert_eq!(
        replay.artifact_reference().unwrap(),
        source.artifact_reference().unwrap()
    );
    assert!(source.structurally_equivalent(&replay).unwrap());

    let child = source
        .commit_value_edit(
            source
                .preview_value_edit(source.aliases()["rate"], 2.0)
                .unwrap(),
        )
        .unwrap();
    assert!(!source.structurally_equivalent(child.document()).unwrap());
}

#[test]
fn nominal_identity_graph_wiring_values_and_operators_remain_meaning() {
    let source = ModelDocument::compile("resistor.eqi", PHYSICAL).unwrap();
    let native = ModelDocument::define(&native_resistor(false)).unwrap();
    let reordered_native = ModelDocument::define(&native_resistor(true)).unwrap();
    assert!(source.structurally_equivalent(&native).unwrap());
    assert!(source.structurally_equivalent(&reordered_native).unwrap());
    assert_ne!(source.digest().unwrap(), native.digest().unwrap());

    let changed_value =
        ModelDocument::compile("value.eqi", &PHYSICAL.replace("= 2;", "= 3;")).unwrap();
    let changed_operator = ModelDocument::compile(
        "operator.eqi",
        &PHYSICAL.replace(
            "across(positive) - across(negative)",
            "across(positive) + across(negative)",
        ),
    )
    .unwrap();
    assert!(!source.structurally_equivalent(&changed_value).unwrap());
    assert!(!source.structurally_equivalent(&changed_operator).unwrap());

    let distinct_domains = physical_domain_aliasing(false);
    let shared_domain = physical_domain_aliasing(true);
    assert!(
        !distinct_domains
            .structurally_equivalent(&shared_domain)
            .unwrap()
    );
}

#[test]
fn mathematical_signed_zero_is_normalized_without_weakening_other_values() {
    let positive = ModelDocument::compile(
        "positive.eqi",
        "model zero { parameter p: 1 = 0.0; relation r continuous { p = 0; } }",
    )
    .unwrap();
    let negative = ModelDocument::compile(
        "negative.eqi",
        "model zero { parameter p: 1 = -0.0; relation r continuous { p = 0; } }",
    )
    .unwrap();
    let nonzero = ModelDocument::compile(
        "nonzero.eqi",
        "model zero { parameter p: 1 = 0.0000000000000001; relation r continuous { p = 0; } }",
    )
    .unwrap();
    assert!(positive.structurally_equivalent(&negative).unwrap());
    assert!(!positive.structurally_equivalent(&nonzero).unwrap());
}

#[test]
fn semantic_types_support_and_model_time_are_fingerprint_meaning() {
    let scalar = ModelDocument::compile(
        "scalar.eqi",
        "model m { field value: 1 = 0; relation r continuous { value = 0; } }",
    )
    .unwrap();
    let dimensioned = ModelDocument::compile(
        "dimensioned.eqi",
        "model m { field value: m = 0; relation r continuous { value = 0; } }",
    )
    .unwrap();
    assert!(!scalar.structurally_equivalent(&dimensioned).unwrap());

    let scalar_spatial = ModelDocument::compile(
        "scalar-spatial.eqi",
        "model m { domain body = box(0, 1, 0, 1); representation space = continuum; field value on body as space: m = 0; relation r continuous on body { value = 0; } }",
    )
    .unwrap();
    let vector_spatial = ModelDocument::compile(
        "vector-spatial.eqi",
        "model m { domain body = box(0, 1, 0, 1); representation space = continuum; field value on body as space: m shape spatial_vector; relation r continuous on body { value = 0; } }",
    )
    .unwrap();
    assert!(
        !scalar_spatial
            .structurally_equivalent(&vector_spatial)
            .unwrap()
    );

    let support_a = ModelDocument::compile(
        "support-a.eqi",
        "model m { domain a = box(0, 1); domain b = box(0, 2); representation space = continuum; field value on a as space: 1 = 0; relation r continuous on a { value = 0; } }",
    )
    .unwrap();
    let support_b = ModelDocument::compile(
        "support-b.eqi",
        "model m { domain a = box(0, 1); domain b = box(0, 2); representation space = continuum; field value on b as space: 1 = 0; relation r continuous on b { value = 0; } }",
    )
    .unwrap();
    assert!(!support_a.structurally_equivalent(&support_b).unwrap());

    let slow_clock = ModelDocument::compile(
        "slow.eqi",
        "model m { field x: 1 = 0; clock tick = periodic(period = 1 / 10, phase = 0 / 1); relation update periodic(tick) { next(x) - x = 0; } }",
    )
    .unwrap();
    let fast_clock = ModelDocument::compile(
        "fast.eqi",
        "model m { field x: 1 = 0; clock tick = periodic(period = 1 / 20, phase = 0 / 1); relation update periodic(tick) { next(x) - x = 0; } }",
    )
    .unwrap();
    assert!(!slow_clock.structurally_equivalent(&fast_clock).unwrap());
}

#[test]
fn expression_allocation_and_model_boundary_have_exact_public_projection_semantics() {
    let first = manually_allocated_program(false, false);
    let reordered_expression = manually_allocated_program(true, false);
    let exposed_port = manually_allocated_program(false, true);

    assert_eq!(
        StructuralSemanticFingerprint::from_program(&first).unwrap(),
        StructuralSemanticFingerprint::from_program(&reordered_expression).unwrap()
    );
    assert_ne!(
        StructuralSemanticFingerprint::from_program(&first).unwrap(),
        StructuralSemanticFingerprint::from_program(&exposed_port).unwrap()
    );
}

#[test]
fn pathological_default_projection_fails_without_a_partial_identity() {
    let mut source = String::from("model symmetric {");
    for index in 0..258 {
        source.push_str(&format!(" parameter p{index}: 1 = 1;"));
    }
    source.push_str(" relation balance continuous { 0 = 0; } }");
    let model = ModelDocument::compile("symmetric.eqi", &source).unwrap();

    let error = model
        .structural_fingerprint()
        .expect_err("fixed public limits must reject pathological exact labelling");
    assert_eq!(error.code().0, "EQ0901");
    assert!(error.message().contains("individualization-depth limit"));
}

fn native_decay(reversed: bool) -> ModelDraft {
    let field = DraftField::new("state", DimExponents::DIMENSIONLESS, 1.0);
    let rate = DraftParameter::new(
        "coefficient",
        DimExponents {
            time: -1,
            ..DimExponents::DIMENSIONLESS
        },
        1.0,
    );
    let relation = DraftRelation::continuous(
        "balance",
        [DraftExpression::derivative(&field) + rate.expression() * field.expression()],
    );
    let declarations = if reversed {
        vec![relation.into(), rate.into(), field.into()]
    } else {
        vec![field.into(), rate.into(), relation.into()]
    };
    ModelDraft::new("native_decay", declarations).unwrap()
}

fn native_resistor(reversed: bool) -> ModelDraft {
    let electrical = DraftPhysicalDomain::new(
        "pin",
        DimExponents {
            mass: 1,
            length: 2,
            time: -3,
            current: -1,
            ..DimExponents::DIMENSIONLESS
        },
        DimExponents {
            current: 1,
            ..DimExponents::DIMENSIONLESS
        },
    );
    let positive = DraftConservingPort::new("p", &electrical);
    let negative = DraftConservingPort::new("n", &electrical);
    let tap = DraftConservingPort::new("t", &electrical);
    let resistance = DraftParameter::new(
        "r",
        DimExponents {
            mass: 1,
            length: 2,
            time: -3,
            current: -2,
            ..DimExponents::DIMENSIONLESS
        },
        2.0,
    );
    let law = DraftRelation::continuous(
        "law",
        [
            DraftExpression::across(&positive)
                - DraftExpression::across(&negative)
                - resistance.expression() * DraftExpression::through(&positive),
            DraftExpression::through(&positive)
                + DraftExpression::through(&negative)
                + DraftExpression::through(&tap),
        ],
    );
    let connection = DraftConservingConnection::new([&positive, &negative, &tap]);
    let declarations = if reversed {
        vec![
            connection.into(),
            law.into(),
            tap.into(),
            resistance.into(),
            negative.into(),
            positive.into(),
            electrical.into(),
        ]
    } else {
        vec![
            electrical.into(),
            positive.into(),
            negative.into(),
            tap.into(),
            resistance.into(),
            law.into(),
            connection.into(),
        ]
    };
    ModelDraft::new("native_resistor", declarations).unwrap()
}

fn physical_domain_aliasing(shared: bool) -> ModelDocument {
    let second_support = if shared { "first" } else { "second" };
    let source = format!(
        r#"
model network {{
  domain first = scalar_physical(across = 1, through = 1);
  domain second = scalar_physical(across = 1, through = 1);
  port a1: conserving on first;
  port a2: conserving on first;
  port b1: conserving on {second_support};
  port b2: conserving on {second_support};
  relation a continuous {{ across(a1)-across(a2)=0; through(a1)+through(a2)=0; }}
  relation b continuous {{ across(b1)-across(b2)=0; through(b1)+through(b2)=0; }}
  connect conserving a1, a2;
  connect conserving b1, b2;
}}
"#
    );
    ModelDocument::compile("aliasing.eqi", &source).unwrap()
}

fn manually_allocated_program(reverse_expression: bool, expose_port: bool) -> KernelProgram {
    let left = Id::<kinds::Field>::new();
    let right = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let port = Id::<kinds::Port>::new();
    let model = OntologyId::<Model>::new();
    let mut expression = ExprDagBuilder::new();
    let (left_value, right_value) = if reverse_expression {
        let right_value = expression.symbol(SymbolRef::Field(right)).unwrap();
        let left_value = expression.symbol(SymbolRef::Field(left)).unwrap();
        (left_value, right_value)
    } else {
        let left_value = expression.symbol(SymbolRef::Field(left)).unwrap();
        let right_value = expression.symbol(SymbolRef::Field(right)).unwrap();
        (left_value, right_value)
    };
    let root = expression.add(left_value, right_value).unwrap();
    let expression = expression.finish([root]).unwrap();
    let members = [
        left.erase(),
        right.erase(),
        relation.erase(),
        activation.erase(),
        port.erase(),
    ];
    let boundary = expose_port.then_some(port.erase());
    let view = ModelView::new(model, members, boundary).unwrap();
    let mut transaction = Transaction::new("manual expression allocation");
    transaction
        .push(Op::DefineKernelNode {
            node: FieldDef::new(left, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
                .unwrap()
                .into(),
        })
        .push(Op::DefineKernelNode {
            node: FieldDef::new(right, DimExponents::DIMENSIONLESS)
                .with_initial(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
                .unwrap()
                .into(),
        })
        .push(Op::DefineKernelNode {
            node: RelationDef::new(relation, expression).into(),
        })
        .push(Op::DefineKernelNode {
            node: ActivationDef::continuous(activation).into(),
        })
        .push(Op::DefineKernelNode {
            node: PortDef::signal(port, SignalDirection::Input, DimExponents::DIMENSIONLESS).into(),
        })
        .push(Op::Connect {
            from: relation.erase(),
            to: left.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: relation.erase(),
            to: right.erase(),
            edge: EdgeKind::DependsOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView { view: view.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}
