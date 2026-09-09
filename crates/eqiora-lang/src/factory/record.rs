//! Checked native construction of closed record source declarations.
use super::{AstConstructionError, SourceAstFactory, checked_identifier, checked_range};
use crate::{Document, RecordDecl, RecordMemberDecl, TextRange, ValueTypeSyntax, VisibilitySyntax};
use std::collections::HashSet;

impl SourceAstFactory {
    /// Construct one typed record member.
    ///
    /// # Errors
    /// Rejects invalid identifiers or source ranges.
    pub fn record_member(
        name: impl Into<String>,
        value_type: ValueTypeSyntax,
        range: TextRange,
    ) -> Result<RecordMemberDecl, AstConstructionError> {
        let name = checked_identifier(name, "record member")?;
        if name == "_" {
            return Err(AstConstructionError::new(
                "record member cannot be a wildcard",
            ));
        }
        Ok(RecordMemberDecl {
            comments: Default::default(),
            name,
            value_type,
            range: checked_range(range)?,
        })
    }

    /// Construct a nonempty closed record with unique declaration-ordered members.
    ///
    /// # Errors
    /// Rejects invalid names/ranges, duplicate members, or excessive cardinality.
    pub fn record(
        visibility: VisibilitySyntax,
        name: impl Into<String>,
        members: Vec<RecordMemberDecl>,
        range: TextRange,
    ) -> Result<RecordDecl, AstConstructionError> {
        if members.is_empty() || members.len() > 65_536 {
            return Err(AstConstructionError::new(
                "record requires between 1 and 65536 members",
            ));
        }
        let mut seen = HashSet::new();
        for member in &members {
            if !seen.insert(member.name()) {
                return Err(AstConstructionError::new("duplicate record member"));
            }
        }
        Ok(RecordDecl {
            comments: Default::default(),
            visibility,
            name: checked_identifier(name, "record name")?,
            members,
            range: checked_range(range)?,
        })
    }

    /// Append a checked closed record declaration to an owned module.
    #[must_use]
    pub fn with_record(mut document: Document, declaration: RecordDecl) -> Document {
        document.records.push(declaration);
        document
    }
}
