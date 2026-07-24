# Changelog

Eqiora follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and
Semantic Versioning. During alpha, compatibility changes remain possible and
are recorded here.

## [Unreleased]

### Changed

- Assembly, meshing, geometry, and solver contracts now use their owning crate
  and `eqiora` namespace paths; aliases formerly exposed from numerics,
  artifact, and realization have been removed.

- Package identity and exact in-memory resolution remain available by default;
  directory package authoring, replay, and installation now require the
  `package-filesystem` facade feature.

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
