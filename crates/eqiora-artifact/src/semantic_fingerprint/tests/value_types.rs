use super::*;
use crate::{ModelDecoderLimits, ModelEnvelope};
use eqiora_core::ScalarDomain;
use eqiora_core::ValueType;
use eqiora_schema::kernel::{AxisBounds, DomainDef, RepresentationDef};

#[test]
fn typed_initial_equations_survive_source_and_model_replay() {
    for ty in [
        "1",
        "complex<1>",
        "array<complex<1>, 3>",
        "vector<complex<m>, 2>",
    ] {
        let source = format!(
            "model M {{ domain body = box(0, 1, 0, 1); state x: {ty} on body; initial {{ x = 0; }} relation r on body {{ x - x = 0; }} }}"
        );
        let original = program(&source);
        let envelope = ModelEnvelope::from_program(&original).unwrap();
        let bytes = envelope.canonical_json().unwrap();
        let replay = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default()).unwrap();
        assert_eq!(replay.canonical_json().unwrap(), bytes);
        let field = original
            .nodes()
            .find_map(|node| match node {
                KernelNode::Field(field) => Some(field),
                _ => None,
            })
            .unwrap();
        assert_eq!(field.role(), eqiora_schema::kernel::FieldRole::State);
        assert!(
            original.nodes().any(
                |node| matches!(node, KernelNode::Relation(relation) if relation.is_initial())
            )
        );
        if field.value_type().scalar_domain() != ScalarDomain::Real || !field.shape().is_scalar() {
            let errors = eqiora_sem::Interpreter::new()
                .run(
                    &original,
                    eqiora_sem::ReferenceConfig::new(0.0, 0.01).unwrap(),
                )
                .unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message().contains("real scalar Fields"))
            );
        }
    }
}

fn parameter_program(value_type: ValueType) -> KernelProgram {
    let parameter = Id::new();
    typed_symbol_program(
        eqiora_schema::kernel::ParameterDef::new(
            parameter,
            eqiora_core::ValueLiteral::from_real(value_type, 0.0).unwrap(),
        )
        .into(),
        SymbolRef::Parameter(parameter),
    )
}

