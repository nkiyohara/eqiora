//! Process-local identity for immutable draft declarations.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct DraftSymbol(Arc<()>);

impl DraftSymbol {
    pub(crate) fn new() -> Self {
        Self(Arc::new(()))
    }
}

impl fmt::Debug for DraftSymbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DraftSymbol(<local>)")
    }
}

impl PartialEq for DraftSymbol {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for DraftSymbol {}

impl Hash for DraftSymbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}
