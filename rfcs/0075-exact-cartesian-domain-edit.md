# RFC 0075: Exact topology-preserving Cartesian Domain edit

- Status: Accepted; bounded implementation verified in
  [`geometry.cartesian-domain-edit-3d`](../verify/geometry/cartesian-domain-edit-3d/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-24
- Depends on: [RFC 0008](0008-canonical-artifact-wire-v1.md),
  [RFC 0037](0037-version-neutral-model-artifact-reference.md),
  [RFC 0049](0049-geometry-identity-and-mesh-correspondence.md), and
  [RFC 0073](0073-structural-semantic-fingerprint.md)

## Summary

One current-v6 Semantic Model containing exactly one three-dimensional
Cartesian body may produce an immutable child Model by changing exactly one
axis interval. The body Domain identity, its oriented boundary Domain
identities, Model membership, and all incident Semantic Kernel edges remain
unchanged. Exact Model and Geometry Identity content changes.

The edit is one ordinary versioned Model transaction:

```text
RevisionIs(base)
RemoveNode(body)
DefineKernelNode(same body ID, one changed AxisBounds)
Connect(each exact incident edge in deterministic order)
```

An application-owned preview plan binds the exact base Model digest, revision,
transaction wire and digest, selected Domain and axis, old and new intervals,
and expected child Model digest. Commit replays that same wire atomically and
rejects any different base or child.

## Motivation

The bounded CAD slice can compare independently compiled geometry revisions
and explicitly associate their semantic selections. It cannot yet express the
user action that creates the successor Model. Adding that action in Studio,
Python, or a CAD adapter first would make a client-specific geometry mutation
authority. Reconstructing a Model outside the graph transaction path would
bypass the shared atomic and versioned edit boundary.

The first owner slice needs only one exact metric edit. It does not need a new
general graph operation, a new transaction generation, or a complete
parameter-driven CAD language.

## Identity decision

A Domain ID identifies one semantic region occurrence and role within a Model
lineage; it is not a digest of the region's metric extent. The ID may therefore
survive an interval edit only when all of the following remain exact:

- spatial dimension;
- Cartesian body kind;
- oriented boundary inventory and each `(axis, side)` role;
- graph incidence;
- Model membership; and
- topological interpretation.

The canonical Model digest changes because bounds are Model meaning. A
Geometry Identity digest changes because the geometry revision changed. This
is not a claim that identity survives dimension, topology, boundary-role,
multi-body, split, merge, or feature-history changes.

## Owning contract

The public application contract uses the native `AxisBounds` mathematical
object and typed Domain ID. It accepts no raw CAD face, mesh entity, source
span, UI state, or adapter object.

Preview:

1. requires the current Model wire v6;
2. finds exactly one three-dimensional `CartesianBox` body;
3. validates the typed target, axis, and non-no-op replacement interval;
4. captures every incident accepted Model edge in canonical order;
5. creates and round-trips the existing versioned transaction wire;
6. commits that wire to a cloned Graph Federation;
7. reconstructs and serializes the complete child Model; and
8. records the exact child digest in the immutable plan.

Commit first compares exact codec, base Model digest, and graph revision. It
then decodes, re-hashes, commits, validates, and serializes the transaction
again. The resulting child digest must equal the previewed digest.

This double replay is intentional. Preview proves the exact proposed result;
commit proves that the same proposal still applies to the selected immutable
base.

## Alternatives considered

### Add a generic `UpdateNode` operation

Rejected for this slice. It enlarges every transaction wire and requires a
generic definition-equality precondition before two independent consumers
exist. Remove, define with the same typed ID, and reconnect already express the
exact atomic change.

### Rebuild the target Model directly from declarations

Rejected as the mutation authority. It can serve as an independent verification
oracle, but it bypasses revision preconditions, ordered transaction identity,
and graph-store provenance when used for the product edit.

### Make Cartesian bounds Parameter expressions first

Deferred. Parameter-driven geometry is the natural later authoring model, but
it additionally owns expression typing, dependency tracking, evaluation,
constraint failure, and regeneration scheduling. Folding those questions into
the first edit would obscure the identity decision this slice must falsify.

### Treat body identity as content-addressed and mint a new Domain ID

Rejected for topology-preserving metric edits. It would unnecessarily retarget
every Field, Relation, boundary, Port, selection, and client reference even
though their semantic region roles are unchanged. Content identity remains in
the Model and Geometry Identity digests.

## Verification

The registered case must use a separately constructed target Model as its
oracle and prove:

- structural equivalence between edited and independently constructed target;
- exact changed interval and independently computed volume;
- preservation of body and boundary IDs, boundary roles, graph incidence, and
  Model membership;
- changed Model and Geometry Identity digests;
- explicit one-to-one geometry revision association and retained selections;
- rejection of stale, foreign, no-op, non-finite, reversed, wrong-target, and
  out-of-axis edits without mutation;
- rejection of a same-revision sibling with a different Model digest; and
- rejection by Geometry Identity replay when a mutant omits one required
  `BoundaryOf` edge.

The independent target and volume oracle may not call the edit implementation.

## Compatibility and migration

No Semantic Kernel node, graph operation, Model/transaction wire field, source
construct, artifact schema, or CAD adapter contract changes. The first Rust
plan/result types are transitional alpha API. Existing Model bytes and edit
transactions remain valid.

## Security and resource bounds

The transaction contains one replacement definition plus the already-bounded
incident edge set of the selected accepted Model. Existing transaction decoder
limits apply before mutation. Preview and commit operate on cloned in-memory
state and expose no partial Model.

## Nonclaims

This RFC does not claim source rewriting, Parameter-parametric geometry,
multiple simultaneous axes, dimensions other than the bounded 3D case,
multiple bodies, topology changes, curved geometry, persistent CAD naming,
feature history, constraint solving, geometry repair, mesh regeneration,
Python or Studio authoring, ALE, optimization, or shape sensitivity.
