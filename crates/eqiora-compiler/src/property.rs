use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{
    BinaryOp, ComponentItem, Expr, ExprKind, InstanceDecl, Item, NamePath, SourceAstFactory,
    TextRange, UnaryOp, VisibilitySyntax,
};

use crate::diagnostics::source_error;
use crate::dimensions::lower_dimension;
use crate::resolved::{AnalyzedSourceUnit, CompilationNamespaceId, ResolvedAlias};

type Key = (CompilationNamespaceId, String);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedPropertyBinding {
    contract: String,
    release: String,
    component: String,
    requirement: String,
    normalized_value: f64,
    validity: &'static str,
    citation: String,
    license: String,
}

impl ResolvedPropertyBinding {
    #[must_use]
    pub fn contract(&self) -> &str {
        &self.contract
    }
    #[must_use]
    pub fn release(&self) -> &str {
        &self.release
    }
    #[must_use]
    pub fn component(&self) -> &str {
        &self.component
    }
    #[must_use]
    pub fn requirement(&self) -> &str {
        &self.requirement
    }
    #[must_use]
    pub const fn normalized_value(&self) -> f64 {
        self.normalized_value
    }
    #[must_use]
    pub const fn validity(&self) -> &'static str {
        self.validity
    }
    #[must_use]
    pub fn citation(&self) -> &str {
        &self.citation
    }
    #[must_use]
    pub fn license(&self) -> &str {
        &self.license
    }
}

struct Contract {
    file: String,
    visibility: VisibilitySyntax,
    dimension: Expr,
}

struct Release {
    visibility: VisibilitySyntax,
    contract: Key,
    value: f64,
    citation: String,
    license: String,
}

