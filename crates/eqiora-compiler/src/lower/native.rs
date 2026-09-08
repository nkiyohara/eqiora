//! Native declarations enter the ordinary complete-document compiler pipeline.
use super::*;

pub(super) fn lower(draft: &ModelDraft) -> Result<CompiledModel, Vec<Diagnostic>> {
    let native = draft.native_ast();
    let mut compiled =
        crate::hierarchy::selected::native_document(&native).map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| crate::diagnostics::native_diagnostic(draft, &native, diagnostic))
                .collect::<Vec<_>>()
        })?;
    // Synthetic native ranges are not authored source provenance.
    compiled.provenance = None;
    Ok(compiled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{ScalarDomain, ValueLiteral, ValueType};
    use eqiora_lang::{
        DraftDeclaration, DraftField, DraftParameter, DraftRelation, FieldRoleSyntax,
    };
    use eqiora_schema::kernel::{FiniteSpaceDef, IndexSetDef};

    #[test]
    fn native_nominal_definitions_keep_supplied_ids_and_fresh_model_occurrences() {
        let space = FiniteSpaceDef::new(Id::new(), ["A".to_owned(), "B".to_owned()]).unwrap();
        let rows = IndexSetDef::new(Id::new(), 3).unwrap();
        let counts = ValueLiteral::integer(space.counts(), [2, 9_007_199_254_740_993]).unwrap();
        let selected = ValueLiteral::from_integer(rows.value_type(), 2).unwrap();
        let observed = DraftField::new(
            "observed",
            ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
                .expect("admitted numeric scalar type"),
            FieldRoleSyntax::Variable,
        );
        let relation = DraftRelation::continuous(
            "observe",
            [(
                observed.expression(),
                eqiora_lang::DraftExpression::constant(
                    eqiora_lang::DecimalLiteral::parse("0").unwrap(),
                ),
            )],
        );
        let draft = ModelDraft::new(
            "Native",
            [
                DraftDeclaration::FiniteSpace {
                    name: "Species".into(),
                    definition: space.clone(),
                },
                DraftDeclaration::IndexSet {
                    name: "Rows".into(),
                    definition: rows.clone(),
                },
                DraftParameter::new("population", counts.clone()).into(),
                DraftParameter::new("selected", selected.clone()).into(),
                observed.into(),
                relation.into(),
            ],
        )
        .unwrap();
        let first = lower(&draft).unwrap();
        let second = lower(&draft).unwrap();
        assert_ne!(first.model(), second.model());
        assert_ne!(
            first.symbols().get("observed"),
            second.symbols().get("observed")
        );
        for model in [first, second] {
            assert!(model.provenance().is_none());
            assert_eq!(model.symbols().get("Species"), Some(space.id().erase()));
            assert_eq!(model.symbols().get("Rows"), Some(rows.id().erase()));
            for (name, value) in [("population", &counts), ("selected", &selected)] {
                let id = model.symbols().get(name).unwrap();
                let actual = model.transaction().ops().iter().find_map(|op| match op {
                    Op::DefineKernelNode {
                        node: KernelNode::Parameter(parameter),
                    } if parameter.id().erase() == id => Some(parameter.value()),
                    _ => None,
                });
                assert_eq!(actual, Some(value));
            }
            assert!(model.transaction().ops().iter().any(|op| matches!(op,
                Op::DefineKernelNode {node: KernelNode::FiniteSpace(value)} if value==&space)));
            assert!(model.transaction().ops().iter().any(|op| matches!(op,
                Op::DefineKernelNode {node: KernelNode::IndexSet(value)} if value==&rows)));
        }
    }
}
