//! Private proof-carrying FEM derivations.

mod elasticity;
mod scalar;
#[cfg(test)]
mod tests;
pub(crate) mod vocabulary;

pub(crate) use elasticity::compile_cartesian_q1_elasticity_form_2d;
pub(crate) use scalar::{
    AcceptedAuthoredScalarPrimalForm, AdmittedScalarGalerkinForm, DerivedScalarGalerkinForm,
    compile_cartesian_q1_form, derive_candidate,
};
#[cfg(test)]
use vocabulary::{
    DIVERGENCE_BY_PARTS, HOMOGENEOUS_ESSENTIAL_DISCHARGE, MatrixSlot, SOURCE_PAIRING, TEST_PAIRING,
    WeakSign, WeakTermSlot,
};
