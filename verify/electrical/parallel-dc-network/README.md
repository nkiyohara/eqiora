# Parallel DC scalar physical network

This case is the first executable boundary of RFC 0024. The same nominally
typed electrical network is authored from source and from client-neutral,
immutable native declarations. It connects a 12 V ideal source, parallel 2 Ω
and 4 Ω resistors, and an explicit ground Relation. The high junction has
three Ports and the ground junction has four.

The source compiles to a complete Semantic Model and round-trips through the
current Model owner. The selected closed subsystem is
then admitted structurally as `R(w) = A w + c`, captured once as a general
canonical CSR system, and submitted to faer BiCGSTAB with identity
preconditioning on one host worker. The exact analytic vector is expressed in
each Model's canonical unknown order and accepted as an initial-residual
witness. It is checked twice: first by the Eqiora-owned CSR action, then by
evaluating the original relation and generated junction DAGs in canonical
order. This case proves semantic lowering and backend reacceptance, not
BiCGSTAB convergence from an arbitrary initial guess.

The analytic checks require 12 V at the high junction, 6 A and 3 A into the
positive resistor terminals, −9 A at the source positive terminal, zero at
ground, and zero signed through sum at both junctions. The test also checks
canonical unknown/root ordering, exact current-artifact reconstruction, and
fail-closed solver admission without fallback. Its registered semantic
residual bound is the solver plan's `1e-12 * ||b|| = 1.2e-11` target; every
reported physical value is also compared with the analytic solution to
`2e-11` absolute error.

The native authoring path uses draft-local Domain and Port identity, including
multi-root Relations and borrowed N-ary Connection membership. Ordinary native
authoring and source compilation both use the current semantic profile and
current Model/Transaction owner. Each reconstructs its canonical bytes and
digest without fallback and matches the source-authored analytic values port
by port.
Source and native CSR systems each reaccept the exact analytic vector through
the same faer request and original semantic DAGs. Repeating unconstrained
Krylov solves for fresh, identity-permuted drafts would test BiCGSTAB
breakdown behavior rather than authoring equivalence; controlled iterative
solver cases own that separate claim. Native declarations do not implement
parallel physical semantics.

An isolation falsification adds an ordinary continuous Relation that has no
physical Port. The whole-model bytes and digest must change, while the selected
physical closure and captured CSR system remain exactly unchanged. The
registered analytic witness already accepts that exact executor input; the
isolation check does not repeat an identity-permuted Krylov solve.

Run:

```bash
cargo test --locked -p eqiora --test scalar_physical_dc
cargo run -p eqiora-verify -- run --case electrical.parallel-dc-network
```

Contract-level companion regressions in `eqiora-sem` reject duplicate
Connection membership, non-continuous physical activation, nominal-domain
mismatch, missing ownership or membership, physical access through signal
Ports, and mixed causal, hybrid, spatial, or model-boundary content before
execution. They protect RFC 0024's general admission boundary without widening
this registered numerical case.

This evidence is limited to Rust source/native authoring of a flat,
time-independent, affine, scalar `f64` network and one serial host backend. It
does not claim Python authoring, nonlinear devices,
transient or DAE execution, hierarchy, inside/outside connector signs,
switched topology, a reusable electrical component library, causal coupling,
MPI, CUDA, or accelerator residency.
