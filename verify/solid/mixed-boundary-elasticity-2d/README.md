# Mixed-boundary elasticity: direct and packaged equivalence

This case is the first execution consumer of the package-neutral elasticity
boundary inventory. The direct Model and the exact
`Eqiora.Solid.LinearElasticity@0.3.0` Model express the same problem using
different authoring forms. Both lower to one essential zero side and three
natural zero sides, then assemble bit-identical reduced and unconstrained-full
CSR systems and right-hand sides. The reduced system alone crosses the solver
boundary. No package name is available to the numerical path.

The root `mu` and `lambda` Parameters are also the exact identities seen by
both the packaged volume law and its boundary law. Component Parameter slots
are typed lexical terms, not new semantic Parameters. The direct and packaged
coefficient expressions therefore agree in primal evaluation, forward JVP,
and evaluated reverse-mode VJP. A near miss that gives the boundary law a
numerically equal but independently declared Parameter is rejected;
coefficient identity is never inferred from the current value. [RFC
0055](../../../rfcs/0055-component-parameter-terms.md) fixes this contract.

The package release is frozen beside the case. Its first two Components and
Connector are unchanged from verified version `0.2.0`; version `0.3.0` adds
only `FixedDisplacement2d` and `ZeroTraction2d`, which are semantic zero-data
terminals and contain no mesh, element, quadrature, solver, or execution
policy.

For `mu = 3 Pa`, `lambda = 0 Pa`, `ell = 1 m`, and
`q = 2 mu x / ell`, the exact displacement is
`u = (x - x^2/(2 ell), 0)`. Uniform Q1 refinements `4, 8, 16, 32` prove the
exact interpolation norms `h^2/sqrt(120)` and `h/sqrt(12)`, nodal agreement,
integrated body force `[6, 0]`, constrained algebraic reaction `[-6, 0]`, and
global balance. Independent facet quadrature recovers zero traction on the
horizontal sides and `3h` on the right, so the raw Q1 traction converges to
the prescribed zero value at first order. The recovered left resultant
`-6 + 3h` is intentionally not confused with the algebraic reaction. A
separate three-Port connection remains `PortBinding` in canonical meaning and
is rejected by this Q1 Realization before mesh construction.

Near-miss falsifiers reject a boundary/volume stress mismatch, equal-valued
but independently identified stress coefficients, a recognized direct law
accompanied by an extra Relation, duplicate exact Cartesian side identity, and
a terminal that prescribes both conjugate variables. These are semantic
admission failures, not solver failures.

Run the evidence with:

```sh
cargo test --locked -p eqiora --test mixed_boundary_elasticity_2d
cargo run --locked -p eqiora-verify -- run --case solid.mixed-boundary-elasticity-2d
```

This case does not claim nonzero boundary data, arbitrary boundary subsets,
live trace-space coupling, nonmatching transfer, mixed/high-order or
unstructured elasticity, three dimensions, Stokes, FSI, or nonlinear/dynamic
solid mechanics.
