# Conforming execution of a packaged elasticity interface

This case executes one live field-valued physical connection between two
independent two-dimensional elastic bodies. The direct Model and the exact
`Eqiora.Solid.LinearElasticity@0.3.0` Model retain two distinct semantic
Domains and the same ordinary two-Port conserving Connection. Both lower to
the same package-neutral pair of elasticity contracts; no package name reaches
the numerical path.

The Realization generates one Cartesian Q1 mesh per body and owns an explicit
topological bijection between the two coincident interface traces. It maps each
paired trace vertex to one quotient displacement degree of freedom. Trace
continuity is therefore identity of the assembled unknown, while contributions
from both bodies accumulate in the same global residual row and impose weak
interface equilibrium. The Semantic Model is not rewritten as one merged
Domain or one merged mesh.

The manufactured problem deliberately uses different materials so that a
missing interface law cannot hide behind a globally smooth polynomial. On the
left and right half-squares, respectively, `mu = 3 Pa` and `mu = 6 Pa`, while
`lambda = 0 Pa` and `grad(q) = (6, 0)`. The exact displacement is continuous at
`x = 1/2` but has strains `1/2` and `1/4` there. Exact outward interface
tractions are `[3, 0]` and `[-3, 0]`.

Uniform refinements prove the exact Q1 interpolation errors
`h^2 / sqrt(192)` and `h sqrt(5/96)`, where `h = 1/(2n)` is the horizontal cell
width for `n` cells across each half-domain. Body-local cut residuals prove
opposite weak interface actions on every free interface row at finite
resolution. A retained mask excludes any endpoint also constrained by an
external support, whose cut residual is not uniquely a coupling action.
Independent raw Q1 stress recovery instead gives `[3 + 3h, 0]` and
`[-3 + 3h, 0]`; its nonzero sum `[6h, 0]` converges at first order and is
intentionally not presented as the algebraic interface balance.

The direct witness permutes Domain declaration order, instance names, and
Connection member order. The packaged witness uses a non-default import alias.
All authoring variants must produce the same canonical pair algebra.

Run the evidence with:

```sh
cargo test --locked -p eqiora --test conforming_elasticity_pair_2d
cargo run --locked -p eqiora-verify -- run --case solid.conforming-elasticity-pair-2d
```

This is an exact two-body, full-side, coincident, matching-Q1, monolithic
solid-solid slice. It does not claim a general multi-domain executor,
nonmatching trace transfer, mortar/Nitsche/penalty coupling, Lagrange
multipliers, arbitrary interface subsets, three dimensions, mixed or
high-order spaces, nonlinear or dynamic solids, Stokes, or FSI.
