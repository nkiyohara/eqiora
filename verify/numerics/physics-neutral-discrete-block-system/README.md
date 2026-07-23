# Physics-neutral discrete block-system verification

This case verifies the first private execution boundary shared by fixed-reference
FSI, coherent-SI MINI Stokes, and the conforming elasticity pair. The contract
retains exact Semantic Field, Relation, support, Connection, and Parameter
identity together with accepted spaces, scales, transformations, algebraic
closures, stable logical packet partitions, and assembly-target membership.

The primary public evidence remains the ordinary fixed-reference FSI path. Its
unchanged canonical CSR, reconstructed Fields, incompressibility, kinematics,
interface-action balance, and energy identity demonstrate that the new block
boundary does not create a second numerical implementation. The existing
`fluid.fieldwise-si-mini-stokes-2d`,
`fluid.mixed-static-pressure-mini-stokes-2d`, and
`solid.conforming-elasticity-pair-2d` cases exercise the same private vocabulary
with a gauge auxiliary, boundary-determined pressure, and an SPD multi-Domain
trace quotient respectively.

White-box tests in `eqiora-numerics::discrete_block` falsify insertion-order
dependence, missing packets, foreign Relations, wrong support, collapsed
Parameter identity, missing Backward Euler or trace-quotient transformations,
and packet-to-target drift before scatter. The common finalized-linear core
then rechecks SolverPlan, orientation, producer/verifier topology, residual
target, and the independently reapplied true residual before typed
reconstruction, including for FSI.

Run:

```bash
cargo test --locked -p eqiora-numerics --lib discrete_block::tests
cargo run --locked -p eqiora-verify -- run --case numerics.physics-neutral-discrete-block-system
```

This is not a public weak-form or provider API, durable block artifact, generic
degree-of-freedom range model, matrix-free implementation, accelerator claim,
or new differentiation claim.
