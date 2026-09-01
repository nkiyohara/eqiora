use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_schema::kernel::BoundarySide;
use eqiora_sem::KernelProgram;

use crate::scalar_conservation::{ScalarConservationDescriptor, ScalarExteriorLaw};
use crate::spatial_expression::ScalarSpatialExpression;

use super::{
    ScalarEllipticCartesianBoundary, ScalarEllipticCartesianModel, collect_parameter_coordinates,
    derive_candidate_with_dimension, lowering_error, model_lowering_error,
};

pub(crate) fn lower_steady_scalar_conservation(
    program: &KernelProgram,
    descriptor: &ScalarConservationDescriptor,
) -> Result<ScalarEllipticCartesianModel, Diagnostic> {
    if descriptor.model() != program.model()
        || descriptor.semantic_revision() != program.revision().0
    {
        return Err(model_lowering_error(
            program,
            "scalar-conservation descriptor differs from the exact Kernel Program",
        ));
    }
    let regions = descriptor.regions().collect::<Vec<_>>();
    let [region] = regions.as_slice() else {
        return Err(model_lowering_error(
            program,
            "steady Cartesian execution requires exactly one scalar-conservation region",
        ));
    };
    if region.storage().is_some() || descriptor.interfaces().len() != 0 {
        return Err(lowering_error(
            region.domain(),
            "steady Cartesian execution does not admit storage or material interfaces",
        ));
    }

    let dimension = region.dimensions();
    let coefficient = region.flux().coefficient().clone();
    let source = region.source().map_or_else(
        || ScalarSpatialExpression::constant(dimension, 0.0),
        |source| source.expression().clone(),
    );
    let mut boundaries = BTreeMap::new();
    for boundary in region.exterior() {
        let condition = match boundary.law() {
            ScalarExteriorLaw::PrescribedTrace { value, .. } => {
                ScalarEllipticCartesianBoundary::Essential(value.clone())
            }
            ScalarExteriorLaw::PrescribedOutwardFlux { value, .. } => {
                ScalarEllipticCartesianBoundary::Natural(value.clone())
            }
            ScalarExteriorLaw::ZeroOutwardFlux { .. } => ScalarEllipticCartesianBoundary::Natural(
                ScalarSpatialExpression::constant(dimension, 0.0),
            ),
            ScalarExteriorLaw::Robin { .. } => {
                return Err(lowering_error(
                    boundary.boundary(),
                    "steady Cartesian Q1/TPFA execution does not yet admit Robin boundaries",
                ));
            }
        };
        boundaries.insert((boundary.axis(), boundary.side()), condition);
    }
    for axis in 0..dimension {
        for side in [BoundarySide::Lower, BoundarySide::Upper] {
            if !boundaries.contains_key(&(axis, side)) {
                return Err(lowering_error(
                    region.domain(),
                    format!("steady Cartesian execution is missing boundary axis {axis} {side:?}"),
                ));
            }
        }
    }

    let (parameter_fields, parameter_values) =
        collect_parameter_coordinates(&coefficient, &source, &boundaries);
    Ok(ScalarEllipticCartesianModel {
        semantic_model: descriptor.model(),
        semantic_revision: descriptor.semantic_revision(),
        domain: region.domain(),
        field: region.field(),
        bounds: region.bounds().to_vec(),
        coefficient,
        source,
        boundaries,
        parameter_fields,
        parameter_values,
        compiled_form: derive_candidate_with_dimension(program, region.domain(), dimension)?,
    })
}
