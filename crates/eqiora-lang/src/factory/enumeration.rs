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
        if tags.is_empty() || tags.len() > 65_536 {
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
    if arms.is_empty() || arms.len() > 65_536 {
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
