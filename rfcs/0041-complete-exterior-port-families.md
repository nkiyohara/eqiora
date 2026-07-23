# RFC 0041: Complete-exterior Port families

- Status: Accepted; bounded implementation verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0021](0021-component-hierarchy-and-instantiation.md),
  [RFC 0033](0033-hierarchical-conserving-connection-sets.md),
  [RFC 0034](0034-occurrence-bound-spatial-supports.md),
  [RFC 0035](0035-field-valued-boundary-interfaces.md), and
  [RFC 0040](0040-occurrence-bound-field-slots.md)

## Summary

A Component may require the complete exterior of one occurrence-bound volume
and declare a finite, statically elaborated family of boundary Ports,
Relations, Activations, or Connections over it. An occurrence supplies an
explicit unordered set of exact Boundary Domains. The compiler proves that
the set is the complete Cartesian exterior, expands each family once per
exact member, and erases the set and family before the Semantic Kernel.

The closed source form is:

```text
public support body: volume(ambient_dimension = 2);
public support exterior: complete_exterior(parent = body);

public port mechanical[boundary in exterior]:
  conserving QuasistaticMechanicalBoundary over boundary;

relation boundary_law[boundary in exterior] continuous on boundary {
  trace(displacement) - trace(mechanical[boundary = boundary]) = 0;
  normal(stress) - flux(mechanical[boundary = boundary]) = 0;
}
```

One occurrence binds the obligation explicitly:

```text
instance solid: IsotropicMechanicalInterface2d(
  support body = body,
  support exterior = boundaries(x_lower, x_upper, y_lower, y_upper),
  field displacement = displacement,
  mu = 3,
  lambda = 2
);
```

`complete_exterior` is an elaboration obligation, not a collection value.
There is no general array, iterator, loop, set algebra, runtime family, or
tenth Kernel node.

## Motivation

One boundary support slot is sufficient for a component whose public
interface has a fixed cardinality. A reusable continuum body is different:
its physical law applies uniformly to every exact exterior member, while the
enclosing Model must retain ownership of the concrete Domains and boundary
conditions. Baking names such as `left`, `right`, `top`, and `bottom` into a
solid package makes geometry spelling part of the package API. Passing mesh
facets or arrays instead moves numerical realization into Model meaning.

The missing abstraction is smaller than either alternative:

```text
definition obligation   complete exterior of body slot
occurrence evidence     explicit finite set of exact Boundary identities
elaboration             one ordinary declaration per exact identity
flattened meaning       existing Relation / Port / Activation / Connection
```

Calling this concept a *partition* would collide with connection-set
partitions, mesh partitions, and execution partitions. The public vocabulary
therefore names the semantic obligation (`complete_exterior`) and the
compiler-local finite carrier (`BoundarySet`).

## Closed source contract

### Complete-exterior support slot

Only an ordinary Component may declare:

```text
public support exterior: complete_exterior(parent = body);
```

`body` must be a public volume support slot in the same Component. Its exact
positive ambient dimension is authoritative. The exterior slot is public,
required, initialization-free, and cannot be used as a singular spatial
support. Private complete exteriors, boundary-of-boundary parents, inferred
parents, and default members are rejected.

An occurrence binds either an explicit finite set:

```text
support exterior = boundaries(a, b, c, d)
```

or forwards one already-resolved enclosing obligation:

```text
support exterior = exterior
```

Every explicit target is a visible bare-name exact Boundary Domain. Qualified
child selection, expressions, ranges, wildcards, implicit discovery, mesh
facets, and callbacks are not admitted. Member order is non-semantic. Empty
sets and duplicate lexical targets are retained through parsing so semantic
validation can issue exact diagnostics and reject the occurrence atomically.

### One restricted family binder

The only family binder is exactly:

```text
[boundary in complete_exterior_slot]
```

