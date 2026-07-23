use eqiora_artifact::{DecoderLimits, ModelEnvelopeV1, ModelTransactionEnvelopeV1};
use eqiora_core::diagnostic::codes;
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, Entity, Id, OntologyId};
use eqiora_graph::{EdgeKind, GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    ActivationDef, ConnectionDef, ConnectionSemantics, ExprDagBuilder, KernelNode, PortDef,
    RelationDef, SymbolRef,
};
use eqiora_schema::{Model, ModelView};
use eqiora_sem::{Interpreter, KernelProgram, ReferenceConfig};
use ulid::Ulid;

const LEGACY_TRANSACTION_DIGEST: &str =
    "6aa8bec09f5223110cd5e3fade3f40bbe9769d5e4d027dfc5d7ed70ef62aad2a";
const LEGACY_MODEL_DIGEST: &str =
    "8dc113e2024e3bdcb0f13141717f8ea329b6feed6d8fc0dce6c9009c878ecc5e";

// Generated once by the unmodified 4f237231 writer. These complete literals
// deliberately do not share encoding helpers with the reader under test.
const LEGACY_TRANSACTION_JSON: &[u8] = br#"{"schema":"eqiora.model-transaction-envelope/v1","encoding":"eqiora.canonical-json/v1","label":"legacy conserving marker golden","ops":[{"op":"define-kernel-node","node":{"id":{"kind":"port","ulid":"01HF7YAT000000000000000002"},"definition":{"kind":"port","port":{"kind":"conserving"},"dimension":{"mass":0,"length":0,"time":0,"current":1,"temperature":0,"amount":0,"luminous_intensity":0}}}},{"op":"define-kernel-node","node":{"id":{"kind":"connection","ulid":"01HF7YAT000000000000000005"},"definition":{"kind":"connection","connection":"conserving"}}},{"op":"define-kernel-node","node":{"id":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"definition":{"kind":"relation","residuals":{"nodes":[{"op":"symbol","symbol":{"kind":"port","id":{"kind":"port","ulid":"01HF7YAT000000000000000001"}}},{"op":"symbol","symbol":{"kind":"port","id":{"kind":"port","ulid":"01HF7YAT000000000000000002"}}},{"op":"sub","left":0,"right":1}],"roots":[2]}}}},{"op":"define-kernel-node","node":{"id":{"kind":"port","ulid":"01HF7YAT000000000000000001"},"definition":{"kind":"port","port":{"kind":"conserving"},"dimension":{"mass":0,"length":0,"time":0,"current":1,"temperature":0,"amount":0,"luminous_intensity":0}}}},{"op":"define-kernel-node","node":{"id":{"kind":"activation","ulid":"01HF7YAT000000000000000004"},"definition":{"kind":"activation","activation":{"kind":"continuous"}}}},{"op":"connect","from":{"kind":"connection","ulid":"01HF7YAT000000000000000005"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000002"},"edge":"connects"},{"op":"connect","from":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000002"},"edge":"depends-on"},{"op":"connect","from":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000002"},"edge":"has-port"},{"op":"connect","from":{"kind":"activation","ulid":"01HF7YAT000000000000000004"},"to":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"edge":"activates"},{"op":"connect","from":{"kind":"connection","ulid":"01HF7YAT000000000000000005"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000001"},"edge":"connects"},{"op":"connect","from":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000001"},"edge":"depends-on"},{"op":"connect","from":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000001"},"edge":"has-port"},{"op":"define-model-view","view":{"ulid":"01HF7YAT000000000000000006","members":[{"kind":"port","ulid":"01HF7YAT000000000000000001"},{"kind":"port","ulid":"01HF7YAT000000000000000002"},{"kind":"relation","ulid":"01HF7YAT000000000000000003"},{"kind":"activation","ulid":"01HF7YAT000000000000000004"},{"kind":"connection","ulid":"01HF7YAT000000000000000005"}],"boundary":[]}}],"preconditions":[]}"#;

