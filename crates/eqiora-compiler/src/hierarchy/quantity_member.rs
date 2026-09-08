//! Split ordinary member syntax without changing exact Port lookup ownership.

use eqiora_lang::{Expr, ExprKind, NamePath, SourceAstFactory};

pub(super) fn split(expression: &Expr) -> Option<(Expr, &str)> {
    match expression.kind() {
        ExprKind::Path(path) if path.is_qualified() => {
            let mut segments = path.segments().collect::<Vec<_>>();
            let member = segments.pop()?;
            let kind = if segments.len() == 1 {
                ExprKind::Name(segments[0].to_owned())
            } else {
                ExprKind::Path(NamePath::from_segments(segments, expression.range()).ok()?)
            };
            Some((
                SourceAstFactory::expression(kind, expression.range()).ok()?,
                member,
            ))
        }
        ExprKind::Member { value, member } => Some((value.as_ref().clone(), member.as_str())),
        _ => None,
    }
}
