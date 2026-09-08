//! Nominal value dimensions must name the selected declaration and its exact extent.
use super::*;
use eqiora_core::{ScalarDomain, ValueFrame, ValueType};

pub(super) fn validate(nodes: &BTreeMap<RawId, KernelNode>, diagnostics: &mut Vec<Diagnostic>) {
    for (&owner, node) in nodes {
        match node {
            KernelNode::Field(field) => check(owner, field.value_type(), nodes, diagnostics),
            KernelNode::Parameter(parameter) => {
                check(owner, parameter.value_type(), nodes, diagnostics)
            }
            KernelNode::Port(port) => {
                if let Some((_, value_type)) = port.signal_contract() {
                    check(owner, value_type, nodes, diagnostics);
                }
            }
            KernelNode::Domain(domain) => {
                if let DomainKind::ScalarPhysical {
                    across_type,
                    through_type,
                } = domain.kind()
                {
                    check(owner, across_type, nodes, diagnostics);
                    check(owner, through_type, nodes, diagnostics);
                }
            }
            _ => {}
        }
    }
}

pub(super) fn check(
    owner: RawId,
    value: &ValueType,
    nodes: &BTreeMap<RawId, KernelNode>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if value.scalar_domain() == ScalarDomain::Integer
        && (value.dimension() != DimExponents::DIMENSIONLESS
            || value.frame() != ValueFrame::Invariant)
    {
        diagnostics.push(kernel_error(
            owner,
            "integer values require dimensionless invariant types",
        ));
    }
    if let Some(space) = value.finite_space() {
        let matches = matches!(nodes.get(&space.erase()), Some(KernelNode::FiniteSpace(definition)) if *value == if value.is_count() { definition.counts() } else { definition.coordinates() });
        if !matches {
            diagnostics.push(kernel_error(
                owner,
                "nominal value requires its exact selected FiniteSpace declaration and cardinality",
            ));
        }
    }
    if let Some(index) = value.index_set() {
        let matches = matches!(nodes.get(&index.erase()), Some(KernelNode::IndexSet(definition)) if *value == definition.value_type());
        if !matches {
            diagnostics.push(kernel_error(
                owner,
                "index value requires its exact selected IndexSet declaration and bound",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_schema::kernel::{
        FieldDef, FieldRole, FiniteSpaceDef, IndexSetDef, PortDef, SignalDirection,
    };

    #[test]
    fn nominal_cardinality_and_selected_identity_are_both_required() {
        let space = FiniteSpaceDef::new(Id::new(), ["A".into(), "B".into()]).unwrap();
        let index = IndexSetDef::new(Id::new(), 3).unwrap();
        let field = Id::<kinds::Field>::new();
        let mut nodes = BTreeMap::from([
            (space.id().erase(), space.clone().into()),
            (index.id().erase(), index.clone().into()),
        ]);
        for (value, expected) in [
            (space.counts(), true),
            (ValueType::counts(space.id(), 3).unwrap(), false),
            (ValueType::counts(Id::new(), 2).unwrap(), false),
            (ValueType::index(index.id(), 3).unwrap(), true),
            (ValueType::index(index.id(), 2).unwrap(), false),
            (ValueType::index(Id::new(), 3).unwrap(), false),
        ] {
            nodes.insert(
                field.erase(),
                FieldDef::new(field, value, FieldRole::State).into(),
            );
            let mut errors = Vec::new();
            validate(&nodes, &mut errors);
            assert_eq!(errors.is_empty(), expected);
        }
    }
    #[test]
    fn discrete_state_has_no_continuous_derivative() {
        let field = Id::<kinds::Field>::new();
        let value = ValueType::scalar(
            eqiora_core::ScalarDomain::Integer,
            DimExponents::DIMENSIONLESS,
        );
        let nodes = BTreeMap::from([(
            field.erase(),
            FieldDef::new(field, value, FieldRole::State).into(),
        )]);
        assert!(symbol_type(SymbolRef::Derivative(field), &nodes, &[], &BTreeMap::new()).is_err());
    }
    #[test]
    fn field_and_port_integer_types_reject_units_and_spatial_frames_without_literals() {
        let space = FiniteSpaceDef::new(Id::new(), ["A".into(), "B".into()]).unwrap();
        let index = IndexSetDef::new(Id::new(), 3).unwrap();
        let field = Id::<kinds::Field>::new();
        let port = Id::<kinds::Port>::new();
        let seconds = DimExponents::from_integers([0, 0, 1, 0, 0, 0, 0]).unwrap();
        let ordinary = ValueType::scalar(ScalarDomain::Integer, DimExponents::DIMENSIONLESS);
        let mut cases = Vec::new();
        for value in [
            ordinary,
            space.counts(),
            space.coordinates(),
            index.value_type(),
        ] {
            cases.push((value.clone(), true));
            cases.push((value.with_dimension(seconds), false));
        }
        cases.push((
            ValueType::shaped(
                ScalarDomain::Integer,
                DimExponents::DIMENSIONLESS,
                eqiora_core::ValueShape::new([2]).unwrap(),
                ValueFrame::SpatialCartesian,
            )
            .unwrap(),
            false,
        ));
        for (value, expected) in cases {
            for node in [
                KernelNode::from(FieldDef::new(field, value.clone(), FieldRole::State)),
                KernelNode::from(PortDef::signal(
                    port,
                    SignalDirection::Output,
                    value.clone(),
                )),
            ] {
                let nodes = BTreeMap::from([
                    (space.id().erase(), space.clone().into()),
                    (index.id().erase(), index.clone().into()),
                    (node.id(), node),
                ]);
                let mut errors = Vec::new();
                validate(&nodes, &mut errors);
                assert_eq!(errors.is_empty(), expected, "type: {value:?}");
            }
        }
    }
    #[test]
    fn scalar_physical_port_domain_cannot_hide_dimensioned_integer_types() {
        let integer = ValueType::scalar(
            ScalarDomain::Integer,
            DimExponents::from_integers([0, 0, 1, 0, 0, 0, 0]).unwrap(),
        );
        let domain =
            eqiora_schema::kernel::DomainDef::scalar_physical(Id::new(), integer.clone(), integer)
                .unwrap();
        let port = PortDef::scalar_physical(Id::new(), domain.id());
        let nodes = BTreeMap::from([
            (domain.id().erase(), domain.into()),
            (port.id().erase(), port.into()),
        ]);
        let mut errors = Vec::new();
        validate(&nodes, &mut errors);
        assert_eq!(errors.len(), 2);
    }
}
