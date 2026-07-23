use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id, OntologyId};
use eqiora_schema::Model;

use crate::{
    FieldwiseRealizationPlan, RealizationCapabilities, RealizationRequirements,
    RealizationRevision, SemanticRevision, invalid_realization,
};

/// Exact lowerer facts against which a field-wise plan is admitted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldwiseRealizationRequirements {
    domain: Id<kinds::Domain>,
    unknown_fields: Vec<Id<kinds::Field>>,
    execution: RealizationRequirements,
}

impl FieldwiseRealizationRequirements {
    /// Construct deterministically ordered exact Domain and unknown-Field requirements.
    ///
    /// # Errors
    /// Returns `EQ0807` for an empty or duplicate unknown-Field inventory.
    pub fn new(
        domain: Id<kinds::Domain>,
        unknown_fields: impl IntoIterator<Item = Id<kinds::Field>>,
        execution: RealizationRequirements,
    ) -> Result<Self, Diagnostic> {
        let mut unknown_fields = unknown_fields.into_iter().collect::<Vec<_>>();
        unknown_fields.sort_by_key(Id::ulid);
        if unknown_fields.is_empty() {
            return Err(invalid_realization(
                "field-wise requirements need at least one unknown Semantic Field",
            ));
        }
        if unknown_fields.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(invalid_realization(
                "field-wise requirements contain a duplicate unknown Semantic Field",
            ));
        }
        Ok(Self {
            domain,
            unknown_fields,
            execution,
        })
    }

    /// Exact Semantic Domain required by the lowerer.
    #[must_use]
    pub const fn domain(&self) -> Id<kinds::Domain> {
        self.domain
    }

    /// Canonically ordered exact unknown-Field inventory.
    #[must_use]
    pub fn unknown_fields(&self) -> &[Id<kinds::Field>] {
        &self.unknown_fields
    }

    /// Dimension, scalar, and vector-layout requirements.
    #[must_use]
    pub const fn execution(&self) -> RealizationRequirements {
        self.execution
    }
}

/// Explicit field-wise realization request at an independent revision.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldwiseRealizationRequest {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    realization_revision: RealizationRevision,
    plan: FieldwiseRealizationPlan,
}

impl FieldwiseRealizationRequest {
    /// Bind one explicit field-wise plan to exact semantic and realization revisions.
    #[must_use]
    pub const fn explicit(
        model: OntologyId<Model>,
        semantic_revision: SemanticRevision,
        realization_revision: RealizationRevision,
        plan: FieldwiseRealizationPlan,
    ) -> Self {
        Self {
            model,
            semantic_revision,
            realization_revision,
            plan,
        }
    }

    /// Semantic Model identity.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.model
    }

    /// Semantic Model revision.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        self.semantic_revision
    }

    /// Independent field-wise Realization revision.
    #[must_use]
    pub const fn realization_revision(&self) -> RealizationRevision {
        self.realization_revision
    }

    /// Explicit unresolved plan.
    #[must_use]
    pub const fn plan(&self) -> &FieldwiseRealizationPlan {
        &self.plan
    }
}

/// Validated field-wise plan and its two-layer provenance.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedFieldwiseRealization {
    model: OntologyId<Model>,
    semantic_revision: SemanticRevision,
    realization_revision: RealizationRevision,
    requirements: FieldwiseRealizationRequirements,
    plan: FieldwiseRealizationPlan,
}

impl ResolvedFieldwiseRealization {
    /// Semantic Model identity.
    #[must_use]
    pub const fn model(&self) -> OntologyId<Model> {
        self.model
    }

    /// Semantic Model revision.
    #[must_use]
    pub const fn semantic_revision(&self) -> SemanticRevision {
        self.semantic_revision
    }

    /// Independently selected Realization revision.
    #[must_use]
    pub const fn realization_revision(&self) -> RealizationRevision {
        self.realization_revision
    }

    /// Exact lowerer and execution requirements used for admission.
    #[must_use]
    pub const fn requirements(&self) -> &FieldwiseRealizationRequirements {
        &self.requirements
    }

    /// Complete validated field-wise plan.
    #[must_use]
    pub const fn plan(&self) -> &FieldwiseRealizationPlan {
        &self.plan
    }
}

/// Resolve one explicit field-wise request without fallback.
///
/// # Errors
/// Returns `EQ0807` for Domain or exact unknown-Field drift, a structurally
/// invalid plan, or an unsupported complete backend capability.
pub fn resolve_fieldwise(
    request: &FieldwiseRealizationRequest,
    requirements: FieldwiseRealizationRequirements,
    capabilities: &RealizationCapabilities,
) -> Result<ResolvedFieldwiseRealization, Diagnostic> {
    request.plan.validate()?;
    if request.plan.spatial().domain() != requirements.domain {
        return Err(invalid_realization(
            "field-wise plan Domain differs from the exact lowerer requirement",
        ));
    }
    let selected_fields = request
        .plan
        .spatial()
        .field_spaces()
        .iter()
        .map(|binding| binding.field())
        .collect::<Vec<_>>();
    if selected_fields != requirements.unknown_fields {
        return Err(invalid_realization(
            "field-wise plan must bind the exact lowerer unknown-Field inventory",
        ));
    }
    capabilities.supports_fieldwise(requirements.execution, &request.plan)?;
    Ok(ResolvedFieldwiseRealization {
        model: request.model,
        semantic_revision: request.semantic_revision,
        realization_revision: request.realization_revision,
        requirements,
        plan: request.plan.clone(),
    })
}

#[cfg(test)]
#[path = "fieldwise_resolution/tests.rs"]
mod tests;
