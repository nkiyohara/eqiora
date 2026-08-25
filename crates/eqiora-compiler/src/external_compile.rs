//! Public method for one bounded external-spatial occurrence.

use std::collections::BTreeMap;

use eqiora_core::diagnostic::codes;
use eqiora_core::{Diagnostic, DynQuantity};
use eqiora_geometry::{CanonicalGeometryRef, NamedEntitySet};
use eqiora_lang::TextRange;
use eqiora_schema::kernel::GeometryDigest;

use crate::diagnostics::source_error;
use crate::external::{
    ExternalComponentBinding, ExternalGeometrySupportBinding, ExternalParameterBinding,
};
use crate::lower::CompiledModel;
use crate::source_identity::LocalSourceIdentityLimits;

impl CompiledModel {
    fn external_component_binding_limit() -> usize {
        LocalSourceIdentityLimits::default().max_bindings_per_instance
    }

    /// Compile one selected public local Component against selections borrowed
    /// from one exact canonical Geometry and explicit dimensioned Parameters.
    ///
    /// Each support is `(slot, selection, parent)`. A full-dimensional region
    /// has no parent; a boundary carries its exact parent slot and borrowed
    /// region selection. The opaque Geometry reference derives digest,
    /// dimensions, revision membership, and parent topology; callers cannot
    /// supply those facts independently.
    ///
    /// # Errors
    /// Returns accumulated source, binding, hierarchy, or typed-lowering
    /// diagnostics. No partial transaction is returned.
    #[allow(
        clippy::type_complexity,
        reason = "the closed tuple avoids a second public selection lifecycle"
    )]
    #[doc(hidden)]
    pub fn compile_external_component(
        file: &str,
        source: &str,
        model: &str,
        component: &str,
        geometry: CanonicalGeometryRef<'_>,
        supports: &[(&str, &NamedEntitySet, Option<(&str, &NamedEntitySet)>)],
        parameters: &[(&str, DynQuantity)],
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_binding_counts(file, supports.len(), parameters.len())?;
        validate_external_name_limits(file, model, component, supports, parameters)?;
        validate_geometry_bindings(file, geometry, supports)?;
        let digest = GeometryDigest::new(geometry.digest_bytes());
        let supports = supports
            .iter()
            .map(|(slot, selection, parent)| match parent {
                Some((parent_slot, _)) => ExternalGeometrySupportBinding::boundary(
                    *slot,
                    digest,
                    selection.name(),
                    *parent_slot,
                ),
                None => ExternalGeometrySupportBinding::region(
                    *slot,
                    digest,
                    selection.name(),
                    geometry.ambient_dimension(),
                ),
            })
            .collect();
        let parameters = parameters
            .iter()
            .map(|(parameter, value)| ExternalParameterBinding::new(*parameter, *value))
            .collect();
        let binding = ExternalComponentBinding::new(model, component, supports, parameters);
        crate::hierarchy::compile_external_component(file, source, &binding)
    }
}

#[allow(
    clippy::type_complexity,
    reason = "the closed tuple avoids a second public selection lifecycle"
)]
fn validate_external_name_limits(
    file: &str,
    model: &str,
    component: &str,
    supports: &[(&str, &NamedEntitySet, Option<(&str, &NamedEntitySet)>)],
    parameters: &[(&str, DynQuantity)],
) -> Result<(), Vec<Diagnostic>> {
    let limits = LocalSourceIdentityLimits::default();
    let mut total = 0_usize;
    observe_external_name(file, "Model", model, limits, &mut total)?;
    observe_external_name(file, "Component", component, limits, &mut total)?;
    for (slot, selection, parent) in supports {
        observe_external_name(file, "support slot", slot, limits, &mut total)?;
        observe_external_name(
            file,
            "Geometry entity set",
            selection.name(),
            limits,
            &mut total,
        )?;
        if let Some((parent_slot, parent_selection)) = parent {
            observe_external_name(file, "parent support slot", parent_slot, limits, &mut total)?;
            observe_external_name(
                file,
                "parent Geometry entity set",
                parent_selection.name(),
                limits,
                &mut total,
            )?;
        }
    }
    for (parameter, _) in parameters {
        observe_external_name(file, "Parameter", parameter, limits, &mut total)?;
    }
    Ok(())
}

fn observe_external_name(
    file: &str,
    label: &str,
    name: &str,
    limits: LocalSourceIdentityLimits,
    total: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    if name.len() > limits.max_name_bytes {
        return Err(vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            TextRange::default(),
            format!(
                "external {label} name requires {} UTF-8 bytes, exceeding the {} byte name limit",
                name.len(),
                limits.max_name_bytes
            ),
        )]);
    }
    *total = total.checked_add(name.len()).ok_or_else(|| {
        vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            TextRange::default(),
            "external binding name bytes overflow usize",
        )]
    })?;
    if *total > limits.max_total_name_bytes {
        return Err(vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            TextRange::default(),
            format!(
                "external binding names require {total} UTF-8 bytes, exceeding the {} byte aggregate name limit",
                limits.max_total_name_bytes
            ),
        )]);
    }
    Ok(())
}

