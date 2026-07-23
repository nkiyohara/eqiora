# RFC 0024: Scalar conserving connection semantics

- Status: Implemented and verified for the bounded Phase 1 slice
- Authors: Eqiora contributors
- Created: 2026-07-19
- Related: RFC 0021

## Summary

Eqiora models a flat scalar physical network with nominally typed conserving
Ports, explicit `Across(port)` and `Through(port)` symbols, and one
deterministic residual DAG generated from each explicit N-ary junction.

## Later compiler boundary

This accepted RFC remains authoritative for the flat Semantic Kernel,
low-level Transaction API, v2 wires, junction equations, and the registered
Phase 1 affine case. The later [RFC 0033 hierarchical connection-set
contract](0033-hierarchical-conserving-connection-sets.md) allows the
hierarchy compiler to treat source declarations as typed fragments and emit
one normalized flat Connection. Consequently, statements below that reject
pairwise transitive union describe the RFC 0024 Kernel/Transaction input and
its registered evidence boundary, not every current compiler-internal staging
path. Direct flat source and hierarchy source use RFC 0033's same bounded
pre-Kernel normalizer, as registered by
[`language.hierarchical-connection-sets`](../verify/language/hierarchical-connection-sets/README.md).
That compiler contract does not widen RFC 0024's flat Kernel evidence claim.

## Motivation

The current `Conserving` Port and Connection variants are structural markers.
They can reject a signal/conserving mismatch, but they define neither the two
physical variables at a Port nor the equations produced by a connection. The
single dimension stored by the current Port cannot describe both an electrical
potential and a current, and dimension equality alone cannot distinguish two
nominal physical domains.

Reinterpreting those markers would also change the meaning of
`eqiora.model-envelope/v1` and the transaction v1 wire without changing their
identities. The first executable slice therefore needs one precise semantic
contract and an explicit wire-version boundary before source, component
libraries, or optimized solvers depend on it.

## Proposed design

### Scalar physical domains are nominal types

`DomainKind` gains one closed logical variant:

```text
ScalarPhysical {
    across_dimension: DimExponents,
    through_dimension: DimExponents,
}
```

The Rust constructor is
`DomainDef::scalar_physical(id, across_dimension, through_dimension)`; the
existing `kind()` accessor exposes the closed payload.

The Domain node ID is the nominal identity. Two scalar physical Domains with
equal dimensions are still different types and cannot be connected. A scalar
conserving Port stores exactly one such Domain ID and does not duplicate either
dimension. `Across(port)` and `Through(port)` derive their dimensions from the
referenced Domain.

A scalar physical Domain is not spatial support. `DefinedOn`, `AppliesOn`,
`BoundaryOf`, coordinates, traces, and geometry realization reject it. This RFC
does not add a tenth kernel node kind or a domain-specific equation hook.

The current v1 `Conserving` marker is not a scalar physical Port. Decoding it
retains its current structural-only meaning, and executable conserving lowering
continues to reject it. There is no implicit migration because v1 contains no
through dimension or nominal Domain identity.

Compatibility includes the old expression meaning: an unqualified
`Port(marker)` continues to type-check from the marker's saved scalar dimension
and a structurally valid marker Connection still admits a `KernelProgram`.
This exception preserves v1 model bytes and meaning; it does not admit the
marker to scalar physical closure, generated junction residuals, or reference
execution, all of which continue to fail closed.

### Port payload and Rust API migration

The current `PortDef { kind, dimension }` shape and
`PortDef::new(id, kind, dimension)` constructor cannot represent a physical
Port without duplicating or ambiguously selecting one of its two dimensions.
They are replaced by one closed payload:

```text
pub enum PortPayload {
    Signal {
        direction: SignalDirection,
        dimension: DimExponents,
    },
    ConservingMarker {
        dimension: DimExponents,
    },
    ScalarPhysical {
        domain: Id<kinds::Domain>,
    },
}

pub struct PortDef {
    id: Id<kinds::Port>,
    payload: PortPayload,
}
```

`ConservingMarker` preserves the exact structural-only v1 value and never
enters physical execution. The public constructors are
`PortDef::signal(id, direction, dimension)`,
`PortDef::conserving_marker(id, dimension)`, and
`PortDef::scalar_physical(id, domain)`. The accessors are `payload()`,
`signal_contract() -> Option<(SignalDirection, DimExponents)>`,
`marker_dimension() -> Option<DimExponents>`, and
`physical_domain() -> Option<Id<kinds::Domain>>`.

