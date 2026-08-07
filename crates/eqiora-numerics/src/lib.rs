//! **eqiora-numerics** — numerical realizations and verified solver kernels.
//!
//! This crate owns approximation choices, not model meaning. Its public
//! surface is organized by scientific responsibility. Implementation modules
//! and lowering bridges remain private; each public item has one owner path.

mod affine_fem;
mod assembled_linearization;
mod canonical;
mod canonical_boundary;
mod canonical_elasticity;
mod canonical_fsi;
mod canonical_stokes;
mod canonical_transport;
mod cartesian_elasticity;
mod cartesian_elliptic;
mod cartesian_fvm_geometry;
mod cartesian_incompressible;
mod cartesian_periodic_3d;
mod cartesian_transport;
mod diffusion;
mod discrete_block;
mod discrete_space;
mod elliptic;
mod finalized_spatial;
mod form_compiler;
mod jacobian_audit;
mod linearized_output;
mod operator;
mod physical_network;
mod poisson;
mod prescribed_dynamic_solid;
mod simplicial_ale_fsi;
mod simplicial_ale_remesh;
mod simplicial_elasticity;
mod simplicial_elliptic;
mod simplicial_fsi;
mod simplicial_mini_transient;
mod simplicial_motion;
mod simplicial_navier_stokes;
mod simplicial_solid_element;
mod simplicial_stokes;
mod spatial_design;
mod spatial_expression;
mod step_count;

/// Moving-domain ALE and remeshing realizations.
pub mod ale;
/// Numerical contracts shared by more than one scientific family.
pub mod common;
/// Incompressible-flow numerical realizations.
pub mod fluid;
/// Fixed-reference fluid-structure interaction realizations.
pub mod fsi;
/// Scalar elliptic, transport, diffusion, and affine-network realizations.
pub mod scalar;
/// Solid-mechanics numerical realizations.
pub mod solid;
