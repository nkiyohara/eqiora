//! Exact mathematical meaning of Relation operands on the Model wire.
use crate::invalid_artifact;
use eqiora_core::{Diagnostic, Id, entity::kinds};
use eqiora_schema::kernel::{ConservationTerms, ExprDag, ExprId, RelationDef, RelationMeaning};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum WireRelationMeaning {
    Equations,
    Conservation { flux: u32, source: u32 },
}

impl WireRelationMeaning {
    pub(crate) fn encode(meaning: &RelationMeaning) -> Self {
        match meaning {
            RelationMeaning::Equations => Self::Equations,
            RelationMeaning::Conservation(terms) => Self::Conservation {
                flux: terms.flux().index(),
                source: terms.source().index(),
            },
        }
    }

    pub(crate) fn decode(
        &self,
        id: Id<kinds::Relation>,
        expression: ExprDag,
        initial: bool,
    ) -> Result<RelationDef, Diagnostic> {
        let result = match self {
            Self::Equations if initial => RelationDef::initial(id, expression),
            Self::Equations => RelationDef::new(id, expression),
            Self::Conservation { flux, source } => {
                if initial {
                    return Err(invalid_artifact(
                        "conservation Law cannot be an initialization-only Relation",
                    ));
                }
                let lookup = |index: u32| -> Result<ExprId, Diagnostic> {
                    expression.node_id(index).ok_or_else(|| {
                        invalid_artifact("Law term index is outside its owning Relation DAG")
                    })
                };
                let terms = ConservationTerms::new(lookup(*flux)?, lookup(*source)?);
                RelationDef::conservation(id, expression, terms)
            }
        };
        result.map_err(|error| invalid_artifact(error.message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{DimExponents, DynQuantity};
    use eqiora_schema::kernel::{ExprDagBuilder, ExprNode};

    #[test]
    fn exact_condition_kind_and_operand_order_survive_wire() {
        let mut builder = ExprDagBuilder::new();
        let gap = builder
            .constant(DynQuantity::new(
                3.0,
                DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap(),
            ))
            .unwrap();
        let force = builder
            .constant(DynQuantity::new(
                0.0,
                DimExponents::from_integers([1, 1, -2, 0, 0, 0, 0]).unwrap(),
            ))
            .unwrap();
        let expression = builder.finish([gap, force]).unwrap();
        let relation = RelationDef::new(Id::new(), expression.clone()).unwrap();
        let wire = WireRelationMeaning::encode(relation.meaning());
        assert_eq!(
            wire.decode(relation.id(), expression.clone(), false)
                .unwrap(),
            relation
        );
    }

    #[test]
    fn conservation_wire_binds_exact_term_nodes_and_rejects_foreign_indices() {
        let mut builder = ExprDagBuilder::new();
        let flux = builder
            .constant(DynQuantity::new(2.0, DimExponents::DIMENSIONLESS))
            .unwrap();
        let divergence = builder.push(ExprNode::Divergence(flux)).unwrap();
        let source = builder
            .constant(DynQuantity::new(0.0, DimExponents::DIMENSIONLESS))
            .unwrap();
        let expression = builder.finish([divergence, source]).unwrap();
        let relation = RelationDef::conservation(
            Id::new(),
            expression.clone(),
            ConservationTerms::new(flux, source),
        )
        .unwrap();
        let wire = WireRelationMeaning::encode(relation.meaning());
        assert_eq!(
            wire.decode(relation.id(), expression.clone(), false)
                .unwrap(),
            relation
        );
        let bad = WireRelationMeaning::Conservation {
            flux: u32::MAX,
            source: source.index(),
        };
        assert!(bad.decode(relation.id(), expression, false).is_err());
    }
}
