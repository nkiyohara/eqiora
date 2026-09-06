//! Bind Model-derived region equations to the exact resolved numerical choices.

use std::collections::BTreeMap;

use eqiora_core::{Diagnostic, DynQuantity, RawId};
use eqiora_meshing::ReferenceCell;
use eqiora_realization::{AlgebraicBlock, CoupledFieldwiseRealizationPlan};

use crate::canonical_fsi::FixedReferenceFsiCartesianModel2d;
use crate::form_compiler::region::{BoundRegionForm, RegionFieldBinding, RegionTimeBinding};

use super::validate::{fluid_pressure, fluid_velocity, invalid_realization, solid_velocity};

mod cells;
pub(super) use cells::prepare_cells;

pub(super) fn bind(
    model: &FixedReferenceFsiCartesianModel2d,
    plan: &CoupledFieldwiseRealizationPlan,
) -> Result<BTreeMap<RawId, BoundRegionForm>, Diagnostic> {
    let reference = ReferenceCell::simplex(2)?;
    let functional = plan.scaling().weak_functional_scale().quantity();
    let state = plan.time_step().eliminated_state();
    model
        .region_forms
        .iter()
        .map(|(&domain, form)| {
            let spatial = plan
                .spatial()
                .domains()
                .iter()
                .find(|entry| entry.domain().erase() == domain)
                .ok_or_else(|| invalid_realization("region has no exact Plan Domain binding"))?;
            let fields = form
                .fields()
                .map(|(field, _)| {
                    let binding = spatial
                        .field_spaces()
                        .iter()
                        .find(|binding| binding.field().erase() == field)
                        .ok_or_else(|| invalid_realization("region Field has no Plan space"))?;
                    let scale = plan
                        .scaling()
                        .block_scales()
                        .iter()
                        .find(|scale| scale.block() == AlgebraicBlock::Field(binding.field()))
                        .ok_or_else(|| invalid_realization("region Field has no Plan scale"))?;
                    Ok(RegionFieldBinding {
                        field,
                        space: binding.space(),
                        scale: scale.scale().quantity(),
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            let rows = form
                .rows()
                .map(|(relation, tested, _)| {
                    let scale = fields
                        .iter()
                        .find(|binding| binding.field == tested)
                        .expect("every residual owns its exact test Field")
                        .scale
                        .try_div(functional)?;
                    // The admitted symmetric mixed formulation tests div(v)=0
                    // with -p, while momentum uses the positive velocity test.
                    // Preserve the recognizer's orientation of the entire
                    // momentum residual, including its forcing and history.
                    let sign = if tested == fluid_pressure(model).erase() {
                        -1.0
                    } else if tested == fluid_velocity(model).erase() {
                        model.fluid.momentum_orientation()
                    } else if tested == solid_velocity(model).erase() {
                        model.solid.momentum_orientation()
                    } else {
                        return Err(invalid_realization(
                            "region row has no admitted test orientation",
                        ));
                    };
                    Ok((
                        relation,
                        DynQuantity::new(sign * scale.value(), scale.dim()),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>, Diagnostic>>()?;
            let time = RegionTimeBinding {
                step: plan.time_step().duration(),
                states: fields
                    .iter()
                    .any(|field| field.field == state.pair().rate().erase())
                    .then_some(state)
                    .into_iter()
                    .collect(),
            };
            form.bind(reference, &fields, &rows, Some(&time))
                .map(|bound| (domain, bound))
        })
        .collect()
}
