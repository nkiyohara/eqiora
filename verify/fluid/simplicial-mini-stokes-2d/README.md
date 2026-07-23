# Nondimensional simplicial MINI Stokes realization

This case executes one narrow numerical realization of steady incompressible
Stokes flow on a connected two-dimensional affine triangular mesh. Velocity
uses continuous `(P1 + cell bubble)^2`, pressure uses continuous P1, and one
global multiplier imposes zero mean pressure. A positive Duffy rule exact
through triangle total degree four integrates the MINI operator; a separate
positive degree-six Duffy rule evaluates the manufactured error norms.

The manufactured unit-square problem is

```text
mu = 1,
u  = (x^2, -2 x y),
p  = x - 1/2,
f  = (-1, 0).
```

The exact velocity is divergence-free and supplies the complete essential
P1 trace; the pressure already has zero mean. Uniform refinements `2, 4, 8`
must produce velocity L2 order above `1.75`, velocity H1-seminorm order above
`0.85`, pressure L2 order above `0.85`, and discrete-divergence L2 order above
`0.85` on every consecutive pair.

Each triangle contributes once to an ordered reduced solve system and an
uneliminated full reaction system. The reduced matrix is asserted and checked
as symmetric indefinite, then solved by the identity-preconditioned,
reproducible reference MINRES path. Every level independently checks the true
residual, exact CSR symmetry, pressure mean, zero compatibility multiplier,
weak continuity equations, and componentwise boundary-reaction plus body-force
balance. The middle refinement also proves bit-identical algebra and fields
between one-worker reference assembly and four-worker ordered Rayon assembly,
while retaining distinct execution provenance.

The fail-closed half of the case rejects incompatible prescribed boundary
flux, a disconnected mesh under one global gauge, quadrature below total
degree four, nonpositive viscosity, non-finite body force, and conjugate
gradient or Jacobi-preconditioned MINRES for the indefinite operator. It also
rejects a non-finite essential velocity and one- or three-dimensional meshes.

Run the evidence with:

```sh
cargo test --locked -p eqiora --test simplicial_mini_stokes_2d
cargo run --locked -p eqiora-verify -- run --case fluid.simplicial-mini-stokes-2d
```

This is a nondimensional numerical oracle. It does not claim canonical Stokes
lowering, a fluid package, a physical fluid Port, faer MINRES, durable MINRES
artifact-v1 execution, natural/open boundaries, Navier--Stokes, distributed or
device execution, or FSI. The Rayon assertion is limited to the existing
ordered host assembly adapter; it is not a parallel MINRES claim.