fn typed_symbol_program(node: KernelNode, symbol: SymbolRef) -> KernelProgram {
    let symbol_id = node.id();
    let relation = Id::new();
    let activation = Id::new();
    let model = OntologyId::new();
    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(symbol).unwrap();
    let root = expression.sub(value, value).unwrap();
    let nodes = [
        node,
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([root]).unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ];
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("typed parameter");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    transaction.push(Op::Connect {
        from: relation.erase(),
        to: symbol_id,
        edge: EdgeKind::DependsOn,
    });
    transaction.push(Op::Connect {
        from: activation.erase(),
        to: relation.erase(),
        edge: EdgeKind::Activates,
    });
    transaction.push(Op::DefineOntologyView { view: view.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
}

#[test]
fn signal_types_survive_model_replay_and_real_execution_rejects_richer_types() {
    let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
    let complex = ValueType::scalar(ScalarDomain::Complex, real.dimension());
    let mut fingerprints = std::collections::BTreeSet::new();
    for value_type in [
        real.clone(),
        real.array(3).unwrap(),
        complex.clone(),
        complex.array(3).unwrap(),
    ] {
        let port = Id::new();
        let program = typed_symbol_program(
            PortDef::signal(port, SignalDirection::Output, value_type.clone()).into(),
            SymbolRef::Port(port),
        );
        let bytes = ModelEnvelope::from_program(&program)
            .unwrap()
            .canonical_json()
            .unwrap();
        let replay = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default()).unwrap();
        assert_eq!(replay.canonical_json().unwrap(), bytes);
        assert!(
            fingerprints.insert(
                StructuralSemanticFingerprint::from_program(&program)
                    .unwrap()
                    .digest()
                    .to_owned()
            )
        );
        if value_type.scalar_domain() == ScalarDomain::Complex || !value_type.shape().is_scalar() {
            let errors = eqiora_sem::Interpreter::new()
                .run(
                    &program,
                    eqiora_sem::ReferenceConfig::new(0.0, 0.01).unwrap(),
                )
                .unwrap_err();
            assert!(
                errors
                    .iter()
                    .any(|error| error.message().contains("real scalar signal Ports"))
            );
        }
    }
}

#[test]
fn constant_types_survive_model_replay_and_change_structural_identity() {
    let scalar = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
    let complex = ValueType::scalar(ScalarDomain::Complex, scalar.dimension());
    let mut fingerprints = std::collections::BTreeSet::new();
    for value_type in [scalar, complex.clone(), complex.array(3).unwrap()] {
        let relation = Id::new();
        let activation = Id::new();
        let model = OntologyId::new();
        let mut builder = ExprDagBuilder::new();
        let root = builder
            .constant(eqiora_core::ValueLiteral::from_real(value_type, 0.0).unwrap())
            .unwrap();
        let nodes = [
            KernelNode::from(RelationDef::new(relation, builder.finish([root]).unwrap())),
            KernelNode::from(ActivationDef::continuous(activation)),
        ];
        let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
        let mut transaction = Transaction::new("typed constant");
        for node in nodes {
            transaction.push(Op::DefineKernelNode { node });
        }
        transaction.push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        });
        transaction.push(Op::DefineOntologyView { view: view.into() });
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
        let fingerprint = StructuralSemanticFingerprint::from_program(&program).unwrap();
        assert!(fingerprints.insert(fingerprint.to_string()));
        let bytes = ModelEnvelope::from_program(&program)
            .unwrap()
            .canonical_json()
            .unwrap();
        let decoded = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default()).unwrap();
        assert_eq!(decoded.canonical_json().unwrap(), bytes);
        assert_eq!(
            StructuralSemanticFingerprint::from_program(&decoded.to_program().unwrap()).unwrap(),
            fingerprint,
        );
    }
}

#[test]
fn admitted_parameter_types_survive_replay_and_remain_distinct() {
    let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
    let complex = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS);
    let mut fingerprints = std::collections::BTreeSet::new();
    for value_type in [
        real,
        complex.clone(),
        complex.clone().array(1).unwrap(),
        complex.array(3).unwrap(),
    ] {
        let program = parameter_program(value_type.clone());
        let relation = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Relation(relation) => Some(relation.id()),
                _ => None,
            })
            .unwrap();
        let typed = program.typed_relation_residual(relation).unwrap();
        assert!(
            typed
                .node_types()
                .iter()
                .all(|value| value.value_type == value_type)
        );
        let fingerprint = StructuralSemanticFingerprint::from_program(&program).unwrap();
        assert!(fingerprints.insert(fingerprint.to_string()));
        let envelope = ModelEnvelope::from_program(&program).unwrap();
        let bytes = envelope.canonical_json().unwrap();
        let decoded = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default()).unwrap();
        assert_eq!(decoded.canonical_json().unwrap(), bytes);
        let replay = decoded.to_program().unwrap();
        assert!(structurally_equivalent(&program, &replay).unwrap());
        let parameter = replay
            .nodes()
            .find_map(|node| match node {
                KernelNode::Parameter(parameter) => Some(parameter),
                _ => None,
            })
            .unwrap();
        assert_eq!(parameter.value_type(), &value_type);
    }
}
fn spatial_program(value_type: ValueType) -> Result<KernelProgram, Vec<Diagnostic>> {
    let domain = Id::new();
    let representation = Id::new();
    let field = Id::new();
    let relation = Id::new();
    let activation = Id::new();
    let model = OntologyId::new();
    let mut expression = ExprDagBuilder::new();
    let value = expression.symbol(SymbolRef::Field(field)).unwrap();
    let root = expression.sub(value, value).unwrap();
    let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap();
    let bounds =
        AxisBounds::new(DynQuantity::new(0.0, length), DynQuantity::new(1.0, length)).unwrap();
    let nodes = [
        KernelNode::from(DomainDef::cartesian_box(domain, vec![bounds, bounds]).unwrap()),
        KernelNode::from(RepresentationDef::continuum(representation)),
        KernelNode::from(FieldDef::new(
            field,
            value_type,
            eqiora_schema::kernel::FieldRole::Variable,
        )),
        KernelNode::from(RelationDef::new(
            relation,
            expression.finish([root]).unwrap(),
        )),
        KernelNode::from(ActivationDef::continuous(activation)),
    ];
    let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
    let mut transaction = Transaction::new("typed spatial field");
    for node in nodes {
        transaction.push(Op::DefineKernelNode { node });
    }
    for target in [domain.erase(), representation.erase()] {
        transaction.push(Op::Connect {
            from: field.erase(),
            to: target,
            edge: EdgeKind::DefinedOn,
        });
    }
    for (from, to, edge) in [
        (relation.erase(), domain.erase(), EdgeKind::AppliesOn),
        (relation.erase(), field.erase(), EdgeKind::DependsOn),
        (activation.erase(), relation.erase(), EdgeKind::Activates),
    ] {
        transaction.push(Op::Connect { from, to, edge });
    }
    transaction.push(Op::DefineOntologyView { view: view.into() });
    let mut store = InMemoryGraphStore::new();
    store.commit(transaction).unwrap();
    KernelProgram::from_snapshot(&store.snapshot(), model)
}

