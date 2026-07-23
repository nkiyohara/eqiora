# RFC 0033: Hierarchical conserving connection sets

- Status: Accepted and implemented for nominal scalar physical Ports
- Authors: Eqiora contributors
- Created: 2026-07-19
- Depends on: [RFC 0021](0021-component-hierarchy-and-instantiation.md),
  [RFC 0024](0024-scalar-conserving-connection-semantics.md),
  [RFC 0030](0030-symbolic-package-definition-validation.md)

## Summary

A conserving `connect` declaration contributes a typed fragment of an
equivalence relation; after hierarchy expansion the compiler normalizes all
fragments once and emits one canonical flat Connection for each maximal set.

## Implementation status

The accepted bounded slice implements:

- the pure bounded identity-parametric fragment normalizer;
- definition-local source identity normalization and occurrence-free public
  boundary partitions;
- selected-hierarchy and direct flat-source occurrence normalization into flat
  Kernel Connections;
- removal of ownerless public exposure aliases;
- canonical Connection ownership from the least common ancestor of
  contributing fragment-owner occurrence paths; and
- deterministic complete multi-origin provenance for a merged Connection.

The local contract is registered by
[`language.hierarchical-connection-sets`](../verify/language/hierarchical-connection-sets/README.md):
hierarchical N-ary/chain canonical equivalence, direct-flat exact maximal-set
membership, 84 ordering permutations, two explicit forwarding levels,
independent occurrence partitions and explicit join, negative alias semantics,
duplicate-fragment provenance, independent resource limits, type separation,
and owner/membership falsifiers.

The bounded exact-package contract is registered by
[`packages.hierarchical-physical-boundary`](../verify/packages/hierarchical-physical-boundary/README.md).
It extends dependency-defined forwarding Ports from a root package, requires
N-ary and partitioned source forms to share package semantic identity,
root-LCA Connection identity, canonical Model v2, ordered affine residual
system, and analytic solution, while preserving distinct source and
compilation lineage. It also checks complete package-qualified fragment
origins and rejects an unclosed selected root before Model exposure.

[`language.component-elaboration`](../verify/language/component-elaboration/README.md)
owns the explicit-flat circuit comparison. A complete identity bijection must
make the normalized hierarchy and explicit N-ary flat model byte-identical;
the proven hierarchy is then solved once and checked against the analytic DC
oracle and the original residual DAGs.

This RFC is intentionally topology-only. An eliminated exposure name is absent
from ordinary entity symbols and is never aliased to a Connection or retained
Port. Across-value and aggregate-through result projections, including their
artifact identity and replay rules, belong to the separate typed
[physical-exposure projection contract](0036-physical-exposure-projection-artifacts.md)
and are not acceptance criteria here. The accepted scope is therefore a
compiler topology contract, not a field-valued physical-interface claim.

## Motivation

The current scalar physical contract deliberately treats every explicit
N-ary `connect` declaration as one flat junction. A Port may occur in exactly
one declaration. That made the first acausal execution slice small and
falsifiable, but it prevents a reusable Component from exposing an internal
physical net and letting its parent extend that net.

For example, the public terminal below is the intentional boundary between an
internal component and its environment:

```text
component Wrapper {
  public port terminal: conserving on Pin;
  instance leaf: Leaf;
  connect conserving terminal, leaf.terminal;
}

model Main {
  instance wrapper: Wrapper;
  instance load: Load;
  connect conserving wrapper.terminal, load.terminal;
}
```

The two declarations do not create two physical junctions. They state two
fragments of one maximal junction. Rejecting the repeated public terminal
makes hierarchy semantically weaker than an equivalent flat model. Preserving
both declarations as flat Connections instead violates the Semantic Kernel's
correct invariant that one physical Port belongs to exactly one canonical
Connection.

This RFC completes elaboration without adding hierarchy to canonical model
meaning and without turning the runtime into a graph-normalization engine.

## Proposed design

### Source declarations are typed fragments

The source grammar does not change. Its interpretation becomes explicit:

```text
connect conserving p0, p1, ...;
```

contributes one undirected `ConnectionFragment`. Every fragment must contain
at least two distinct visible conserving Ports. The existing shared Port
contract validates one fragment before it can participate in normalization:

- all members belong to the same conserving family;
- scalar physical members have the exact same nominal Connector identity;
- signal Ports never enter a conserving fragment.

Signal Connections retain their ordered output and unordered input-set
meaning. Their source/sink, direction, fan-out, and duplicate-use rules are
unchanged and are not implemented through the conserving normalizer.

