# Hierarchical conserving connection sets

This case verifies the bounded scalar-physical slice of hierarchical
conserving connection normalization. A conserving source declaration is a
typed fragment of an equivalence relation; occurrence expansion unions all
fragments once and emits one flat canonical Connection for each maximal set.
Together with `packages.hierarchical-physical-boundary` and the explicit-flat
oracle in `language.component-elaboration`, it consumes the
`hierarchical-conserving-connection-sets-v1` conformance kit. The kit shares
only canonical set observation and stable diagnostic assertions; local source
compilation remains this case's own entry path.

The `nary.eqi` and `chain.eqi` fixtures describe the same three-terminal net
as one N-ary declaration and as two overlapping binary declarations. The
evidence requires equal Model identity, transaction operations, Model v2
canonical bytes, and semantic digest. It also requires one final Connection
with three members. The chain retains both declaration origins in provenance,
showing that traversal-local witnesses do not enter semantic identity.

The `flat-nary.eqi` and `flat-chain.eqi` fixtures exercise the public parsed
Model path without Connector or Component declarations. Both must enter the
same pre-Kernel normalizer and emit one Connection over exactly `a`, `b`, and
`c`; the direct path does not retain persistent cross-compilation IDs.

The `wrapper-exposure.eqi` fixture joins an ownerless public Wrapper terminal
to an internally owned Leaf terminal, then extends that set from the parent
Model. The Wrapper terminal must be removed from the Kernel relation network,
must not be fabricated as an entity alias, and must leave the one canonical
two-member Connection produced for the retained endpoints. A compiler sidecar
retains its full occurrence identity, scalar Connector, non-graph provenance,
the exact final Connection, and the one-member interior cut. It does not add a
Kernel Port or choose an arbitrary retained endpoint as an alias.

`two-level-forwarding.eqi` extends that proof through two explicit public
forwarding levels. Both exposure names disappear, the two owned leaf Ports
form one set, both distinct exposure identities project to their exact nested
occurrence cuts, and all three fragment origins remain complete. `disjoint.eqi`
and `joined.eqi` prove that repeated Component occurrences remain independent
until an explicit parent fragment joins them.

`distinct-exposure-cuts.eqi` is the projection falsifier: two public Ports in
one Component occurrence forward to different internal Leaves and are joined
only by the parent Model. Each projection must follow only the fragments
declared at or below its own occurrence, so the two one-member cuts stay
distinct even though the parent union emits one maximal Connection. Selecting
the whole occurrence subtree would fail this case.

The executable permutation matrix varies declaration order, N-ary member
order, binary fragment order, and binary member order across 84 compilations.
Every form must retain the same model identity, symbols, transaction, and
canonical Model v2 bytes. Duplicate fragments are topology-idempotent while
retaining both source origins; a duplicate member inside one fragment remains
an error.

The invalid fixtures require source-representable missing membership,
multiple Relation ownership, an entirely ownerless physical set, and an
untouched grandchild interface to fail before a transaction is exposed.
Conversely, `owned-public-forwarding.eqi` proves that a public Port with one
Relation owner remains an ordinary endpoint instead of being mistaken for a
transparent exposure alias.
Independent topology, identity, and provenance budgets are exercised at their
contract owners. Nominally different physical types never union, and signal
fan-out remains on its separate directed path.

Run:

```bash
cargo test --locked -p eqiora --test hierarchical_connection_sets
cargo run -p eqiora-verify -- run --case language.hierarchical-connection-sets
```

This evidence is limited to local source, nominal scalar physical Ports, and
static compiler-side projection identity. It does not provide an
artifact-sealed observation, value payload, or numerical sampling contract.
Field-valued boundary projection evidence, package replay, numerical
solvability, nonlinear or transient physics, MPI, and GPU execution remain
outside this case. The exact-package and explicit-flat numerical claims are
owned by the other two conformance-kit consumers rather than duplicated here.
