use std::collections::BTreeSet;

use eqiora_core::{Diagnostic, OntologyId};
use eqiora_schema::Model;
use eqiora_solver::LinearOperatorProperties;

use crate::{
    DefaultPolicyVersion, RealizationCapabilities, RealizationPlan, RealizationRequest,
    RealizationRequirements, RealizationRevision, Selection, SemanticRevision, default_plan_v0,
    invalid_realization,
};

/// Origin of a resolved plan, kept separate from plan equality.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionSource {
    /// Project default selected by version.
    Default(DefaultPolicyVersion),
    /// Explicit Realization Graph revision.
    Explicit(RealizationRevision),
}

/// Semantic and independently revisioned Realization lineage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealizationLineage {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    source: ResolutionSource,
}

impl RealizationLineage {
    pub(crate) const fn new(
        model: OntologyId<Model>,
        semantic_revision: SemanticRevision,
        source: ResolutionSource,
    ) -> Self {
        Self {
            model,
            semantic_revision,
            source,
        }
    }

    /// Construct explicit Model and Realization lineage for a graph-native resolver.
    #[must_use]
    pub const fn explicit(
        model: OntologyId<Model>,
        semantic_revision: SemanticRevision,
        realization_revision: RealizationRevision,
    ) -> Self {
        Self::new(
            model,
            semantic_revision,
            ResolutionSource::Explicit(realization_revision),
        )
    }

    /// Exact Semantic Model identity.
    #[must_use]
    pub const fn model(self) -> OntologyId<Model> {
        self.model
    }

    /// Exact Semantic Model revision.
    #[must_use]
    pub const fn semantic_revision(self) -> SemanticRevision {
        self.semantic_revision
    }

    /// Named-default or independent explicit Realization revision.
    #[must_use]
    pub const fn source(self) -> ResolutionSource {
        self.source
    }
}

/// Validated plan plus the two-layer revision provenance used to obtain it.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedRealization {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    source: ResolutionSource,
    requirements: RealizationRequirements,
    plan: RealizationPlan,
    admitted_operator_properties: BTreeSet<LinearOperatorProperties>,
}

impl ResolvedRealization {
    /// Semantic model identity.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.model
    }

    /// Semantic revision.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        self.semantic_revision
    }

    /// Selection origin.
    #[must_use]
    pub const fn source(&self) -> ResolutionSource {
        self.source
    }

    /// Model/lowering facts against which the plan was admitted.
    #[must_use]
    pub const fn requirements(&self) -> RealizationRequirements {
        self.requirements
    }

    /// Validated realization plan.
    #[must_use]
    pub const fn plan(&self) -> &RealizationPlan {
        &self.plan
    }

    /// Require one equation-aware operator property admitted by this legacy
    /// resolution.
    ///
    /// Compatibility [`RealizationPlan`] does not contain an operator
    /// assertion. Resolution therefore retains the properties of every exact
    /// matching capability tuple. A numerical finalizer that does not project
    /// through the portable graph must call this before materializing its
    /// equation-specific operator.
    ///
    /// # Errors
    /// Returns `EQ0807` when the property was not present in the retained exact
    /// capability candidates.
    pub fn require_admitted_operator_properties(
        &self,
        operator_properties: LinearOperatorProperties,
    ) -> Result<(), Diagnostic> {
        if self
            .admitted_operator_properties
            .contains(&operator_properties)
        {
            return Ok(());
        }
        Err(invalid_realization(format!(
            "resolved realization was not admitted for operator properties {operator_properties:?}",
        )))
    }
}

/// Resolve one request without mutating the semantic model or silently falling back.
///
/// # Errors
/// Returns `EQ0807` for an unknown default policy version, contradictory plan,
/// or unsupported backend capability. Explicit failures never become defaults.
pub fn resolve(
    request: &RealizationRequest,
    requirements: RealizationRequirements,
    capabilities: &RealizationCapabilities,
) -> Result<ResolvedRealization, Diagnostic> {
    let (source, plan) = match request.selection() {
        Selection::Default(version) => {
            if *version != DefaultPolicyVersion::V0 {
                return Err(invalid_realization(format!(
                    "unknown default realization policy version {}",
                    version.get()
                )));
            }
            (ResolutionSource::Default(*version), default_plan_v0()?)
        }
        Selection::Explicit {
            realization_revision,
            plan,
        } => {
            plan.validate()?;
            (
                ResolutionSource::Explicit(*realization_revision),
                plan.clone(),
            )
        }
    };
    let admitted_operator_properties = capabilities.supports(requirements, &plan)?;
    Ok(ResolvedRealization {
        model: request.model(),
        semantic_revision: request.semantic_revision(),
        source,
        requirements,
        plan,
        admitted_operator_properties,
    })
}
