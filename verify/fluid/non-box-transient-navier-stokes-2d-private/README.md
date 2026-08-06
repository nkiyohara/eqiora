# Private non-box transient Navier--Stokes oracle

This pre-implementation case freezes the crate-private composition required by
the future cylinder-wake gallery slice. It requires the already accepted
transient MINI/P1 grammar to bind one exact circular-hole source, its
deterministic chordal realization, its
content-addressed simplicial mesh, and its authored correspondence before a
single exact-zero backward-Euler step can execute.

The case is intentionally RED on protected base `3dfb1086`: the production
private module and therefore the exact library-test selector do not exist yet.
After integration, run:

```bash
mise run affected -- --case fluid.non-box-transient-navier-stokes-2d-private
```

The oracle checks source and artifact identity, correspondence-only named-set
partitioning, exact boundary dispositions, the complete portable realization
graph, checked assembly, and an exact-zero one-step fixed point. It also
falsifies a second valid source, a second valid same-source mesh owner,
relabeled/omitted/incomplete/overlapping authored sets, boundary-condition
drift, and additional unsupported geometry relations.

The claim boundary is exact:

- no DFG2D-1, DFG2D-2, S1, S2, or benchmark acceptance;
- no scientific reference result, validation, target value, tolerance,
  convergence rate, or performance claim;
- no drag, lift, force, surface traction, constrained reaction, pressure,
  pressure difference, Strouhal number, flux observable, or time-series
  extraction;
- no O3 separation definition, wall shear, boundary gradient, root
  classification, or separation result;
- no new cylinder geometry, circle representation, curved element, mesh
  generation, mesh refinement, second mesh family, or geometry-authoring
  capability; the accepted circular-hole value is borrowed only as the current
  source-bound non-box witness;
- no stationary Navier--Stokes, continuation, periodic state, shedding, long
  trajectory, phase, peak, or frequency claim;
- no DFG/classical do-nothing outlet meaning; only the accepted Eqiora
  zero-constant-traction disposition is reused;
- no new element, quadrature, time method, nonlinear method, solver,
  preconditioner, backend, planner integration, device, MPI, scale, or
  performance claim;
- no durable trajectory, Snapshot, State, Run, Result, schema, wire, wire
  identity, artifact version, registry, cache, or publication receipt;
- no image, plot, animation, video, gallery, publication, or heavy-candidate
  production;
- no Python, installed-Python, Studio, browser, application workflow, or public
  end-user API;
- no general `GeometryRegion` lowering beyond this accepted circular-hole
  source kind and exact four-name boundary inventory; and
- no compatibility promise outside crate-private Rust implementation.

The oracle additionally claims no initial-state equality, direct replay-call
count, ledger, marker, duplicate equality, environment ordinal, observer,
output, persistence, or artifact emission. The larger cylinder-wake gallery
capability remains outside this prerequisite and is not promoted by it.
