//! Case branch context uses the same exact literal and selection type owners.
use super::*;

impl ExpressionChecker<'_, '_, '_> {
    pub(super) fn check_case(
        &mut self,
        expression: &Expr,
        value: &Expr,
        arms: &[eqiora_lang::CaseArm],
        expected: Option<eqiora_core::ScalarDomain>,
    ) -> Result<ExpressionType<String>, Diagnostic> {
        let selector = self.check(value)?;
        crate::enumeration::case_patterns(
            self.scope.file,
            expression.range(),
            &selector.value_type,
            arms,
        )?;
        let mut branches = arms
            .iter()
            .map(|arm| self.check(arm.value()))
            .collect::<Result<Vec<_>, _>>()?;
        if expected == Some(eqiora_core::ScalarDomain::Integer)
            || branches.iter().any(|branch| {
                branch.value_type.scalar_domain() == eqiora_core::ScalarDomain::Integer
            })
        {
            for (arm, branch) in arms.iter().zip(&mut branches) {
                if integer::numeric_tree(arm.value()) {
                    *branch = self
                        .check_numeric_context(arm.value(), eqiora_core::ScalarDomain::Integer)?;
                }
            }
        }
        crate::enumeration::result_type(selector, &branches)
            .map_err(|error| type_error(self.scope.file, expression, error))
    }
}
