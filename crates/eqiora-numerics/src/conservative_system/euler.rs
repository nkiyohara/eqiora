//! Adapter only: all ideal-gas formulas remain in the semantic closure owner.
use super::contract::{ConservativePhysics, SemanticContract, invalid};
use crate::canonical_euler::{
    EulerConservativeState1d, IdealGasEulerModel1d, recognize_ideal_gas_euler_1d,
};
use eqiora_core::{Diagnostic, DynQuantity};
use eqiora_sem::KernelProgram;

#[derive(Debug, Clone)]
pub(crate) struct EulerPhysics {
    contract: SemanticContract,
    closure: IdealGasEulerModel1d,
}

impl EulerPhysics {
    pub(crate) fn from_program(program: &KernelProgram) -> Result<Self, Diagnostic> {
        let closure = recognize_ideal_gas_euler_1d(program)?;
        let contract = SemanticContract::from_program(
            program,
            closure.domain(),
            &closure.conservative_fields(),
            &closure.boundaries(),
        )?;
        Ok(Self { contract, closure })
    }
    fn state(values: &[f64]) -> Result<EulerConservativeState1d, Diagnostic> {
        let [density, momentum, energy] = values else {
            return Err(invalid(
                "Euler adapter requires exactly three conservative components",
            ));
        };
        Ok(EulerConservativeState1d::new(*density, *momentum, *energy))
    }
}

impl ConservativePhysics<1> for EulerPhysics {
    fn contract(&self) -> &SemanticContract {
        &self.contract
    }
    fn admit(&self, state: &[f64]) -> Result<(), Diagnostic> {
        self.closure
            .conservative_to_primitive(Self::state(state)?)
            .map(|_| ())
    }
    fn normal_flux_and_wave_bound(
        &self,
        state: &[f64],
        normal: &[f64; 1],
    ) -> Result<(Vec<DynQuantity>, DynQuantity), Diagnostic> {
        let state = Self::state(state)?;
        let flux = self
            .closure
            .physical_flux(state)?
            .into_iter()
            .zip(&self.contract.components)
            .map(|(value, component)| {
                component
                    .value_type
                    .dimension()
                    .mul(super::contract::velocity_dimension())
                    .map(|dimension| DynQuantity::new(normal[0] * value, dimension))
                    .ok_or_else(|| invalid("Euler flux dimension overflow"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bound = normal[0].abs() * self.closure.characteristic_speed_bound(state)?;
        Ok((
            flux,
            DynQuantity::new(bound, super::contract::velocity_dimension()),
        ))
    }
}
