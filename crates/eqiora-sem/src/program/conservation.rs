//! Admission of retained physical conservation meaning on fixed volume support.

use super::*;
use eqiora_core::{ScalarDomain, ValueFrame};
use eqiora_schema::kernel::ConservationTerms;

pub(super) fn validate_conservation_types(
    owner: RawId,
    terms: ConservationTerms,
    typed: &TypedResidual<RawId>,
    support: Option<&SpatialSupport<RawId>>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(support @ SpatialSupport::Volume { .. }) = support else {
        diagnostics.push(kernel_error(
            owner,
            "fixed-domain conservation Law requires an exact volume support",
        ));
        return;
    };
    for (role, value) in [("flux", terms.flux()), ("source", terms.source())].into_iter() {
        let Some(value_type) = typed.node_type(value) else {
            diagnostics.push(kernel_error(
                owner,
                format!("Law {role} expression is missing"),
            ));
            continue;
        };
        if value_type.value_type.scalar_domain() != ScalarDomain::Real
            || value_type.value_type.array_rank() != 0
        {
            diagnostics.push(kernel_error(
                owner,
                format!("initial scalar conservation Law requires real {role}"),
            ));
        }
        if role != "flux"
            && (!value_type.shape().is_scalar() || value_type.frame() != ValueFrame::Invariant)
        {
            diagnostics.push(kernel_error(
                owner,
                format!("scalar conservation Law requires invariant scalar {role}"),
            ));
        }
        if value_type
            .support
            .as_ref()
            .is_some_and(|actual| actual != support)
        {
            diagnostics.push(kernel_error(
                owner,
                format!("Law {role} uses a foreign support"),
            ));
        }
    }
}
