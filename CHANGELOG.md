# Changelog

Eqiora follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
Semantic Versioning. During alpha, compatibility changes remain possible and
are recorded here.

## [Unreleased]

### Added

- Added an optional `eqiora.matplotlib` adapter that plots the accepted
  exact-cylinder P1 pressure Result as a caller-owned, headless-saveable
  Matplotlib Figure without making Matplotlib a base dependency.

- Added `eqiora.solid.solve_mixed_boundary_elasticity(...)` and a packaged
  Python example for the accepted exact-v4 structural case. Studio and Python
  now consume one Rust-owned Model-to-Run result, and the optional Matplotlib
  adapter can render its original and explicitly scaled displacement meshes.

- Added `eqiora.fsi.solve_fixed_reference_fsi(...)` and a packaged two-step
  Python example for the accepted exact-v4 monolithic FSI case. Studio and
  Python now consume one Rust-owned Model-to-trajectory-to-Run result; the
  optional Matplotlib adapters present exact scalar and deformed Fields from
  either accepted trajectory state with support-restricted topology.

### Changed

- Replaced the demo-specific `plot_fixed_reference_fsi(...)` entry point with
  `plot_scalar_field(...)` and `plot_deformed_field(...)` over common
  `Trajectory` and exact Model-bound `FieldRef` values. No compatibility alias
  is retained.

- Reset the pre-release Model artifact epoch to one current contract. Rust's
  generation-selecting codec API and `eqiora::compatibility`, Python
  `eqiora.compatibility`, Model/Transaction v1--v7 runtime support, and
  control-v1 dispatch were removed. Their ordinary replacements are Rust
  `ModelDocument::{compile, define, replay}`, unversioned current Model and
  Transaction artifact owners, Python `eqiora.compile`, `Model.define`, and
  `eqiora.replay`, plus `eqiora.control/v2`. Old Model/Transaction bytes and
  control-v1 requests reject; no automatic migration is provided.

- The pre-release `eqiora::numerics` facade no longer re-exports
  `SteadyStokesGeometryBinding2d` or
  `solve_resolved_steady_stokes_geometry_mini_2d`. Application consumers use
  `eqiora::api::CircularHoleSteadyStokesResult2d`; lower-level numerical
  composition remains available from its owning `eqiora-numerics` crate.

- The pre-release `eqiora::numerics` facade no longer re-exports the ten
  low-level fixed-reference FSI construction and solution names. Application
  consumers use `eqiora::api::FixedReferenceFsiResult2d`; lower-level
  numerical composition remains available from `eqiora-numerics`.

- Assembly, meshing, geometry, and solver contracts now use their owning crate
  and `eqiora` namespace paths; aliases formerly exposed from numerics,
  artifact, and realization have been removed.

- Package identity and exact in-memory resolution remain available by default;
  directory package authoring, replay, and installation now require the
  `package-filesystem` facade feature.

- Split harmonic ALE naming into the Realization-level
  `P1HarmonicMeshMotionPolicy` and the executable numerical
  `P1HarmonicMeshMotionAction`; the ambiguous pre-release Rust names have no
  compatibility aliases. Artifact wire names and canonical bytes are
  unchanged.

- Renamed the Rust `eqiora-fabric` crate to `eqiora-backend-rayon`, its
  `threaded` facade feature to `rayon`, and the `eqiora-diff` crate to
  `eqiora-differentiation`. The pre-publication Rust names have no
  compatibility aliases.

## [0.1.0a1] - 2026-07-23

The first public alpha establishes one coherent, evidence-gated project
boundary:

- a small typed semantic kernel, Eqiora Language frontend, canonical
  transactions, reference hybrid execution, and scalar Operator IR;
- bounded scalar elliptic FEM/FVM, solver, time, differentiation, artifact,
  package, geometry, I/O, CPU, CUDA, and MPI vertical slices;
- an immutable Python modeling API with synchronous and asynchronous native
  execution, structured diagnostics, explicit NumPy/DLPack ownership, and
  bounded PyTorch/JAX differentiation adapters;
- a thin Studio projection over the same canonical model and typed application
  service;
- versioned artifacts, falsifying verification cases, a public capability
  matrix, and exact release-candidate manifests.

This release supports ordinary-GIL CPython 3.11–3.14 on
manylinux x86-64. It does not claim macOS or Windows wheels, free-threaded
Python, GPU wheels, bundled MPI, a complete physics/component catalogue,
stable-1.0 compatibility, or safety certification.

Detailed claims and nonclaims are the responsibility of the
[capability matrix](docs/capability-matrix.md) and registered
[`verify/`](verify/) cases rather than this summary.

[Unreleased]: https://github.com/nkiyohara/eqiora/compare/v0.1.0a1...HEAD
[0.1.0a1]: https://github.com/nkiyohara/eqiora/releases/tag/v0.1.0a1
