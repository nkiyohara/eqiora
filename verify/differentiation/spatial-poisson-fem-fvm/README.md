# Spatial Poisson FEM/FVM differentiation

One canonical two-dimensional Poisson model is lowered once per revision and
realized independently as Q1 FEM and orthogonal cell-centred TPFA FVM. The
analysis explicitly selects `source_scale`, `diffusion`, and
`boundary_offset`, in an order different from their lowered first occurrence;
`wave_number` remains frozen. This exercises source, constitutive, and
essential-boundary actions without making every model Parameter a design
coordinate.

For each realization, the test checks the assembled analytic `R_p` through:

- every component of the forward state sensitivity against a centred finite
  difference of independently compiled and solved canonical revisions; and
- the adjoint derivative of the arithmetic mean of the method-native unknown
  vector against both that finite difference and the forward contraction.

The same test additionally compiles an opaque exact Model/Realization-bound
application program. Program identity fixes the ordered Parameter coordinates,
output, shapes, scalar/device contract, solver policy, canonical Model, and
Realization; it does not contain mutable current-point state. Each immutable
evaluation owns one exact Parameter point, its accepted primal, paired
linearized relation and output, and solve evidence.

The `y = O(w, p)` projection publishes the complete primary Field, not the
method-native unknown vector. Q1 FEM therefore scatters free-state tangents and
retains direct essential-boundary Parameter actions; TPFA publishes its cell
vector directly. At both the canonical default point and one non-default point,
the complete-Field primal/JVP/VJP agrees with independent rebuilds and centered
differences and satisfies JVP/VJP duality. The test returns through
`p0 -> p1 -> p0`, exercises default and alternate evaluations concurrently,
and proves that neither operation mutates the static program or another
evaluation.

Recompiling the same exact program reproduces program and default state-system
identity. Foreign Model roles, stale Realizations, wrong tangent/cotangent or
Parameter-point shapes, non-finite points, and inadmissible coefficients fail
closed. Bound model, exact Realization, finalized system, and accepted solution
remain one owned handoff, so callers cannot pair a solution with another
point's linearization. A separate `diffusion²` falsifier uses `p = +1` and
`p = -1` to produce identical primal systems with opposite derivative actions;
both accepted points retain the correct action without inferring identity from
assembled bytes.

This case does not claim shape derivatives, mesh derivatives, nonorthogonal
FVM, mixed/high-order elements, a continuous-objective discretization, Python
alternate-point evaluation, framework operators, batching, persistence, GPU,
or distributed evaluation.
