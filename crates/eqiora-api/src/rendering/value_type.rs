use super::{Math, MathReference, MathRendering};
use eqiora_core::{Diagnostic, ValueType};
use eqiora_lang::NotationProfile;

impl MathRendering {
    /// Present one checked mathematical type, retaining nominal basis identities.
    ///
    /// # Errors
    /// Rejects a type exceeding the bounded accessible presentation size.
    pub fn value_type(value: &ValueType, profile: NotationProfile) -> Result<Self, Diagnostic> {
        if value.shape().rank() > super::MAX_NODES {
            return Err(super::expression::failure(
                "type exceeds bounded presentation size",
            ));
        }
        let targets = [
            value.enum_definition().map(Into::into),
            value.index_set().map(Into::into),
            value.finite_space().map(Into::into),
        ];
        let references = targets
            .into_iter()
            .flatten()
            .map(|id| MathReference {
                graph_id: Some(id),
                role: None,
                declarations: vec![],
                operator: None,
            })
            .collect();
        super::output::render(Math::Type(value.clone()), references, profile)
    }
}
