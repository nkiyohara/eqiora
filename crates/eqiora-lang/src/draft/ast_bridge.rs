//! Synthetic ranges and physical accessors for the native AST bridge.

use std::collections::HashMap;

use eqiora_core::GraphPath;

use super::{DraftPortReference, NativeModelAst};
use crate::ast::{Expr, ExprKind, ModelDecl, NamePath, TextRange};

#[derive(Debug, Default)]
pub(super) struct RangeAllocator {
    next: u32,
}

impl RangeAllocator {
    pub(super) fn allocate(
        &mut self,
        path: &GraphPath,
        paths: &mut HashMap<TextRange, GraphPath>,
    ) -> TextRange {
        let start = self.next;
        self.next = self.next.saturating_add(1);
        let range = TextRange::new(start, self.next);
        paths.insert(range, path.clone());
        range
    }
}

pub(super) fn physical_accessor_ast(
    callee: &str,
    reference: &DraftPortReference,
    path: &GraphPath,
    ranges: &mut RangeAllocator,
    paths: &mut HashMap<TextRange, GraphPath>,
) -> ExprKind {
    ExprKind::Call {
        callee: NamePath::single(callee.to_owned(), ranges.allocate(path, paths)),
        arguments: vec![Expr {
            kind: ExprKind::Name(reference.name.clone()),
            range: ranges.allocate(path, paths),
        }],
    }
}

impl NativeModelAst {
    /// Source-shaped model consumed by the shared compiler lowerer.
    #[must_use]
    pub const fn model(&self) -> &ModelDecl {
        &self.model
    }

    /// Native declaration path associated with one synthetic range.
    #[must_use]
    pub fn graph_path(&self, range: TextRange) -> Option<&GraphPath> {
        self.paths.get(&range)
    }
}
