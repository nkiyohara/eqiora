//! Explicit transition typing and separate consumer activation obligations.
use super::*;

impl ExpressionChecker<'_, '_, '_> {
    pub(super) fn check_transition(
        &mut self,
        expression: &Expr,
        name: &str,
        arguments: &[Expr],
    ) -> Result<ExpressionType<String>, Diagnostic> {
        let error = |message| {
            source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                expression.range(),
                message,
            )
        };
        if name == "hold" {
            let [argument] = arguments else {
                return Err(error("hold requires one periodic State"));
            };
            let ExprKind::Name(name) = argument.kind() else {
                return Err(error("hold requires one periodic State name"));
            };
            let target = match self.scope.symbols.get(name) {
                Some(SymbolContract::Alias(alias)) => alias.field_target.as_deref().unwrap_or(name),
                _ => name,
            };
            return match self.scope.symbols.get(target) {
                Some(SymbolContract::Field(
                    inferred,
                    eqiora_lang::FieldRoleSyntax::State,
                    ActivationSyntax::Periodic(_),
                )) => Ok(inferred.clone()),
                _ => Err(error("hold requires one periodic State")),
            };
        }
        let [value, clock] = arguments else {
            return Err(error("sample requires a value and one clock name"));
        };
        let ExprKind::Name(clock) = clock.kind() else {
            return Err(error("sample requires one clock name"));
        };
        if !matches!(self.scope.symbols.get(clock), Some(SymbolContract::Clock)) {
            return Err(error("sample requires one declared periodic clock"));
        }
        if self.sampling
            || (!self.intrinsic
                && (self.initial
                    || !self.scope.activation_matches(
                        self.activation,
                        &ActivationSyntax::Periodic(clock.clone()),
                    )))
        {
            return Err(error("sample requires its exact clock's update relation"));
        }
        if self.intrinsic {
            self.contextual.push(expression.clone());
        }
        let activation = self.activation;
        let intrinsic = self.intrinsic;
        self.activation = &ActivationSyntax::Continuous;
        self.intrinsic = false;
        self.sampling = true;
        let result = self.check(value);
        self.sampling = false;
        self.intrinsic = intrinsic;
        self.activation = activation;
        result
    }
}
