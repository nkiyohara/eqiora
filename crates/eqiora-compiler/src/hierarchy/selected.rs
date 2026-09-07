//! Signature-directed external compilation reuses ordinary occurrence allocation.
use super::*;
use crate::StaticBindingValue;
use crate::resolved::ValidatedResolvedHierarchy;
use eqiora_lang::{ModelDecl, SignatureItem};

pub(crate) fn local(
    file: &str,
    source: &str,
    entry: &str,
    bindings: &[(&str, StaticBindingValue<'_>)],
) -> Result<CompiledModel, Vec<Diagnostic>> {
    let document = parse(file, source).into_document()?;
    let identity = LocalSourceIdentity::from_document(&document).map_err(|error| vec![error])?;
    let document = crate::dimensions::elaborate_dimension_aliases(file, &document)?;
    let limits = HierarchyLimits::default();
    let elaborator = Elaborator::new(file, source.len(), document.as_ref(), identity, limits)?;
    let checked = check::validate(&elaborator)?;
    compile(
        &elaborator,
        &checked,
        entry,
        bindings,
        &preflight::DefinitionNamespace::Local,
        None,
    )
}

pub(crate) fn resolved(
    hierarchy: &ValidatedResolvedHierarchy,
    entry: &str,
    bindings: &[(&str, StaticBindingValue<'_>)],
) -> Result<CompiledModel, Vec<Diagnostic>> {
    let elaborator = Elaborator::new_resolved(&hierarchy.analysis, HierarchyLimits::default())?;
    compile(
        &elaborator,
        &hierarchy.checked,
        entry,
        bindings,
        &preflight::DefinitionNamespace::Resolved(hierarchy.analysis.root.clone()),
        Some(hierarchy),
    )
}

fn compile(
    elaborator: &Elaborator<'_>,
    checked: &CheckedDefinitionGraph,
    entry: &str,
    bindings: &[(&str, StaticBindingValue<'_>)],
    root_namespace: &preflight::DefinitionNamespace,
    hierarchy: Option<&ValidatedResolvedHierarchy>,
) -> Result<CompiledModel, Vec<Diagnostic>> {
    let limits = elaborator.limits;
    if let Ok(model) = elaborator.entry_model(entry) {
        let signature = authored_signature(hierarchy, &model.namespace, model.name(), true)
            .unwrap_or_else(|| model.signature());
        let prepared = prepare(
            model.file,
            model.name(),
            signature,
            bindings,
            |requirement, value| {
                property(hierarchy, &model.namespace, model.file, requirement, value)
            },
        )?;
        let bound = bind_model(model.declaration, &prepared)?;
        let definition = preflight::ModelDefinition {
            namespace: model.namespace.clone(),
            file: model.file,
            owned_interfaces: preflight::owned_model_items(&bound),
            declaration: &bound,
        };
        let mut size = checked_model_expansion_size(checked, &model)?;
        size.declarations = checked_external_footprint(
            "declarations",
            size.declarations,
            prepared.supports().len() + prepared.clocks.len() + prepared.parameters().len(),
            limits.max_declarations,
        )?;
        return RootExpansion::new(elaborator, definition, size)
            .map_err(|error| vec![error])?
            .expand_bound(prepared.supports(), &prepared.clocks)?
            .compile(limits);
    }
    let path = NamePath::from_segments(entry.split('.'), TextRange::default())
        .map_err(|error| vec![hierarchy_error(error.message())])?;
    let component = elaborator
        .resolve_component(root_namespace, &path, "<selected-entry>", path.range())
        .map_err(|error| vec![error])?;
    let signature = authored_signature(hierarchy, &component.namespace, component.name(), false)
        .unwrap_or_else(|| component.signature());
    let prepared = prepare(
        component.file,
        component.name(),
        signature,
        bindings,
        |requirement, value| {
            property(
                hierarchy,
                &component.namespace,
                component.file,
                requirement,
                value,
            )
        },
    )?;
    let compiled = compile_external_component_from_definition(
        elaborator,
        checked,
        component.clone(),
        &prepared,
        limits,
    )?;
    if component.formulations().len() != 0 {
        let geometry = bindings
            .iter()
            .find_map(|(_, value)| match value {
                StaticBindingValue::GeometrySupport { geometry, .. } => Some(*geometry),
                _ => None,
            })
            .ok_or_else(|| {
                vec![hierarchy_error(
                    "authored formulations require exact Geometry support bindings",
                )]
            })?;
        let formulations = crate::formulation::compile_component_formulations(
            component.file,
            component.declaration,
            compiled.symbols(),
            compiled.transaction(),
            geometry.ambient_dimension(),
            geometry.topological_dimension(),
        )?;
        Ok(compiled.with_authored_formulations(formulations))
    } else {
        Ok(compiled)
    }
}

fn authored_signature<'a>(
    hierarchy: Option<&'a ValidatedResolvedHierarchy>,
    namespace: &preflight::DefinitionNamespace,
    name: &str,
    model: bool,
) -> Option<&'a [SignatureItem]> {
    let preflight::DefinitionNamespace::Resolved(namespace) = namespace else {
        return None;
    };
    let unit = hierarchy?
        .analysis
        .units
        .iter()
        .find(|unit| &unit.module == namespace)?;
    if model {
        unit.authored_document
            .models()
            .iter()
            .find(|value| value.name() == name)
            .map(ModelDecl::signature)
    } else {
        unit.authored_document
            .components()
            .iter()
            .find(|value| value.name() == name)
            .map(eqiora_lang::ComponentDecl::signature)
    }
}
fn property(
    hierarchy: Option<&ValidatedResolvedHierarchy>,
    namespace: &preflight::DefinitionNamespace,
    file: &str,
    requirement: &eqiora_lang::ComponentPropertyDecl,
    value: &eqiora_lang::Expr,
) -> Result<eqiora_core::ValueLiteral, Vec<Diagnostic>> {
    let (Some(hierarchy), preflight::DefinitionNamespace::Resolved(namespace)) =
        (hierarchy, namespace)
    else {
        return Err(vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            value.range(),
            "selected property binding requires an analyzed source property scope",
        )]);
    };
    crate::property::selected_value(
        &hierarchy.analysis.units,
        &hierarchy.analysis.aliases,
        namespace,
        file,
        requirement,
        value,
    )
}

fn prepare(
    file: &str,
    name: &str,
    signature: &[SignatureItem],
    bindings: &[(&str, StaticBindingValue<'_>)],
    mut property: impl FnMut(
        &eqiora_lang::ComponentPropertyDecl,
        &eqiora_lang::Expr,
    ) -> Result<eqiora_core::ValueLiteral, Vec<Diagnostic>>,
) -> Result<ExternalComponentBinding, Vec<Diagnostic>> {
    use std::collections::{BTreeMap, BTreeSet};
    let fail = |range, message| {
        vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            message,
        )]
    };
    if bindings.len()
        > crate::source_identity::LocalSourceIdentityLimits::default().max_bindings_per_instance
    {
        return Err(fail(
            TextRange::default(),
            "selected binding count exceeds resource limit".to_owned(),
        ));
    }
    let mut seen = BTreeSet::new();
    let mut prepared = ExternalComponentBinding::new(name, name, Vec::new(), Vec::new());
    let mut parameters = Vec::new();
    let mut supports = Vec::new();
    let mut geometry_groups = BTreeMap::<
        [u8; 32],
        (
            &eqiora_geometry::CanonicalGeometryV1,
            Vec<(
                &str,
                &eqiora_geometry::NamedEntitySet,
                Option<(&str, &eqiora_geometry::NamedEntitySet)>,
            )>,
        ),
    >::new();
    for &(name, value) in bindings {
        if !seen.insert(name) {
            return Err(fail(
                TextRange::default(),
                format!("duplicate selected binding `{name}`"),
            ));
        }
        let target = signature
            .iter()
            .find(|item| item.name() == name)
            .ok_or_else(|| {
                fail(
                    TextRange::default(),
                    format!("selected signature has no binding target `{name}`"),
                )
            })?;
        match (target, value) {
            (SignatureItem::Parameter(parameter), StaticBindingValue::Expression(value)) => {
                let target =
                    crate::value_types::lower_value_type::<()>(file, parameter.value_type(), None)
                        .map_err(|error| vec![error])?;
                parameters.push(ExternalParameterBinding::new(
                    name,
                    closed_value(file, value, target).map_err(|error| vec![error])?,
                ));
            }
            (SignatureItem::Property(requirement), StaticBindingValue::Expression(value)) => {
                parameters.push(ExternalParameterBinding::new(
                    name,
                    property(requirement, value)?,
                ))
            }
            (SignatureItem::Clock(_), StaticBindingValue::Clock(clock))
                if matches!(
                    clock.kind(),
                    eqiora_schema::kernel::ClockKind::Periodic { .. }
                ) =>
            {
                prepared.clocks.push((name.to_owned(), clock.clone()))
            }
            (
                SignatureItem::Support(slot),
                StaticBindingValue::GeometrySupport {
                    geometry,
                    selection,
                    parent,
                },
            ) => {
                let digest = eqiora_schema::kernel::GeometryDigest::new(geometry.digest_bytes());
                let parent_binding = match slot.syntax() {
                    eqiora_lang::SupportSlotSyntax::Volume { ambient_dimension }
                        if parent.is_none()
                            && *ambient_dimension == geometry.ambient_dimension() =>
                    {
                        None
                    }
                    eqiora_lang::SupportSlotSyntax::Boundary {
                        parent: parent_slot,
                    } => Some((
                        parent_slot.as_str(),
                        parent.ok_or_else(|| {
                            fail(
                                slot.range(),
                                "boundary support requires its exact parent selection".to_owned(),
                            )
                        })?,
                    )),
                    _ => {
                        return Err(fail(
                            slot.range(),
                            "Geometry binding does not match the selected support contract"
                                .to_owned(),
                        ));
                    }
                };
                geometry_groups
                    .entry(geometry.digest_bytes())
                    .or_insert_with(|| (geometry, Vec::new()))
                    .1
                    .push((name, selection, parent_binding));
                supports.push(match parent_binding {
                    Some((parent_slot, _)) => ExternalGeometrySupportBinding::boundary(
                        name,
                        digest,
                        selection.name(),
                        parent_slot,
                    ),
                    None => ExternalGeometrySupportBinding::region(
                        name,
                        digest,
                        selection.name(),
                        geometry.ambient_dimension(),
                    ),
                });
            }
            _ => {
                return Err(fail(
                    target.range(),
                    format!("selected argument `{name}` has the wrong signature binding category"),
                ));
            }
        }
    }
    for item in signature {
        if seen.contains(item.name()) {
            continue;
        }
        let required = match item {
            SignatureItem::Parameter(value) => value.default().is_none(),
            SignatureItem::Clock(_)
            | SignatureItem::Support(_)
            | SignatureItem::Field(_)
            | SignatureItem::Property(_) => true,
            _ => false,
        };
        if required {
            return Err(fail(
                item.range(),
                format!("selected entry requires binding `{}`", item.name()),
            ));
        }
    }
    for (_, (geometry, bindings)) in geometry_groups {
        crate::external_compile::validate_geometry_bindings(file, geometry, &bindings)?;
    }
    parameters.sort_by(|a, b| a.parameter().cmp(b.parameter()));
    supports.sort_by(|a, b| a.slot().cmp(b.slot()));
    let clocks = prepared.clocks;
    prepared = ExternalComponentBinding::new(name, name, supports, parameters);
    prepared.clocks = clocks;
    prepared.clocks.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(prepared)
}

fn bind_model(
    model: &ModelDecl,
    bindings: &ExternalComponentBinding,
) -> Result<ModelDecl, Vec<Diagnostic>> {
    let signature = model
        .signature()
        .iter()
        .map(|item| {
            let SignatureItem::Parameter(declaration) = item else {
                return Ok(item.clone());
            };
            let Some(binding) = bindings
                .parameters()
                .iter()
                .find(|binding| binding.parameter() == declaration.name())
            else {
                return Ok(item.clone());
            };
            let value = SourceAstFactory::value_literal(binding.value(), declaration.range())?;
            SourceAstFactory::component_parameter(
                eqiora_lang::VisibilitySyntax::Public,
                declaration.name(),
                declaration.value_type().clone(),
                Some(value),
                declaration.range(),
            )
            .map(SignatureItem::Parameter)
        })
        .collect::<Result<Vec<_>, eqiora_lang::AstConstructionError>>()
        .map_err(|error| vec![hierarchy_error(error.message())])?;
    SourceAstFactory::model(
        model.visibility(),
        model.name(),
        signature,
        model.items().to_vec(),
        model.range(),
    )
    .map_err(|error| vec![hierarchy_error(error.message())])
}