const LEGACY_MODEL_JSON: &[u8] = br#"{"schema":"eqiora.model-envelope/v1","encoding":"eqiora.canonical-json/v1","source_revision":1,"model_ulid":"01HF7YAT000000000000000006","nodes":[{"id":{"kind":"port","ulid":"01HF7YAT000000000000000001"},"definition":{"kind":"port","port":{"kind":"conserving"},"dimension":{"mass":0,"length":0,"time":0,"current":1,"temperature":0,"amount":0,"luminous_intensity":0}}},{"id":{"kind":"port","ulid":"01HF7YAT000000000000000002"},"definition":{"kind":"port","port":{"kind":"conserving"},"dimension":{"mass":0,"length":0,"time":0,"current":1,"temperature":0,"amount":0,"luminous_intensity":0}}},{"id":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"definition":{"kind":"relation","residuals":{"nodes":[{"op":"symbol","symbol":{"kind":"port","id":{"kind":"port","ulid":"01HF7YAT000000000000000001"}}},{"op":"symbol","symbol":{"kind":"port","id":{"kind":"port","ulid":"01HF7YAT000000000000000002"}}},{"op":"sub","left":0,"right":1}],"roots":[2]}}},{"id":{"kind":"activation","ulid":"01HF7YAT000000000000000004"},"definition":{"kind":"activation","activation":{"kind":"continuous"}}},{"id":{"kind":"connection","ulid":"01HF7YAT000000000000000005"},"definition":{"kind":"connection","connection":"conserving"}}],"values":[],"edges":[{"from":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000001"},"kind":"depends-on"},{"from":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000001"},"kind":"has-port"},{"from":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000002"},"kind":"depends-on"},{"from":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000002"},"kind":"has-port"},{"from":{"kind":"activation","ulid":"01HF7YAT000000000000000004"},"to":{"kind":"relation","ulid":"01HF7YAT000000000000000003"},"kind":"activates"},{"from":{"kind":"connection","ulid":"01HF7YAT000000000000000005"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000001"},"kind":"connects"},{"from":{"kind":"connection","ulid":"01HF7YAT000000000000000005"},"to":{"kind":"port","ulid":"01HF7YAT000000000000000002"},"kind":"connects"}],"boundary":[]}"#;

fn fixed<E: Entity>(random: u128) -> Id<E> {
    Id::from_ulid(Ulid::from_parts(1_700_000_000_000, random))
}

fn fixture() -> (Transaction, OntologyId<Model>) {
    let first = fixed::<kinds::Port>(1);
    let second = fixed::<kinds::Port>(2);
    let relation = fixed::<kinds::Relation>(3);
    let activation = fixed::<kinds::Activation>(4);
    let connection = fixed::<kinds::Connection>(5);
    let model = OntologyId::<Model>::from_ulid(Ulid::from_parts(1_700_000_000_000, 6));
    let dimension = DimExponents {
        current: 1,
        ..DimExponents::DIMENSIONLESS
    };

    let mut expression = ExprDagBuilder::new();
    let first_value = expression.symbol(SymbolRef::Port(first)).unwrap();
    let second_value = expression.symbol(SymbolRef::Port(second)).unwrap();
    let root = expression.sub(first_value, second_value).unwrap();
    let nodes = [
        KernelNode::from(PortDef::conserving_marker(second, dimension)),
        KernelNode::from(ConnectionDef::new(
            connection,
            ConnectionSemantics::Conserving,
        )),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([root]).unwrap(),
        )),
        KernelNode::from(PortDef::conserving_marker(first, dimension)),
        KernelNode::from(ActivationDef::continuous(activation)),
    ];
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("legacy conserving marker golden");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for (from, to, edge) in [
        (connection.erase(), second.erase(), EdgeKind::Connects),
        (relation.erase(), second.erase(), EdgeKind::DependsOn),
        (relation.erase(), second.erase(), EdgeKind::HasPort),
        (activation.erase(), relation.erase(), EdgeKind::Activates),
        (connection.erase(), first.erase(), EdgeKind::Connects),
        (relation.erase(), first.erase(), EdgeKind::DependsOn),
        (relation.erase(), first.erase(), EdgeKind::HasPort),
    ] {
        transaction.push(Op::Connect { from, to, edge });
    }
    transaction.push(Op::DefineOntologyView { view: view.into() });
    (transaction, model)
}