The old `PortKind`, `PortDef::new`, `kind()`, and universal `dimension()` API is
removed in this pre-alpha migration. Callers must match `PortPayload`; physical
across/through dimensions are resolved through the validated Domain and are
never returned as one ambiguous Port dimension.

### Across and Through are distinct symbols

The expression vocabulary gains the logical symbols:

```text
Across(port)
Through(port)
```

An unqualified `Port(port)` symbol remains the scalar value of a causal signal
Port or, solely for v1 compatibility, the saved scalar value of a
`ConservingMarker`. Whole-model validation rejects `Across` or `Through` on a
signal or marker Port and rejects an unqualified reference to a scalar physical
Port. In an ordinary Relation DAG, every `Across` or `Through` reference must be
matched by that Relation's ordinary `DependsOn` edge to the Port.

The source projection is explicit and does not add a second meaning:

```text
domain electrical = scalar_physical(
  across = kg * m ^ 2 / (s ^ 3 * A),
  through = A
);
port terminal: conserving on electrical;
across(terminal)
through(terminal)
```

The compiler resolves the Domain name to its nominal typed ID and emits the
same Domain, Port, symbol, ownership, dependency, and Connection contracts as
the Rust transaction API. An unqualified physical Port or an accessor applied
to a signal or marker Port fails during typed source lowering.

Every scalar physical Port is the target of exactly one `HasPort` edge from one
owning Relation. `Through(port)` is positive from the junction into that
Relation. Constitutive equations use the same orientation; reversing a
component's displayed direction does not change the canonical convention.
Every owning or referencing Relation has one continuous Activation in this
slice.

### One explicit scalar physical Connection is one flat junction

A scalar physical conserving Connection contains at least two Ports. Every
member must:

- be a scalar physical Port;
- name the exact same physical Domain ID;
- have one owning Relation; and
- belong to no other Connection.

Every scalar physical Port must belong to exactly one Connection. Unconnected
Ports do not acquire an implicit zero-through equation. A model-root physical
boundary is not admitted; a reference such as electrical ground is an explicit
Relation and Port in an ordinary junction.

Connections are explicit N-ary sets, not pairwise edges merged by transitive
closure. Overlapping pairwise nets therefore fail as duplicate membership.
Component elaboration may later produce an N-ary set, but hierarchy and
inside/outside orientation are not part of this flat contract.

### One closed scalar physical subsystem

Phase 1 composes one closed physical subsystem, not every Relation in a
`KernelProgram`. Subsystems are the connected components of three typed sets:
conserving Connections `C`, scalar physical Ports `P`, and participating
Relations `R`. Starting from one scalar physical Connection, closure repeats
until stable:

1. Add to `P` every Port in the `Connects` membership of each Connection in
   `C`.
2. Add to `R` the unique `HasPort` owner of every Port in `P`, and every
   Relation whose ordinary DAG references a Port in `P` through `Across` or
   `Through` and the matching `DependsOn` edge.
3. Add to `P` every scalar physical Port owned or referenced through
   `Across` or `Through` by a Relation in `R`.
4. Add to `C` the unique conserving Connection containing every Port in `P`.

The subsystem identity is the lowest canonical Connection ID in `C`.
Components are enumerated by that identity. Missing or duplicate ownership or
membership, a non-physical Port encountered by this closure, or disagreement
between symbols and `DependsOn` fails before composition.

A participating Relation admitted to the Phase 1 static composition is
deliberately algebraic and physical-only. Its DAG may contain dimensioned
constants, scalar arithmetic and dimension-valid unary math, `Across` or
`Through` for Ports in `P`, `Parameter` references, and `Time`. Parameters and
Time are known inputs at the evaluation instant; they are not unknown slots.
Parameter references retain the ordinary matching `DependsOn` requirement.

The Phase 1 static-composition boundary rejects `Field`, `Derivative`, `Pre`,
`Next`, unqualified causal `Port`, or an `Across`/`Through` reference outside
the closed subsystem. It also rejects spatial coordinates or operators,
`AppliesOn`, a causal or marker Port owned by a participating Relation, and any
Activation other than exactly one continuous Activation. Thus a physical
Relation cannot silently enter the affine lowerer as a mixed signal, hybrid,
or spatial Relation. Whole-model admission may accept the strictly wider RFC
0031 reference profile; that profile remains a separate execution contract.

