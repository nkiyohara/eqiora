//! Nominal value dimensions must name the selected declaration and its exact extent.
use super::*;
use eqiora_core::ValueType;

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
    if let Some(space) = value.finite_space() {
        let matches = matches!(nodes.get(&space.erase()), Some(KernelNode::FiniteSpace(definition)) if value.shape().extents().last().is_some_and(|extent| extent.get() as usize == definition.labels().len()));
        if !matches {
            diagnostics.push(kernel_error(
                owner,
                "nominal value requires its exact selected FiniteSpace declaration and cardinality",
            ));
        }
    }
    if let Some(index) = value.index_set() {
        let matches = matches!(nodes.get(&index.erase()), Some(KernelNode::IndexSet(definition)) if value.index_extent() == Some(definition.extent()));
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
    use eqiora_schema::kernel::{FieldDef, FieldRole, FiniteSpaceDef, IndexSetDef};

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
}
