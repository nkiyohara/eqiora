use super::*;

impl CommonFormulationDescription {
    pub(super) fn mixed(
        correspondence: &crate::form_compiler::vocabulary::MixedGalerkinCorrespondence,
        requested: FormulationSelectionMode,
        reason: &'static str,
    ) -> Self {
        Self {
            requested,
            kind: correspondence.formulation.kind,
            boundary_treatment: correspondence.formulation.boundary_treatment.id(),
            rule_ids: correspondence
                .formulation
                .rules
                .map(crate::form_compiler::vocabulary::MixedFormulationRule::id)
                .into(),
            selection_reason_codes: Box::new([reason]),
            requested_source_identity: None,
        }
    }

    pub(super) fn integral(
        correspondence: &crate::form_compiler::vocabulary::IntegralConservativeCorrespondence,
        requested: FormulationSelectionMode,
    ) -> Self {
        Self {
            requested,
            kind: correspondence.formulation.kind,
            boundary_treatment: correspondence.formulation.boundary_treatment.id(),
            rule_ids: correspondence
                .formulation
                .rules
                .map(crate::form_compiler::vocabulary::IntegralConservativeRule::id)
                .into(),
            selection_reason_codes: Box::new([match requested {
                FormulationSelectionMode::Automatic => {
                    "eqiora.formulation.auto.integral-conservative-for-cell-centered-fvm/v1"
                }
                FormulationSelectionMode::Exact => {
                    "eqiora.formulation.exact.integral-conservative-admitted/v1"
                }
                FormulationSelectionMode::Authored => {
                    unreachable!("authored integral-conservative forms are not admitted")
                }
            }]),
            requested_source_identity: None,
        }
    }

    /// Requested selection mode. The first inspection slice is automatic-only.
    #[must_use]
    pub const fn requested(&self) -> FormulationSelectionMode {
        self.requested
    }

    /// Fresh-compile source identity when selection admitted an authored form.
    #[must_use]
    pub fn requested_source_identity(&self) -> Option<&str> {
        self.requested_source_identity.as_deref()
    }

    /// Exact effective mathematical form.
    #[must_use]
    pub const fn effective(&self) -> FormulationKind {
        self.kind
    }

    /// Versioned boundary-treatment identifier.
    #[must_use]
    pub const fn boundary_treatment(&self) -> &'static str {
        self.boundary_treatment
    }

    /// Complete ordered closed-rule inventory consumed by derivation.
    #[must_use]
    pub fn rule_ids(&self) -> &[&'static str] {
        &self.rule_ids
    }

    /// Stable reasons for the automatic choice.
    #[must_use]
    pub fn selection_reason_codes(&self) -> &[&'static str] {
        &self.selection_reason_codes
    }
}