The normalization algebra is Port-family agnostic: it accepts only already
typed endpoint identities and never reinterprets their payloads. This RFC's
admission and lowering slice is limited to `ScalarPhysical` Ports. Structural
`ConservingMarker` values keep their RFC 0024 behavior and may not overlap;
future field-valued physical Ports may reuse the same algebra only after their
own compatibility contract is accepted.

### One pure bounded normalizer

`eqiora-compiler` owns an identity-parametric, allocation-bounded normalizer:

```text
ConnectionFragment<I> {
  members: sorted distinct I
}

CanonicalConnectionSet<I> {
  members: sorted distinct I
}

ConnectionSetNormalization<I> {
  sets: sorted CanonicalConnectionSet<I>
  fragment_sets: input fragment -> set index
}
```

The normalizer assigns exact endpoint identities in ascending order and uses
an iterative deterministic disjoint-set union. Its output satisfies:

- sets are pairwise disjoint;
- every input membership occurs in exactly one output set;
- members are in ascending exact identity order;
- sets are ordered by their minimum member and then full member sequence; and
- fragment order, member order, source traversal, and map insertion cannot
  affect the output.

The topology result contains only the member partition. The separately
returned fragment-to-set array follows input order and is a non-semantic
witness through which the caller aggregates provenance. Input indices and
source origins never enter set or Connection identity. Equivalent duplicate
fragments are idempotent topology claims while their distinct origins are all
retained in the caller-owned sidecar. Exact duplicates may be reported by a
non-blocking authoring lint, but compiler admission cannot give an idempotent
mathematical claim a second meaning.

The normalizer is compiler-owned because fragments and their origins do not
exist in the Semantic Kernel. `eqiora-schema` continues to own reusable scalar
Port compatibility and final one-owner/one-canonical-set closure predicates;
it does not acquire a DSU or a source-fragment witness.

Limits independently bound fragment count, members per fragment, total
fragment memberships, distinct endpoints, normalized set count, and members
per set. Identity construction and provenance independently bound canonical
identity bytes, origins, and source paths. Checked arithmetic and fallible
reservation occur before mutation or large allocation. The algorithm is
iterative with near-linear DSU complexity; no model recursion is introduced.

### Definition proofs retain boundary partitions

Occurrence-free body checking records each validated conserving fragment
separately. It no longer treats a second fragment touching one physical Port
as a second final membership.

The scalar physical definition summary becomes a proof of:

- the independent zero-or-one Relation owner of each public Port;
- the exact partition induced over public Ports by internal fragments;
- a bounded retained-endpoint count for each boundary class;
- the open, explicitly exposed, or internally closed status of each class;
- closure of every private local endpoint; and
- explicit treatment of every child public endpoint.

A child obligation may be consumed by a parent fragment or explicitly
forwarded through a parent public Port in the same normalized class. Merely
leaving a child endpoint untouched is still an invalid implicit grandchild
re-export: the source language cannot name such an interface through the
parent.

Two instances of one Component instantiate independent partitions. Exact
package aliases resolve to the same nominal Connector identity before their
fragments are compared.

Relation ownership remains a separate linear resource. Repeated `across` and
`through` references inside one Relation constitute one owner; two distinct
Relations owning one retained endpoint remain invalid.

### Normalize once after occurrence expansion

Hierarchy expansion emits typed Ports, Relations, signal Connections, and
conserving fragments into a staging blueprint. It does not assign a Kernel
Connection identity to a fragment.

After every selected occurrence has expanded and before graph identities are
sealed, one pass:

1. unions all conserving fragments by exact flattened Port identity;
2. checks the occurrence-level owner and closure proofs;
3. classifies public Ports with no unique validated `HasPort` owner as
   transparent exposure aliases;
4. removes those aliases from the canonical member set;
5. rejects a set with fewer than two retained physical endpoints;
6. derives one canonical Connection identity from the retained member set;
7. aggregates every source origin contributing to the set; and
8. emits one ordinary flat N-ary Connection.

Direct source Models use the same pre-kernel normalizer. This requires parsed
flat source to stage typed endpoint identities and fragments before
`lower_model`; it may no longer request one Connection ID per declaration on
the way into the Kernel. There is not one meaning for flat source and another
for hierarchical or package source.
Both local and exact-package hierarchy entry points consume the same complete
definition proof before occurrence normalization. The local path may not
silently reproduce only a subset of the owner and closure checks performed by
package compilation.
Low-level Transaction authoring remains a canonical-graph API: it supplies
already normalized Connections and the Semantic Kernel defensively rejects
overlap.

