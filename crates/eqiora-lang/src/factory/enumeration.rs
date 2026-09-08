//! Checked module enum declarations and explicit case arms.
use super::{
    AstConstructionError, SourceAstFactory, checked_identifier, checked_range, validate_expression,
    validate_name_path,
};
use crate::{CaseArm, EnumDecl, Expr, NamePath, TextRange, VisibilitySyntax};
use std::collections::HashSet;

impl SourceAstFactory {
    /// Construct a module enum with nonempty distinct tag tokens in authored order.
    ///
    /// # Errors
    /// Rejects malformed names/ranges, duplicate tags, or excessive cardinality.
    pub fn enumeration(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        tags: Vec<NamePath>,
        range: TextRange,
    ) -> Result<EnumDecl, AstConstructionError> {
        if tags.is_empty() || tags.len() > eqiora_core::ValueType::MAX_ENUM_MEMBERS as usize {
            return Err(AstConstructionError::new(
                "enum requires between 1 and 65536 tags",
            ));
        }
        let mut seen = HashSet::new();
        for tag in &tags {
            validate_name_path(tag)?;
            if tag.is_qualified() || tag.as_str() == "_" {
                return Err(AstConstructionError::new(
                    "enum tag requires one non-wildcard identifier",
                ));
            }
            if !seen.insert(tag.as_str()) {
                return Err(AstConstructionError::new("duplicate enum tag"));
            }
        }
        Ok(EnumDecl {
            comments: Default::default(),
            visibility,
            name: checked_identifier(name, "enum name")?,
            tags,
            range: checked_range(range)?,
        })
    }
    /// Replace a case arm value while preserving its checked pattern identity.
    ///
    /// # Errors
    /// Rejects malformed replacement expressions or arm ranges.
    pub fn case_arm_value(arm: &CaseArm, value: Expr) -> Result<CaseArm, AstConstructionError> {
        validate_expression(&value)?;
        checked_range(arm.range())?;
        Ok(CaseArm {
            resolved_pattern: arm.resolved_pattern.clone(),
            pattern: arm.pattern.clone(),
            value,
            range: arm.range,
        })
    }
    /// Construct an explicit qualified-tag case arm without introducing a binder.
    ///
    /// # Errors
    /// Rejects nonqualified patterns, wildcards, malformed expressions or ranges.
    pub fn case_arm(
        pattern: NamePath,
        value: Expr,
        range: TextRange,
    ) -> Result<CaseArm, AstConstructionError> {
        validate_pattern(&pattern)?;
        validate_expression(&value)?;
        Ok(CaseArm {
            resolved_pattern: None,
            pattern,
            value,
            range: checked_range(range)?,
        })
    }
}
fn validate_pattern(pattern: &NamePath) -> Result<(), AstConstructionError> {
    validate_name_path(pattern)?;
    if !pattern.is_qualified() || pattern.segments().any(|segment| segment == "_") {
        return Err(AstConstructionError::new(
            "case pattern requires a qualified enum tag without wildcards",
        ));
    }
    Ok(())
}
pub(super) fn validate_arms(arms: &[CaseArm]) -> Result<(), AstConstructionError> {
    if arms.is_empty() || arms.len() > eqiora_core::ValueType::MAX_ENUM_MEMBERS as usize {
        return Err(AstConstructionError::new(
            "case requires between 1 and 65536 arms",
        ));
    }
    let mut seen = HashSet::new();
    for arm in arms {
        validate_pattern(arm.pattern())?;
        checked_range(arm.range())?;
        if !seen.insert(arm.pattern().as_str()) {
            return Err(AstConstructionError::new("duplicate case pattern"));
        }
    }
    Ok(())
}

impl SourceAstFactory {
    /// Bind an authored qualified enum member to its exact checked declaration.
    ///
    /// # Errors
    /// Rejects wrong paths, unknown tags, foreign rebinding, or inconsistent metadata.
    #[doc(hidden)]
    pub fn bind_enum_member(
        expression: &mut Expr,
        declaration: &NamePath,
        definition: &eqiora_schema::kernel::EnumDef,
    ) -> Result<(), AstConstructionError> {
        let crate::ExprKind::Path(path) = expression.kind() else {
            return Err(AstConstructionError::new(
                "enum member requires its qualified declaration path",
            ));
        };
        let literal = resolve_member(path, declaration, definition)?;
        if expression
            .resolved_enum()
            .is_some_and(|previous| previous != &literal)
            || expression
                .resolved_nominal()
                .is_some_and(|previous| previous != literal.value_type())
        {
            return Err(AstConstructionError::new(
                "enum member cannot be rebound to another declaration or tag",
            ));
        }
        expression.resolved_nominal = None;
        expression.resolved_enum = Some(Box::new(literal));
        Ok(())
    }
    /// Bind a qualified case pattern to the same exact enum-member owner.
    ///
    /// # Errors
    /// Rejects wrong paths, unknown tags, or foreign rebinding.
    #[doc(hidden)]
    pub fn bind_case_pattern(
        arm: &mut CaseArm,
        declaration: &NamePath,
        definition: &eqiora_schema::kernel::EnumDef,
    ) -> Result<(), AstConstructionError> {
        let literal = resolve_member(arm.pattern(), declaration, definition)?;
        if arm
            .resolved_pattern()
            .is_some_and(|previous| previous != &literal)
        {
            return Err(AstConstructionError::new(
                "case pattern cannot be rebound to another declaration or tag",
            ));
        }
        arm.resolved_pattern = Some(Box::new(literal));
        Ok(())
    }
}

fn resolve_member(
    path: &NamePath,
    declaration: &NamePath,
    definition: &eqiora_schema::kernel::EnumDef,
) -> Result<eqiora_core::ValueLiteral, AstConstructionError> {
    validate_name_path(path)?;
    validate_name_path(declaration)?;
    let mut parts = path.segments();
    for expected in declaration.segments() {
        if parts.next() != Some(expected) {
            return Err(AstConstructionError::new(
                "enum member path has a different declaration prefix",
            ));
        }
    }
    let tag = parts
        .next()
        .ok_or_else(|| AstConstructionError::new("enum member path is missing its tag"))?;
    if parts.next().is_some() {
        return Err(AstConstructionError::new(
            "enum member path has extra segments",
        ));
    }
    let tag = definition
        .members()
        .iter()
        .position(|member| member == tag)
        .ok_or_else(|| AstConstructionError::new("unknown enum tag"))?;
    definition
        .value(tag as u32)
        .map_err(|_| AstConstructionError::new("enum tag is outside declaration bounds"))
}
