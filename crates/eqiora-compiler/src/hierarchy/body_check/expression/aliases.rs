//! Intrinsic alias types and shared, use-context evolution obligations.

use super::*;
use std::collections::BTreeSet;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub(in crate::hierarchy::body_check) struct AliasContract {
    pub(super) inferred: ExpressionType<String>,
    pub(super) field_target: Option<String>,
    dependencies: Vec<Arc<AliasContract>>,
    evolution: Vec<EvolutionRequirement>,
    endpoints: PhysicalEndpointSelections,
}

#[derive(Debug, Clone)]
pub(super) struct EvolutionRequirement {
    operator: String,
    target: String,
    range: eqiora_lang::TextRange,
}

pub(in crate::hierarchy::body_check) fn validate_aliases<'a>(
    scope: &mut DefinitionScope<'_, '_>,
    declarations: impl Iterator<Item = &'a eqiora_lang::LetDecl>,
    static_values: &crate::hierarchy::parameters::SymbolicParameterMap,
) -> Result<(), Vec<Diagnostic>> {
    let order = crate::hierarchy::parameters::alias_order(scope.file, declarations)?;
    let mut errors = Vec::new();
    for declaration in order {
        if static_values.contains_key(declaration.name()) {
            if declaration.domain().is_some() {
                errors.push(source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    scope.file,
                    declaration.range(),
                    "static let alias cannot assert spatial support",
                ));
            }
            continue;
        }
        let mut checker = ExpressionChecker {
            scope,
            relation_support: None,
            family_scope: None,
            allow_discrete_symbols: false,
            initial: false,
            activation: &ActivationSyntax::Continuous,
            physical_endpoints: PhysicalEndpointSelections::new(),
            intrinsic: true,
            alias_dependencies: Vec::new(),
            evolution: Vec::new(),
        };
        let inferred = match checker.check(declaration.value()).and_then(|inferred| {
            if let Some(domain) = declaration.domain() {
                let expected = scope.spatial_support(domain).ok_or_else(|| {
                    scope.wrong_local_kind(declaration.range(), domain, "let alias spatial support")
                })?;
                if inferred.support.as_ref() != Some(&expected) {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        scope.file,
                        declaration.range(),
                        "let alias support assertion does not match its inferred exact support",
                    ));
                }
            }
            if let Some(assertion) = declaration.value_type() {
                let expected = crate::value_types::lower_value_type(
                    scope.file,
                    assertion,
                    inferred.support.as_ref(),
                )?;
                if expected != inferred.value_type {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        scope.file,
                        declaration.range(),
                        "let alias type assertion does not match its inferred value type",
                    ));
                }
            }
            Ok(inferred)
        }) {
            Ok(value) => value,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };
        let field_target = match declaration.value().kind() {
            ExprKind::Name(name) => match scope.symbols.get(name) {
                Some(SymbolContract::Field(..)) => Some(name.clone()),
                Some(SymbolContract::Alias(alias)) => alias.field_target.clone(),
                _ => None,
            },
            _ => None,
        };
        let alias = AliasContract {
            inferred,
            field_target,
            dependencies: checker.alias_dependencies,
            evolution: checker.evolution,
            endpoints: checker.physical_endpoints,
        };
        scope.symbols.insert(
            declaration.name().to_owned(),
            SymbolContract::Alias(Arc::new(alias)),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

impl ExpressionChecker<'_, '_, '_> {
    pub(super) fn use_alias(
        &mut self,
        alias: Arc<AliasContract>,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        let inferred = alias.inferred.clone();
        if self.intrinsic {
            self.alias_dependencies.push(alias);
            return Ok(inferred);
        }
        let mut pending = vec![alias];
        let mut seen = BTreeSet::new();
        while let Some(alias) = pending.pop() {
            if !seen.insert(Arc::as_ptr(&alias) as usize) {
                continue;
            }
            self.physical_endpoints
                .extend(alias.endpoints.iter().cloned());
            for requirement in &alias.evolution {
                let argument = eqiora_lang::SourceAstFactory::expression(
                    ExprKind::Name(requirement.target.clone()),
                    requirement.range,
                )
                .expect("retained source name and range");
                self.check_evolution(&requirement.operator, &argument, &argument)?;
            }
            pending.extend(alias.dependencies.iter().cloned());
        }
        Ok(inferred)
    }

    pub(super) fn check_evolution(
        &mut self,
        callee_name: &str,
        expression: &Expr,
        argument: &Expr,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        let ExprKind::Name(name) = argument.kind() else {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                argument.range(),
                format!("{callee_name}(...) requires one Field name"),
            ));
        };
        let target = match self.scope.symbols.get(name) {
            Some(SymbolContract::Alias(alias)) => {
                alias.field_target.as_deref().ok_or_else(|| {
                    source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        argument.range(),
                        "evolution operator requires an identity alias of one eligible Field",
                    )
                })?
            }
            _ => name.as_str(),
        };
        let inferred = match self.scope.symbols.get(target) {
            Some(SymbolContract::Field(inferred, role, activation)) => {
                if matches!(callee_name, "derivative" | "pre" | "next")
                    && (*role != eqiora_lang::FieldRoleSyntax::State
                        || (callee_name == "derivative"
                            && !matches!(activation, ActivationSyntax::Continuous)))
                {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        expression.range(),
                        "evolution operator requires an eligible declared state",
                    ));
                }
                if matches!(callee_name, "pre" | "next")
                    && (!matches!(activation, ActivationSyntax::Periodic(_))
                        || (!self.intrinsic && !self.initial && activation != self.activation)
                        || (!self.intrinsic && self.initial && callee_name == "next"))
                {
                    return Err(source_error(
                        codes::LANGUAGE_TYPE_ERROR,
                        self.scope.file,
                        expression.range(),
                        "discrete state operator requires the exact clock and cannot assign next during initialization",
                    ));
                }
                inferred.clone()
            }
            _ => {
                return Err(unresolved(
                    self.scope.file,
                    argument.range(),
                    name,
                    "Field operator argument",
                ));
            }
        };
        if matches!(callee_name, "pre" | "next") && !self.intrinsic && !self.allow_discrete_symbols
        {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                format!("continuous Relation cannot use `{callee_name}`"),
            ));
        }
        if callee_name == "derivative"
            && !self.intrinsic
            && self.allow_discrete_symbols
            && !self.initial
        {
            return Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                "clocked Relation cannot use `derivative`",
            ));
        }
        if self.intrinsic {
            self.evolution.push(EvolutionRequirement {
                operator: callee_name.to_owned(),
                target: target.to_owned(),
                range: expression.range(),
            });
        }
        match callee_name {
            "derivative" => typing::time_derivative(&inferred)
                .map_err(|error| type_error(self.scope.file, expression, error)),
            "pre" | "next" => Ok(inferred),
            _ => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                format!("unknown scalar operator `{callee_name}`"),
            )),
        }
    }
}
