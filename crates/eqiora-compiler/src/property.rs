use std::collections::{BTreeMap, BTreeSet};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_lang::{
    BinaryOp, ComponentItem, Expr, ExprKind, InstanceDecl, Item, NamePath, SourceAstFactory,
    TextRange, UnaryOp, VisibilitySyntax,
};

use crate::diagnostics::source_error;
use crate::dimensions::lower_dimension;
use crate::resolved::{AnalyzedSourceUnit, CompilationModuleId, ResolvedAlias};

type Key = (CompilationModuleId, String);

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ResolvedPropertyBinding {
    composition: Option<String>,
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
    pub fn composition(&self) -> Option<&str> {
        self.composition.as_deref()
    }
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

struct Composition {
    visibility: VisibilitySyntax,
    properties: Vec<(String, Key, TextRange)>,
}

pub(crate) fn validate_and_elaborate(
    units: &mut [AnalyzedSourceUnit],
    aliases: &[ResolvedAlias],
) -> Result<Box<[ResolvedPropertyBinding]>, Vec<Diagnostic>> {
    let has_property_syntax = units.iter().any(|unit| {
        unit.document.property_contract_syntax().len() != 0
            || unit.document.property_release_syntax().len() != 0
            || unit.document.material_composition_syntax().len() != 0
            || unit.document.components().iter().any(|component| {
                component.property_requirement_syntax().len() != 0
                    || component.items().iter().any(|item| matches!(
                        item, ComponentItem::Instance(value) if value.property_binding_syntax().len() != 0
                            || value.material_binding_syntax().is_some()
                    ))
            })
            || unit.document.models().iter().any(|model| model.items().iter().any(|item| matches!(
                item, Item::Instance(value) if value.property_binding_syntax().len() != 0
                    || value.material_binding_syntax().is_some()
            )))
    });
    if !has_property_syntax {
        return Ok(Box::new([]));
    }
    let mut diagnostics = Vec::new();
    let mut contracts = BTreeMap::new();
    for unit in units.iter() {
        for (visibility, name, dimension, range) in unit.document.property_contract_syntax() {
            if name == crate::math::ROOT {
                diagnostics.push(error(
                    &unit.file,
                    range,
                    "identifier `math` is reserved for compiler-owned scalar mathematics",
                ));
                continue;
            }
            if let Err(diagnostic) = lower_dimension(&unit.file, dimension) {
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
            let value = match crate::units::normalize_value(source_value, scale) {
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

    let components = units
        .iter()
        .flat_map(|unit| {
            unit.document.components().iter().map(move |value| {
                (
                    (unit.module.clone(), value.name().to_owned()),
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
                    &unit.module,
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
        let mut material_values = BTreeMap::new();
        for component in unit.document.components() {
            for item in component.items() {
                if let ComponentItem::Instance(instance) = item {
                    validate_instance(
                        instance,
                        &unit.module,
                        &unit.file,
                        aliases,
                        &components,
                        &contracts,
                        &releases,
                        &compositions,
                        &mut values,
                        &mut material_values,
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
                        &unit.module,
                        &unit.file,
                        aliases,
                        &components,
                        &contracts,
                        &releases,
                        &compositions,
                        &mut values,
                        &mut material_values,
                        &mut projections,
                        &mut diagnostics,
                    );
                }
            }
        }
        if diagnostics.is_empty()
            && let Err(failure) = SourceAstFactory::elaborate_property_terms(
                &mut unit.document,
                &dimensions,
                &values,
                &material_values,
            )
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
    namespace: &CompilationModuleId,
    file: &str,
    aliases: &[ResolvedAlias],
    components: &BTreeMap<Key, (eqiora_lang::ComponentDecl, String)>,
    contracts: &BTreeMap<Key, Contract>,
    releases: &BTreeMap<Key, Release>,
    compositions: &BTreeMap<Key, Composition>,
    values: &mut BTreeMap<String, f64>,
    material_values: &mut BTreeMap<String, Vec<(String, f64)>>,
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
    if instance.material_binding_syntax().is_some() && !binding_syntax.is_empty() {
        diagnostics.push(error(
            file,
            instance.range(),
            "an instance cannot combine a material composition with direct property bindings",
        ));
        return;
    }
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
    let mut supplied = BTreeMap::<String, (Key, TextRange, Option<Key>)>::new();
    if let Some(material_path) = instance.material_binding_syntax() {
        let Some(composition_key) = resolve_path(
            namespace,
            material_path,
            aliases,
            compositions,
            |value| value.visibility,
            file,
            diagnostics,
        ) else {
            return;
        };
        for (property, release, range) in &compositions[&composition_key].properties {
            supplied.insert(
                property.clone(),
                (release.clone(), *range, Some(composition_key.clone())),
            );
        }
    } else {
        for (property, release_path, range) in &binding_syntax {
            if let Some(release) = resolve_path(
                namespace,
                release_path,
                aliases,
                releases,
                |value| value.visibility,
                file,
                diagnostics,
            ) {
                supplied.insert((*property).to_owned(), (release, *range, None));
            }
        }
    }
    let mut bound_material_values = Vec::new();
    for (requirement, contract_path, _) in component.property_requirement_syntax() {
        let Some((release_key, binding_range, composition_key)) = supplied.get(requirement) else {
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
        let release = &releases[release_key];
        if release.contract != required_contract {
            diagnostics.push(error(
                file,
                *binding_range,
                "property release implements a different nominal contract",
            ));
            continue;
        }
        if composition_key.is_some() {
            bound_material_values.push((requirement.to_owned(), release.value));
        } else if let Some((_, release_path, _)) = binding_syntax
            .iter()
            .find(|(property, _, _)| *property == requirement)
        {
            values.insert(release_path.to_string(), release.value);
        }
        projections.push(ResolvedPropertyBinding {
            composition: composition_key.as_ref().map(qualified),
            contract: qualified(&required_contract),
            release: qualified(release_key),
            component: qualified(&component_key),
            requirement: requirement.to_owned(),
            normalized_value: release.value,
            validity: "unconditional",
            citation: release.citation.clone(),
            license: release.license.clone(),
        });
    }
    if let Some(material) = instance.material_binding_syntax() {
        material_values.insert(material.to_string(), bound_material_values);
    }
    for (property, (_, range, _)) in &supplied {
        if !component
            .property_requirement_syntax()
            .any(|(name, _, _)| name == property.as_str())
        {
            diagnostics.push(error(
                file,
                *range,
                format!("component has no property requirement `{property}`"),
            ));
        }
    }
}

fn resolve_path<T>(
    namespace: &CompilationModuleId,
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
            .find(|value| value.declaring_module() == namespace && value.alias() == segments[0])
        else {
            diagnostics.push(error(
                file,
                path.range(),
                format!("unknown import alias `{}`", segments[0]),
            ));
            return None;
        };
        (alias.target_module().clone(), segments[1].to_owned())
    } else {
        diagnostics.push(error(
            file,
            path.range(),
            "property paths support one optional import alias",
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
    use std::collections::BTreeMap;

    use crate::{
        CanonicalDeclarationKind, CompilationNamespaceId, ResolvedHierarchyInput,
        ResolvedSourceUnit, analyze_resolved_hierarchy,
    };

    #[test]
    fn rational_property_units_and_pure_operator_dimensions_share_exact_algebra() {
        let source = r#"
pure operator square(x: scalar) -> scalar = component(x) * component(x);
property contract Amplitude { scalar value: m ^ (-1 / 2); }
property release Reference implements Amplitude {
  value = 8;
  source_unit: m ^ (-2 / 4) = 1 / 4;
  validity = unconditional;
  citation = org.example.measurement;
  license = spdx.CC0_1_0;
}
component Wave {
  public support body: volume(ambient_dimension = 1);
  representation space = continuum;
  public property amplitude: Amplitude;
  field value on body as space: m ^ (-1 / 2) = 0;
  field intensity on body as space: m ^ -1 = 0;
  relation law continuous on body { value = amplitude; intensity = square(value); }
}
model Main {
  domain interval = box(0, 1);
  instance wave: Wave(support body = interval, property amplitude = Reference);
}
"#;
        let input = |text: &str| {
            let root = CompilationNamespaceId::new(["root", "1.0.0", "dimension-test"]).unwrap();
            ResolvedHierarchyInput::new(
                root.clone(),
                vec![ResolvedSourceUnit::new(root, "src/main.eqi", text).unwrap()],
                vec![],
            )
        };
        let analyzed = analyze_resolved_hierarchy(input(source)).unwrap();
        // The existing conversion owner applies 8 * (1/4) once.
        assert_eq!(analyzed.property_bindings().next().unwrap().5, 2.0);
        analyzed
            .validate_definitions()
            .unwrap()
            .compile_root("Main")
            .unwrap();
        let wrong_unit = source.replace("source_unit: m ^ (-2 / 4)", "source_unit: m ^ -1");
        assert!(analyze_resolved_hierarchy(input(&wrong_unit)).is_err());
        let wrong_output = source.replace("as space: m ^ -1", "as space: m ^ (-1 / 2)");
        let invalid = analyze_resolved_hierarchy(input(&wrong_output)).unwrap();
        assert!(invalid.validate_definitions().is_err());
    }

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
            vec![
                ResolvedSourceUnit::new(root.clone(), "src/main.eqi", source).expect("source path"),
            ],
            vec![],
        );
        let analyzed = analyze_resolved_hierarchy(input).expect("property graph analyzes");
        assert_eq!(analyzed.property_bindings().len(), 1);
        assert_eq!(analyzed.property_bindings().next().unwrap().5, 0.025);
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
            vec![ResolvedSourceUnit::new(root, "src/main.eqi", direct).expect("source path")],
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
    fn material_composition_binds_multiple_properties_to_one_component_law() {
        let root = CompilationNamespaceId::new(["root", "1.0.0", "semantic-digest"]).unwrap();
        let source = r#"
public property contract Conductivity { scalar value: 1; }
public property contract Capacity { scalar value: 1; }
public property release ConductivityA implements Conductivity {
  value = 2; source_unit: 1 = 1; validity = unconditional;
  citation = org.example.a; license = spdx.CC0_1_0;
}
public property release CapacityA implements Capacity {
  value = 4; source_unit: 1 = 1; validity = unconditional;
  citation = org.example.a; license = spdx.CC0_1_0;
}
public material composition MaterialA {
  property capacity = CapacityA;
  property conductivity = ConductivityA;
}
public component DiffusionLaw {
  public property conductivity: Conductivity;
  public property capacity: Capacity;
  relation law continuous { conductivity / capacity = 0; }
}
model Main { instance domain: DiffusionLaw(material = MaterialA); }
"#;
        let analyzed = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
            root.clone(),
            vec![ResolvedSourceUnit::new(root, "src/main.eqi", source).expect("source path")],
            vec![],
        ))
        .expect("material composition analyzes");
        let references = analyzed.resolved_references().collect::<Vec<_>>();
        assert_eq!(references.len(), 8);
        assert!(references.windows(2).all(|pair| {
            (pair[0].1, pair[0].2.start(), pair[0].2.end())
                < (pair[1].1, pair[1].2.start(), pair[1].2.end())
        }));
        let reference_kinds = references.iter().fold(
            BTreeMap::<CanonicalDeclarationKind, usize>::new(),
            |mut counts, (target, _, _, _, _)| {
                *counts.entry(target.kind()).or_default() += 1;
                counts
            },
        );
        assert_eq!(
            reference_kinds[&CanonicalDeclarationKind::PropertyContract],
            4
        );
        assert_eq!(
            reference_kinds[&CanonicalDeclarationKind::PropertyRelease],
            2
        );
        assert_eq!(reference_kinds[&CanonicalDeclarationKind::Component], 1);
        assert_eq!(
            reference_kinds[&CanonicalDeclarationKind::MaterialComposition],
            1
        );
        for (_, _, range, _, _) in references {
            let spelling = &source
                [usize::try_from(range.start()).unwrap()..usize::try_from(range.end()).unwrap()];
            assert!(!spelling.contains(char::is_whitespace));
        }
        let bindings = analyzed.property_bindings().collect::<Vec<_>>();
        assert_eq!(bindings.len(), 2);
        assert!(bindings.iter().all(|binding| binding.0.is_some()));
        assert_eq!(bindings[0].0, bindings[1].0);
        analyzed
            .validate_definitions()
            .expect("composed definitions validate")
            .compile_root("Main")
            .expect("composed Law compiles");
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
            vec![
                ResolvedSourceUnit::new(root.clone(), "src/main.eqi", wrong_dimension)
                    .expect("source path"),
            ],
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
            vec![ResolvedSourceUnit::new(root, "src/main.eqi", missing).expect("source path")],
            vec![],
        ))
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|value| value.message().contains("requires property"))
        );

        for (source, expected) in [
            (
                r#"
property contract A { scalar value: 1; }
property release A1 implements A {
  value = 1; source_unit: 1 = 1; validity = unconditional;
  citation = org.example; license = spdx.CC0_1_0;
}
material composition Duplicate {
  property value = A1;
  property value = A1;
}
component Law { public property value: A; relation law continuous { value = 0; } }
model Main { instance law: Law(material = Duplicate); }
"#,
                "duplicate material property",
            ),
            (
                r#"
property contract A { scalar value: 1; }
property contract B { scalar value: 1; }
property release B1 implements B {
  value = 1; source_unit: 1 = 1; validity = unconditional;
  citation = org.example; license = spdx.CC0_1_0;
}
material composition Foreign { property value = B1; }
component Law { public property value: A; relation law continuous { value = 0; } }
model Main { instance law: Law(material = Foreign); }
"#,
                "different nominal contract",
            ),
            (
                r#"
property contract A { scalar value: 1; }
property release A1 implements A {
  value = 1; source_unit: 1 = 1; validity = unconditional;
  citation = org.example; license = spdx.CC0_1_0;
}
material composition EmptyForLaw { property other = A1; }
component Law { public property value: A; relation law continuous { value = 0; } }
model Main { instance law: Law(material = EmptyForLaw); }
"#,
                "requires property `value`",
            ),
            (
                r#"
property contract A { scalar value: 1; }
property release A1 implements A {
  value = 1; source_unit: 1 = 1; validity = unconditional;
  citation = org.example; license = spdx.CC0_1_0;
}
material composition MaterialA { property value = A1; }
component Law { public property value: A; relation law continuous { value = 0; } }
model Main {
  instance law: Law(material = MaterialA, property value = A1);
}
"#,
                "cannot combine",
            ),
        ] {
            let root = CompilationNamespaceId::new(["root", "1.0.0", "material-invalid"]).unwrap();
            let diagnostics = analyze_resolved_hierarchy(ResolvedHierarchyInput::new(
                root.clone(),
                vec![ResolvedSourceUnit::new(root, "src/main.eqi", source).expect("source path")],
                vec![],
            ))
            .unwrap_err();
            assert!(
                diagnostics
                    .iter()
                    .any(|value| value.message().contains(expected)),
                "expected {expected:?} in {diagnostics:?}"
            );
        }
    }
}
