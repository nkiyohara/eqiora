# RFC 0086: Private numerics--differentiation composition

- Status: Accepted
- Authors: Eqiora contributors
- Created: 2026-08-11
- Related RFCs and evidence: [RFC 0011](0011-implicit-differentiation-contracts.md),
  [`differentiation.materialized-direct-output`](../verify/differentiation/materialized-direct-output/README.md)

## Summary

Permit exactly one directed same-layer dependency from `eqiora-numerics` (L3)
to `eqiora-differentiation` (L3), solely for the first private Stokes E2
consumer to reuse the accepted materialized direct-output forward and adjoint
composition, without a reverse edge, public bridge, transitive registry, or
authority for a second consumer.

## Motivation

The Stokes E1 geometry, mesh, assembly, finalized saddle-point system, and
private product state already have one numerical owner in
`eqiora-numerics`. RFC 0011 separately assigns accepted relation/output
pairing, forward output sensitivity, adjoint output gradients, and total-
gradient algebra to `eqiora-differentiation`. Copying either owner into the
other crate would create a second invariant-bearing implementation.

The accepted materialized direct-output integration, commit
`dfa4d9693a1278f28d4f0de1635cfbc04a916ff2`, supplied the previously missing
generic composition. The registered
`differentiation.materialized-direct-output` reference binds an accepted
relation/output pair to its canonical CSR coefficient source, keeps the
primal and derivative right-hand sides distinct, and executes the existing
normal and transposed output paths through direct sparse LU before relation
JVP/VJP replay. That commit is an ancestor of this draft's exact proposal base,
`c62cd28ce21881fac5e4ffb0fe35b871d36fb802`.

An earlier edge-only design was correctly rejected at its inspected revision:
the generic differentiation path then hid the canonical materialization from
the required direct solver. This RFC does not relabel that historical review.
It proposes the same ownership-aligned edge only after the accepted RFC 0011
successor removed the named executable blocker.

The current manifests contain no `eqiora-numerics` to
`eqiora-differentiation` edge and no reverse edge. The latter crate's direct
normal workspace dependencies are `eqiora-core`, `eqiora-ir`, and
`eqiora-solver`; because `eqiora-ir` normally depends on `eqiora-schema`, its
complete normal workspace closure is `eqiora-core`, `eqiora-ir`,
`eqiora-schema`, and `eqiora-solver`. Neither that closure nor the current
`eqiora-numerics` normal closure contains the other sibling in a reverse
direction, so the proposed direction creates no cycle at the proposal base.
The layer checker has no matching exception and therefore fails closed until
an accepted numbered version of this RFC is cited by the exact allowlist pair.

The separate
`Dunavant-degree-6-12-point-normalized-area-v1` Realization identity remains
an independent E2 STOP. Its accepted design is neither incorporated nor
authorized here. This RFC adds no quadrature identity, wire generation,
scientific formulation, expected value, tolerance, fixture, or Stokes E2
capability claim.

## Proposed design

### Exact permission

The RFC-only lifecycle first reviews and accepts this draft, assigns its
permanent RFC number, and merges the numbered RFC without any dependency,
source, implementation, or evidence change. That accepted numbered RFC must
predate the corrected E2 outcome contract, which binds its exact path,
revision, and content before source work starts. Only then may the later
Stokes E2 capability closure add exactly this normal workspace dependency:

```text
eqiora-numerics (L3) -> eqiora-differentiation (L3)
```

The dependency permits one production import site only:

```text
crates/eqiora-numerics/src/canonical_stokes/dissipation_design/linearization.rs
```

That private module may construct the Stokes-specific accepted relation and
scalar output from one already accepted Stokes occurrence, then call the
existing RFC 0011 accepted-output, forward-output, and adjoint-output
interfaces. Its numerical results return only to the private E2 owner. No E1,
linearization, derivative, optimizer, or history type crosses a crate or
facade boundary.

The edge is package-wide in Cargo but its architectural authority is not. A
second production import site, a second numerical consumer, or a generic
numerics-side forwarding module is outside this permission and triggers STOP
for a new architecture decision. Tests may exercise the sole consumer; they
do not become additional production consumers or widen the edge.

### Invariant ownership

Existing owners remain unchanged:

- `eqiora-ir` owns `LinearizedRelation`, `LinearizedOutput`, and scalar
  tangent/cotangent vocabulary;
