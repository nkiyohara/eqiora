//! Exclusive lowered mathematical Relation bodies.

use super::{LoweringEquation, LoweringExpression};

#[derive(Debug, Clone)]
pub(crate) enum LoweringRelationBody {
    Conditions(Vec<LoweringEquation>),
    Conservation {
        storage: Option<LoweringExpression>,
        flux: LoweringExpression,
        source: LoweringExpression,
    },
}

impl From<Vec<LoweringEquation>> for LoweringRelationBody {
    fn from(value: Vec<LoweringEquation>) -> Self {
        Self::Conditions(value)
    }
}

impl LoweringRelationBody {
    pub(crate) fn expressions(&self) -> impl Iterator<Item = &LoweringExpression> {
        let (conditions, storage, flux, source) = match self {
            Self::Conditions(values) => (values.as_slice(), None, None, None),
            Self::Conservation {
                storage,
                flux,
                source,
            } => (&[][..], storage.as_ref(), Some(flux), Some(source)),
        };
        conditions
            .iter()
            .flat_map(|condition| [&condition.left, &condition.right])
            .chain(storage)
            .chain(flux)
            .chain(source)
    }
}
