use super::*;
use crate::{ModelDecoderLimits, ModelEnvelope};
use eqiora_core::ScalarDomain;
use eqiora_schema::kernel::{AxisBounds, DomainDef, RepresentationDef, ValueType};

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
        KernelNode::from(FieldDef::new(field, value_type)),
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
  representation space = continuum;
  field value on body as space: {value_type};
  relation balance continuous on body {{ value - value = 0; }}
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
