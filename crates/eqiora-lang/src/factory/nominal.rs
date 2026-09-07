//! Checked construction of finite nominal declarations and family binders.

use std::collections::BTreeSet;

use crate::ast::nominal::{FiniteSpaceDecl, IndexFamilyBinderSyntax, IndexSetDecl};
use crate::ast::{Expr, NamePath, TextRange, VisibilitySyntax};

use super::{
    AstConstructionError, SourceAstFactory, checked_identifier, checked_range, validate_expression,
    validate_name_path,
};

impl SourceAstFactory {
    /// Construct a finite space with nonempty, distinct, ordered basis labels.
    ///
    /// # Errors
    /// Rejects invalid identifiers, repeated or absent labels, and malformed ranges.
    pub fn finite_space(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        labels: Vec<String>,
        range: TextRange,
    ) -> Result<FiniteSpaceDecl, AstConstructionError> {
        if labels.is_empty() {
            return Err(AstConstructionError::new(
                "a finite space requires at least one basis label",
            ));
        }
        let mut distinct = BTreeSet::new();
        for label in &labels {
            checked_identifier(label.clone(), "finite space basis label")?;
            if !distinct.insert(label) {
                return Err(AstConstructionError::new(
                    "finite space basis labels must be distinct",
                ));
            }
        }
        Ok(FiniteSpaceDecl {
            comments: Default::default(),
            visibility,
            name: checked_identifier(name, "finite space")?,
            labels,
            range: checked_range(range)?,
        })
    }

    /// Construct an index-set declaration, preserving its exact extent expression.
    /// Integer type, positivity, and structural resource bounds are elaboration checks.
    ///
    /// # Errors
    /// Rejects malformed names, expression trees, and byte ranges.
    pub fn index_set(
        name: impl Into<String>,
        extent: Expr,
        range: TextRange,
    ) -> Result<IndexSetDecl, AstConstructionError> {
        validate_expression(&extent)?;
        Ok(IndexSetDecl {
            comments: Default::default(),
            name: checked_identifier(name, "index set")?,
            extent,
            range: checked_range(range)?,
        })
    }

    /// Construct a local binder over one nominal index-set name.
    ///
    /// # Errors
    /// Rejects malformed identifiers, name paths, and byte ranges.
    pub fn index_family_binder(
        binder: impl Into<String>,
        set: NamePath,
        range: TextRange,
    ) -> Result<IndexFamilyBinderSyntax, AstConstructionError> {
        validate_name_path(&set)?;
        Ok(IndexFamilyBinderSyntax {
            binder: checked_identifier(binder, "index family binder")?,
            set,
            range: checked_range(range)?,
        })
    }
}
