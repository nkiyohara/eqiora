use eqiora::Diagnostic;
use eqiora::diagnostic::codes;
use pyo3::prelude::*;

use crate::error::{diagnostic_error, validation_error};
use crate::result::PyFieldOutput;

use super::scene::{LayerMetadata, PresentationScale, ScalarFieldLayer, SceneBuilder};

pub(super) fn add_scalar_field(
    py: Python<'_>,
    builder: &mut SceneBuilder,
    output: &PyFieldOutput,
) -> PyResult<()> {
    if !output.value_shape_value().is_empty() {
        return Err(unsupported(
            py,
            "private v0 viewer accepts scalar FieldOutput values only",
        ));
    }
    let mesh = output.mesh_handle(py);
    let mesh_digest = mesh.borrow(py).exact_mesh_digest().to_owned();
    let target = builder.mesh_target(&mesh_digest).cloned().ok_or_else(|| {
        validation_error(
            py,
            &[Diagnostic::error(
                codes::INVALID_ARTIFACT,
                "ScalarFieldLayer requires its exact MeshLayer in the same scene",
            )],
        )
    })?;
    let field = output.field_handle(py);
    let field = field.borrow(py);
    let model_digest = field.exact_model_digest().to_owned();
    let field_id = field.exact_id().to_owned();
    let dimension = output.dimension_value();
    let dimension = dimension.exponents();
    for block in output.blocks() {
        let association = block.association();
        let expected = match association {
            "vertex" => target.vertex_count,
            "cell" => target.cell_count,
            _ => {
                return Err(unsupported(
                    py,
                    &format!(
                        "private v0 viewer supports only vertex or cell scalar association, received {association:?}"
                    ),
                ));
            }
        };
        let values = block.snapshot(py)?;
        if block.coefficient_count() != expected
            || values.len() != expected
            || block.logical_shape().iter().product::<usize>() != expected
            || values.iter().any(|value| !value.is_finite())
        {
            return Err(validation_error(
                py,
                &[Diagnostic::error(
                    codes::INVALID_ARTIFACT,
                    "ScalarFieldLayer values disagree with exact association support or contain a non-finite value",
                )],
            ));
        }
        let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
        let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        let layer_id =
            format!("scalar-field:{mesh_digest}:{model_digest}:{field_id}:{association}");
        let values = builder
            .push_f64(format!("{layer_id}:values"), vec![expected], values)
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
        builder
            .push_layer(LayerMetadata::ScalarField(ScalarFieldLayer {
                id: layer_id,
                target_layer: target.layer_id.clone(),
                mesh_digest: mesh_digest.clone(),
                model_digest: model_digest.clone(),
                field_id: field_id.clone(),
                association: association.to_owned(),
                component_shape: Vec::new(),
                unit: "coherent-si".to_owned(),
                dimension,
                frame: "scalar".to_owned(),
                space: output.space_value().to_owned(),
                values,
                scale: PresentationScale {
                    provenance: "presentation-linear-range-from-accepted-values/v0".to_owned(),
                    minimum,
                    maximum,
                },
            }))
            .map_err(|diagnostic| validation_error(py, &[diagnostic]))?;
    }
    Ok(())
}

fn unsupported(py: Python<'_>, message: &str) -> PyErr {
    diagnostic_error(py, &[Diagnostic::error(codes::NOT_IMPLEMENTED, message)])
}