It is admitted on a field-physical Port, continuous Relation, or conserving
Connection inside a Component. `boundary` is a lexical binder whose value is
one exact Boundary member. It cannot escape the declaration, shadow a member,
appear in arithmetic, nest another family, select a subset, or drive a runtime
loop. Families over Parameters, Fields, scalar Ports, arbitrary collections,
or a singular boundary slot are rejected.

A family Port must use its binder as its exact support:

```text
public port mechanical[boundary in exterior]:
  conserving QuasistaticMechanicalBoundary over boundary;
```

A family Relation must use the same binder as its activation support. A
family Port reference is always selected explicitly:

```text
mechanical[boundary = boundary]
```

Outside that family scope, the selector resolves one visible exact Boundary:

```text
solid.mechanical[boundary = x_lower]
```

The source name is only a path to an exact Domain identity. Selection is
neither ordinal indexing nor string-key lookup.

A conserving Connection may itself carry the same restricted binder:

```text
connect conserving[boundary in exterior]
  child.mechanical[boundary = boundary],
  mechanical[boundary = boundary];
```

It expands pointwise. After expansion each ordinary fragment enters RFC
0033's existing maximal conserving-set normalizer. Family membership never
causes an implicit Connection, and noncoincident boundary admission still
happens before connection-set union.

### Nested forwarding

A wrapper may forward the complete exact member set to a child occurrence,
declare its own family, and connect the two families pointwise. Forwarding
preserves member identities and carries every transitive binding origin. It
does not copy Domains, create aliases, or discover additional members from the
enclosing scope.

## Complete-exterior proof

The compiler uses an identity-parametric, elaboration-only contract:

```text
BoundarySet<I> {
  exact_parent: I,
  members: sorted unique exact Boundary<I>
}

CompleteExteriorWitness<I> {
  exact_parent: I,
  sides: canonical bijection (axis, Lower|Upper) -> exact Boundary<I>
}
```

Ambient dimension is derived from the exact parent volume and is never a
second authored field. In the first admitted Cartesian-box slice, validation
requires:

1. every member is an exact Boundary Domain;
2. every member has the same exact bound parent;
3. its ambient dimension equals the parent dimension;
4. every `(axis, Lower|Upper)` side of the parent appears exactly once; and
5. no two distinct Domain identities describe the same side.

These checks prove the complete box exterior directly. They do not enumerate
all Boundary declarations in scope, compare floating-point geometry, infer
corner intersections, or accept an incomplete set because unused sides happen
not to be referenced. Missing sides, duplicate identities, duplicate geometry
under distinct identities, wrong parents, volume members,
boundary-of-boundary members, wrong dimensions, and arithmetic overflow fail
before a Transaction exists.

Curved and non-Cartesian domains will require a future domain-specific
completeness witness. V1 does not pretend that lexical membership proves
geometric coverage.

## Definition checking and occurrence elaboration

Component definition checking uses one synthetic identity-parametric member
whose exact parent is the declared volume slot. This checks Connector,
Field, expression shape, dimension, frame, support, trace/flux role, and
Relation root typing without choosing an occurrence.

Occurrence elaboration then:

1. resolves the parent volume support;
2. resolves or forwards the explicit BoundarySet;
3. proves the complete-exterior witness;
4. resolves Parameter, support, and Field obligations;
5. substitutes each exact Boundary identity into every family declaration;
6. emits ordinary Port, Relation, Activation, and Connection blueprints;
7. performs ordinary physical admission and maximal-set normalization; and
8. constructs a Transaction only after every occurrence succeeds.

No family, set, witness, binder, member selector, package alias, or forwarding
alias survives into Kernel, Model wire, artifact, Realization, runtime, or
solver contracts. Downstream code sees the same flat typed relation network it
already understands and cannot branch on package names or family syntax.

## Identity and source identity

One generated semantic identity is derived from:

```text
root semantic namespace
+ component occurrence path
+ definition-relative family declaration identity
+ generated entity kind or role
+ exact full Boundary identity
```

