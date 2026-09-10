//! Typed mathematical relations, separate from numerical enforcement.

use crate::kernel::{ConservationTerms, ExprDag, ExprId};
use eqiora_core::{Diagnostic, Id, diagnostic::codes, entity::kinds};

/// Exclusive mathematical owner of a Relation's retained expression roots.
#[derive(Debug, Clone, PartialEq)]
pub enum RelationMeaning {
    /// Ordered equality conditions.
    Equations,
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
        if !expression.roots().len().is_multiple_of(2) {
            return Err(Diagnostic::error(
                codes::INVALID_KERNEL_DEFINITION,
                "Relation equations require consecutive left/right root pairs",
            ));
        }
        Ok(Self {
            id,
            expression,
            meaning: RelationMeaning::Equations,
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