An ordinary Relation with no ownership of or dependency on a Port in `P`
remains outside this subsystem. It may coexist in the same `KernelProgram`, but
Phase 1 neither executes it nor claims a coupled mixed-model solve. Sharing a
known Parameter alone does not join two subsystems.

### Deterministic junction residuals

For a validated Connection with Ports sorted by canonical Port ID,

```text
p[0] < p[1] < ... < p[n - 1]
```

the lowest-ID Port is the across anchor. The normative residual roots, in
order, are:

```text
Across(p[i]) - Across(p[0]) = 0    for i = 1, ..., n - 1
Through(p[0]) + ... + Through(p[n - 1]) = 0
```

The through sum is a left-associated fold in sorted Port order. Every term has
a positive coefficient because each `Through` symbol already points from the
junction into its owning Relation. A source delivering current consequently
has a negative through value at its positive terminal; no source-specific sign
rule is added.

The validated semantic program owns this exact representation:

```text
ComposedResidualSystem {
    subsystem: ScalarPhysicalSubsystemId,
    unknowns: [PhysicalUnknown],
    parameters: [Id<kinds::Parameter>],
    uses_time: bool,
    relations: [RelationResidual { relation, dag }],
    junctions: [JunctionResidual { connection, dag }],
}
```

`relations` contains only `R`, sorted by canonical Relation ID, and each group
retains the root order already stored in its Relation DAG. `junctions` contains
only `C`, sorted by canonical Connection ID, and each group contains the
normative across roots followed by its through root as specified above. The
global residual order is all participating Relation groups first, then all
Junction groups, preserving the root order inside every group.

A `JunctionResidual` has no independent Activation. It is continuously active
because Phase 1 admits only continuously activated Relations in its member
physical network.

`unknowns` contains only `Across` and `Through` for `P`, ordered by canonical
Port ID with `Across(port)` immediately followed by `Through(port)`.
`parameters` is the sorted unique set referenced by `R`; `uses_time` records
whether any Relation in `R` reads the known model time.

Each `JunctionResidual` is keyed and owned by its Connection. Its DAG may
reference exactly the Ports in that Connection's sorted `Connects` membership;
it does not use or invent `DependsOn` edges. This is distinct from the
Relation-to-Port dependency rule above.

`KernelProgram` materializes one immutable `ComposedResidualSystem` for a
selected closed subsystem. It does not mutate the model by inserting hidden
Relation or Field nodes. The reference DAG evaluator and scalar Operator IR
consume that same composed system and slot mapping; neither path may rebuild,
regroup, or reorder junction equations.

The generated equations are ordinary implicit residuals. Solver selection,
structural rank analysis, scaling, and internal sparse permutations remain
realization concerns. An adapter must restore the canonical slots and residual
order above; an internal permutation cannot change model meaning.

### Wire v2 is required

Scalar physical semantics require both:

- a new model identity, `eqiora.model-envelope/v2`; and
- a new transaction identity, `eqiora.model-transaction-envelope/v2`.

Their closed schemas must encode the scalar physical Domain payload, exact
Port-to-Domain identity, and the two new symbol variants. Connection membership
remains explicit and canonically ordered. Unknown variants and locally
wrong-kind IDs fail closed.

The two wire identities have deliberately different admission boundaries.
`ModelEnvelopeV2` is a complete model identity: it resolves every internal ID
and rejects dangling or wrong-kind references before reconstructing its private
graph. `ModelTransactionEnvelopeV2` is an ordered edit identity. Its decoder
validates the closed operation grammar, local ID kinds, canonical sets, and
resource budgets, but a reference may resolve against the selected store
revision or a later operation. Decoding or committing an edit is therefore not
a Semantic Model admission claim. A consumer atomically commits to a candidate
revision and must construct `KernelProgram` from that snapshot before exposing
the candidate as a valid model; store-dependent dangling or wrong-kind
references fail at that boundary.

V2 admits every semantic value admitted by v1, including
`ConservingMarker`, and additionally admits `ScalarPhysical` Domains and Ports
plus `Across` and `Through`. A v2 envelope containing only a conserving marker
does not acquire physical meaning or execution.

