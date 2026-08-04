//! Private proof-carrying FEM derivations.

mod elasticity;
mod scalar;

pub(crate) use elasticity::{
    compile_cartesian_q1_elasticity_form_2d, derive_cartesian_q1_elasticity_form_2d,
};
use scalar::{MatrixSlot, WeakSign, WeakTermSlot};
pub(crate) use scalar::{
    AdmittedScalarGalerkinForm, DerivedScalarGalerkinForm, compile_cartesian_q1_form,
    derive_candidate,
};