#[allow(
    clippy::type_complexity,
    reason = "the closed tuple avoids a second public selection lifecycle"
)]
fn validate_geometry_bindings(
    file: &str,
    geometry: CanonicalGeometryRef<'_>,
    supports: &[(&str, &NamedEntitySet, Option<(&str, &NamedEntitySet)>)],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut regions = BTreeMap::new();
    for (slot, selection, parent) in supports {
        if parent.is_none()
            && geometry.selection_dimension(selection) == Some(geometry.topological_dimension())
        {
            regions
                .entry(*slot)
                .and_modify(|selection| *selection = None)
                .or_insert(Some(*selection));
        }
    }
    for (slot, selection, parent) in supports {
        let Some(dimension) = geometry.selection_dimension(selection) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                TextRange::default(),
                format!("external support `{slot}` has a foreign or stale Geometry selection"),
            ));
            continue;
        };
        match parent {
            None if dimension != geometry.topological_dimension() => diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                TextRange::default(),
                format!(
                    "external region support `{slot}` has selection dimension {dimension}, expected {}",
                    geometry.topological_dimension(),
                ),
            )),
            None => {}
            Some((parent_slot, parent_selection)) => {
                if geometry.selection_dimension(parent_selection).is_none() {
                    diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        TextRange::default(),
                        format!(
                            "external boundary support `{slot}` has a foreign or stale parent selection"
                        ),
                    ));
                    continue;
                }
                let exact_parent = regions
                    .get(parent_slot)
                    .and_then(|selection| *selection)
                    .is_some_and(|selection| std::ptr::eq(selection, *parent_selection));
                if !exact_parent
                    || !geometry.selection_is_boundary_of(selection, parent_selection)
                {
                    diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        TextRange::default(),
                        format!(
                            "external boundary selection `{}` does not bind exact parent region slot `{parent_slot}` in the supplied Geometry revision",
                            selection.name(),
                        ),
                    ));
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn validate_binding_counts(
    file: &str,
    support_count: usize,
    parameter_count: usize,
) -> Result<(), Vec<Diagnostic>> {
    let limit = CompiledModel::external_component_binding_limit();
    let mut diagnostics = Vec::new();
    for (label, count) in [
        ("external support bindings", support_count),
        ("external Parameter bindings", parameter_count),
    ] {
        if count > limit {
            diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                TextRange::default(),
                format!("{label} require {count}, exceeding the {limit} binding limit"),
            ));
        }
    }
    match support_count.checked_add(parameter_count) {
        Some(total) if total <= limit => {}
        Some(total) => diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            TextRange::default(),
            format!(
                "external Component occurrence requires {total} bindings, exceeding the {limit} binding limit"
            ),
        )),
        None => diagnostics.push(source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            TextRange::default(),
            "external Component occurrence binding count overflows usize",
        )),
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::DimExponents;
    use eqiora_geometry::CanonicalGeometryV1;

    fn geometry() -> CanonicalGeometryV1 {
        CanonicalGeometryV1::from_circular_hole_named_roles(
            [[0.0, 2.2], [0.0, 0.41]],
            [0.2, 0.2],
            0.05,
            1.0e-12,
            "fluid",
            "inlet",
            "outlet",
            "walls-lower",
            "walls-upper",
            "cylinder",
        )
        .expect("bounded canonical Geometry")
    }

    #[test]
    fn support_count_fails_before_source_or_projection_allocation() {
        let limit = CompiledModel::external_component_binding_limit();
        let geometry = geometry();
        let support = ("fluid", geometry.entity_set("fluid").unwrap(), None);
        let supports = vec![support; limit + 1];
        let diagnostics = CompiledModel::compile_external_component(
            "oversized.eqi",
            "not valid source",
            "Root",
            "Law",
            (&geometry).into(),
            &supports,
            &[],
        )
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("external support bindings require")
        }));
    }

    #[test]
    fn parameter_count_fails_before_source_or_binding_allocation() {
        let limit = CompiledModel::external_component_binding_limit();
        let geometry = geometry();
        let parameter = ("value", DynQuantity::new(1.0, DimExponents::DIMENSIONLESS));
        let parameters = vec![parameter; limit + 1];
        let diagnostics = CompiledModel::compile_external_component(
            "oversized.eqi",
            "not valid source",
            "Root",
            "Law",
            (&geometry).into(),
            &[],
            &parameters,
        )
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("external Parameter bindings require")
        }));
    }

    #[test]
    fn combined_count_fails_while_each_input_is_individually_bounded() {
        let limit = CompiledModel::external_component_binding_limit();
        let geometry = geometry();
        let support = ("fluid", geometry.entity_set("fluid").unwrap(), None);
        let supports = vec![support; limit];
        let parameters = &[("value", DynQuantity::new(1.0, DimExponents::DIMENSIONLESS))];
        let diagnostics = CompiledModel::compile_external_component(
            "oversized.eqi",
            "not valid source",
            "Root",
            "Law",
            (&geometry).into(),
            &supports,
            parameters,
        )
        .unwrap_err();
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message()
                .contains("external Component occurrence requires")
        }));
        assert!(diagnostics.iter().all(|diagnostic| {
            !diagnostic
                .message()
                .contains("external support bindings require")
                && !diagnostic
                    .message()
                    .contains("external Parameter bindings require")
        }));
    }

    #[test]
    fn oversized_external_name_fails_before_source_or_binding_clones() {
        let geometry = geometry();
        let limits = LocalSourceIdentityLimits::default();
        let oversized = "m".repeat(limits.max_name_bytes + 1);
        let diagnostics = CompiledModel::compile_external_component(
            "oversized-name.eqi",
            "not valid source",
            &oversized,
            "Law",
            (&geometry).into(),
            &[],
            &[],
        )
        .unwrap_err();
        assert!(diagnostics[0].message().contains("byte name limit"));
    }

    #[test]
    fn aggregate_external_name_budget_fails_without_materializing_all_names() {
        let limits = LocalSourceIdentityLimits::default();
        let mut total = limits.max_total_name_bytes - 1;
        let diagnostics =
            observe_external_name("aggregate.eqi", "support slot", "ab", limits, &mut total)
                .unwrap_err();
        assert!(diagnostics[0].message().contains("aggregate name limit"));
    }
}
