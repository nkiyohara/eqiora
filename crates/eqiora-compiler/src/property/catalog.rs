//! One checked nominal property catalog reused by authored and selected bindings.
use super::*;
pub(super) struct Catalog {
    pub(super) contracts: BTreeMap<Key, Contract>,
    pub(super) releases: BTreeMap<Key, Release>,
    pub(super) compositions: BTreeMap<Key, Composition>,
}
pub(super) fn build(
    units: &[AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
) -> Result<Catalog, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut contracts = BTreeMap::new();
    for unit in units.iter() {
        for (visibility, name, value_type, range) in unit.document.property_contract_syntax() {
            if name == crate::math::ROOT {
                diagnostics.push(error(
                    &unit.file,
                    range,
                    "identifier `math` is reserved for compiler-owned scalar mathematics",
                ));
                continue;
            }
            if let Err(diagnostic) =
                crate::value_types::lower_value_type::<()>(&unit.file, value_type, None)
            {
                diagnostics.push(diagnostic);
                continue;
            }
            let key = (unit.module.clone(), name.to_owned());
            if contracts
                .insert(
                    key,
                    Contract {
                        file: unit.file.clone(),
                        visibility,
                        value_type: value_type.clone(),
                    },
                )
                .is_some()
            {
                diagnostics.push(error(
                    &unit.file,
                    range,
                    format!("duplicate property contract `{name}`"),
                ));
            }
        }
    }

    let mut releases = BTreeMap::new();
    for unit in units.iter() {
        for (
            visibility,
            name,
            contract_path,
            source_value_expr,
            source_dimension_expr,
            scale_expr,
            citation,
            license,
            range,
        ) in unit.document.property_release_syntax()
        {
            if name == crate::math::ROOT {
                diagnostics.push(error(
                    &unit.file,
                    range,
                    "identifier `math` is reserved for compiler-owned scalar mathematics",
                ));
                continue;
            }
            let Some(contract_key) = resolve_path(
                &unit.module,
                contract_path,
                aliases,
                &contracts,
                |value| value.visibility,
                &unit.file,
                &mut diagnostics,
            ) else {
                continue;
            };
            let contract = &contracts[&contract_key];
            let source_dimension = match lower_dimension(&unit.file, source_dimension_expr) {
                Ok(value) => value,
                Err(value) => {
                    diagnostics.push(value);
                    continue;
                }
            };
            let contract_type = match crate::value_types::lower_value_type::<()>(
                &contract.file,
                &contract.value_type,
                None,
            ) {
                Ok(value) => value,
                Err(value) => {
                    diagnostics.push(value);
                    continue;
                }
            };
            if source_dimension != contract_type.dimension() {
                diagnostics.push(error(
                    &unit.file,
                    source_dimension_expr.range(),
                    "property release source unit does not match its contract dimension",
                ));
                continue;
            }
            let source_value = match crate::hierarchy::closed_value(
                &unit.file,
                source_value_expr,
                contract_type.clone().with_dimension(source_dimension),
            ) {
                Ok(value) => value,
                Err(value) => {
                    diagnostics.push(value);
                    continue;
                }
            };
            let scale = match constant(&unit.file, scale_expr) {
                Ok(value) if value.is_finite() && value > 0.0 => value,
                Ok(_) => {
                    diagnostics.push(error(
                        &unit.file,
                        scale_expr.range(),
                        "coherent-SI scale must be finite and strictly positive",
                    ));
                    continue;
                }
                Err(value) => {
                    diagnostics.push(value);
                    continue;
                }
            };
            let normalized = if source_value.value_type().scalar_domain()
                == eqiora_core::ScalarDomain::Integer
            {
                if scale != 1.0 {
                    Err("integer property values require unit scale one")
                } else {
                    crate::typed_values::retype(&source_value, contract_type.clone())
                        .map_err(|_| "invalid integer property type")
                }
            } else {
                source_value
                    .components()
                    .expect("real or complex property")
                    .map(|(real, imag)| {
                        Ok((
                            crate::units::normalize_value(real, scale)?,
                            crate::units::normalize_value(imag, scale)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, &'static str>>()
                    .and_then(|values| {
                        eqiora_core::ValueLiteral::new(contract_type.clone(), values)
                            .map_err(|_| "invalid normalized property value")
                    })
            };
            let value = match normalized {
                Ok(value) => value,
                Err(message) => {
                    diagnostics.push(error(&unit.file, range, message));
                    continue;
                }
            };
            let key = (unit.module.clone(), name.to_owned());
            if releases
                .insert(
                    key,
                    Release {
                        visibility,
                        contract: contract_key,
                        value,
                        citation: citation.to_string(),
                        license: license.to_string(),
                    },
                )
                .is_some()
            {
                diagnostics.push(error(
                    &unit.file,
                    range,
                    format!("duplicate property release `{name}`"),
                ));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let mut compositions = BTreeMap::new();
    for unit in units.iter() {
        for (visibility, name, properties, range) in unit.document.material_composition_syntax() {
            if properties.is_empty() {
                diagnostics.push(error(
                    &unit.file,
                    range,
                    "material composition requires at least one property",
                ));
                continue;
            }
            let mut seen = BTreeSet::new();
            let mut resolved = Vec::new();
            for (property, release_path, binding_range) in properties {
                if !seen.insert(property) {
                    diagnostics.push(error(
                        &unit.file,
                        binding_range,
                        format!("duplicate material property `{property}`"),
                    ));
                    continue;
                }
                if let Some(release) = resolve_path(
                    &unit.module,
                    release_path,
                    aliases,
                    &releases,
                    |value| value.visibility,
                    &unit.file,
                    &mut diagnostics,
                ) {
                    resolved.push((property.to_owned(), release, binding_range));
                }
            }
            let key = (unit.module.clone(), name.to_owned());
            if compositions
                .insert(
                    key,
                    Composition {
                        visibility,
                        properties: resolved,
                    },
                )
                .is_some()
            {
                diagnostics.push(error(
                    &unit.file,
                    range,
                    format!("duplicate material composition `{name}`"),
                ));
            }
        }
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(Catalog {
        contracts,
        releases,
        compositions,
    })
}
