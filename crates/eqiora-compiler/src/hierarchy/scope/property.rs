//! Exact release binding in the caller's lexical scope.
use super::super::preflight::{DefinitionNamespace, Elaborator};
use super::*;
use std::sync::Arc;

impl Scope {
    pub(in crate::hierarchy) fn extend_properties(
        &mut self,
        properties: &BTreeMap<String, Arc<eqiora_schema::kernel::PropertyRelease>>,
    ) {
        for (name, release) in properties {
            self.insert_property(name.clone(), release.clone());
        }
    }

    pub(in crate::hierarchy) fn insert_property(
        &mut self,
        name: String,
        release: Arc<eqiora_schema::kernel::PropertyRelease>,
    ) {
        if let Some(value) = release.meaning().constant_value() {
            self.insert_parameter(
                name.clone(),
                super::super::parameters::ResolvedParameter {
                    value: value.clone(),
                    expression: LoweringExpression::property(
                        release.clone(),
                        Vec::new(),
                        TextRange::default(),
                    ),
                    lineage: super::super::parameters::ParameterLineage::Constant,
                },
            );
        }
        self.properties.insert(name, release);
    }

    pub(in crate::hierarchy) fn bind_properties(
        &mut self,
        elaborator: &Elaborator<'_>,
        namespace: &DefinitionNamespace,
        component: &eqiora_lang::ComponentDecl,
        instance: &eqiora_lang::InstanceDecl,
        parent: &Scope,
        file: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        for requirement in component.signature().iter().filter_map(|item| match item {
            eqiora_lang::SignatureItem::Property(value) => Some(value),
            _ => None,
        }) {
            let bindings = instance
                .bindings()
                .iter()
                .filter(|binding| binding.name() == requirement.name())
                .collect::<Vec<_>>();
            let [binding] = bindings.as_slice() else {
                return Err(vec![source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    instance.range(),
                    format!(
                        "instance requires property `{}` exactly once",
                        requirement.name()
                    ),
                )]);
            };
            let forwarded = match binding.value().kind() {
                ExprKind::Name(name) => parent.properties.get(name).cloned(),
                _ => None,
            };
            let release = if let Some(release) = forwarded {
                let expected = elaborator
                    .property_contract_identity(namespace, requirement.contract(), file)
                    .map_err(|error| vec![error])?;
                if release.contract() != expected {
                    return Err(vec![source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        file,
                        binding.range(),
                        "forwarded property release implements a different nominal contract",
                    )]);
                }
                release
            } else {
                let caller = parent.lexical_namespace.as_ref().ok_or_else(|| {
                    vec![super::super::hierarchy_error(
                        "property caller is missing its lexical namespace",
                    )]
                })?;
                Arc::new(elaborator.bind_property(
                    namespace,
                    requirement,
                    caller,
                    binding.value(),
                    file,
                )?)
            };
            let component = super::super::preflight::DefinitionKey {
                namespace: namespace.clone(),
                name: component.name().to_owned(),
            }
            .display();
            let release = (*release)
                .clone()
                .for_requirement(component, requirement.name().to_owned())
                .map_err(|error| vec![error])?;
            self.insert_property(requirement.name().to_owned(), Arc::new(release));
        }
        Ok(())
    }

    pub(super) fn property_value(
        &self,
        file: &str,
        name: &str,
        range: TextRange,
    ) -> Result<Option<LoweringExpression>, Diagnostic> {
        let Some(release) = self.properties.get(name) else {
            return Ok(None);
        };
        if !release.inputs().is_empty() {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                file,
                range,
                "analytic property requires all declared named inputs",
            ));
        }
        Ok(Some(LoweringExpression::property(
            release.clone(),
            Vec::new(),
            range,
        )))
    }
}

pub(super) fn rewrite(
    file: &str,
    expression: &Expr,
    callee: &NamePath,
    arguments: &eqiora_lang::CallArguments,
    scope: &Scope,
    active: Option<ActiveBoundaryMember<'_>>,
) -> Result<Option<LoweringExpression>, Diagnostic> {
    let Some(release) = scope.properties.get(callee.as_str()).cloned() else {
        return Ok(None);
    };
    let arguments = crate::pure_operator::ordered_arguments(
        file,
        expression.range(),
        release.inputs().iter().map(String::as_str),
        arguments,
    )?
    .into_iter()
    .map(|argument| rewrite_expression_with_boundary_member(file, argument, scope, active))
    .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(LoweringExpression::property(
        release,
        arguments,
        expression.range(),
    )))
}