Model and transaction v1 keep their exact accepted values, bytes, digests, and
meaning. New fields or enum tags must not be added under either v1 schema
identity. Asking a v1 encoder to serialize any v2-only semantic value fails
closed. A v2 decoder may reconstruct the new typed model only through the same
validated constructors and transaction boundary as native authoring.

The v1 model and transaction codecs map `Signal` and `ConservingMarker` back to
their existing v1 Port tag and dimension fields. They reject
`ScalarPhysical` Domains or Ports and `Across` or `Through` before producing
canonical bytes or a digest.

Encoder version is selected by type, never by an implicit "latest" default:
`ModelEnvelopeV1::from_program` or `ModelEnvelopeV2::from_program`, and
`ModelTransactionEnvelopeV1::from_transaction` or
`ModelTransactionEnvelopeV2::from_transaction`. A facade that later provides a
single entry point must require an explicit version argument.

Model identity and edit identity remain distinct. `ModelEnvelopeV2`
canonicalizes graph sets, so two admitted transactions that produce the same
final program with the same IDs have identical canonical model bytes and
digest. `ModelTransactionEnvelopeV2` instead preserves the exact operation
order as ordered edit identity; reordering its operations normally changes its
canonical bytes and digest even when a later commit produces the same final
model. Canonicalization may sort only explicitly set-valued members inside one
operation, never the operation sequence itself.

Explicitly decoding a v1 envelope and encoding the same admitted semantic
values as v2 is re-enveloping, not identity-preserving migration. The schema
tag, canonical bytes, and domain-separated digest all change even when the
model ULID and semantic graph values do not.

## Prior art and deliberate differences