#[test]
fn admitted_models_keep_type_identity_through_exact_replay_and_semantic_comparison() {
    let spatial = |domain, extents| {
        ValueType::shaped(
            domain,
            DimExponents::DIMENSIONLESS,
            ValueShape::new(extents).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap()
    };
    let mut fingerprints = std::collections::BTreeSet::new();
    for value_type in [
        spatial(ScalarDomain::Real, vec![2, 2]),
        spatial(ScalarDomain::Complex, vec![2, 2]),
        spatial(ScalarDomain::Complex, vec![2]).array(2).unwrap(),
        // Channel count is unrelated to the two-dimensional spatial support.
        spatial(ScalarDomain::Complex, vec![2]).array(3).unwrap(),
    ] {
        let program = spatial_program(value_type.clone()).unwrap();
        let fingerprint = StructuralSemanticFingerprint::from_program(&program).unwrap();
        assert!(fingerprints.insert(fingerprint.to_string()));
        let fresh = spatial_program(value_type).unwrap();
        assert!(structurally_equivalent(&program, &fresh).unwrap());
        let envelope = ModelEnvelope::from_program(&program).unwrap();
        let bytes = envelope.canonical_json().unwrap();
        let decoded = ModelEnvelope::from_json(&bytes, ModelDecoderLimits::default()).unwrap();
        assert_eq!(decoded.canonical_json().unwrap(), bytes);
        assert_eq!(decoded.digest().unwrap(), envelope.digest().unwrap());
        let replayed = decoded.to_program().unwrap();
        assert_eq!(
            StructuralSemanticFingerprint::from_program(&replayed).unwrap(),
            fingerprint
        );
    }
}

#[test]
fn array_elements_still_require_the_exact_spatial_extent() {
    let wrong = ValueType::shaped(
        ScalarDomain::Complex,
        DimExponents::DIMENSIONLESS,
        ValueShape::new([3]).unwrap(),
        ValueFrame::SpatialCartesian,
    )
    .unwrap()
    .array(2)
    .unwrap();
    let errors = spatial_program(wrong).unwrap_err();
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("Cartesian spatial Field extents must equal its Domain ambient dimension")
    }));
}

