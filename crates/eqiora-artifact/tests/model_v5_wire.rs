use eqiora_artifact::{
    CanonicalModelArtifact, DecoderLimits, ModelEnvelopeV1, ModelEnvelopeV2, ModelEnvelopeV3,
    ModelEnvelopeV4, ModelEnvelopeV5, ModelTransactionEnvelopeV1, ModelTransactionEnvelopeV2,
    ModelTransactionEnvelopeV3, ModelTransactionEnvelopeV4, ModelTransactionEnvelopeV5,
    ReplayableCanonicalModelArtifact,
};
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, DynQuantity, Id, OntologyId, ValueShape};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::pure_operator::PureOperatorDefinition;
use eqiora_schema::kernel::{
    ActivationDef, ActivationKind, AxisBounds, DomainDef, EventDirection, ExprDagBuilder, FieldDef,
    KernelNode, RelationDef, RepresentationDef, SymbolRef, ValueFrame,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::KernelProgram;
use serde_json::{Map, Value};

struct Fixture {
    program: KernelProgram,
    transaction: Transaction,
    model: OntologyId<Model>,
}

fn fixture() -> Fixture {
    let body = Id::<kinds::Domain>::new();
    let representation = Id::<kinds::Representation>::new();
    let left = Id::<kinds::Field>::new();
    let right = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let model = OntologyId::<Model>::new();
    let length = DimExponents {
        length: 1,
        ..DimExponents::DIMENSIONLESS
    };
    let bounds = [
        AxisBounds::new(DynQuantity::new(0.0, length), DynQuantity::new(1.0, length)).unwrap(),
        AxisBounds::new(DynQuantity::new(0.0, length), DynQuantity::new(1.0, length)).unwrap(),
    ];
    let definition = PureOperatorDefinition::dyadic_product().unwrap();
    let mut expression = ExprDagBuilder::new();
    let left_value = expression.symbol(SymbolRef::Field(left)).unwrap();
    let right_value = expression.symbol(SymbolRef::Field(right)).unwrap();
    let root = expression
        .pure_operator(&definition, [left_value, right_value])
        .unwrap();
    let nodes = [
        KernelNode::from(DomainDef::cartesian_box(body, bounds.to_vec()).unwrap()),
        KernelNode::from(RepresentationDef::continuum(representation)),
        KernelNode::from(
            FieldDef::shaped(
                left,
                DimExponents::DIMENSIONLESS,
                ValueShape::new([2]).unwrap(),
                ValueFrame::SpatialCartesian,
            )
            .unwrap(),
        ),
        KernelNode::from(
            FieldDef::shaped(
                right,
                DimExponents::DIMENSIONLESS,
                ValueShape::new([2]).unwrap(),
                ValueFrame::SpatialCartesian,
            )
            .unwrap(),
        ),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([root]).unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ];
    let mut transaction = Transaction::new("pure operator model v5 fixture");
    for node in &nodes {
        transaction.push(Op::DefineKernelNode { node: node.clone() });
    }
    for field in [left, right] {
        transaction
            .push(Op::Connect {
                from: field.erase(),
                to: body.erase(),
                edge: EdgeKind::DefinedOn,
            })
            .push(Op::Connect {
                from: field.erase(),
                to: representation.erase(),
                edge: EdgeKind::DefinedOn,
            })
            .push(Op::Connect {
                from: relation.erase(),
                to: field.erase(),
                edge: EdgeKind::DependsOn,
            });
    }
    transaction
        .push(Op::Connect {
            from: relation.erase(),
            to: body.erase(),
            edge: EdgeKind::AppliesOn,
        })
        .push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        })
        .push(Op::DefineOntologyView {
            view: ModelView::new(model, nodes.iter().map(KernelNode::id), [])
                .unwrap()
                .into(),
        });
    let mut retained = Transaction::new(transaction.label());
    for op in transaction.ops() {
        retained.push(op.clone());
    }
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    Fixture {
        program,
        transaction: retained,
        model,
    }
}

fn relation_and_guard_transaction() -> Transaction {
    let left = Id::<kinds::Field>::new();
    let right = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let activation = Id::<kinds::Activation>::new();
    let definition = PureOperatorDefinition::dyadic_product().unwrap();
    let expression = |left, right| {
        let mut expression = ExprDagBuilder::new();
        let left = expression.symbol(SymbolRef::Field(left)).unwrap();
        let right = expression.symbol(SymbolRef::Field(right)).unwrap();
        let root = expression
            .pure_operator(&definition, [left, right])
            .unwrap();
        expression.finish([root]).unwrap()
    };
    let mut transaction = Transaction::new("relation and Activation guard aggregate fixture");
    transaction
        .push(Op::DefineKernelNode {
            node: RelationDef::new(relation, expression(left, right)).into(),
        })
        .push(Op::DefineKernelNode {
            node: ActivationDef::new(
                activation,
                ActivationKind::Event {
                    guard: expression(left, right),
                    direction: EventDirection::Any,
                },
            )
            .unwrap()
            .into(),
        });
    transaction
}