The [Modelica 3.7 connection
specification](https://specification.modelica.org/maint/3.7/connectors-and-connections.html)
generates equality equations for potential variables and signed zero-sum
equations for flow variables. Eqiora adopts that mathematical core.

The first Eqiora slice deliberately does less. It has one scalar across/through
pair, explicit N-ary junctions, and one flat orientation convention. It does
not merge pairwise connection sets, infer zero flow for an unconnected Port,
or implement hierarchical inside/outside signs, stream variables, expandable
connectors, or overconstrained connection graphs. Those differences keep the
first canonical form independent of RFC 0021 hierarchy and make every emitted
equation directly falsifiable.

## Alternatives considered

| Formulation | Mathematical fit | Runtime cost | Implementation and compatibility | Decision |
|---|---|---:|---|---|
| One unqualified scalar per conserving Port | Cannot represent across and through simultaneously | Low | Small, but preserves the current ambiguity | Rejected |
| Generated Field nodes and hidden Relations | Expressible as residuals, but generated graph identity becomes model meaning | Low | Adds mutation, provenance, and wire bookkeeping | Rejected |
| Pairwise connect statements plus union-find | Natural with hierarchy and familiar from Modelica | Near-linear elaboration | Requires merge, inside/outside, and boundary rules before the flat slice | Deferred |
| Native `Across`/`Through` symbols plus explicit N-ary residuals | Direct expression of equality and conservation | Linear in junction size | Requires wire v2, but keeps one residual and operator path | Adopted |

## Compatibility and migration

This RFC makes a deliberate pre-alpha Rust API migration and introduces new
model and transaction wire types. Existing v1 artifacts remain valid under v1
and remain non-executable for conserving networks. They are not rewritten or
assigned a guessed through dimension. V2 can represent the admitted v1 values,
but explicit re-enveloping changes bytes, digest, and schema identity as stated
above.

Migration is explicit: create one scalar physical Domain, bind each conserving
Port to its exact ID, replace every unqualified physical Port reference with
`Across` or `Through`, and emit a v2 transaction and model artifact. Source,
native Rust, Python, and Studio authoring must all call the same constructor and
transaction contracts once their separate surfaces graduate.

## Verification

The first verified model is a flat 12 V source with parallel 2 ohm and 4 ohm
resistors and an explicit ground. Its high junction has three Ports; its ground
junction has four. The case must prove, within declared solution and residual
tolerances:

- high potential is 12 V;
- resistor currents are 6 A and 3 A into their positive terminals;
- source current at its positive terminal is -9 A;
- both signed through sums are zero;
- reference-DAG and scalar-operator executions accept the same solution and
  residual roots;
- permuting transaction and Connection-member insertion with fixed IDs leaves
  deterministic junction ordering and the final `ModelEnvelopeV2` canonical
  bytes and digest unchanged; the ordered `ModelTransactionEnvelopeV2`
  identity is expected to differ when operation order differs; and
- v2 model and transaction round trips reconstruct the exact Domain and Port
  identities;
- adding an unrelated ordinary Relation leaves the selected physical closure,
  composed residual order, and accepted physical solution unchanged, while the
  whole-model artifact identity correctly changes.

Falsifying negative cases 1--8 must reject before complete Semantic Model
admission and therefore before execution. A transaction-only fixture may atomically
commit a locally typed candidate revision first; constructing `KernelProgram`
from that candidate must then fail, and the candidate must never be exposed as
a valid Semantic Model:

1. `Across` or `Through` applied to a signal Port.
2. An unqualified expression reference to a scalar physical Port.
3. Equal dimensions but different physical Domain IDs in one junction.
4. Duplicate Connection membership or an unconnected scalar physical Port.
5. A Port without exactly one owning Relation, including a model-root physical
   boundary.
6. Periodic, event, or guard activation of a Relation in the physical network.
7. Unnormalized hierarchical or overlapping pairwise nets presented directly
   to the Phase 1 flat Kernel/Transaction boundary.
8. Any v2-only value presented to a v1 model or transaction encoder.
9. The Phase 1 static composition and affine lowerer must reject a
   participating physical Relation containing a Field, Derivative, `Pre`,
   `Next`, causal Port, or spatial expression. Non-continuous Activation and
   spatial scope remain invalid at whole-model admission. RFC 0031 separately
   admits only its bounded continuous Field/Derivative/causal vocabulary to
   the reference interpreter.

Capability gates graduate independently:

- **C** becomes ✅ only after the typed kernel payload, validation, and both v2
  wires are implemented and round-trip under their versioned contracts.
- **X** becomes ✅ only after a real end-to-end path consumes the composed
  residual system and solves a scalar physical network.
- **V** becomes ✅ only after the exact registered case under `verify/` passes
  reproducibly with its declared positive and negative evidence.

The typed kernel payload, closed-subsystem validation and composition, and both
explicit v2 wires implement the **C** gate. Exact bound-affine admission,
canonical `General`-CSR handoff, serial faer execution, and original-DAG
reacceptance implement **X**. The registered
[`electrical.parallel-dc-network`](../verify/electrical/parallel-dc-network/README.md)
case, together with fail-closed semantic regressions, implements **V** for this
bounded flat affine slice. None of these gates widens the nonclaims below.

## Security, safety, and governance

Model and transaction decoders treat IDs, expression nodes, Connection
members, and dimensions as untrusted data. Byte, node, root, member, and
transaction-operation limits apply before artifact admission or graph
mutation; the byte and nesting caps apply before deserialization. Checked
arithmetic bounds aggregate counts and equation generation. A model envelope
rejects dangling IDs locally. A transaction rejects locally malformed IDs and
the resulting complete `KernelProgram` rejects store-dependent dangling IDs.
Newer variants are rejected rather than ignored.

Changing the through orientation, anchor choice, root order, nominal-domain
rule, or v2 canonical encoding changes model meaning or identity and requires a
new RFC. A solver adapter cannot override these choices.

## Nonclaims

This RFC does not define vector or tensor connectors, aggregate connectors,
stream variables, expandable connectors, implicit ground, hierarchy
inside/outside signs, root physical boundaries, overconstrained frames,
switched topology, fluid transport, domain-wide equations, overlapping
pairwise-net source lowering, reusable component libraries, or a
production DAE solver. Phase 1 also does not define coupled execution between
a physical subsystem and signal, state, hybrid, or spatial Relations;
unrelated coexistence in one program is isolation evidence, not mixed-model
support. [RFC 0031](0031-joint-physical-periodic-reference-execution.md) is a
separate, explicitly bounded reference-execution admission profile that reuses
this RFC's nominal topology, physical unknown order, sign convention, and
junction DAGs without widening the static affine lowerer.
The later
[hierarchical connection-set contract](0033-hierarchical-conserving-connection-sets.md)
owns compiler source-fragment union; it does not retroactively broaden this
RFC's flat evidence.

## Unresolved questions

- Structural-rank and scaling diagnostics beyond the first square DC network.
- Production transient physical lowering beyond the RFC 0031 reference oracle.