The exact Boundary identity is a dedicated discriminator, not text appended
to a declaration or instance path. List ordinal, source spelling, source span,
iteration order, Cartesian side nickname, and connection-set representative
are excluded. Generated Activation identities carry the same member
discriminator as their Relation.

Consequently, two bindings of the same exact set in different orders produce
identical flattened identities. The RFC does not claim that an accepted
complete exterior can gain an extra member: for a fixed Cartesian parent that
would be invalid. Nor does it promise identity stability across arbitrary
source edits, because the existing root semantic namespace intentionally
binds the whole canonical source identity.

Source identity receives append-only records for the new slot, set binding,
family binder, and exact selector. Set members are canonicalized by resolved
source path after duplicates have been diagnosed. Existing tags and field
layouts are unchanged, and sources containing none of the new syntax remain
byte-for-byte identical to their current source-identity goldens.

## Provenance and diagnostics

Every generated Port, Relation, Activation, and Connection fragment retains:

- the family declaration span;
- every enclosing instance declaration span;
- parent support and complete-exterior binding spans;
- the selected member token span for an explicit binding;
- transitive forwarding spans; and
- relevant Field and Parameter binding spans.

These sorted, distinct origins explain how one exact member entered the flat
network without affecting semantic identity. Connection normalization
continues to aggregate all fragment origins as specified by RFC 0033.

Diagnostics identify the Component, occurrence, exterior slot, expected exact
parent, and offending member or missing Cartesian side. Failure is atomic:
no partial graph, Transaction, package execution record, or numerical
allocation is exposed.

## Bounded expansion

Two independent limits are added:

- members per explicit BoundarySet; and
- total BoundarySet memberships in one compilation.

The exterior slot and binding also consume the existing Component-member and
per-instance shared binding budgets. Generated Ports, Relations,
Activations, and Connections consume existing declaration, staged-identity,
fragment, provenance, and canonical-byte budgets. `2 * dimension` and every
family expansion product use checked arithmetic. Dynamic preflight computes
the exact occurrence count before blueprint allocation; the static definition
count alone is not accepted as a bound.

## Package, Model, and Realization boundary

An exact Model Package may export a Component containing this syntax. The
package digest sees its canonical source through the existing package wire;
there is no universal resource payload or new package resolver. The package
may own:

- the complete-exterior obligation;
- nominal displacement/traction or velocity/traction Connectors;
- constitutive, balance, trace, and outward-flux Relations;
- occurrence Field obligations and material Parameters; and
- semantic boundary-condition Components.

The root Model owns exact volume and Boundary Domains. Realization owns mesh
facets, trace spaces, quadrature, essential elimination, conforming merge,
mortar, Nitsche, transfer maps, stabilization, monolithic or partitioned
policy, solver, scheduling, MPI, and devices. A semantic boundary condition
does not become a Realization option merely because an assembler later uses
it for elimination.

The first checked-in package application will add a separate boundary-law
Component to `Eqiora.Solid.LinearElasticity`. It will not widen or break the
already verified closed `IsotropicBalanceWithPotential2d` Component.

## Pairing terminology

RFC 0035's `EuclideanBoundaryDuality` is a generic trace/flux pairing. Its
physical interpretation depends on the quantity pair:

- velocity and traction pair as power density; and
- displacement and traction pair as virtual-work density.

The compiler enforces exact nominal Connector identity, dimensions, shape,
frame, support, and role. It does not infer physical interpretation from equal
array shapes or call every pairing power.

## Prior art and deliberate differences

Modelica 3.7 expands arrayed connect equations elementwise and constructs
connection sets from the expanded scalar elements. Eqiora keeps the useful
static-elaboration and maximal-set ideas, but deliberately does not adopt
general arrays, `for` equations, expandable connectors, stream connectors,
or index-based generated identity. A complete exterior family has one closed
geometric obligation and exact-identity selectors.