#[test]
fn old_main_legacy_marker_bytes_and_meaning_are_a_complete_golden() {
    let (transaction, model) = fixture();
    let written_transaction = ModelTransactionEnvelopeV1::from_transaction(&transaction).unwrap();
    assert_eq!(
        written_transaction.canonical_json().unwrap(),
        LEGACY_TRANSACTION_JSON
    );
    assert_eq!(
        written_transaction.digest().unwrap().as_str(),
        LEGACY_TRANSACTION_DIGEST
    );

    let decoded_transaction =
        ModelTransactionEnvelopeV1::from_json(LEGACY_TRANSACTION_JSON, DecoderLimits::default())
            .unwrap();
    assert_eq!(
        decoded_transaction.canonical_json().unwrap(),
        LEGACY_TRANSACTION_JSON
    );
    assert_eq!(
        decoded_transaction.digest().unwrap().as_str(),
        LEGACY_TRANSACTION_DIGEST
    );
    let mut store = InMemoryGraphStore::new();
    store
        .commit(decoded_transaction.to_transaction().unwrap())
        .unwrap();
    let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();

    let written_model = ModelEnvelopeV1::from_program(&program).unwrap();
    assert_eq!(written_model.canonical_json().unwrap(), LEGACY_MODEL_JSON);
    assert_eq!(
        written_model.digest().unwrap().as_str(),
        LEGACY_MODEL_DIGEST
    );

    let decoded_model =
        ModelEnvelopeV1::from_json(LEGACY_MODEL_JSON, DecoderLimits::default()).unwrap();
    assert_eq!(decoded_model.canonical_json().unwrap(), LEGACY_MODEL_JSON);
    assert_eq!(
        decoded_model.digest().unwrap().as_str(),
        LEGACY_MODEL_DIGEST
    );
    let replay = decoded_model.to_program().unwrap();
    assert_eq!(
        ModelEnvelopeV1::from_program(&replay)
            .unwrap()
            .canonical_json()
            .unwrap(),
        LEGACY_MODEL_JSON
    );

    let diagnostics = Interpreter::new()
        .run(&replay, ReferenceConfig::new(0.1, 0.1).unwrap())
        .unwrap_err();
    assert_eq!(diagnostics[0].code(), codes::NOT_IMPLEMENTED);
    assert!(
        replay
            .compose_scalar_physical_subsystem(fixed::<kinds::Connection>(5))
            .is_err()
    );
}

#[test]
fn v1_model_and_transaction_enforce_independent_root_member_and_boundary_budgets() {
    for limits in [
        DecoderLimits {
            max_expression_roots: 0,
            ..DecoderLimits::default()
        },
        DecoderLimits {
            max_model_view_members: 4,
            ..DecoderLimits::default()
        },
    ] {
        assert_eq!(
            ModelEnvelopeV1::from_json(LEGACY_MODEL_JSON, limits)
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );
        assert_eq!(
            ModelTransactionEnvelopeV1::from_json(LEGACY_TRANSACTION_JSON, limits)
                .unwrap_err()
                .code(),
            codes::INVALID_ARTIFACT
        );
    }

    let mut model: serde_json::Value = serde_json::from_slice(LEGACY_MODEL_JSON).unwrap();
    let port = model["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"]["kind"] == "port")
        .unwrap()["id"]
        .clone();
    model["boundary"] = serde_json::Value::Array(vec![port]);
    assert_eq!(
        ModelEnvelopeV1::from_json(
            &serde_json::to_vec(&model).unwrap(),
            DecoderLimits {
                max_model_boundary: 0,
                ..DecoderLimits::default()
            },
        )
        .unwrap_err()
        .code(),
        codes::INVALID_ARTIFACT
    );

    let mut transaction: serde_json::Value =
        serde_json::from_slice(LEGACY_TRANSACTION_JSON).unwrap();
    let view = transaction["ops"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|op| op["op"] == "define-model-view")
        .unwrap()["view"]
        .as_object_mut()
        .unwrap();
    let port = view["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["kind"] == "port")
        .unwrap()
        .clone();
    view.insert("boundary".to_owned(), serde_json::Value::Array(vec![port]));
    assert_eq!(
        ModelTransactionEnvelopeV1::from_json(
            &serde_json::to_vec(&transaction).unwrap(),
            DecoderLimits {
                max_model_boundary: 0,
                ..DecoderLimits::default()
            },
        )
        .unwrap_err()
        .code(),
        codes::INVALID_ARTIFACT
    );
}
