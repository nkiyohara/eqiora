use eqiora_artifact::{
    ModelDecoderLimits, ModelEnvelopeV1, ModelEnvelopeV2, ModelEnvelopeV3,
    ModelTransactionEnvelopeV1, ModelTransactionEnvelopeV2, ModelTransactionEnvelopeV3,
};
use eqiora_compiler::compile;
use eqiora_core::entity::kinds;
use eqiora_core::{DimExponents, Id, ValueShape};
use eqiora_graph::{GraphStore, InMemoryGraphStore, Op, Transaction};
use eqiora_schema::kernel::{
    BoundaryPairing, BoundaryPhysicalConnector, DomainDef, ExprDagBuilder, FieldDef, PortDef,
    RelationDef, SymbolRef, ValueFrame,
};
use eqiora_sem::KernelProgram;

const POISSON: &str = include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");

fn scalar_program() -> KernelProgram {
    let compiled = compile("poisson.eqi", POISSON).unwrap().remove(0);
    let (transaction, model, _) = compiled.into_parts();
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

fn boundary_transaction() -> Transaction {
    let connector = Id::<kinds::Domain>::new();
    let boundary = Id::<kinds::Domain>::new();
    let port = Id::<kinds::Port>::new();
    let field = Id::<kinds::Field>::new();
    let relation = Id::<kinds::Relation>::new();
    let velocity = DimExponents {
        length: 1,
        time: -1,
        ..DimExponents::DIMENSIONLESS
    };
    let traction = DimExponents {
        mass: 1,
        length: -1,
        time: -2,
        ..DimExponents::DIMENSIONLESS
    };
    let shape = ValueShape::new([2]).unwrap();
    let connector_contract = BoundaryPhysicalConnector::new(
        velocity,
        traction,
        shape.clone(),
        ValueFrame::SpatialCartesian,
        BoundaryPairing::EuclideanBoundaryDuality,
    )
    .unwrap();
    let mut residual = ExprDagBuilder::new();
    let trace = residual.symbol(SymbolRef::PortTrace(port)).unwrap();
    let flux = residual.symbol(SymbolRef::PortFlux(port)).unwrap();
    let root = residual.add(trace, flux).unwrap();

    let mut transaction = Transaction::new("boundary physical wire v3 fixture");
    for node in [
        DomainDef::boundary_physical(connector, connector_contract).into(),
        DomainDef::cartesian_boundary(boundary, 0, eqiora_schema::kernel::BoundarySide::Upper)
            .into(),
        PortDef::boundary_physical(port, connector, boundary).into(),
        FieldDef::shaped(field, velocity, shape, ValueFrame::SpatialCartesian)
            .unwrap()
            .into(),
        RelationDef::new(relation, residual.finish([root]).unwrap()).into(),
    ] {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction
}

#[test]
fn v3_uses_one_shaped_field_representation_and_explicit_schema() {
    let program = scalar_program();
    let v1 = ModelEnvelopeV1::from_program(&program).unwrap();
    let v2 = ModelEnvelopeV2::from_program(&program).unwrap();
    let v3 = ModelEnvelopeV3::from_program(&program).unwrap();
    let v1_bytes = v1.canonical_json().unwrap();
    let v2_bytes = v2.canonical_json().unwrap();
    let v3_bytes = v3.canonical_json().unwrap();

    assert!(!String::from_utf8_lossy(&v1_bytes).contains("shaped-field"));
    assert!(!String::from_utf8_lossy(&v2_bytes).contains("shaped-field"));
    assert!(String::from_utf8_lossy(&v3_bytes).contains("shaped-field"));
    assert!(String::from_utf8_lossy(&v3_bytes).contains("\"shape\":[]"));
    assert!(ModelEnvelopeV1::from_json(&v3_bytes, Default::default()).is_err());
    assert!(ModelEnvelopeV2::from_json(&v3_bytes, Default::default()).is_err());

    let decoded = ModelEnvelopeV3::from_json(&v3_bytes, Default::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), v3_bytes);
    assert_eq!(decoded.digest().unwrap(), v3.digest().unwrap());
    assert!(
        ModelEnvelopeV3::from_json(
            &v3_bytes,
            ModelDecoderLimits {
                max_value_shape_components: 0,
                ..Default::default()
            },
        )
        .is_err()
    );
}

#[test]
fn boundary_physical_transaction_is_v3_only_and_round_trips_exact_contracts() {
    let transaction = boundary_transaction();
    assert!(ModelTransactionEnvelopeV1::from_transaction(&transaction).is_err());
    assert!(ModelTransactionEnvelopeV2::from_transaction(&transaction).is_err());

    let v3 = ModelTransactionEnvelopeV3::from_transaction(&transaction).unwrap();
    let bytes = v3.canonical_json().unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("boundary-physical"));
    assert!(text.contains("boundary-physical-port"));
    assert!(text.contains("port-trace"));
    assert!(text.contains("port-flux"));
    assert!(text.contains("\"shape\":[2]"));
    assert!(text.contains("spatial-cartesian"));

    let decoded = ModelTransactionEnvelopeV3::from_json(&bytes, Default::default()).unwrap();
    assert_eq!(decoded.canonical_json().unwrap(), bytes);
    assert_eq!(decoded.to_transaction().unwrap().ops(), transaction.ops());
    assert!(
        ModelTransactionEnvelopeV3::from_json(
            &bytes,
            ModelDecoderLimits {
                max_value_shape_rank: 0,
                ..Default::default()
            },
        )
        .is_err()
    );

    let mut forged_v2: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    forged_v2["schema"] =
        serde_json::Value::String("eqiora.model-transaction-envelope/v2".to_owned());
    assert!(
        ModelTransactionEnvelopeV2::from_json(
            &serde_json::to_vec(&forged_v2).unwrap(),
            Default::default(),
        )
        .is_err()
    );
}

#[test]
fn v3_rejects_legacy_field_spelling_even_when_schema_is_forged() {
    let v2 = ModelEnvelopeV2::from_program(&scalar_program()).unwrap();
    let mut forged: serde_json::Value =
        serde_json::from_slice(&v2.canonical_json().unwrap()).unwrap();
    forged["schema"] = serde_json::Value::String("eqiora.model-envelope/v3".to_owned());
    assert!(
        ModelEnvelopeV3::from_json(&serde_json::to_vec(&forged).unwrap(), Default::default(),)
            .is_err()
    );
}

#[test]
fn v3_diagnostics_name_the_selected_wire_version() {
    let v3 = ModelEnvelopeV3::from_program(&scalar_program()).unwrap();
    let mut malformed: serde_json::Value =
        serde_json::from_slice(&v3.canonical_json().unwrap()).unwrap();
    malformed["nodes"] = serde_json::Value::Array(Vec::new());
    let diagnostic =
        ModelEnvelopeV3::from_json(&serde_json::to_vec(&malformed).unwrap(), Default::default())
            .unwrap_err();
    assert!(diagnostic.message().contains("model v3 envelope"));
    assert!(!diagnostic.message().contains("model v2 envelope"));
}
