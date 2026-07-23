use eqiora_core::OntologyId;
use eqiora_schema::Model;

use crate::RealizationPlan;

/// Revision of mathematical meaning in the Semantic Model Graph.
///
/// Semantic and realization revisions cannot be exchanged accidentally:
///
/// ```compile_fail
/// use eqiora_core::OntologyId;
/// use eqiora_realization::{DefaultPolicyVersion, RealizationRequest, RealizationRevision};
/// use eqiora_schema::Model;
///
/// RealizationRequest::default(
///     OntologyId::<Model>::new(),
///     RealizationRevision::new(3),
///     DefaultPolicyVersion::V0,
/// );
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SemanticRevision(u64);

impl SemanticRevision {
    /// Construct from a Graph Federation revision number.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Underlying revision number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Independently advancing revision of a Realization Graph selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RealizationRevision(u64);

impl RealizationRevision {
    /// Construct from a Realization Graph revision number.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Underlying revision number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Version of a named project default policy, not an artifact schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DefaultPolicyVersion(u16);

impl DefaultPolicyVersion {
    /// The only default policy implemented by this prototype.
    pub const V0: Self = Self(0);

    /// Construct a version for explicit compatibility checking.
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    /// Numeric policy version.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// One request binding semantic identity to an independent realization choice.
#[derive(Debug, Clone, PartialEq)]
pub struct RealizationRequest {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    selection: Selection,
}

impl RealizationRequest {
    /// Request a named default without inventing a Realization Graph revision.
    #[must_use]
    pub const fn default(
        model: OntologyId<Model>,
        semantic_revision: SemanticRevision,
        policy: DefaultPolicyVersion,
    ) -> Self {
        Self {
            model,
            semantic_revision,
            selection: Selection::Default(policy),
        }
    }

    /// Request an explicit plan at an independently identified realization revision.
    #[must_use]
    pub const fn explicit(
        model: OntologyId<Model>,
        semantic_revision: SemanticRevision,
        realization_revision: RealizationRevision,
        plan: RealizationPlan,
    ) -> Self {
        Self {
            model,
            semantic_revision,
            selection: Selection::Explicit {
                realization_revision,
                plan,
            },
        }
    }

    /// Semantic model identity.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.model
    }

    /// Semantic revision, never inferred from a realization revision.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        self.semantic_revision
    }

    /// Explicit realization revision, absent for a named default.
    #[must_use]
    pub const fn realization_revision(&self) -> Option<RealizationRevision> {
        match self.selection {
            Selection::Default(_) => None,
            Selection::Explicit {
                realization_revision,
                ..
            } => Some(realization_revision),
        }
    }

    pub(crate) const fn selection(&self) -> &Selection {
        &self.selection
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Selection {
    Default(DefaultPolicyVersion),
    Explicit {
        realization_revision: RealizationRevision,
        plan: RealizationPlan,
    },
}
