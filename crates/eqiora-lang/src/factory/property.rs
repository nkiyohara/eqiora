use std::collections::BTreeMap;

use super::{AstConstructionError, SourceAstFactory, checked_range, validate_identifier};
use crate::ast::{
    ComponentItem, ComponentParameterDecl, Document, Expr, ExprKind, InstanceDecl, Item, NamePath,
    ParameterBindingDecl, TextRange, VisibilitySyntax,
};

impl SourceAstFactory {
    /// Materialize compiler-validated nominal properties as ordinary scalar
    /// Parameters and bindings while retaining their source metadata.
    pub fn elaborate_property_terms(
        document: &mut Document,
        contract_dimensions: &BTreeMap<String, Expr>,
        release_values: &BTreeMap<String, f64>,
        material_values: &BTreeMap<String, Vec<(String, f64)>>,
    ) -> Result<(), AstConstructionError> {
        document.discard_retained_source();
        for component in &mut document.components {
            for requirement in &component.property_requirements {
                let dimension = contract_dimensions
                    .get(requirement.contract.as_str())
                    .ok_or_else(|| {
                        AstConstructionError::new(format!(
                            "unresolved property contract `{}`",
                            requirement.contract
                        ))
                    })?;
                component
                    .items
                    .push(ComponentItem::Parameter(ComponentParameterDecl {
                        visibility: VisibilitySyntax::Public,
                        name: requirement.name.clone(),
                        dimension: dimension.clone(),
                        default: None,
                        range: requirement.range,
                    }));
            }
            elaborate_component_instances(&mut component.items, release_values, material_values)?;
        }
        for model in &mut document.models {
            for item in &mut model.items {
                if let Item::Instance(instance) = item {
                    elaborate_instance_properties(instance, release_values, material_values)?;
                }
            }
        }
        Ok(())
    }
}

impl NamePath {
    /// Construct a structurally segmented, nonempty source name.
    pub fn from_segments<I, S>(segments: I, range: TextRange) -> Result<Self, AstConstructionError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let segments = segments.into_iter().map(Into::into).collect::<Vec<_>>();
        if segments.is_empty() {
            return Err(AstConstructionError::new("a NamePath cannot be empty"));
        }
        for segment in &segments {
            validate_identifier(segment, "NamePath segment")?;
        }
        Ok(Self::from_parsed_segments(segments, checked_range(range)?))
    }
}

fn elaborate_component_instances(
    items: &mut [ComponentItem],
    release_values: &BTreeMap<String, f64>,
    material_values: &BTreeMap<String, Vec<(String, f64)>>,
) -> Result<(), AstConstructionError> {
    for item in items {
        if let ComponentItem::Instance(instance) = item {
            elaborate_instance_properties(instance, release_values, material_values)?;
        }
    }
    Ok(())
}

fn elaborate_instance_properties(
    instance: &mut InstanceDecl,
    release_values: &BTreeMap<String, f64>,
    material_values: &BTreeMap<String, Vec<(String, f64)>>,
) -> Result<(), AstConstructionError> {
    for binding in &instance.property_bindings {
        let value = *release_values
            .get(binding.release.as_str())
            .ok_or_else(|| {
                AstConstructionError::new(format!(
                    "unresolved property release `{}`",
                    binding.release
                ))
            })?;
        if !value.is_finite() {
            return Err(AstConstructionError::new(
                "property release value must be finite",
            ));
        }
        instance.bindings.push(ParameterBindingDecl {
            parameter: binding.property.clone(),
            value: Expr {
                kind: ExprKind::Number(value),
                range: binding.range,
            },
            range: binding.range,
        });
    }
    if let Some(material) = &instance.material_binding {
        let values = material_values.get(material.as_str()).ok_or_else(|| {
            AstConstructionError::new(format!("unresolved material composition `{material}`"))
        })?;
        for (property, value) in values {
            instance.bindings.push(ParameterBindingDecl {
                parameter: property.clone(),
                value: Expr {
                    kind: ExprKind::Number(*value),
                    range: instance.range,
                },
                range: instance.range,
            });
        }
    }
    Ok(())
}
