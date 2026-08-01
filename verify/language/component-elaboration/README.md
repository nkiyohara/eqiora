# Deterministic component elaboration

This case verifies the bounded local-source implementation of RFC 0021. A
root model instantiates one `ParallelDc` system component. That system binds
and owns four nested leaf instances: a voltage source, two instances of the
same resistor definition, and an explicit ground. Its two conserving
Connections form a complete scalar physical network before the hierarchy is
lowered.

The compiler resolves typed public Parameter terms and Ports, rejects access
to private implementation members, assigns each materialized declaration a
deterministic identity from its exact instance path, and emits the existing
flat Semantic Kernel vocabulary. A public Parameter is a lexical term: binding
it through both component levels to a root Model Parameter preserves that one
mutable and differentiable identity instead of fabricating occurrence-local
copies. Component and instance are compiler concepts; neither becomes a new
kernel node. Definition, instance, and binding spans remain in an immutable
provenance sidecar rather than affecting semantic identity.

The canonical and permuted fixtures have the same typed declarations,
bindings, residual order, and connection sets. They deliberately vary
component order, member order, instance order, binding order, and Connection
member order. Compilation must produce identical graph IDs, current Model
bytes, and digest while retaining source-specific provenance.
The two resistor instances must remain distinct from one another and stable
between the fixtures.

The separate `explicit-parallel-dc.eqi` fixture writes every expanded Domain,
Parameter, Port, Relation, and Connection without using components. It owns
the same three root Parameters used by the leaf Relations. The test
defines a complete one-to-one correspondence for all named entities, derives
generated Activation identities from their Relation edges and Connection
identities from their normalized member sets, and requires that this mapping
cover every ID in both current Model artifacts. Rewriting only those IDs must
then produce byte-identical canonical current Model artifacts and identical
semantic digests. This is the bounded semantic-equivalence claim; equal
numerical answers alone are not used as its proof.

This explicit-flat comparison is also the circuit oracle for the
`hierarchical-conserving-connection-sets-v1` conformance kit. It proves that a
normalized hierarchy and its explicit N-ary flat form have the same complete
semantic artifact after the established identity bijection. The numerical
problem is solved once from that proven hierarchy, avoiding a second iterative
solve that would add nondeterminism without adding a distinct claim.

After elaboration, the selected closed subsystem from the hierarchical fixture
is admitted as the 14-by-14 affine scalar physical problem proven above to be
semantically identical to the explicit fixture. The problem is solved once by
faer BiCGSTAB with identity preconditioning on one host worker, then accepted
again through the original Relation and generated junction DAGs. The analytic
oracle requires 12 V at the high junction, 6 A and 3 A through the two resistor
positive terminals, -9 A at the source positive terminal, zero ground
potential, and zero signed through sum at both junctions.

Run:

```bash
cargo test --locked -p eqiora --test component_hierarchy
cargo run -p eqiora-verify -- run --case language.component-elaboration
```

The registered falsifiers cover missing, duplicate, unknown, private, and
dimension-incompatible Parameter bindings; private member selection; nominal
connector mismatch; and direct or indirect recursive instantiation. Failure
must return no compiled model or graph Transaction. Lower-level compiler tests
separately exercise configured expansion and identity resource limits.

This evidence is limited to local declarations in one source unit, scalar
`f64`, continuous time-independent affine Relations, explicit N-ary
Connections declared at one lexical level, and serial host execution. It does
not claim modules, imports, Model Package resolution, a published component
library, native/Python/Studio hierarchy authoring, port forwarding through
overlapping connection sets, inside/outside signs, root physical boundaries,
stream or expandable connectors, arrays, records, replaceable or conditional
components, recursive or dynamic hierarchy, nonlinear or transient devices,
hybrid or mixed-signal execution, MPI, CUDA, performance, or a durable public
provenance wire.
