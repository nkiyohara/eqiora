# Stokes minimum-drag shape-optimization experience

Status: future-state experience contract. It narrows, but does not advance,
`optimization.cylinder-drag`.

## Responsibility and public claim

The film closes an accepted steady Stokes solve into one constrained,
PDE-driven shape-design history. Starting from a smooth equal-area body, the
accepted two-dimensional geometry revisions reduce drag toward the
Richardson minimum-drag profile while preserving the declared cross-sectional
area.

The retained first optimization flagship is Stokes, not a turbulent airfoil.
It claims an independently checked shape derivative, accepted optimization
history, decreasing objective under the chosen algorithm, and comparison with
the first-order optimality condition. It does not claim a unique global
minimum, general differentiable CAD, Navier--Stokes optimization, topology
optimization, or aerodynamic airfoil performance.

## Storyboard

| Presentation time | Content |
|---|---|
| 0--2 s | Initial body, flow direction, viscosity, design boundary, area constraint, and design variables |
| 2--11 s | Selected accepted iterations morph the exact geometry; pressure is the sole primary field and iteration/objective stay visible |
| 11--15 s | Drag and constraint histories finish beside the accepted terminal profile and optimality residual |
| 15--18 s | Labelled diagram reset to the initial design; iterations are never played backward as physics |

Iteration sampling is explicit. The film does not imply that omitted optimizer
steps were never executed.

## Evidence and falsifiers

The prerequisite evidence includes the steady exact-geometry Stokes solve,
physical force projection, parameter-to-geometry and mesh correspondence,
state sensitivity, shape derivative, constraint derivative, and optimizer
acceptance. Directional derivatives are checked on a frozen mesh family before
any optimization result is admitted.

The decisive observable family is drag and area/volume constraint versus
accepted iteration, the adjoint or analytic directional derivative versus an
independent finite-difference or complex-step route, and the terminal
first-order boundary condition. The experience is rejected if the gradient
check misses its precommitted asymptotic band, if the constraint drifts beyond
its independent budget, or if the terminal profile fails the source-derived
optimality condition. Objective decrease alone is insufficient.

The derivative, constant-surface-vorticity optimality condition, and analytic
two-dimensional reference profile are derivation-bearing and therefore use
dual independent oracles before implementation.

## Capability and artifact dependencies

- exact parameter-driven boundary revisions with persistent semantic identity;
- error-controlled remeshing or deformation whose correspondence is retained;
- physically scaled drag and an accepted differentiable solve/shape-derivative
  path;
- constrained optimization with immutable iteration and rejection history;
- accepted geometry/result histories and a gallery profile for design
  iterations.

This experience follows Turek--Hron in public delivery because it reuses the
accepted force, moving-geometry, trajectory, and coupled-result presentation
seams. Its independent scientific lanes may start earlier once those contracts
are frozen.

## Accessibility and promotion

The reduced-motion still overlays initial and final outlines without using
color alone and reports drag reduction, constraint defect, and gradient-check
status. The text alternative describes the fore/aft profile change and states
that intermediate iterations are sampled.

Promotion requires accepted primal, derivative, constraint, and optimization
evidence; a complete immutable design history; and common publication
admission.

## Primary sources

S. Richardson,
[“Optimum Profiles in Two-Dimensional Stokes Flow”](https://doi.org/10.1098/rspa.1995.0103),
1995, owns the fixed-area two-dimensional reference. O. Pironneau's
[foundational Stokes optimality result](https://doi.org/10.1017/S002211207300145X)
supplies the variational lineage but does not widen this experience to the
three-dimensional fixed-volume body.
