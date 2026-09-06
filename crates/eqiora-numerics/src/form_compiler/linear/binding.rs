use super::CompiledLinearBlockForm;
use crate::scalar_conservation::ScalarExteriorLaw;
use eqiora_core::entity::kinds;
use eqiora_core::{Diagnostic, Id};

impl CompiledLinearBlockForm {
    pub(crate) fn bind_parameter_point(
        &self,
        fields: &[Id<kinds::Parameter>],
        values: &[f64],
    ) -> Result<Self, Diagnostic> {
        let mut bound = self.clone();
        for row in &mut bound.rows {
            row.constant = row.constant.bind_parameter_point(fields, values)?;
            for data in row.diffusion.values_mut().chain(row.reaction.values_mut()) {
                *data = data.bind_parameter_point(fields, values)?;
            }
        }
        for law in bound
            .boundary_laws
            .values_mut()
            .flat_map(|laws| laws.values_mut())
        {
            match law {
                ScalarExteriorLaw::PrescribedTrace { value, .. }
                | ScalarExteriorLaw::PrescribedOutwardFlux { value, .. } => {
                    *value = value.bind_parameter_point(fields, values)?;
                }
                ScalarExteriorLaw::Robin {
                    trace_coefficient,
                    value,
                    ..
                } => {
                    *trace_coefficient = trace_coefficient.bind_parameter_point(fields, values)?;
                    *value = value.bind_parameter_point(fields, values)?;
                }
                ScalarExteriorLaw::ZeroOutwardFlux { .. } => {}
            }
        }
        Ok(bound)
    }
}
