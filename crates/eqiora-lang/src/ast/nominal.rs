//! Source declarations for nominal finite spaces and bounded index sets.

use super::{NamePath, TextRange};

pub(crate) fn definition_call(
    name: &str,
    arguments: Vec<super::Expr>,
    range: TextRange,
) -> super::Expr {
    super::Expr {
        kind: super::ExprKind::Call {
            callee: NamePath::single(name.to_owned(), range),
            arguments,
        },
        range,
    }
}