fn first_expression_mut(value: &mut Value) -> Option<&mut Map<String, Value>> {
    match value {
        Value::Object(object) => {
            if object.contains_key("nodes") && object.contains_key("roots") {
                return Some(object);
            }
            object.values_mut().find_map(first_expression_mut)
        }
        Value::Array(values) => values.iter_mut().find_map(first_expression_mut),
        _ => None,
    }
}

fn encoded_model_value() -> Value {
    let fixture = fixture();
    serde_json::from_slice(
        &ModelEnvelopeV5::from_program(&fixture.program)
            .unwrap()
            .canonical_json()
            .unwrap(),
    )
    .unwrap()
}

fn reject_model(value: &Value) -> String {
    ModelEnvelopeV5::from_json(
        &serde_json::to_vec(value).unwrap(),
        DecoderLimits::default(),
    )
    .unwrap_err()
    .message()
    .to_owned()
}

#[test]
fn model_v5_round_trips_a_closed_generic_application_and_typed_reference() {
    let fixture = fixture();
    for legacy in [
        ModelEnvelopeV1::from_program(&fixture.program).is_err(),
        ModelEnvelopeV2::from_program(&fixture.program).is_err(),
        ModelEnvelopeV3::from_program(&fixture.program).is_err(),
        ModelEnvelopeV4::from_program(&fixture.program).is_err(),
    ] {
        assert!(legacy);
    }

    let envelope = ModelEnvelopeV5::from_program(&fixture.program).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("eqiora.model-envelope/v5"));
    assert!(text.contains("eqiora.pure-component-calculus/v1"));
    assert!(text.contains("pure-operator-application"));
    assert!(
        text.contains(
            &PureOperatorDefinition::dyadic_product()
                .unwrap()
                .digest()
                .to_string()
        )
    );

    let decoded = ModelEnvelopeV5::from_json(&bytes, DecoderLimits::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
    assert_eq!(decoded.to_program().unwrap(), fixture.program);
    let reference = decoded.artifact_reference().unwrap();
    assert_eq!(reference.model(), fixture.model);
    reference.validate_artifact(&decoded).unwrap();
    assert_eq!(decoded.replay_model().unwrap().program(), &fixture.program);

    assert!(ModelEnvelopeV4::from_json(&bytes, DecoderLimits::default()).is_err());
    let mut forged_v4: Value = serde_json::from_slice(&bytes).unwrap();
    forged_v4["schema"] = Value::String("eqiora.model-envelope/v4".to_owned());
    assert!(
        ModelEnvelopeV4::from_json(
            &serde_json::to_vec(&forged_v4).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn transaction_v5_round_trips_without_graph_mutation_or_version_fallback() {
    let fixture = fixture();
    assert!(ModelTransactionEnvelopeV1::from_transaction(&fixture.transaction).is_err());
    assert!(ModelTransactionEnvelopeV2::from_transaction(&fixture.transaction).is_err());
    assert!(ModelTransactionEnvelopeV3::from_transaction(&fixture.transaction).is_err());
    assert!(ModelTransactionEnvelopeV4::from_transaction(&fixture.transaction).is_err());

    let envelope = ModelTransactionEnvelopeV5::from_transaction(&fixture.transaction).unwrap();
    let bytes = envelope.canonical_json().unwrap();
    let decoded = ModelTransactionEnvelopeV5::from_json(&bytes, DecoderLimits::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
    let replay = decoded.to_transaction().unwrap();
    assert_eq!(replay.label(), fixture.transaction.label());
    assert_eq!(replay.ops(), fixture.transaction.ops());
    assert_eq!(replay.preconditions(), fixture.transaction.preconditions());

    let mut forged_v4: Value = serde_json::from_slice(&bytes).unwrap();
    forged_v4["schema"] = Value::String("eqiora.model-transaction-envelope/v4".to_owned());
    assert!(
        ModelTransactionEnvelopeV4::from_json(
            &serde_json::to_vec(&forged_v4).unwrap(),
            DecoderLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn v5_rejects_missing_duplicate_unused_and_unknown_definitions() {
    let original = encoded_model_value();

    let mut missing = original.clone();
    first_expression_mut(&mut missing)
        .unwrap()
        .get_mut("definitions")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .clear();
    assert!(reject_model(&missing).contains("missing local definition"));

    let mut duplicate = original.clone();
    let definitions = first_expression_mut(&mut duplicate)
        .unwrap()
        .get_mut("definitions")
        .unwrap()
        .as_array_mut()
        .unwrap();
    definitions.push(definitions[0].clone());
    assert!(reject_model(&duplicate).contains("duplicate pure-operator definition"));

    let mut unused = original.clone();
    let expression = first_expression_mut(&mut unused).unwrap();
    expression
        .get_mut("nodes")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .pop();
    expression.insert("roots".to_owned(), Value::Array(vec![Value::from(0)]));
    assert!(reject_model(&unused).contains("must supply exactly"));

    let mut unknown = original;
    let definition = first_expression_mut(&mut unknown)
        .unwrap()
        .get_mut("definitions")
        .unwrap()
        .as_array_mut()
        .unwrap()
        .first_mut()
        .unwrap();
    definition["required_features"][0] =
        Value::String("eqiora.unknown-component-calculus/v9".to_owned());
    assert!(reject_model(&unknown).contains("unknown feature"));

    let mut missing_feature = encoded_model_value();
    first_expression_mut(&mut missing_feature).unwrap()["definitions"][0]["required_features"] =
        Value::Array(Vec::new());
    assert!(reject_model(&missing_feature).contains("missing its required"));

    let mut duplicate_feature = encoded_model_value();
    let features = first_expression_mut(&mut duplicate_feature).unwrap()["definitions"][0]
        ["required_features"]
        .as_array_mut()
        .unwrap();
    features.push(features[0].clone());
    assert!(reject_model(&duplicate_feature).contains("duplicate-free"));
}

#[test]
fn v5_rejects_nonlowercase_or_mismatched_digests_prior_operands_and_wrong_arity() {
    let original = encoded_model_value();

    let mut uppercase = original.clone();
    let digest = first_expression_mut(&mut uppercase).unwrap()["definitions"][0]["digest"]
        .as_str()
        .unwrap()
        .to_uppercase();
    first_expression_mut(&mut uppercase).unwrap()["definitions"][0]["digest"] =
        Value::String(digest);
    assert!(reject_model(&uppercase).contains("lowercase hexadecimal"));

    let mut mismatch = original.clone();
    let digest = first_expression_mut(&mut mismatch).unwrap()["definitions"][0]["digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let replacement = if digest.starts_with('0') { '1' } else { '0' };
    let forged = format!("{replacement}{}", &digest[1..]);
    first_expression_mut(&mut mismatch).unwrap()["definitions"][0]["digest"] =
        Value::String(forged);
    assert!(reject_model(&mismatch).contains("digest mismatch"));

    let mut prior = original.clone();
    let expression = first_expression_mut(&mut prior).unwrap();
    let application_index = expression["nodes"].as_array().unwrap().len() - 1;
    expression["nodes"][application_index]["arguments"][0] = Value::from(application_index);
    assert!(reject_model(&prior).contains("not topologically prior"));

    let mut arity = original;
    let expression = first_expression_mut(&mut arity).unwrap();
    let application_index = expression["nodes"].as_array().unwrap().len() - 1;
    expression["nodes"][application_index]["arguments"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(reject_model(&arity).contains("requires exactly 2 ordered arguments"));

    let mut calculus_prior = encoded_model_value();
    first_expression_mut(&mut calculus_prior).unwrap()["definitions"][0]["nodes"][2]["left"] =
        Value::from(2);
    assert!(reject_model(&calculus_prior).contains("pure calculus operand"));
}

#[test]
fn v5_decode_order_applies_aggregate_limits_before_features_and_definitions_before_applications() {
    let mut unknown_feature = encoded_model_value();
    first_expression_mut(&mut unknown_feature).unwrap()["definitions"][0]["required_features"][0] =
        Value::String("eqiora.unknown-component-calculus/v9".to_owned());
    let diagnostic = ModelEnvelopeV5::from_json(
        &serde_json::to_vec(&unknown_feature).unwrap(),
        DecoderLimits {
            max_pure_operator_definitions: 0,
            ..DecoderLimits::default()
        },
    )
    .unwrap_err();
    assert!(diagnostic.message().contains("definition"));
    assert!(diagnostic.message().contains("exceeds decoder limit"));

    let mut duplicate_and_forward = encoded_model_value();
    let expression = first_expression_mut(&mut duplicate_and_forward).unwrap();
    let definitions = expression["definitions"].as_array_mut().unwrap();
    definitions.push(definitions[0].clone());
    let application_index = expression["nodes"].as_array().unwrap().len() - 1;
    expression["nodes"][application_index]["arguments"][0] = Value::from(application_index);
    let diagnostic = reject_model(&duplicate_and_forward);
    assert!(diagnostic.contains("duplicate pure-operator definition"));
    assert!(!diagnostic.contains("topologically prior"));
}

#[test]
fn v5_resource_limits_aggregate_relations_and_activation_guards() {
    let transaction = relation_and_guard_transaction();
    let bytes = ModelTransactionEnvelopeV5::from_transaction(&transaction)
        .unwrap()
        .canonical_json()
        .unwrap();
    ModelTransactionEnvelopeV5::from_json(&bytes, DecoderLimits::default()).unwrap();

    for limits in [
        DecoderLimits {
            max_pure_operator_definitions: 1,
            ..DecoderLimits::default()
        },
        DecoderLimits {
            max_pure_operator_formals: 3,
            ..DecoderLimits::default()
        },
        DecoderLimits {
            max_pure_operator_calculus_nodes: 5,
            ..DecoderLimits::default()
        },
        DecoderLimits {
            max_pure_operator_application_arguments: 3,
            ..DecoderLimits::default()
        },
    ] {
        let diagnostic = ModelTransactionEnvelopeV5::from_json(&bytes, limits).unwrap_err();
        assert!(diagnostic.message().contains("pure-operator"));
        assert!(diagnostic.message().contains("exceeds"));
    }
}