### Ownerless exposure aliases are eliminated, not projected

An ownerless public Port used only to join an internal fragment to an external
fragment is a transparent exposure alias. Keeping it as an ordinary flat Port
would invent an unowned across/through unknown; generating an alias Relation
would invent model equations. Both are rejected.

The compiler removes the alias from the canonical Relation network. Its source
name is absent from ordinary entity symbols and does not map to the canonical
Connection or to an arbitrary retained Port. The final Connection provenance
still contains every complete fragment origin that contributed to the set.

This topology RFC assigns no result-query meaning to the eliminated name. A
real physical-boundary result interface must type the boundary class, common
across quantity, aggregate-through orientation, frame, support, and artifact
identity together. That contract belongs to
[RFC 0036](0036-physical-exposure-projection-artifacts.md). It is not
approximated here by exposing a raw Connection ID.

A public Port with a unique validated `HasPort` owner remains a physical
endpoint and an ordinary entity symbol; it is not a transparent alias.

### Canonical identity has one occurrence owner

A canonical Connection key contains:

```text
root semantic namespace
+ least common ancestor of contributing fragment-owner occurrence paths
+ the existing reserved net declaration path at that occurrence
+ sorted retained full Port identities
```

The v1 key does not add a separate Connection-semantics field. The exact Port
identities and admitted typed fragment determine the family, while adding a
new encoded field would change every existing Connection identity. A later
key-version migration may make that redundancy explicit, but cannot be
smuggled into this compatibility-preserving RFC.

The occurrence scope is derived from semantic fragment ownership, never from a
source span or encounter order. For every existing single-fragment Connection,
its owner occurrence and reserved net path are exactly the scope already used
by its current `ElaborationKey`, preserving its full and projected identity.
Fragments in one scope retain that scope after union. Fragments joining a set
across hierarchy boundaries acquire their least common owner occurrence. A
well-formed cross-boundary set must contain an explicit contributing fragment
owned by that least-common scope; otherwise compilation fails instead of
inventing an owner path. The individual declaration path, source span, DSU
root, and encounter index never enter the key.

This deliberately preserves Eqiora's existing occurrence identity discipline:
splitting or joining statements inside one owner scope does not change
Connection identity, but reorganizing the Component hierarchy may change the
compilation namespace, occurrence paths, or canonical Connection owner even
when a separately normalized residual-system comparison finds the same
mathematics. Removing hierarchy from every entity identity would be a broader
identity migration and is not smuggled into connection normalization.

The compiler's structural source identity normalizes conserving fragments
inside each Component and Model before encoding connection records. This
source-level partition is definition-local; it cannot and does not perform
occurrence or cross-package union. Selected compilation performs the second,
occurrence-level normalization over the complete exact closure.

This is an intentional pre-release correction to the observable local source
identity for one class of previously rejected documents:

- every previously valid non-overlapping declaration encodes byte-for-byte as
  before;
- newly admitted overlapping fragments encode as their equivalent maximal
  N-ary record;
- `LocalSourceIdentity::from_document` for an overlapping document may differ
  from the old value even though that document could not pass semantic
  admission before this RFC; and
- every previously admitted package, source, Model, and artifact golden remains
  unchanged.

Consequently an N-ary declaration and an equivalent partitioned declaration in
the same definition produce the same compilation namespace, Port identities,
Connection identity, canonical Model bytes, and digest. A regression test must
prove both the new equivalence and all old golden values. Cross-hierarchy
refactoring retains the established weaker comparison after occurrence
identity normalization.

This structural normalization does not claim algebraic equivalence of
Relation expressions. The boundary in RFC 0022 remains unchanged.

### Provenance preserves every origin

One canonical Connection can have many source contributors. Provenance is
therefore generalized from one origin to a bounded ordered collection:

```text
ElaborationSourceOrigin {
  definition_span
  instance_span
  binding_spans
}

ElaborationProvenance {
  origins: sorted distinct ElaborationSourceOrigin
}
```

Existing single-origin accessors may return the fields of the same first
complete origin for source compatibility, while new APIs expose the complete
collection. Definition and instance spans may never be minimized
independently. Origins sort by definition span, instance span, then binding
spans and are deduplicated.
Limits bound origins per identity, binding spans per origin, and total path
bytes. Source locations never enter semantic identity.

