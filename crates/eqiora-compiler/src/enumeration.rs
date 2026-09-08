//! Checked enum declaration binding and exhaustive source case admission.
mod binding;
pub(crate) use binding::{
    BoundEnum, bind_document, bind_resolved, declarations, resolved_namespace,
};

use std::collections::BTreeSet;

use eqiora_core::{Diagnostic, ValueLiteral, ValueType, diagnostic::codes};
use eqiora_lang::{CaseArm, TextRange};

use crate::diagnostics::source_error;

pub(crate) fn case_patterns(
    file: &str,
    range: TextRange,
    scrutinee: &ValueType,
    arms: &[CaseArm],
) -> Result<Vec<ValueLiteral>, Diagnostic> {
    let patterns = arms
        .iter()
        .map(|arm| {
            arm.resolved_pattern().cloned().ok_or_else(|| {
                source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    arm.range(),
                    "case pattern requires its exact lexical enum member binding",
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_patterns(file, range, scrutinee, &patterns)?;
    Ok(patterns)
}

pub(crate) fn validate_patterns(
    file: &str,
    range: TextRange,
    scrutinee: &ValueType,
    patterns: &[ValueLiteral],
) -> Result<(), Diagnostic> {
    let invalid = |message| source_error(codes::LANGUAGE_TYPE_ERROR, file, range, message);
    let count = scrutinee
        .enum_member_count()
        .ok_or_else(|| invalid("case selector requires one exact enum value"))?;
    let mut seen = BTreeSet::new();
    for pattern in patterns {
        if pattern.value_type() != scrutinee {
            return Err(invalid(
                "case pattern belongs to a foreign enum declaration",
            ));
        }
        let tag = pattern
            .enum_tag()
            .ok_or_else(|| invalid("case pattern is not an enum member"))?;
        if !seen.insert(tag) {
            return Err(invalid("case contains a duplicate enum member"));
        }
    }
    if seen.len() != count as usize {
        return Err(invalid("case must cover every enum member exactly once"));
    }
    Ok(())
}

pub(crate) fn result_type<I: Clone + Eq>(
    selector: eqiora_schema::kernel::typing::ExpressionType<I>,
    branches: &[eqiora_schema::kernel::typing::ExpressionType<I>],
) -> Result<
    eqiora_schema::kernel::typing::ExpressionType<I>,
    eqiora_schema::kernel::typing::TypeViolation<I>,
> {
    use eqiora_schema::kernel::{ComparisonOp, typing::TypeViolation};
    if selector.value_type.enum_definition().is_none() {
        return Err(TypeViolation::ScalarDomainMismatch);
    }
    let condition = selector.clone().compare(ComparisonOp::Equal, selector)?;
    let mut result = branches
        .first()
        .cloned()
        .ok_or(TypeViolation::ScalarDomainMismatch)?;
    // Even a singleton case must retain the selector's support obligation.
    result = condition.clone().select(result.clone(), result)?;
    for branch in &branches[1..] {
        result = condition.clone().select(result, branch.clone())?;
    }
    Ok(result)
}
