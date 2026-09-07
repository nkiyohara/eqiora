//! Model defaults share the same typed dependency graph as Component defaults.

use super::*;
use eqiora_lang::{Item, ModelDecl, SignatureItem, SourceAstFactory};
use eqiora_schema::kernel::RationalTime;

fn resolve(
    file: &str,
    model: &ModelDecl,
    required_policy: RequiredParameterPolicy,
    resolve_clock: &mut dyn FnMut(&str) -> Option<Option<RationalTime>>,
) -> Result<SymbolicParameterMap, Vec<Diagnostic>> {
    let mut declarations = model
        .signature()
        .iter()
        .filter_map(|item| match item {
            SignatureItem::Parameter(value) => Some(value.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for item in model.items() {
        if let Item::Parameter(value) = item {
            declarations.push(
                SourceAstFactory::component_parameter(
                    VisibilitySyntax::Private,
                    value.name(),
                    value.value_type().clone(),
                    Some(value.value().clone()),
                    value.range(),
                )
                .map_err(|error| vec![hierarchy_error(error.to_string())])?,
            );
        }
    }
    SymbolicParameterResolver {
        declaration_file: file,
        declarations: declarations
            .iter()
            .map(|value| (value.name().to_owned(), value))
            .collect(),
        overrides: BTreeMap::new(),
        resolved: BTreeMap::new(),
        required_policy,
    }
    .resolve_all(resolve_clock)
}

pub(in crate::hierarchy) fn resolve_model_parameters_symbolically(
    file: &str,
    model: &ModelDecl,
    mut resolve_clock: impl FnMut(&str) -> Option<Option<RationalTime>>,
) -> Result<SymbolicParameterMap, Vec<Diagnostic>> {
    resolve(
        file,
        model,
        RequiredParameterPolicy::PublicIsFree,
        &mut resolve_clock,
    )
}

pub(in crate::hierarchy) fn resolve_model_parameters(
    file: &str,
    model: &ModelDecl,
    mut resolve_clock: impl FnMut(&str) -> Option<Option<RationalTime>>,
) -> Result<BTreeMap<String, ResolvedParameter>, Vec<Diagnostic>> {
    resolve(
        file,
        model,
        RequiredParameterPolicy::RejectUnbound,
        &mut resolve_clock,
    )
    .and_then(concrete_parameters)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn model(source: &str) -> ModelDecl {
        let document = eqiora_lang::parse("period.eqi", source).unwrap();
        document.models()[0].clone()
    }
    #[test]
    fn period_defaults_are_typed_symbolically_then_resolved_in_dependency_order() {
        let model = model(
            "model M(clock tick: periodic, parameter twice: s = 2 * dt, parameter dt: s = period(tick)) { parameter four: s = 2 * twice; }",
        );
        let symbolic = resolve_model_parameters_symbolically("period.eqi", &model, |name| {
            (name == "tick").then_some(None)
        })
        .unwrap();
        assert_eq!(
            symbolic["twice"].value_type.dimension(),
            crate::dimensions::time_dimension()
        );
        assert!(symbolic["four"].value.is_none());
        let concrete = resolve_model_parameters("period.eqi", &model, |name| {
            (name == "tick").then_some(Some(RationalTime::new(1, 8).unwrap()))
        })
        .unwrap();
        assert_eq!(
            concrete["dt"].value.real_scalar_value().unwrap().value(),
            0.125
        );
        assert_eq!(
            concrete["twice"].value.real_scalar_value().unwrap().value(),
            0.25
        );
        assert_eq!(
            concrete["four"].value.real_scalar_value().unwrap().value(),
            0.5
        );
    }
    #[test]
    fn period_requires_nominal_clock_and_time_dimension() {
        for source in [
            "model M(parameter dt: s = period(missing)) {}",
            "model M(parameter dt: s = period(1)) {}",
            "model M(parameter dt: s = period()) {}",
            "model M(clock tick: periodic, parameter dt: m = period(tick)) {}",
        ] {
            let model = model(source);
            assert!(
                resolve_model_parameters_symbolically("period.eqi", &model, |name| (name
                    == "tick")
                    .then_some(None))
                .is_err(),
                "{source}"
            );
        }
        let model = model("model M(clock tick: periodic, parameter dt: s = period(tick)) {}");
        assert!(resolve_model_parameters("period.eqi", &model, |_| Some(None)).is_err());
    }
}
