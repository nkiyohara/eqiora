use std::collections::BTreeMap;

use crate::form_compiler::DerivedScalarGalerkinForm;
use crate::scalar_conservation::{ScalarConservationDescriptor, ScalarExteriorLaw};
use crate::spatial_expression::ScalarSpatialExpression;

use super::{
    ScalarEllipticCartesianBoundary, ScalarEllipticCartesianModel, collect_parameter_coordinates,
};

pub(crate) fn project_scalar_conservation_for_differentiation(
    descriptor: &ScalarConservationDescriptor,
    compiled_form: Option<&DerivedScalarGalerkinForm>,
) -> ScalarEllipticCartesianModel {
    let region = descriptor
        .regions()
        .next()
        .expect("steady scalar admission owns exactly one region");
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
                unreachable!("steady scalar admission rejects Robin boundaries")
            }
        };
        boundaries.insert((boundary.axis(), boundary.side()), condition);
    }

    let (parameter_fields, parameter_values) =
        collect_parameter_coordinates(&coefficient, &source, &boundaries);
    ScalarEllipticCartesianModel {
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
        compiled_form: compiled_form.cloned(),
    }
}
