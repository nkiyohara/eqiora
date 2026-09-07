//! Metadata-only expansion of an explicit global shared/mapped partition.

use eqiora_core::{Id, entity::kinds};

use super::*;

impl EvaluationMapPlan {
    /// Admit dense mapped inputs and explicitly shared Parameter coordinates.
    ///
    /// `shared_values` follows `shared_inputs`; all remaining inputs follow
    /// Program order in `mapped_values`, shaped `point_shape + [mapped_count]`.
    /// Point axes are row-major, rank at most 32. Empty shape means one point;
    /// zero extents mean no points. Sharing applies to the entire point grid,
    /// not selected individual axes. Shared values are validated even when empty.
    ///
    /// # Errors
    /// Rejects foreign/duplicate coordinates, nonfinite values, wrong shapes,
    /// unaddressable products and excess retained bytes before expanding points.
    pub fn from_partition(
        program: Arc<DifferentiableProgram>,
        shared_inputs: &[Id<kinds::Parameter>],
        shared_values: &[f64],
        mapped_values: &[f64],
        point_shape: &[usize],
        retained_bytes_limit: usize,
    ) -> Result<Self, Diagnostic> {
        if point_shape.len() > 32 {
            return Err(invalid("mapped input point rank exceeds 32"));
        }
        let count = axes::product(point_shape)
            .map_err(|error| invalid(format!("invalid mapped input point shape: {error:?}")))?;
        let inputs = program.identity().inputs();
        let mut shared = Vec::with_capacity(shared_inputs.len().min(inputs.len()));
        for (position, id) in shared_inputs.iter().enumerate() {
            if shared_inputs[..position].contains(id) {
                return Err(invalid(format!(
                    "shared input {position} repeats a Parameter"
                )));
            }
            shared.push(
                inputs
                    .iter()
                    .position(|candidate| candidate == id)
                    .ok_or_else(|| {
                        invalid(format!(
                            "shared input {position} is foreign to the exact Program"
                        ))
                    })?,
            );
        }
        let mapped = (0..inputs.len())
            .filter(|input| !shared.contains(input))
            .collect::<Vec<_>>();
        let mut shape = point_shape.to_vec();
        shape.push(mapped.len());
        let expected = axes::product(&shape).map_err(|error| {
            invalid(format!("invalid mapped input coordinate shape: {error:?}"))
        })?;
        if shared_values.len() != shared.len() || mapped_values.len() != expected {
            return Err(invalid(format!(
                "partition inputs require {} shared values and {expected} mapped values",
                shared.len()
            )));
        }
        for (index, value) in shared_values.iter().enumerate() {
            if !value.is_finite() {
                return Err(invalid(format!("shared input {index} must be finite")));
            }
        }
        for (index, value) in mapped_values.iter().enumerate() {
            if !value.is_finite() {
                return Err(invalid(format!(
                    "mapped input at occurrence {}, coordinate {} must be finite",
                    index / mapped.len(),
                    index % mapped.len()
                )));
            }
        }
        let estimated_retained_bytes = retained_bytes(program.map_occurrence_bytes()?, count)?;
        if estimated_retained_bytes > retained_bytes_limit
            || estimated_retained_bytes > isize::MAX as usize
        {
            return Err(invalid(
                "partitioned map exceeds the retained numerical byte limit",
            ));
        }
        let mut points = Vec::with_capacity(count);
        for occurrence in 0..count {
            let mut values = vec![0.0; inputs.len()];
            for (&input, &value) in shared.iter().zip(shared_values) {
                values[input] = value;
            }
            for (coordinate, &input) in mapped.iter().enumerate() {
                values[input] = mapped_values[occurrence * mapped.len() + coordinate];
            }
            program.validate_map_point(&values)?;
            points.push(program.map_point(&values));
        }
        Ok(Self {
            program,
            points: points.into(),
            estimated_retained_bytes,
        })
    }
}