#[test]
fn source_field_types_reach_semantic_admission_and_exact_model_replay() {
    for value_type in [
        "complex<V>",
        "vector<complex<V>, 2>",
        "array<vector<complex<V>, 2>, 3>",
        "tensor<Pa, 2, 2>",
    ] {
        let source = format!(
            r#"
model Typed {{
  domain body = box(0, 1, 0, 1);
  variable value: {value_type} on body;
  relation balance on body {{ value - value = 0; }}
}}
"#
        );
        let program = program(&source);
        let envelope = ModelEnvelope::from_program(&program).unwrap();
        let replayed = ModelEnvelope::from_json(
            &envelope.canonical_json().unwrap(),
            ModelDecoderLimits::default(),
        )
        .unwrap()
        .to_program()
        .unwrap();
        assert!(structurally_equivalent(&program, &replayed).unwrap());
        let expected_domain = if value_type.contains("complex") {
            ScalarDomain::Complex
        } else {
            ScalarDomain::Real
        };
        let field = replayed
            .nodes()
            .find_map(|node| match node {
                KernelNode::Field(field) => Some(field),
                _ => None,
            })
            .unwrap();
        assert_eq!(field.value_type().scalar_domain(), expected_domain);
    }
}

#[test]
fn literal_projection_preserves_imaginary_channel_order_and_type() {
    fn project(value: &eqiora_core::ValueLiteral) -> Vec<u8> {
        let mut encoder = Encoder::new(4096);
        encode_literal(&mut encoder, value).unwrap();
        encoder.finish().unwrap()
    }
    let complex = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
        .array(2)
        .unwrap();
    let baseline = project(
        &eqiora_core::ValueLiteral::new(complex.clone(), [(1.0, 2.0), (3.0, 4.0)]).unwrap(),
    );
    let mut expected_tail = vec![1]; // nonzero dense payload
    expected_tail.extend_from_slice(&2_u64.to_be_bytes());
    for component in [1.0_f64, 2.0, 3.0, 4.0] {
        expected_tail.extend_from_slice(&component.to_bits().to_be_bytes());
    }
    assert!(baseline.ends_with(&expected_tail));
    for values in [
        [(1.0, 5.0), (3.0, 4.0)],
        [(3.0, 4.0), (1.0, 2.0)],
        [(2.0, 1.0), (3.0, 4.0)],
    ] {
        assert_ne!(
            baseline,
            project(&eqiora_core::ValueLiteral::new(complex.clone(), values).unwrap())
        );
    }
    let real = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
        .array(2)
        .unwrap();
    let values = [(1.0, 0.0), (3.0, 0.0)];
    assert_ne!(
        project(&eqiora_core::ValueLiteral::new(real, values).unwrap()),
        project(&eqiora_core::ValueLiteral::new(complex, values).unwrap())
    );
    let huge = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
        .array(1_000_000_000)
        .unwrap();
    assert!(project(&eqiora_core::ValueLiteral::from_real(huge, 0.0).unwrap()).len() < 128);
}

#[test]
fn model_and_transaction_limits_charge_all_typed_payload_occurrences() {
    use crate::ModelTransactionEnvelope;
    use eqiora_core::ValueLiteral;
    use eqiora_graph::Precondition;
    let parameter = Id::new();
    let ty = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS)
        .array(2)
        .unwrap();
    let value = ValueLiteral::new(ty, [(1.0, 2.0), (3.0, 4.0)]).unwrap();
    let program = typed_symbol_program(
        eqiora_schema::kernel::ParameterDef::new(parameter, value.clone()).into(),
        SymbolRef::Parameter(parameter),
    );
    let bytes = ModelEnvelope::from_program(&program)
        .unwrap()
        .canonical_json()
        .unwrap();
    let limits = ModelDecoderLimits {
        max_value_literal_components: 3,
        ..Default::default()
    };
    // Definition and current value each own two component pairs: total four.
    assert!(ModelEnvelope::from_json(&bytes, limits).is_err());
    let limits = ModelDecoderLimits {
        max_value_literal_components: 4,
        ..limits
    };
    let replay = ModelEnvelope::from_json(&bytes, limits)
        .unwrap()
        .to_program()
        .unwrap();
    assert_eq!(replay.typed_value(parameter.erase()), Some(&value));
    let mut transaction = Transaction::new("typed before and after");
    transaction.require(Precondition::ValueEquals {
        target: parameter.erase(),
        expected: value.clone(),
    });
    let changed = ValueLiteral::new(value.value_type().clone(), [(1.0, 5.0), (3.0, 4.0)]).unwrap();
    transaction.push(Op::SetValue {
        target: parameter.erase(),
        value: changed.clone(),
    });
    let bytes = ModelTransactionEnvelope::from_transaction(&transaction)
        .unwrap()
        .canonical_json()
        .unwrap();
    assert!(
        ModelTransactionEnvelope::from_json(
            &bytes,
            ModelDecoderLimits {
                max_value_literal_components: 3,
                ..limits
            }
        )
        .is_err()
    );
    let decoded = ModelTransactionEnvelope::from_json(&bytes, limits)
        .unwrap()
        .to_transaction()
        .unwrap();
    assert_eq!(decoded.ops(), transaction.ops());
    assert_eq!(decoded.preconditions(), transaction.preconditions());
    let (seed, model) = ModelEnvelope::from_program(&program)
        .unwrap()
        .to_transaction()
        .unwrap();
    let mut store = InMemoryGraphStore::new();
    store.commit(seed).unwrap();
    store.commit(decoded).unwrap();
    let edited = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
    assert_eq!(edited.typed_value(parameter.erase()), Some(&changed));
    let replay = ModelEnvelope::from_program(&edited)
        .unwrap()
        .to_program()
        .unwrap();
    assert_eq!(replay.typed_value(parameter.erase()), Some(&changed));
    assert_ne!(
        StructuralSemanticFingerprint::from_program(&program).unwrap(),
        StructuralSemanticFingerprint::from_program(&replay).unwrap()
    );
}

