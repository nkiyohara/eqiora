# Stokes exact-area dissipation shape-optimization experience

Status: future-state experience contract. It narrows, but does not advance,
`optimization.stokes-cell-dissipation`. The deferred Richardson/physical-drag
target remains separately owned by `optimization.cylinder-drag`.

## Responsibility and public claim

The film closes an accepted bounded steady-Stokes solve into one immutable,
PDE-driven shape-design history. Starting from the exact circle in a fixed
square container, two smooth centred shape coordinates preserve analytic body
area exactly while every admitted trial regenerates the geometry, moves one
fixed-topology body-fitted mesh harmonically, re-solves the all-Dirichlet
state, and evaluates finite-element viscous dissipation.

The retained first optimization flagship is a bounded two-dimensional
dissipation design, not exterior minimum drag. It claims an independently
checked complete discrete reduced gradient, at least one accepted nonzero
step with lower discrete dissipation, immutable accepted and rejected trials,
and objective ordering that survives a distinct precommitted refined
topology. The last state is the **accepted final iterate**, not an optimum,
unless a later independently precommitted stationarity criterion passes.

It does not claim physical force or drag, a Richardson profile or exterior-
flow approximation, a continuous shape derivative, a mesh-independent or
global optimum, general differentiable CAD, remeshing, topology optimization,
Navier--Stokes optimization, or aerodynamic airfoil performance.

## Storyboard

| Presentation time | Content |
|---|---|
| 0--2 s | Fixed square, exact circular start, uniform outer velocity, no-slip body, viscosity, exact-area profile family, and two design coordinates |
| 2--11 s | Selected accepted iterates morph the exact geometry; pressure is the sole primary field while discrete dissipation, analytic area defect, and gradient-check status stay visible |
| 11--15 s | Complete-history summary, reference/refined objective ordering, and the exact termination reason finish beside the accepted final iterate |
| 15--18 s | Labelled diagram reset to the initial circle; design iterations are never played backward as physical time |

Iteration sampling is explicit. The film does not imply that omitted accepted
or rejected trials were never executed, and rejected trials remain part of
the admitted history.

## Evidence and falsifiers

Dual independent routes precommit and reconcile the exact polar-area identity,
complete discrete Stokes dissipation and reduced derivative, units, gradient-
comparison predicate, sufficient-decrease rule, refinement predicate, and
terminal labels before implementation output is visible. One route derives
the analytic/discrete formulation; the other independently realizes the same
finite-cell problem without importing Eqiora arrays, ordering, or results.

The ordinary positive path proves an accepted bounded Stokes state and
dissipation first, passes independently regenerated centred coefficient
differences, accepts at least one nonzero decreasing step, retains the full
trial lineage, and preserves initial-to-final objective ordering on the
distinct refined topology. The experience stops if either independent route
disagrees or any formula, value, criterion, tolerance, or solver rule would be
chosen after seeing implementation output.

The minimum falsifier family includes a corrupted exact-area normalization,
swapped harmonics, a wrong body-normal sign, an outer side changed to inlet or
traction meaning, a missing pressure gauge, an incorrect dissipation factor,
an incomplete state/geometry derivative, finite differences over a stale
polygon, parent-state reuse without a fresh child solve, aliased refinement,
invalid-mesh acceptance, discarded rejected trials, and budget exhaustion
mislabelled as stationarity. Objective decrease alone is insufficient.

## Capability and artifact dependencies

- accepted exact analytic-source geometry and source-bound mesh identity;
- accepted fixed-topology harmonic coordinate motion and correspondence;
- the bounded all-Dirichlet steady-Stokes Result and pressure boundary;
- accepted reduced-differentiation and backtracking mechanics;
- a private native full-history owner plus the bounded read-only
  `StokesDissipationDesignProjection` product surface; and
- common typed presentation, media admission, accessibility, and publication
  seams.

The future registered executable case is
`optimization.stokes-cell-dissipation-2d`. It neither replaces nor promotes
`optimization.cylinder-drag`. This experience consumes no force or drag
capability, and it never interprets design trials as a physical Trajectory.

## Accessibility and promotion

The reduced-motion still overlays the initial and accepted-final outlines
without using color alone and reports discrete dissipation change, analytic
area defect, gradient-check status, refined-ordering status, and the honest
termination reason. The text alternative describes the shape-coordinate
change and states that displayed iterations are sampled from a complete
immutable history.

Promotion requires accepted exact-profile and fixed-topology evidence; the
ordinary state, objective, complete-gradient, stale-state, and refinement
falsifiers; one exact-head accepted candidate with every attempted trial; the
bounded installed-Python product projection; and common publication
admission.

## Primary sources

O. Pironneau's
[bounded Stokes variational result](https://doi.org/10.1017/S002211207300145X)
supplies the fixed-outer-boundary, moving no-slip body, symmetric-gradient
dissipation, and fixed-volume descent lineage. Its worked theorem does not
supply Eqiora's two-dimensional discrete oracle or optimizer values.

S. Richardson's
[two-dimensional exterior minimum-drag result](https://doi.org/10.1098/rspa.1995.0103)
owns the separately deferred physical-drag target. No Richardson profile
coefficient, effective-radius value, derivative density, or tolerance is used
by this bounded-cell experience.
