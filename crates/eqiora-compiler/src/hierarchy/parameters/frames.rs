//! Frame context for uniform Parameter types, without attaching spatial support.
use super::*;
use eqiora_lang::{ValueTypeSyntax, ValueTypeSyntaxKind};
use eqiora_schema::kernel::typing::SpatialSupport;

pub(super) fn parameter_type(
    file: &str,
    syntax: &ValueTypeSyntax,
    initializer: Option<&Expr>,
    frames: &BTreeMap<String, SpatialSupport<String>>,
) -> Result<ValueType, Diagnostic> {
    fn spatial(syntax: &ValueTypeSyntax) -> bool {
        match syntax.kind() {
            ValueTypeSyntaxKind::Vector { .. } | ValueTypeSyntaxKind::Tensor { .. } => true,
            ValueTypeSyntaxKind::Array { element, .. } => spatial(element),
            _ => false,
        }
    }
    if !spatial(syntax) {
        return crate::value_types::lower_value_type::<String>(file, syntax, None);
    }
    let invalid = |message: &str| source_error(codes::LANGUAGE_TYPE_ERROR, file, syntax.range(), message);
    let frame = if let Some(ExprKind::Call { callee, arguments }) = initializer.map(Expr::kind)
        && callee.as_str() == "tensor_value"
    {
        let name = arguments.first().and_then(|value| match value.kind() {
            ExprKind::Name(name) => Some(name.as_str()),
            ExprKind::Path(name) => Some(name.as_str()),
            _ => None,
        }).ok_or_else(|| invalid("tensor_value requires an exact named frame support"))?;
        frames.get(name).ok_or_else(|| invalid("tensor_value frame is not an existing Cartesian support in this scope"))?
    } else {
        let mut unique = Vec::new();
        for support in frames.values() {
            if !unique.contains(&support) { unique.push(support); }
        }
        match unique.as_slice() {
            [frame] => *frame,
            [] => return Err(invalid("spatial Parameter requires an exact frame support context")),
            _ => return Err(invalid("spatial Parameter frame context is ambiguous; use tensor_value with an explicit frame")),
        }
    };
    crate::value_types::lower_value_type(file, syntax, Some(frame))
}
