use std::collections::BTreeMap;

use super::{AstConstructionError, SourceAstFactory, checked_range, validate_identifier};
use crate::ast::{
    ComponentItem, ComponentParameterDecl, Document, Expr, InstanceDecl, Item, NamePath, TextRange,
    VisibilitySyntax,
};

impl SourceAstFactory {
    /// Materialize compiler-validated property requirements and exact release bindings.
    ///
    /// # Errors
    /// Rejects absent validated contract or release projections.
    pub fn elaborate_property_terms(
        document: &mut Document,
        contract_types: &BTreeMap<String, crate::ValueTypeSyntax>,
        release_values: &BTreeMap<String, Expr>,
        property_targets: &BTreeMap<String, Vec<String>>,
    ) -> Result<(), AstConstructionError> {
        for component in &mut document.components {
            elaborate_signature_properties(&mut component.signature, contract_types)?;
            for item in &mut component.items {
                if let ComponentItem::Instance(instance) = item {
                    elaborate_instance_properties(instance, release_values, property_targets)?;
                }
            }
        }
        for model in &mut document.models {
            elaborate_signature_properties(&mut model.signature, contract_types)?;
            for item in &mut model.items {
                if let Item::Instance(instance) = item {
                    elaborate_instance_properties(instance, release_values, property_targets)?;
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

fn elaborate_signature_properties(
    signature: &mut [crate::SignatureItem],
    contract_types: &BTreeMap<String, crate::ValueTypeSyntax>,
) -> Result<(), AstConstructionError> {
    for item in signature {
        if let crate::SignatureItem::Property(requirement) = item {
            let value_type = contract_types
                .get(requirement.contract.as_str())
                .ok_or_else(|| {
                    AstConstructionError::new(format!(
                        "unresolved property contract `{}`",
                        requirement.contract
                    ))
                })?;
            *item = crate::SignatureItem::Parameter(ComponentParameterDecl {
                comments: requirement.comments.clone(),
                visibility: VisibilitySyntax::Public,
                name: requirement.name.clone(),
                value_type: value_type.clone(),
                default: None,
                range: requirement.range,
            });
        }
    }
    Ok(())
}

fn elaborate_instance_properties(
    instance: &mut InstanceDecl,
    release_values: &BTreeMap<String, Expr>,
    property_targets: &BTreeMap<String, Vec<String>>,
) -> Result<(), AstConstructionError> {
    let Some(targets) = property_targets.get(instance.definition.as_str()) else {
        return Ok(());
    };
    for binding in &mut instance.bindings {
        if !targets.contains(&binding.name) {
            continue;
        }
        let path = match binding.value.kind() {
            crate::ExprKind::Name(name) => name.as_str(),
            crate::ExprKind::Path(path) => path.as_str(),
            _ => {
                return Err(AstConstructionError::new(
                    "property binding requires an exact release path",
                ));
            }
        };
        let value = release_values.get(path).ok_or_else(|| {
            AstConstructionError::new(format!("unresolved property release `{path}`"))
        })?;
        super::validate_expression(value)?;
        binding.value = value.clone();
    }
    Ok(())
}