pub(crate) fn validate_and_elaborate(
    units: &mut [AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
) -> Result<Box<[ResolvedPropertyBinding]>, Vec<Diagnostic>> {
    let has_property_syntax = units.iter().any(|unit| {
        unit.document.property_contract_syntax().len() != 0
            || unit.document.property_release_syntax().len() != 0
            || unit.document.components().iter().any(|component| {
                component.property_requirement_syntax().len() != 0
                    || component.items().iter().any(|item| matches!(
                        item, ComponentItem::Instance(value) if value.property_binding_syntax().len() != 0
                    ))
            })
            || unit.document.models().iter().any(|model| model.items().iter().any(|item| matches!(
                item, Item::Instance(value) if value.property_binding_syntax().len() != 0
            )))
    });
    if !has_property_syntax {
        return Ok(Box::new([]));
    }
    let mut diagnostics = Vec::new();
    let mut contracts = BTreeMap::new();
    for unit in units.iter() {
        for (visibility, name, dimension, range) in unit.document.property_contract_syntax() {
            if let Err(diagnostic) = lower_dimension(&unit.file, dimension) {
                diagnostics.push(diagnostic);
                continue;
            }
            let key = (unit.namespace.clone(), name.to_owned());
            if contracts
                .insert(
                    key,
                    Contract {
                        file: unit.file.clone(),
                        visibility,
                        dimension: dimension.clone(),
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
            let Some(contract_key) = resolve_path(
                &unit.namespace,
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
            let contract_dimension = match lower_dimension(&contract.file, &contract.dimension) {
                Ok(value) => value,
                Err(value) => {
                    diagnostics.push(value);
                    continue;
                }
            };
            if source_dimension != contract_dimension {
                diagnostics.push(error(
                    &unit.file,
                    source_dimension_expr.range(),
                    "property release source unit does not match its contract dimension",
                ));
                continue;
            }
            let source_value = match constant(&unit.file, source_value_expr) {
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
            let value = source_value * scale;
            if !value.is_finite() {
                diagnostics.push(error(
                    &unit.file,
                    range,
                    "normalized property release value must be finite",
                ));
                continue;
            }
            let key = (unit.namespace.clone(), name.to_owned());
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

    let components = units
        .iter()
        .flat_map(|unit| {
            unit.document.components().iter().map(move |value| {
                (
                    (unit.namespace.clone(), value.name().to_owned()),
                    (value.clone(), unit.file.clone()),
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    let mut projections = Vec::new();
    for unit in units.iter_mut() {
        let mut dimensions = BTreeMap::new();
        for component in unit.document.components() {
            for (_, contract_path, _) in component.property_requirement_syntax() {
                if let Some(key) = resolve_path(
                    &unit.namespace,
                    contract_path,
                    aliases,
                    &contracts,
                    |value| value.visibility,
                    &unit.file,
                    &mut diagnostics,
                ) {
                    dimensions.insert(contract_path.to_string(), contracts[&key].dimension.clone());
                }
            }
        }
        let mut values = BTreeMap::new();
        for component in unit.document.components() {
            for item in component.items() {
                if let ComponentItem::Instance(instance) = item {
                    validate_instance(
                        instance,
                        &unit.namespace,
                        &unit.file,
                        aliases,
                        &components,
                        &contracts,
                        &releases,
                        &mut values,
                        &mut projections,
                        &mut diagnostics,
                    );
                }
            }
        }
        for model in unit.document.models() {
            for item in model.items() {
                if let Item::Instance(instance) = item {
                    validate_instance(
                        instance,
                        &unit.namespace,
                        &unit.file,
                        aliases,
                        &components,
                        &contracts,
                        &releases,
                        &mut values,
                        &mut projections,
                        &mut diagnostics,
                    );
                }
            }
        }
        if diagnostics.is_empty()
            && let Err(failure) =
                SourceAstFactory::elaborate_property_terms(&mut unit.document, &dimensions, &values)
        {
            diagnostics.push(error(&unit.file, TextRange::default(), failure.to_string()));
        }
    }
    if diagnostics.is_empty() {
        projections.sort_by(|a, b| {
            (&a.component, &a.requirement, &a.release).cmp(&(
                &b.component,
                &b.requirement,
                &b.release,
            ))
        });
        Ok(projections.into_boxed_slice())
    } else {
        Err(diagnostics)
    }
}

#[allow(clippy::too_many_arguments)]
fn validate_instance(
    instance: &InstanceDecl,
    namespace: &CompilationNamespaceId,
    file: &str,
    aliases: &[ResolvedAlias],
    components: &BTreeMap<Key, (eqiora_lang::ComponentDecl, String)>,
    contracts: &BTreeMap<Key, Contract>,
    releases: &BTreeMap<Key, Release>,
    values: &mut BTreeMap<String, f64>,
    projections: &mut Vec<ResolvedPropertyBinding>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(component_key) = resolve_path(
        namespace,
        instance.definition(),
        aliases,
        components,
        |value| value.0.visibility(),
        file,
        diagnostics,
    ) else {
        return;
    };
    let (component, _) = &components[&component_key];
    let binding_syntax = instance.property_binding_syntax().collect::<Vec<_>>();
    let supplied = binding_syntax
        .iter()
        .map(|(property, release, range)| (*property, (*release, *range)))
        .collect::<BTreeMap<_, _>>();
    let mut seen = BTreeSet::new();
    for (property, _, range) in &binding_syntax {
        if !seen.insert(*property) {
            diagnostics.push(error(
                file,
                *range,
                format!("duplicate property binding `{property}`"),
            ));
        }
    }
    for (requirement, contract_path, _) in component.property_requirement_syntax() {
        let Some((release_path, binding_range)) = supplied.get(requirement).copied() else {
            diagnostics.push(error(
                file,
                instance.range(),
                format!(
                    "instance `{}` requires property `{}`",
                    instance.name(),
                    requirement
                ),
            ));
            continue;
        };
        let Some(required_contract) = resolve_path(
            &component_key.0,
            contract_path,
            aliases,
            contracts,
            |value| value.visibility,
            file,
            diagnostics,
        ) else {
            continue;
        };
        let Some(release_key) = resolve_path(
            namespace,
            release_path,
            aliases,
            releases,
            |value| value.visibility,
            file,
            diagnostics,
        ) else {
            continue;
        };
        let release = &releases[&release_key];
        if release.contract != required_contract {
            diagnostics.push(error(
                file,
                binding_range,
                "property release implements a different nominal contract",
            ));
            continue;
        }
        values.insert(release_path.to_string(), release.value);
        projections.push(ResolvedPropertyBinding {
            contract: qualified(&required_contract),
            release: qualified(&release_key),
            component: qualified(&component_key),
            requirement: requirement.to_owned(),
            normalized_value: release.value,
            validity: "unconditional",
            citation: release.citation.clone(),
            license: release.license.clone(),
        });
    }
    for (property, _, range) in binding_syntax {
        if !component
            .property_requirement_syntax()
            .any(|(name, _, _)| name == property)
        {
            diagnostics.push(error(
                file,
                range,
                format!("component has no property requirement `{property}`"),
            ));
        }
    }
}

fn resolve_path<T>(
    namespace: &CompilationNamespaceId,
    path: &NamePath,
    aliases: &[ResolvedAlias],
    values: &BTreeMap<Key, T>,
    visibility: impl Fn(&T) -> VisibilitySyntax,
    file: &str,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Key> {
    let segments = path.segments().collect::<Vec<_>>();
    let key = if segments.len() == 1 {
        (namespace.clone(), segments[0].to_owned())
    } else if segments.len() == 2 {
        let Some(alias) = aliases
            .iter()
            .find(|value| value.declaring() == namespace && value.alias() == segments[0])
        else {
            diagnostics.push(error(
                file,
                path.range(),
                format!("unknown package alias `{}`", segments[0]),
            ));
            return None;
        };
        (alias.target().clone(), segments[1].to_owned())
    } else {
        diagnostics.push(error(
            file,
            path.range(),
            "property paths support one optional package alias",
        ));
        return None;
    };
    let Some(value) = values.get(&key) else {
        diagnostics.push(error(
            file,
            path.range(),
            format!("unresolved property declaration `{path}`"),
        ));
        return None;
    };
    if &key.0 != namespace && visibility(value) != VisibilitySyntax::Public {
        diagnostics.push(error(
            file,
            path.range(),
            format!("property declaration `{path}` is private"),
        ));
        return None;
    }
    Some(key)
}

fn constant(file: &str, expression: &Expr) -> Result<f64, Diagnostic> {
    let value = match expression.kind() {
        ExprKind::Number(value) => *value,
        ExprKind::Path(path) => crate::math::constant(path).ok_or_else(|| {
            error(
                file,
                expression.range(),
                "property values and normalization must be closed scalar constants",
            )
        })?,
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => -constant(file, value)?,
        ExprKind::Binary { op, left, right } => {
            let left = constant(file, left)?;
            let right = constant(file, right)?;
            match op {
                BinaryOp::Add => left + right,
                BinaryOp::Sub => left - right,
                BinaryOp::Mul => left * right,
                BinaryOp::Div => left / right,
                BinaryOp::Pow => left.powf(right),
            }
        }
        _ => {
            return Err(error(
                file,
                expression.range(),
                "property values and normalization must be closed scalar constants",
            ));
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(error(
            file,
            expression.range(),
            "property constant must be finite",
        ))
    }
}

fn qualified(key: &Key) -> String {
    format!("{}::{}", key.0, key.1)
}

fn error(file: &str, range: TextRange, message: impl Into<String>) -> Diagnostic {
    source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message)
}

#[cfg(test)]
mod tests {
    use crate::{
        CompilationNamespaceId, ResolvedHierarchyInput, ResolvedSourceUnit,
        analyze_resolved_hierarchy,
    };

    #[test]
    fn exact_scalar_property_elaborates_through_parameter_terms() {
        let root = CompilationNamespaceId::new(["root", "1.0.0", "semantic-digest"]).unwrap();
        let source = r#"
public property contract Diffusivity { scalar value: m ^ 2 / s; }
property release ReferenceDiffusivity implements Diffusivity {
  value = 25;
  source_unit: m ^ 2 / s = 1 / 1000;
  validity = unconditional;
  citation = org.example.measurement;
  license = spdx.CC0_1_0;
}
public component Diffusion {
  public property diffusivity: Diffusivity;
  relation law continuous { diffusivity = 0; }
}
model Main { instance domain: Diffusion(property diffusivity = ReferenceDiffusivity); }
"#;
        let input = ResolvedHierarchyInput::new(
            root.clone(),
            vec![ResolvedSourceUnit::new(root.clone(), "root.eqi", source)],
            vec![],
        );
        let analyzed = analyze_resolved_hierarchy(input).expect("property graph analyzes");
        assert_eq!(analyzed.property_bindings().len(), 1);
        assert_eq!(analyzed.property_bindings().next().unwrap().4, 0.025);
        let property_model = analyzed
            .validate_definitions()
            .expect("property definitions validate")
            .compile_root("Main")
            .expect("property model compiles");
        let direct = r#"
public component Diffusion {
  public parameter diffusivity: m ^ 2 / s;
  relation law continuous { diffusivity = 0; }
}
model Main { instance domain: Diffusion(diffusivity = 0.025); }
"#;
        let direct_model = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root.clone(),
            vec![ResolvedSourceUnit::new(root, "direct.eqi", direct)],
            vec![],
        ))
        .unwrap()
        .validate_definitions()
        .unwrap()
        .compile_root("Main")
        .unwrap();
        assert_eq!(
            property_model.symbols().get("domain.law"),
            direct_model.symbols().get("domain.law"),
            "property binding reuses the same effective scalar Law"
        );
    }

    #[test]
    fn incompatible_and_incomplete_property_bindings_fail_before_compilation() {
        let root = CompilationNamespaceId::new(["root", "1.0.0", "semantic-digest"]).unwrap();
        let wrong_dimension = r#"
property contract Diffusivity { scalar value: m ^ 2 / s; }
property release Wrong implements Diffusivity {
  value = 1; source_unit: kg = 1; validity = unconditional;
  citation = org.example.measurement; license = spdx.CC0_1_0;
}
model Main {}
"#;
        let diagnostics = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root.clone(),
            vec![ResolvedSourceUnit::new(
                root.clone(),
                "wrong.eqi",
                wrong_dimension,
            )],
            vec![],
        ))
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|value| value.message().contains("does not match"))
        );

        let missing = r#"
property contract Diffusivity { scalar value: m ^ 2 / s; }
component Diffusion {
  public property diffusivity: Diffusivity;
  relation law continuous { diffusivity = 0; }
}
model Main { instance domain: Diffusion; }
"#;
        let diagnostics = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root.clone(),
            vec![ResolvedSourceUnit::new(root, "missing.eqi", missing)],
            vec![],
        ))
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|value| value.message().contains("requires property"))
        );
    }
}
