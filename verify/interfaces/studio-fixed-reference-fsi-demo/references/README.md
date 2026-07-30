# Reference strategy

This interface case introduces no new scientific oracle and no new tolerance.
The existing `fixed_reference_monolithic_fsi_step_2d` Rust target owns the weak
form, the exact trace quotient, backward-Euler displacement elimination, the
complete-operator pressure closure, the recovered interface actions, the
discrete energy identity, and their falsifiers. The existing
`fixed_reference_fsi_spatial_trajectory` target owns the two accepted spatial
states, the complete Field inventory with its MINI vertex and cell-bubble
blocks, and the immutable segmented trajectory and Run-output lineage.

Every acceptance threshold below is reused from those cases by reference and
is neither re-derived, re-tuned, nor relaxed here:

- true residual no greater than the solver-owned target;
- numerical residual and continuity residual below `1e-9`;
- kinematic residual below `1e-14`;
- shared-trace velocity jump exactly zero;
- interface action imbalance below `1e-9 N/m`; and
- absolute energy defect below `1e-9 J/m`.

The interface oracle is structural and independent of those equations:

- exact 9-vertex, 8-triangle, 4-fluid-cell, 4-solid-cell, 2-facet cardinality
  and ordered nondegenerate connectivity;
- P1 pressure carried only on the fluid closure, solid displacement exactly
  zero outside the solid closure, and a retained MINI cell-bubble block;
- exactly one free-interface action, at the midpoint of the `x = 1 m` side;
- finite, coherent-SI payloads that keep intrinsic 2D action in `N/m` distinct
  from intrinsic 2D energy in `J/m`;
- the frozen fieldwise plan, scale profile, time step, and reference
  MINRES/identity/reproducible solver tuple;
- solver stopping evidence presented separately from physics acceptance;
- two ordered, distinct, consecutive accepted steps at `0.05 s` and `0.10 s`;
- exact Model, geometry, correspondence, mesh, Realization, final Run, state,
  and trajectory lineage agreement;
- compile-time attribution to both registered verified cases; and
- fail-closed asynchronous and browser publication.

Studio recomputes no physics. It applies only reversible presentation
transforms — display scaling, formatting, and selection — over the retained
solver-owned values; the retained values and evidence are never changed,
re-derived, or substituted, and no scientific value is fabricated when native
execution is unavailable.

This case is presentation and composition only. Because it derives no expected
value, it carries no dual independent oracle gate of its own: that gate was
discharged by the two scientific cases it composes.