- `eqiora-solver` owns canonical coefficient capture, operator properties,
  normal/transposed solve orientation, solver plans, and accepted reports;
- `eqiora-differentiation` owns accepted relation/output admission, generic
  forward and adjoint output composition, replay, and total-gradient algebra;
  and
- `eqiora-numerics` owns Stokes geometry, mesh, assembly, method-native
  residual/objective actions, accepted-occurrence association, reconstruction,
  and private E2 state.

This RFC owns only permission for the directed composition edge. It does not
move, duplicate, extend, or become a second owner of any consumed interface.
The private Stokes adapter must use the accepted RFC 0011 contracts as they
exist. If they cannot express the consumer without changes in
`eqiora-differentiation`, `eqiora-ir`, or `eqiora-solver`, the E2 lane stops
and returns the missing requirement to that contract's owner.

### Exact later registration delta

RFC acceptance, numbering, and merge do not themselves add the dependency or
any E2 source. In the later E2 capability-closure envelope, after the accepted
numbered RFC has been bound by the corrected outcome contract, the manifest
dependency, mechanical lockfile relationship, exact allowlist pair, and
architecture text authorized by that RFC must land with the sole first
executable consumer. This prevents both a dormant dependency edge and dormant
E2 code. Only this dependency-registration and architecture delta is
authorized:

```text
crates/eqiora-numerics/Cargo.toml
Cargo.lock
tools/xtask/src/main.rs
docs/architecture.md
```

The manifest adds `eqiora-differentiation = { workspace = true }`. The lockfile
may record only the mechanical addition of that existing workspace package to
`eqiora-numerics`; no package source or version changes. The layer checker adds
exactly

```text
("eqiora-numerics", "eqiora-differentiation")
```

to `same_layer_dependency_is_allowed`, with a citation to the accepted
permanent RFC number. It does not admit a crate family, wildcard, transitive
closure, or second pair. `docs/architecture.md` records the same direction,
sole private consumer, unchanged owners, and deletion condition. The root
workspace manifest needs no change because `eqiora-differentiation` is already
a workspace dependency.

This exact allowlist tuple is permission, not a registry of consumers. No new
registry, feature, trait, type, crate, facade export, or forwarding API may be
introduced to make the dependency appear generic or transitive.

### Failure and STOP conditions

The later capability lane fails closed before derivative execution, history
mutation, Run acceptance, or Result acceptance for any of these conditions:

- this RFC is not accepted under a permanent number, or the later contract
  does not bind its exact accepted path, revision, and content;
- the dependency direction is reversed, a Cargo cycle appears, another
  same-layer exception is needed, or the exact layer-checker pair is absent;
- any production source outside the sole private import site consumes
  `eqiora-differentiation` through this edge;
- a second consumer is proposed before a new architecture decision reviews
  whether the edge should remain exceptional, move, or be replaced;
- a public item, facade export, `#[doc(hidden)] pub` bridge, public E1 or E2
  type, wire, schema, feature, registry, or new crate is required;
- generic accepted-linearization, forward, adjoint, replay, or total-gradient
  logic is copied into `eqiora-numerics` or made Stokes-specific;
- Stokes assembly, residual/objective meaning, artifact association, or private
  history moves into `eqiora-differentiation`;
- the exact finalized occurrence does not supply the relation, output,
  canonical coefficient source, operator properties, solver plan, and layout
  association required by the accepted generic path; or
- the independent exact quadrature identity, corrected E2 outcome contract,
  scientific evidence, or ordinary positive E2 path remains unresolved.

Owner drift is a terminal architecture event, not an implementation detail.
The affected owner must receive a new reviewed decision before work resumes.

### Deletion condition

This permission creates no public compatibility promise. When the sole
accepted Stokes E2 consumer is removed, remove in the same change:

1. the manifest dependency and its mechanical lockfile entry;
2. the exact layer-checker allowlist pair; and
3. the corresponding architecture text.

The same deletion applies after an independently accepted replacement moves
the generic algorithms while preserving every accepted consumer and evidence
path. The edge must not remain as dormant convenience or be retained for an
anticipated consumer.

## Alternatives considered

