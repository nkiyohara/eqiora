# Fixed-reference monolithic fluid--structure step in 2D

This case closes Eqiora's first executable fluid--structure interaction path
without adding an FSI object to the Semantic Kernel. Direct Relations and
exact immutable packages describe one inertial incompressible Newtonian fluid,
one first-order linear solid, and one ordinary conserving velocity/traction
Connection.

One exact content-addressed affine-triangle mesh is partitioned by RFC 0049
correspondence. Fluid velocity uses MINI, fluid pressure uses P1, and solid
velocity and displacement state use vector P1. The exact interface facet
closure induces one shared P1 velocity trace; no coordinate matching,
interpolation, penalty, multiplier, or traction callback is present.

One backward-Euler step eliminates the next solid displacement and produces a
single symmetric-indefinite operator over shared velocity and fluid pressure.
The complete coupled operator, not a copied standalone Stokes rule, determines
absolute pressure without a zero-integral gauge. The accepted CPU reference
solution proves true residual, weak incompressibility, exact kinematics,
opposite body-cut interface actions, and the complete discrete energy identity.

The accepted multi-Domain v3 plan also projects into the common typed portable
Realization DAG. Its exact kinematic Relation drives backward-Euler state
elimination, and its exact conserving Connection drives the cross-Domain trace
quotient. The finalized solver consumes this graph while v3 compatibility data
remains outside the in-memory projection. The separate
`artifacts.realization-run-wire` case owns the committed v3 canonical-byte
golden fixture.

The artifact path binds the exact Model, Geometry Identity, correspondence,
mesh, multi-Domain Realization, Run inputs, and finalized operator. Reconstructed
values retain exact Field IDs in memory. This case makes no durable fixed-mesh
State or trajectory publication claim.

Run:

```bash
cargo test --locked -p eqiora --test fixed_reference_monolithic_fsi_step_2d
cargo run --locked -p eqiora-verify -- run --case fsi.fixed-reference-monolithic-step-2d
```

This case does not claim Navier--Stokes advection, multiple time steps,
partitioned coupling, ALE, remeshing, GPU, MPI, adjoints, CAD, or durable
scientific result storage.