- [Modelica Language Specification 3.7](https://specification.modelica.org/maint/3.7/)
- [Modelica arrays and connection equations](https://specification.modelica.org/maint/3.7/connectors-and-connections.html)

UFL provides distinct exterior-facet measures and lets a host environment
define boundary subsets. That is appropriate for variational realization,
but it intentionally leaves geometric domain and boundary construction to
the host. Eqiora's contract is earlier and semantic: a Model explicitly owns
exact Boundary Domains, while a Component requires a complete identity set.
Facet markers, integration measures, and trace spaces remain Realization data.

- [UFL form language](https://docs.fenicsproject.org/ufl/main/manual/form_language.html)

## Falsifying conformance

The registered `packages.complete-exterior-port-families` case must prove:

- missing side, duplicate exact member, duplicate side under distinct IDs,
  wrong parent, volume member, boundary-of-boundary member, wrong dimension,
  empty set, and member or expansion overflow fail before a Transaction;
- member, binding, declaration, file, and dependency-alias order cannot change
  package identity or flattened Model meaning;
- one ordinary Port, Relation, and Activation exists per exact member, with no
  set, family, selector, or forwarding object in the flat Model;
- generated identity uses exact Boundary identity rather than member ordinal;
- every generated declaration retains complete transitive provenance;
- nested forwarding and an explicit-flat network have a verification-private
  complete identity bijection and identical accepted semantics;
- family members are never connected implicitly, while pointwise family
  Connections normalize through the ordinary maximal-set path;
- aliases of the same exact Connector are accepted, while a structurally
  equal distinct Connector, velocity trace, wrong shape, frame, dimension, or
  boundary is rejected; and
- no compiler, runtime, or numerics path inspects a package name.

Narrow compiler unit tests additionally prove legacy source-identity bytes,
the exact member discriminator, checked expansion arithmetic, and limits that
cannot all inhabit one accepted end-to-end fixture.

## Downstream executable slice

Mixed-boundary elasticity execution is deliberately separate. It derives
one package-neutral method-level inventory from the ordinary flat network:

```text
EssentialZero | NaturalZero | PortBinding
```

An exact two-Port terminal connection may be structurally eliminated when its
ordinary Relations prove zero trace or zero flux. A live PortBinding fails
closed until a Realization-owned coincident trace method exists. The
registered [`solid.mixed-boundary-elasticity-2d`](../verify/solid/mixed-boundary-elasticity-2d/README.md)
case closes the first numerical falsifier: `lambda = 0`,
`q = 2 mu x / ell`, `ell = 1 m`, and
`u = (x - x^2 / (2 ell), 0)`, with left essential zero and the other three
sides natural zero. Direct Relations and exact packaged
`FixedDisplacement2d`/`ZeroTraction2d` terminals produce the same inventory,
reduced CSR, right-hand side, solution, Q1 convergence, constrained reaction,
and global balance. Independent facet quadrature also recovers the raw Q1
traction: it is exactly zero on the horizontal natural sides and `mu h` on the
right side, hence converges to the prescribed zero traction at first order.
The left recovered stress resultant `-2 mu + mu h` remains deliberately
distinct from the exact algebraic constrained reaction `-2 mu`; neither is
silently substituted for the other. The assembler receives only a private
complete-side mask; it cannot inspect the Semantic Model, package identities,
or source names. Near-miss evidence rejects boundary/volume coefficient
mismatch, extra direct Relations, duplicate side identities, and a terminal
that simultaneously prescribes trace and flux.

## Nonclaims

This RFC does not provide arbitrary boundary subsets or set algebra, runtime
arrays or loops, dynamic Port counts, a collection Kernel node, curved or
non-Cartesian completeness, implicit boundary discovery, mesh/facet data,
nonmatching transfer, mortar, Nitsche, live coupled execution, mixed-boundary
data beyond the exact zero Cartesian slice, Stokes, FSI, ALE, contact, or
dynamics.
