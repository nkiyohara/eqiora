# Changelog

Eqiora follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
Semantic Versioning. During alpha, compatibility changes remain possible and
are recorded here.

## [Unreleased]

### Changed

- `eqiora.compile` is now keyword-only and accepts exactly one of `path=` or
  `source=`. Definitions-only component source can be closed directly with a
  Python-authored `Geometry` and coherent-SI Parameter values; source owns
  dimensions and abstract support names, while Python is the sole source of
  concrete shape. The exact-cylinder example now follows this route through
  the common root `resolve` / `run` lifecycle. No positional compatibility
  overload or application-specific binding path is retained.

## [0.1.0a3] - 2026-08-23

### Changed

- Automatic Python meshing for the bounded exact-cylinder workflow now invokes
  Gmsh 4.15.2 and imports its linear 2D MSH 4.1 output. Missing, mismatched, or
  failed Gmsh executions reject explicitly; the former reference-mesh fallback
  is not retained.

- The exact-cylinder Result and Gallery pressure image now use the accepted
  662-vertex, 1,210-triangle Gmsh realization. This does not
  claim arbitrary geometry, curved elements, adaptive sizing, or cross-platform
  generated-mesh byte identity.

## [0.1.0a2] - 2026-08-21

### Added

- Added `eqiora.compile_package(...)` for one exact installed-Python locked
  Model Package path. Callers provide an explicit content-addressed store,
  canonical resolution bytes, and a bare root-local Model selector; discovery,
  authoring, installation, registry/network access, execution, and Studio
  workflows remain outside the claim.

- Added `eqiora.check_package_conformance(...)` for one explicit locked package
  closure and the exact `eqiora.package.structural-conformance-v1` profile. Its
  immutable in-process report proves structural replay agreement only; it is
  not scientific verification, execution support, trust, certification, or a
  Studio workflow.

- Added one exact serial-host prescribed dynamic-solid publication path with a
  content-addressed standalone Realization, retained prior and accepted-next
  two-Field States, and a Run whose sole output is the accepted-next State.
  This remains the bounded accepted unit-cube reference, not a general
  standalone structural time-integration API.

- Added one failure-atomic external-boundary-provider path for the exact
  prescribed dynamic-solid occurrence. An application-created, already-connected
  subprocess supplies the admitted boundary-displacement candidate; Eqiora
  records a complete provider-occurrence artifact and a separate two-output Run
  linking it to the unchanged accepted-next State. The first verified provider
  is limited to ordinary-GIL CPython 3.12 with NumPy 2.1.0; the direct
  singleton-output Run, process launch authority, and broader coupling remain
  unchanged.

- Added an optional `eqiora.matplotlib` adapter that plots the accepted
  exact-cylinder P1 pressure Result as a caller-owned, headless-saveable
  Matplotlib Figure without making Matplotlib a base dependency.

- Added `eqiora.solid.solve_mixed_boundary_elasticity(...)` and a packaged
  Python example for the accepted exact-v4 structural case. Studio and Python
  now consume one Rust-owned Model-to-Run result, and the optional Matplotlib
  adapter can render its original and explicitly scaled displacement meshes.

- Added an explicit `eqiora.fsi.FixedMeshMonolithic` intent and inspectable
  Model-bound Plan for the accepted two-step FSI case. Studio and Python share
  one Rust-owned resolver; the common Run returns a common Result whose exact
  Trajectory and typed state evidence feed the general scalar and deformed
  Field adapters.

### Changed

- Removed the unreleased Python `FixedReferenceFsiStep`,
  `FixedReferenceFsiResult`, and `solve_fixed_reference_fsi(...)` names. Exact
  spatial Fields now live only on `State`/`FieldSnapshot`, while
  partition, interface, energy, residual, solve, and assembly observations are
  selected through `fixed_mesh_monolithic_evidence(result)`.

- Replaced the demo-specific `plot_fixed_reference_fsi(...)` entry point with
  `plot_scalar_field(...)` and `plot_deformed_field(...)` over common
  `Trajectory` and exact Model-bound `FieldRef` values. No compatibility alias
  is retained.

- The exact-cylinder fluid path now returns the common `Result`,
  `FieldSnapshot`, and `Mesh` owners. Scientific observations move behind
  `fluid.steady_stokes_evidence(result)`, while the demo-specific
  `CircularHoleSteadyStokesResult` and `plot_pressure(...)` surfaces are
  removed in favor of `plot_scalar_field(result, field=...)`.

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

[Unreleased]: https://github.com/nkiyohara/eqiora/compare/v0.1.0a3...HEAD
[0.1.0a3]: https://github.com/nkiyohara/eqiora/releases/tag/v0.1.0a3
[0.1.0a2]: https://github.com/nkiyohara/eqiora/releases/tag/v0.1.0a2
[0.1.0a1]: https://github.com/nkiyohara/eqiora/releases/tag/v0.1.0a1
