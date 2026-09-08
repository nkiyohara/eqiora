//! Resolve the existing authored shape and frame syntax.
use super::*;

pub(in crate::hierarchy) fn resolve_value_shape(
    file: &str,
    range: TextRange,
    syntax: &ValueShapeSyntax,
    ambient_dimension: usize,
) -> Result<ValueShape, Vec<Diagnostic>> {
    let extents = match syntax {
        ValueShapeSyntax::Scalar => return Ok(ValueShape::scalar()),
        ValueShapeSyntax::Exact(extents) => extents.clone(),
        ValueShapeSyntax::SpatialVector => {
            vec![u32::try_from(ambient_dimension).map_err(|_| {
                vec![source_error(
                    codes::LANGUAGE_TYPE_ERROR,
                    file,
                    range,
                    "support ambient dimension exceeds portable u32 shape range",
                )]
            })?]
        }
        _ => {
            return Err(vec![source_error(
                codes::LANGUAGE_LOWERING_ERROR,
                file,
                range,
                "value shape is newer than this compiler",
            )]);
        }
    };
    ValueShape::new(extents).map_err(|error| {
        vec![source_error(
            codes::LANGUAGE_TYPE_ERROR,
            file,
            range,
            error.to_string(),
        )]
    })
}

pub(super) fn resolve_frame(
    file: &str,
    range: TextRange,
    syntax: FrameSyntax,
) -> Result<ValueFrame, Vec<Diagnostic>> {
    match syntax {
        FrameSyntax::Invariant => Ok(ValueFrame::Invariant),
        FrameSyntax::Spatial => Ok(ValueFrame::SpatialCartesian),
        _ => Err(vec![source_error(
            codes::LANGUAGE_LOWERING_ERROR,
            file,
            range,
            "frame syntax is newer than this compiler",
        )]),
    }
}