#[test]
fn typed_expression_edges_and_sharing_affect_structural_identity() {
    fn model(reverse: bool, selection: u32, swap_complex: bool, duplicate: bool) -> KernelProgram {
        let mut builder = ExprDagBuilder::new();
        let one = builder
            .constant(DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))
            .unwrap();
        let two = builder
            .constant(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
            .unwrap();
        let repeated = if duplicate {
            builder
                .constant(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
                .unwrap()
        } else {
            two
        };
        let array = builder
            .array(if reverse {
                [two, one, repeated]
            } else {
                [one, two, repeated]
            })
            .unwrap();
        let selected = builder.index(array, selection).unwrap();
        let root = if swap_complex {
            builder.complex(two, selected)
        } else {
            builder.complex(selected, two)
        }
        .unwrap();
        let relation = Id::new();
        let activation = Id::new();
        let model = OntologyId::new();
        let nodes = [
            KernelNode::from(RelationDef::new(relation, builder.finish([root]).unwrap())),
            KernelNode::from(ActivationDef::continuous(activation)),
        ];
        let view = ModelView::new(model, nodes.iter().map(KernelNode::id), []).unwrap();
        let mut transaction = Transaction::new("typed DAG");
        for node in nodes {
            transaction.push(Op::DefineKernelNode { node });
        }
        transaction.push(Op::Connect {
            from: activation.erase(),
            to: relation.erase(),
            edge: EdgeKind::Activates,
        });
        transaction.push(Op::DefineOntologyView { view: view.into() });
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        KernelProgram::from_snapshot(&store.snapshot(), model).unwrap()
    }
    let baseline =
        StructuralSemanticFingerprint::from_program(&model(false, 0, false, false)).unwrap();
    assert_eq!(
        baseline,
        StructuralSemanticFingerprint::from_program(&model(false, 0, false, false)).unwrap()
    );
    for variant in [
        (true, 0, false, false),
        (false, 1, false, false),
        (false, 0, true, false),
        (false, 0, false, true),
    ] {
        let changed = model(variant.0, variant.1, variant.2, variant.3);
        assert_ne!(
            baseline,
            StructuralSemanticFingerprint::from_program(&changed).unwrap()
        );
        let replay = ModelEnvelope::from_program(&changed)
            .unwrap()
            .to_program()
            .unwrap();
        assert_eq!(
            StructuralSemanticFingerprint::from_program(&changed).unwrap(),
            StructuralSemanticFingerprint::from_program(&replay).unwrap()
        );
    }
}