No separate provenance or symbol-sidecar entry is created for an eliminated
alias. Its contribution remains recoverable only as a source origin of the
canonical Connection. A later result-projection contract may define a durable
boundary artifact, but it must introduce and seal its own typed identity rather
than reinterpret this topology provenance as a result projection.

### The Semantic Kernel remains the normal form

No node or edge kind changes. The Kernel continues to receive explicit
Connections with canonical member sets. Whole-model validation retains the
invariant that each scalar physical Port has exactly one Relation owner and
one canonical Connection membership. Overlapping hand-authored Kernel
Connections fail as an unnormalized graph.

Junction residual construction is unchanged. It sees one sorted maximal set,
chooses the same across anchor, and emits the same equality and through-balance
roots as an equivalent explicit N-ary flat model.

## Prior art and deliberate differences

The [Modelica 3.7 connection-set
semantics](https://specification.modelica.org/maint/3.7/connectors-and-connections.html)
also forms connection sets transitively before generating potential-equality
and flow-balance equations. Eqiora adopts maximal-set normalization and keeps
it before equation generation.

Eqiora does not adopt Modelica's complete inside/outside, expandable,
overconstrained, or stream-connector semantics in this RFC. Scalar
`Through(port)` retains RFC 0024's orientation from a junction into its owning
Relation. This RFC normalizes the canonical junction topology only; it does not
define a hierarchical boundary-flow query or insert a source-position-dependent
sign into the canonical junction equation.

## Alternatives considered

| Alternative | Mathematical fit | Runtime cost | Compatibility and provenance | Decision |
|---|---|---:|---|---|
| Reject overlap at every hierarchy level | Makes public physical composition impossible | Low | Current bounded behavior | Rejected |
| Preserve every source fragment as a Kernel Connection | Confuses generators with equivalence classes | Low | Violates one-membership closure | Rejected |
| Normalize at runtime | Mathematically possible | Repeated near-linear work | Executors can disagree; late diagnostics | Rejected |
| Generate alias Relations and unknown Ports | Adds artificial equations and variables | Higher systems | Pollutes Model identity and AD | Rejected |
| Compile-time maximal sets plus alias elimination | Matches conserving topology | One bounded pass | Canonical graph and complete Connection provenance remain | Adopted |

## Compatibility and migration

The source grammar, Semantic Kernel node kinds, model/transaction wire v2,
and scalar junction equations do not change. Existing accepted source and
artifact bytes remain fixed.

Source previously rejected only because two valid conserving fragments
overlapped becomes valid and lowers to the same graph as one equivalent N-ary
declaration. Source with a nominal mismatch, duplicate member inside one
fragment, unresolved or private Port, signal/conserving mix, missing Relation
owner, or unclosed implicit child interface remains invalid.

The internal provenance value becomes multi-origin. It has no durable public
wire today. A future provenance artifact must version that sidecar explicitly;
this RFC does not smuggle it into Model identity.

Existing `ModelSymbols` entries that still denote graph entities retain their
IDs. A newly eliminated alias is absent from that entity-only map. It cannot
preserve a `RawId` by pointing at a Connection or unrelated retained Port,
because either choice would be a semantic lie. This RFC introduces no public
projection variant.

The occurrence-free physical membership summary introduced by RFC 0030 is
superseded by the partition proof in this RFC. Repeated fragments may extend
one normalized membership, and an explicitly connected parent public Port may
forward a child interface. The immediate parent must still handle the child
explicitly. RFC 0030's Relation-owner proof and flat semantic defense remain
unchanged, and an owner slot can never be refilled.

RFC 0024's statement that flat Connections are explicit non-overlapping N-ary
sets remains true for the Kernel normal form. Its source-level statement that
pairwise declarations are not transitively merged is superseded by this RFC:
source `connect` declarations are fragments and only their maximal sets become
flat Connections.

## Verification

1. `connect a, b, c` and `connect a, b; connect b, c` produce identical source
   identity, Port IDs, Connection ID, canonical Model bytes, digest, residual
   order, and numerical result.
2. Fragment order, member order, declaration order, file input order,
   dependency order, and internal map insertion do not alter the result.
3. Two disjoint nets remain two canonical Connections.
4. A child-internal fragment extended by a parent fragment becomes one set;
   the same proof crosses one exact package boundary.
5. Two levels of explicit public forwarding work, while an untouched child
   obligation still fails without a fabricated grandchild path.
6. Two instances of one Component remain disjoint until a parent fragment
   explicitly joins them.
7. Ownerless exposure aliases disappear from the flat graph and ordinary
   entity symbols; they never alias a Connection or retained Port, and an
   alias-only final set fails.
8. A parent Relation using a public Port retains that endpoint, while a second
   owner fails.
9. Same-dimension but nominally distinct Connectors fail before union and
   before graph mutation.
10. Signal fan-out, direction, and positional source identity remain on their
    separate directed path; diagnostic codes and asserted message fragments
    retain their stable contract.
11. Every contributing definition, instance, and binding span appears once in
    deterministic provenance, and every legacy accessor returns fields from
    one complete origin.
12. Fragment, membership, endpoint, set, identity-byte, origin, and path-byte
    limits fail closed before graph mutation.
13. A hand-built flat graph with overlapping canonical Connections is rejected
    by semantic validation.
14. The normalized hierarchy agrees with an explicit N-ary flat circuit after
    the established hierarchy identity normalization, including residual
    dimensions, independent residual acceptance, analytic solution, and
    balance evidence. Exact Model bytes are required only when connection
    partition is the sole source difference.
15. All existing source, package, hierarchy, flat physical, Model v2, and
    transaction v2 golden identities remain unchanged.
16. Exact duplicate fragments are topology-idempotent, preserve all origins,
    and preserve the same residual system. Identity is also unchanged when the
    duplicates have the same owner scope; duplicate members inside one fragment
    remain a source error.

The acceptance map is explicit. The first three rows are registered consumers
of `hierarchical-conserving-connection-sets-v1`. The kit is a private test
contract, not a new public runtime or package API.

| Verification items | Evidence owner |
|---|---|
| 1--7, 9, 12, 16 | `language.hierarchical-connection-sets`: exact N-ary/chain artifacts, 84 ordering permutations, disjoint and explicitly joined occurrences, two forwarding levels, alias elimination, nominal separation, independent topology/identity/provenance limits, duplicate-fragment origins, and fail-closed source fixtures |
| 1, 4, 7, 8, 11, 14 | `packages.hierarchical-physical-boundary`: exact dependency forwarding, root-LCA identity, complete package-qualified origins and legacy accessors, N-ary/partitioned package and affine-system equality, analytic solution, and unclosed-root rejection |
| 14 | `language.component-elaboration`: complete hierarchy-to-explicit-flat identity bijection, byte-identical normalized Model v2 artifacts, one analytic solve, and original-DAG balance acceptance |
| 2 | compiler `connection_sets`, `source_identity`, hierarchy, and provenance unit tests plus `packages.offline-model-package`: fragment/member/map/declaration/file/dependency ordering and exact replay invariance |
| 8 | local and exact-package valid owners plus the `invalid-double-owner` falsifier |
| 10 | compiler source-identity legacy-byte and directed-signal tests plus the local signal-fan-out admission check |
| 11 | compiler provenance tuple-order/deduplication tests and the exact-package complete-origin assertions |
| 13 | `eqiora-sem/tests/scalar_conserving.rs`: duplicate canonical physical membership rejection |
| 15 | compiler stable source-identity and legacy-byte tests, artifact v1/v2 wire goldens, existing component/package/flat-physical cases, and the complete locally run workspace regression suite |

The map reuses the authoritative owner of dependency-order and wire-golden
claims instead of cloning those assertions into this feature case. Every
failure is observed before a compiled Model or committed graph is exposed.

## Security, safety, and governance

Normalization performs no I/O, network access, dynamic loading, or unsafe
code. Inputs are exact compiler-owned identities and typed source proofs.
Aggregate counts are bounded before DSU allocation, checked reservations are
used for bounded structures, and no graph Transaction is emitted or committed
before normalization succeeds. Parsing and bounded fragment collection are
not falsely claimed to allocate nothing.

No package gains authority through connection. Exact dependency visibility
and nominal Connector identity are resolved before fragments reach the
normalizer. Provenance aggregation does not change semantic identity or trust.

## Explicit nonclaims and future contracts

- Across-value and aggregate-through boundary results, durable boundary
  artifact identity, and result-query replay require the separate typed
  result-projection contract in
  [RFC 0036](0036-physical-exposure-projection-artifacts.md).
  They are not a missing sidecar in this proposal.
- General frame and orientation transforms, field-valued trace/flux Ports,
  stream variables, and overconstrained connection graphs require separate
  contracts and do not block this scalar topology normalization.
