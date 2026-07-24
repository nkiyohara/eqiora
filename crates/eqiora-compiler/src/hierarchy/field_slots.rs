//! Occurrence-bound continuum Field interfaces for reusable Components.
//!
//! A Field slot is a definition-time obligation. Support binding specializes
//! its identity-parametric type, then one exact enclosing Field satisfies the
//! obligation. Neither the slot nor its binding becomes a Kernel entity.

use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{ComponentDecl, ComponentItem, FieldSlotDecl, InstanceDecl, Item, ModelDecl};
use eqiora_schema::kernel::typing::{ExpressionType, SpatialSupport};

use crate::diagnostics::source_error;

use super::body_check::{field_expression_type, field_value_type};
use super::supports::SupportInterface;

/// Closed semantic representation family admitted by Field-slot v1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FieldRepresentationContract {
    Continuum,
}

/// Complete identity-parametric contract for one semantic Field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct FieldContract<I> {
    value: ExpressionType<I>,
    representation: FieldRepresentationContract,
}

impl<I> FieldContract<I> {
    pub(super) fn continuum(value: ExpressionType<I>) -> Self {
        Self {
            value,
            representation: FieldRepresentationContract::Continuum,
        }
    }

    pub(super) const fn value(&self) -> &ExpressionType<I> {
        &self.value
    }
}

#[derive(Clone, Debug)]
struct FieldSlotContract {
    support_slot: String,
    field: FieldContract<String>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct FieldInterface {
    slots: BTreeMap<String, FieldSlotContract>,
}

impl FieldInterface {
    pub(super) fn field(&self, name: &str) -> Option<&FieldContract<String>> {
        self.slots.get(name).map(|slot| &slot.field)
    }

    fn get(&self, name: &str) -> Option<&FieldSlotContract> {
        self.slots.get(name)
    }

    fn iter(&self) -> impl Iterator<Item = (&str, &FieldSlotContract)> {
        self.slots
            .iter()
            .map(|(name, contract)| (name.as_str(), contract))
    }
}

pub(super) fn component_field_interface(
    file: &str,
    component: &ComponentDecl,
    supports: &SupportInterface,
) -> Result<FieldInterface, Vec<Diagnostic>> {
    let mut slots = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for item in component.items() {
        let ComponentItem::FieldSlot(declaration) = item else {
            continue;
        };
        match field_slot_contract(file, declaration, supports) {
            Ok(contract) => {
                if slots
                    .insert(declaration.name().to_owned(), contract)
                    .is_some()
                {
                    diagnostics.push(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        declaration.range(),
                        format!("duplicate Field slot `{}`", declaration.name()),
                    ));
                }
            }
            Err(error) => diagnostics.push(error),
        }
    }
    if diagnostics.is_empty() {
        Ok(FieldInterface { slots })
    } else {
        Err(diagnostics)
    }
}

fn field_slot_contract(
    file: &str,
    declaration: &FieldSlotDecl,
    supports: &SupportInterface,
) -> Result<FieldSlotContract, Diagnostic> {
    let Some(support) = supports.get(declaration.support()) else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            declaration.range(),
            format!(
                "Field slot `{}` refers to unknown support slot `{}`",
                declaration.name(),
                declaration.support()
            ),
        ));
    };
    let SpatialSupport::Volume { .. } = support.support() else {
        return Err(source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            declaration.range(),
            format!(
                "Field slot `{}` requires a volume support slot",
                declaration.name()
            ),
        ));
    };
    let value = field_value_type(
        file,
        declaration.range(),
        declaration.dimension(),
        declaration.shape(),
        Some(support.support().clone()),
    )?;
    Ok(FieldSlotContract {
        support_slot: declaration.support().to_owned(),
        field: FieldContract::continuum(value),
    })
}

/// Collect Fields visible as bare binding targets in one Component body.
pub(super) fn component_field_contracts(
    file: &str,
    component: &ComponentDecl,
    supports: &SupportInterface,
    slots: &FieldInterface,
) -> BTreeMap<String, FieldContract<String>> {
    let mut fields = slots
        .iter()
        .map(|(name, slot)| (name.to_owned(), slot.field.clone()))
        .collect::<BTreeMap<_, _>>();
    for item in component.items() {
        let ComponentItem::Field(declaration) = item else {
            continue;
        };
        let support = declaration
            .domain()
            .and_then(|name| supports.visible_support(name).cloned());
        if let Ok(value) = field_expression_type(file, declaration, support) {
            fields.insert(
                declaration.name().to_owned(),
                FieldContract::continuum(value),
            );
        }
    }
    fields
}

/// Collect Fields visible as bare binding targets in one root Model.
pub(super) fn model_field_contracts(
    file: &str,
    model: &ModelDecl,
    supports: &BTreeMap<String, SpatialSupport<String>>,
) -> BTreeMap<String, FieldContract<String>> {
    model
        .items()
        .iter()
        .filter_map(|item| {
            let Item::Field(declaration) = item else {
                return None;
            };
            let support = declaration
                .domain()
                .and_then(|name| supports.get(name).cloned());
            field_expression_type(file, declaration, support)
                .ok()
                .map(|value| {
                    (
                        declaration.name().to_owned(),
                        FieldContract::continuum(value),
                    )
                })
        })
        .collect()
}

