# Installed Python linear-elasticity Plan and common Result

This case verifies that the installed Python package compiles the accepted
byte-exact mixed-boundary source through the current `Model` owner, resolves an
explicit keyword-only `LinearElasticity` intent, and submits the resulting
exact-Model-bound `LinearElasticityPlan` through the ordinary `Run` path:

```python
intent = eqiora.solid.LinearElasticity(
    cells_per_axis=16,
    relative_tolerance=1.0e-12,
    absolute_tolerance=1.0e-14,
    maximum_iterations=10_000,
)
plan = eqiora.solid.resolve(model, intent)
run = eqiora.submit(model, plan=plan)
result = run.result()

displacement = model.field("displacement")
snapshot = result.field(displacement)
mesh = result.mesh(displacement)
evidence = eqiora.solid.linear_elasticity_evidence(result)
figure = eqiora.matplotlib.plot_deformed_field(
    result,
    field=displacement,
    scale=1.0,
)
```

The Plan makes the effective generated-Cartesian Q1 Realization, solver,
backend, execution, and worker facts inspectable before execution. Resolution
admits only the already verified tuple above and rejects other values without
fallback. `submit` and `run` accept this Plan as a distinct typed dispatch arm,
not as a universal option bag.

The common `Result` owns one immutable vector `FieldSnapshot` and its paired
exact `Mesh`. Both are selected by the caller's exact Model-bound displacement
`FieldRef`; names, field-id strings, and structural Model equivalence are not
identity. Their canonical 289-by-2 coordinates, 256-by-4 Q1 connectivity, and
289-by-2 displacement values are co-indexed, memoized, read-only, and survive
release of the Result. `LinearElasticityEvidence` owns the unchanged Run
digest, reference-CG solve summary, assembly counts, constrained reaction,
integrated body force, and exact bounds. Cross-physics evidence selection
rejects rather than presenting an empty or optional evidence bag.

The optional Matplotlib adapter draws each canonical unique undirected Q1 edge
once. Its deformed coordinates are exactly `coordinates + scale * displacement`;
the finite nonnegative scale is visible, changes no accepted value or identity,
and the returned headless Figure remains caller-owned.

Scientific meaning, expected values, and tolerances remain owned by
[`solid.mixed-boundary-elasticity-2d`](../../solid/mixed-boundary-elasticity-2d/README.md).
Exact generated spatial artifacts remain owned by
[`artifacts.generated-cartesian-q1-spatial-output`](../../artifacts/generated-cartesian-q1-spatial-output/README.md).
The native Studio consumer remains
[`interfaces.studio-mixed-boundary-elasticity-demo`](../studio-mixed-boundary-elasticity-demo/README.md).
This case changes only the installed Python ownership path; it derives, tunes,
or relaxes no scientific value, tolerance, lineage relation, or falsifier.

For one subsequent prerelease, `MixedBoundaryElasticityResult`,
`solve_mixed_boundary_elasticity`, and `plot_displacement` remain actionable
`DeprecationWarning`-emitting compatibility shims. They delegate to the path
above and own no Result storage, execution, lineage, plotting implementation,
documentation quick start, test oracle, or evidence. Their physical removal at
the next prerelease boundary is the deletion condition for this compatibility
surface.

This case does not claim a new structural formulation, tolerance, material
law, load, stress, strain, traction recovery, analytic error, convergence
order, nonzero boundary data, other mesh or refinement, 3D, nonlinear or
dynamic structure, FSI, generic Field plotting, colour or magnitude field,
interactive viewer, exact image bytes or dimensions, or scientific validation
from pixels. The complete executable contract is in [`case.toml`](case.toml).