| Option | Mathematical naturalness | Runtime cost | Implementation and audit complexity | Compatibility | Approximation quality | Experimental promise | Decision |
| --- | --- | ---: | --- | --- | --- | --- | --- |
| Exact private `numerics -> differentiation` edge | Preserves native method-specific `R(w,p)` and scalar output while generic implicit analysis stays generic | No work beyond the accepted forward/adjoint solves | One exact dependency, allowlist pair, RFC citation, and private import site | No public surface or migration | Exact accepted discrete actions | Directly joins two accepted executable seams | **Select** |
| L4 API composition | Application composition is natural, but the private Stokes occurrence is not an application contract | Same solves | Requires a public or hidden-public E1/E2 bridge and splits accepted-point ownership | Creates durable public audit surface | Could be exact | Poor: architecture widens before the first private result | Reject |
| New L3 composition crate | Superficially isolates sibling dependencies | Same solves | Adds a crate and still requires public numerics types to cross the crate boundary | Creates new registration and compatibility surfaces | Could be exact | Poor for one consumer | Reject |
| Move generic analysis into an L2 crate | Algebra remains generic | Same solves | Migrates existing consumers and creates another same-layer or new-crate contract | Broad source/API migration | Exact if migrated correctly | No demonstrated architecture benefit | Reject now |
| Copy a private Stokes adjoint into numerics | Locally convenient but gives generic algebra a second owner | Same solves | Duplicates admission, transpose solve, replay, and gradient invariants | No public API, but permanent evidence duplication | Numerically matching output could hide the wrong product path | False-success risk | Reject |

The selected formulation is the smallest one that preserves both native
mathematical ownership and the ordinary accepted differentiation path. A new
lower abstraction becomes credible only after an independently demonstrated
second consumer or owner failure, not in anticipation of one.

## Compatibility and migration

The RFC changes no source, schema, wire, artifact, public Rust item, facade,
feature, solver tuple, or existing dependency before its later implementation.
The eventual dependency is private and additive. Existing callers of both
crates remain source-compatible, and no artifact migration exists.

The architecture checker remains fail-closed: before the accepted exact pair
is registered, the same-layer edge is an error; every other unlisted same-layer
edge remains an error afterward. Downstream exhaustive matching and public
semver are unaffected because no public enum or trait changes.

The permanent RFC number is assigned only after acceptance. The draft path and
`0000` label are review identities, not implementation authority.

## Verification

RFC review must first confirm the accepted RFC 0011 predecessor, the absent
current/reverse dependency, the cycle-free current closure, and the exact
layer-checker state at the proposal base.

The later E2 capability closure must then demonstrate all of these on its exact
integration head:

1. `cargo metadata` and the repository layer gate show exactly the one new
   directed pair, no reverse edge, and no dependency cycle.
2. A source/import audit finds the dependency's production use only in the
   named private Stokes E2 `linearization.rs` consumer.
3. Public-surface and facade gates show no new bridge, re-export, type, trait,
   feature, wire, schema, registry, or crate.
4. One ordinary accepted Stokes E2 path reaches the existing generic forward
   and adjoint output operations before any E2 falsifier counts.
5. The independently owned E2 package rejects stale occurrence association,
   wrong orientation or canonical source, copied/substituted solver policy,
   quadrature identity drift, and any owner-local algorithm bypass at their
   named boundaries.
6. Removing the sole consumer makes the manifest edge and exact allowlist pair
   unnecessary and therefore requires their deletion.

This RFC authors no oracle, expected value, tolerance, fixture, or falsifier.
The accepted materialized direct-output case owns generic conformance; the
later independently authored Stokes E2 evidence owns only its exact consumer
and scientific claim.

## Security, safety, and governance

The edge introduces no unsafe code, external authority, filesystem or network
access, native plugin, persistent state, or user-visible API. Its risk is
architectural: a package-wide Cargo capability could silently become a general
same-layer license. The exact allowlist tuple, sole import site, STOP on a
second consumer, and deletion condition constrain that risk.

Because this is a dependency-layer exception and architecture decision, the
complete RFC delta requires fresh-context non-writer review before acceptance.
The writer cannot supply that review. Acceptance of this RFC does not accept
the quadrature decision's implementation, the E2 scientific contract, or any
later source/evidence delta; each retains its own authority and review gates.

## Unresolved questions

- The permanent RFC number is assigned only after acceptance.
- The corrected E2 outcome contract must bind the accepted numbered RFC and
  then-current implementation predecessor before any source work starts.
- The exact quadrature identity remains an independent STOP for E2 capability
  closure, not an unresolved question this RFC may answer.