/// Validate one instance and return `child Field slot -> enclosing target`.
pub(super) fn resolve_instance_fields<I: Clone + Eq>(
    binding_file: &str,
    component: &ComponentDecl,
    interface: &FieldInterface,
    instance: &InstanceDecl,
    mut resolve_child_support: impl FnMut(&str) -> Option<SpatialSupport<I>>,
    mut resolve_parent: impl FnMut(&str) -> Option<FieldContract<I>>,
) -> Result<BTreeMap<String, String>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut targets = BTreeMap::new();
    let mut actual = BTreeMap::new();
    let mut seen = BTreeSet::new();

    for binding in instance.field_bindings() {
        if !seen.insert(binding.slot()) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "duplicate binding for Field slot `{}` in instance `{}`",
                    binding.slot(),
                    instance.name()
                ),
            ));
            continue;
        }
        let Some(slot) = interface.get(binding.slot()) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "unknown Field slot `{}` on component `{}`",
                    binding.slot(),
                    component.name()
                ),
            ));
            continue;
        };
        let Some(target) = resolve_parent(binding.target()) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "Field binding target `{}` is not an enclosing Field",
                    binding.target()
                ),
            ));
            continue;
        };
        let Some(support) = resolve_child_support(&slot.support_slot) else {
            diagnostics.push(source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                binding_file,
                binding.range(),
                format!(
                    "Field slot `{}` has no resolved exact support binding",
                    binding.slot()
                ),
            ));
            continue;
        };
        let expected = FieldContract {
            value: ExpressionType::shaped(
                slot.field.value.dimension,
                slot.field.value.shape.clone(),
                slot.field.value.frame,
                Some(support),
            ),
            representation: slot.field.representation,
        };
        if let Some(message) = field_contract_mismatch(binding.slot(), &expected, &target) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                binding.range(),
                message,
            ));
            continue;
        }
        targets.insert(binding.slot().to_owned(), binding.target().to_owned());
        actual.insert(binding.slot().to_owned(), binding.range());
    }

    for (name, _) in interface.iter() {
        if !actual.contains_key(name) {
            diagnostics.push(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                binding_file,
                instance.range(),
                format!(
                    "instance `{}` has no binding for required Field slot `{name}`",
                    instance.name()
                ),
            ));
        }
    }

    if diagnostics.is_empty() {
        Ok(targets)
    } else {
        Err(diagnostics)
    }
}

fn field_contract_mismatch<I: Eq>(
    slot: &str,
    expected: &FieldContract<I>,
    actual: &FieldContract<I>,
) -> Option<String> {
    let mismatch = if expected.value.dimension != actual.value.dimension {
        "physical dimension"
    } else if expected.value.shape != actual.value.shape {
        "exact value shape"
    } else if expected.value.frame != actual.value.frame {
        "coordinate frame"
    } else if expected.value.support != actual.value.support {
        "exact spatial support"
    } else if expected.representation != actual.representation {
        "representation family"
    } else {
        return None;
    };
    Some(format!(
        "Field slot `{slot}` and its target disagree in {mismatch}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_lang::{Document, Item, ModelDecl};

    fn parse(source: &str) -> Document {
        eqiora_lang::parse("field_slots.eqi", source)
            .into_document()
            .expect("Field-slot fixture parses")
    }

    fn component(document: &Document) -> &ComponentDecl {
        document.components().first().expect("fixture component")
    }

    fn instance(document: &Document) -> &InstanceDecl {
        document
            .models()
            .iter()
            .flat_map(ModelDecl::items)
            .find_map(|item| match item {
                Item::Instance(instance) => Some(instance),
                _ => None,
            })
            .expect("fixture instance")
    }

    #[test]
    fn exact_support_shape_frame_and_dimension_are_required() {
        let document = parse(
            r#"
component Law {
  public support body: volume(ambient_dimension = 2);
  public field slot displacement on body as continuum: m shape spatial_vector;
}
model Use {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field displacement on body as space: m shape spatial_vector;
  instance law: Law(support body = body, field displacement = displacement);
}
"#,
        );
        let component = component(&document);
        let supports =
            super::super::supports::component_support_interface("field_slots.eqi", component)
                .expect("support interface");
        let interface = component_field_interface("field_slots.eqi", component, &supports)
            .expect("Field interface");
        let exact_support = SpatialSupport::Volume {
            domain: "body-id",
            dimensions: 2,
        };
        let target = FieldContract::continuum(ExpressionType::shaped(
            eqiora_core::DimExponents {
                length: 1,
                ..eqiora_core::DimExponents::DIMENSIONLESS
            },
            eqiora_core::ValueShape::new([2]).expect("shape"),
            eqiora_schema::kernel::ValueFrame::SpatialCartesian,
            Some(exact_support.clone()),
        ));
        let resolved = resolve_instance_fields(
            "field_slots.eqi",
            component,
            &interface,
            instance(&document),
            |_| Some(exact_support.clone()),
            |name| (name == "displacement").then(|| target.clone()),
        )
        .expect("exact Field contract binds");
        assert_eq!(
            resolved.get("displacement").map(String::as_str),
            Some("displacement")
        );
    }
}
