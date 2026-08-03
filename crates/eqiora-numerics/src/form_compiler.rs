//! Private proof-carrying FEM derivations.

mod scalar;

pub(crate) use scalar::{
    AdmittedScalarGalerkinForm, DerivedScalarGalerkinForm, compile_cartesian_q1_form,
    derive_candidate,
};
