//! Exact independent binding admission for explicit partials.
use super::*;

impl ExpressionChecker<'_, '_, '_> {
    pub(super) fn partial_binding(
        &self,
        binding: &eqiora_lang::NamePath,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        match self.scope.resolve_symbol(binding)? {
            SymbolContract::Parameter(ty)
            | SymbolContract::Field(
                ty,
                eqiora_lang::FieldRoleSyntax::State,
                ActivationSyntax::Continuous,
            ) => Ok(ty),
            _ => Err(source_error(
                codes::LANGUAGE_TYPE_ERROR,
                self.scope.file,
                binding.range(),
                "partial binding must name a declared independent Parameter or continuous state Field; aliases and algebraic solutions are not independent",
            )),
        }
    }
}
