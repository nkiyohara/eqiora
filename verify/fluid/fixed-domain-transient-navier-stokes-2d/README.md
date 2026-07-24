# Fixed-domain transient incompressible Navier--Stokes 2D

This case closes the first nonlinear transient CFD path without introducing a
fluid-specific semantic node. The canonical Model is the typed conservative
relation

```text
rho derivative(u)
  + div(rho outer_product(u, u))
  - div(2 mu symmetric_part(grad(u)) - isotropic_lift(p))
  - grad(q) = 0,
div(u) = 0.
```

`outer_product` is an ordinary proof-carrying pure operator. The lowerer
requires both operands to be the exact current velocity and requires inertia
and conservative flux to share one density tape. A relative or ALE velocity,
an advective-form shortcut, a different but equal-valued density Parameter,
or any unconsumed model node fails closed. Model wire v5 is the first wire that
can preserve this meaning; older wires remain unchanged and reject it.

## One realization, not a second meaning

The CPU reference realization chooses the MINI/P1 pair, a zero-integral
pressure constraint, backward Euler, and a skew-symmetric weak convective form.
`EnergySkewConvection` records this as an explicit Realization transformation,
not as a second Semantic Relation and not as an assertion that the two
discrete forms are identical. The skew form makes convective self-work zero;
accepted evidence also records its defect from the conservative weak form and
verifies the exact `-rho/2 div(u_h) u_h` consistency identity after global
assembly.

Physical data are converted with explicit `L/U/P` scales:

```text
t_hat   = t U/L,
rho_hat = rho U^2/P,
mu_hat  = mu U/(P L).
```

The typed transient Realization retains exact Model and independent
Realization revisions, mesh, Field/Relation/Parameter identities,
backward-Euler duration, nonlinear policy, and the fused
mass/advection/stiffness/constraint/load inventory. Step count is a Run
directive, not Realization identity. Every Newton materialization binds the
private block identity to exact CSR bytes before execution.

After compatibility resolution, the same value is normalized into the common
typed portable Realization DAG. Exact Domain and Field nodes feed the Backward
Euler and energy-skew transformations, one scaled monolithic algebraic system,
and a nonlinear root that names its sole linearization and ordinal-free host
placement requirement. This graph drives admission for the numerical path;
it is not an unused serialization view. Runtime backend identity and any local
device ordinal belong to later Deployment binding, not portable identity.

The nonlinear residual is integrated and globally assembled directly from the
weak form; it is not recovered as `Jw-b`. Every column of the analytic
Jacobian is compared with centered differences of that independent residual.
The audit derives a conservative column-intersection graph from the exact
topology, mixed quotient, local operator closures, essential constraints, and
pressure constraint. Deterministic colors perturb only columns with disjoint
proven residual-row support, while acceptance still reconstructs and checks
each column separately. Step evidence exposes audited-column count, color
count, complete residual-assembly count, and maximum error; the exact private
color representation is not a public numerical API.
Acceptance records the complete and momentum-block residual norms, weak mass
balance, pressure closure, nonzero convective action, zero skew self-work, and
the conservative-form defect.

The initial condition is a nonzero weakly incompressible MINI Stokes state on
the same mesh. It is an initial-value input, not a hidden steady-flow term in
the canonical Model. A prepared application service owns one Semantic
Program, typed Realization, owned solver-adapter handle and accepted capability
snapshot, and validated mesh envelope. Its opaque coherent-SI initial condition cannot enter the
method-native kernel or pair mesh bytes with another digest. Before any
nonlinear Jacobian or CSR is built, the selected Realization rechecks
essential values, weak continuity, pressure mean, gauge consistency, Field
identity, and mesh identity. The public run operation takes no backend, so a
different solver adapter cannot execute the prepared service. The accepted trajectory retains the complete resolved
Realization, exact executing solver identity, and count of validated block
materializations.

## Temporal evidence and bounded claim

The same canonical-to-block-system path advances to `0.04 s` with steps
`0.02`, `0.01`, and `0.005 s`. Fixed-mesh step-doubling of the complete MINI
velocity fields in the consistent MINI mass norm must have a ratio between 1.7
and 2.4. The finest trajectory is repeated with tolerances tightened by a
factor of ten; its final-state difference in that same mass norm must be below
one thousandth of the finest time difference. This bounds accumulated
nonlinear-solve sensitivity without comparing primal and dual norms, and
demonstrates expected first-order backward-Euler behavior on the fixed mesh.

Run the registered evidence with:

```sh
cargo test --locked -p eqiora --test transient_mini_navier_stokes_2d --features faer
cargo run --locked -p eqiora-verify -- run --case fluid.fixed-domain-transient-navier-stokes-2d
```

This is one affine-triangle 2D, constant-property, complete-zero-trace,
MINI/P1, serial-host reference slice. It does not claim turbulence,
compressibility, free surfaces, multiphase flow, ALE, remeshing, production
preconditioning, GPU, MPI, adjoints, or a durable trajectory artifact.
