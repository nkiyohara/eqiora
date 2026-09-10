//! Exclusive lowered mathematical Relation bodies.

use super::{LoweringEquation, LoweringExpression};

#[derive(Debug, Clone)]
pub(crate) enum LoweringRelationBody {
    Equations(Vec<LoweringEquation>),
    Conservation {
        flux: LoweringExpression,
        source: LoweringExpression,
    },
}

impl From<Vec<LoweringEquation>> for LoweringRelationBody {
    fn from(value: Vec<LoweringEquation>) -> Self {
        Self::Equations(value)
    }
}

impl LoweringRelationBody {
    pub(crate) fn expressions(&self) -> impl Iterator<Item = &LoweringExpression> {
        let (conditions, flux, source) = match self {
            Self::Equations(values) => (values.as_slice(), None, None),
            Self::Conservation { flux, source } => (&[][..], Some(flux), Some(source)),
        };
        conditions
            .iter()
            .flat_map(|condition| [&condition.left, &condition.right])
            .chain(flux)
            .chain(source)
    }
}
