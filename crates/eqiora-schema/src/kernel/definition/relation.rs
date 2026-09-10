//! Typed mathematical relations, separate from numerical enforcement.

use crate::kernel::{ConservationTerms, ExprDag, ExprId};
use eqiora_core::{Diagnostic, Id, diagnostic::codes, entity::kinds};

/// Mathematical meaning of one ordered pair of expression roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RelationConditionKind {
    /// Both operands are equal and have the same type.
    Equality,
}

/// Exclusive mathematical owner of a Relation's retained expression roots.
#[derive(Debug, Clone, PartialEq)]
pub enum RelationMeaning {
    /// Ordered equality conditions.
    Conditions(Vec<RelationConditionKind>),
    /// One physical conservation balance with its exact authored terms.
    Conservation(ConservationTerms),
}

/// Ordered mathematical conditions retaining their authored operands.
/// Numerical enforcement is deliberately absent from the Model definition.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationDef {
    id: Id<kinds::Relation>,
    expression: ExprDag,
    meaning: RelationMeaning,
    initial: bool,
}

impl RelationDef {
    /// Define equalities from consecutive left/right expression root pairs.
    ///
    /// # Errors
    /// Rejects an odd number of roots.
    pub fn new(id: Id<kinds::Relation>, expression: ExprDag) -> Result<Self, Diagnostic> {
        let conditions = vec![RelationConditionKind::Equality; expression.roots().len() / 2];
        Self::with_conditions(id, expression, conditions)
    }

    /// Define typed mathematical conditions with one descriptor per root pair.
    /// Operand types and support are checked by semantic admission.
    ///
    /// # Errors
    /// Rejects unpaired roots or a mismatched descriptor count.
    pub fn with_conditions(
        id: Id<kinds::Relation>,
        expression: ExprDag,
        conditions: Vec<RelationConditionKind>,
    ) -> Result<Self, Diagnostic> {
        if !expression.roots().len().is_multiple_of(2)
            || conditions.len() != expression.roots().len() / 2
        {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Relation conditions require one descriptor per consecutive left/right root pair",
            ));
        }
        Ok(Self {
            id,
            expression,
            meaning: RelationMeaning::Conditions(conditions),
            initial: false,
        })
    }

    /// Define simultaneous equalities used only for fresh initialization.
    ///
    /// # Errors
    /// Rejects an odd number of roots.
    pub fn initial(id: Id<kinds::Relation>, expression: ExprDag) -> Result<Self, Diagnostic> {
        let mut result = Self::new(id, expression)?;
        result.initial = true;
        Ok(result)
    }

    /// Whether these conditions apply only to fresh initialization.
    #[must_use]
    pub const fn is_initial(&self) -> bool {
        self.initial
    }

    /// Exact mathematical Relation identity.
    #[must_use]
    pub const fn id(&self) -> Id<kinds::Relation> {
        self.id
    }

    /// Shared expression arena retaining all ordered condition operands.
    #[must_use]
    pub const fn expression(&self) -> &ExprDag {
        &self.expression
    }

    /// Mathematical descriptors in authored order.
    #[must_use]
    pub fn conditions(&self) -> &[RelationConditionKind] {
        match &self.meaning {
            RelationMeaning::Conditions(conditions) => conditions,
            RelationMeaning::Conservation(_) => &[RelationConditionKind::Equality],
        }
    }

    /// Exclusive mathematical meaning, including retained physical Law terms.
    #[must_use]
    pub const fn meaning(&self) -> &RelationMeaning {
        &self.meaning
    }

    /// Define a physical conservation Law with exact balance operands.
    ///
    /// # Errors
    /// Rejects a term descriptor that does not exactly name the balance roots.
    pub fn conservation(
        id: Id<kinds::Relation>,
        expression: ExprDag,
        terms: ConservationTerms,
    ) -> Result<Self, Diagnostic> {
        terms.validate_balance(&expression)?;
        Ok(Self {
            id,
            expression,
            meaning: RelationMeaning::Conservation(terms),
            initial: false,
        })
    }

    /// Ordered operands of the retained equations or physical balance.
    pub fn equation_sides(&self) -> impl ExactSizeIterator<Item = (ExprId, ExprId)> + '_ {
        self.expression
            .roots()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| (pair[0], pair[1]))
    }
}
