# RFC 0035: Field-valued physical boundary interfaces

- Status: Accepted and verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0033](0033-hierarchical-conserving-connection-sets.md),
  [RFC 0034](0034-occurrence-bound-spatial-supports.md)

## Summary

One field-valued physical Port denotes a mesh-independent trace/flux dual pair
on an exact continuous boundary. Connected Ports share one exact nominal
Connector and one coincident boundary point set. Their traces are pointwise
continuous and their parent-outward fluxes sum pointwise to zero.

This contract extends value typing from tensor rank to exact extents, keeps
coordinate-frame meaning explicit, and scalarizes shaped residuals only after
canonical semantics has lowered to Operator IR.

## Motivation

Scalar acausal Ports are sufficient for circuits and lumped components, but a
solid/fluid interface carries fields over a boundary. Passing a mesh array
would make discretization an accidental part of model meaning. Repeating
shape, support, frame, parent, and sign in several payloads would instead make
multiple canonical sources that can disagree.

The interface must therefore state only mathematical meaning:

```text
exact Connector identity
  trace dimension
  outward-flux dimension
  exact value shape
  component frame
  dual pairing

Port
  exact Connector
  exact boundary Domain
```

The boundary's unique `BoundaryOf` edge derives the parent. Orientation is
always `OutwardOf(parent)`. Neither value is repeated in the Port payload.

## Decision

### Exact value shape and frame

`ValueShape` is one Layer-0 value type shared across canonical Field,
expression, Port, Operator-lowering, and wire contracts:

```text
[]       scalar
[2]      two-component vector
[2, 2]   exact two-by-two tensor
```

Extents are positive portable `u32` values. The empty list has one scalar
component and remains distinct from `[1]`. Component products use checked
arithmetic. Rank alone is insufficient because `[2]` and `[3]` must not
coerce.

`ValueFrame` is closed in the first version:

```text
Invariant
SpatialCartesian
```

`SpatialCartesian` means the model-global Cartesian frame. A spatial extent
must equal its exact parent ambient dimension. Arbitrary local frames and
implicit frame transformations are rejected rather than guessed.

Pure expression typing operates on exact `{dimension, shape, frame, support}`:

- addition and subtraction require all four to agree;
- an invariant scalar may scale a shaped value;
- `grad` appends the ambient spatial extent;
- `div` and outward `normal` require and remove the trailing ambient extent;
- `trace` preserves shape and frame while moving support to the exact
  boundary; and
- every residual root, scalar or shaped, means componentwise zero.

A non-scalar Field cannot receive the scalar `DynQuantity` initialization or
`SetValue` contract. A later shaped-value wire may add that operation
explicitly; no broadcast is inferred.

### Nominal Connector and quantity identity

No tenth Semantic Kernel node kind is introduced. One nominal Connector is a
closed Domain payload:

```text
BoundaryPhysicalConnector {
  trace_dimension,
  flux_dimension,
  shape,
  frame,
  pairing = EuclideanBoundaryDuality
}
```

The exact trace or flux quantity identity is `(Connector Domain ID,
Trace|Flux)`. Equal dimensions, shape, and frame do not make two separately
defined Connectors compatible. A package alias resolving to the same exact
package name, version, semantic digest, and declaration does.

Source `spatial_vector` is convenience syntax, not a canonical variant. It is
specialized to `[ambient_dimension]` from the bound boundary support before
flattening. Connector specialization identity therefore includes the exact
resolved shape; the same generic declaration used in 2D and 3D creates two
different nominal specializations.

### Port support and connection law

The closed Port payload is:

```text
BoundaryPhysical {
  connector: DomainId,
  boundary: DomainId
}
```

Whole-model validation derives one parent from the boundary's unique
`BoundaryOf` edge and rejects a missing parent, multiple parents, volume
support, boundary-of-boundary support, wrong Connector kind, or a Cartesian
spatial shape inconsistent with the parent dimension.

For a maximal conserving set `C`, with canonical anchor `p0`, every point `x`
on the common boundary and every exact component satisfies:

```text
Trace(p, x) - Trace(p0, x) = 0    for p in C except p0
sum(Flux(p, x) for p in C) = 0
```

Flux is already oriented outward from each Port's exact parent. The
connection law is always a sum; source never authors compensating signs.
Euclidean boundary power is the componentwise inner product. Trace continuity
and flux balance therefore conserve total interface power.

RFC 0033's bounded, order-independent maximal-set normalizer remains
unchanged. Type, nominal-identity, support, and geometry admission happen
before fragments enter that normalizer.

The first geometric slice admits coincident axis-aligned Cartesian boundaries.
Coincidence requires equal ambient dimension, normal axis, fixed hyperplane
coordinate, and all tangential intervals. `-0.0` and `0.0` compare numerically
equal. Curved or noncoincident interfaces need a Realization-owned transfer
contract and are not silently accepted.

### Canonical expressions and Operator scalarization

Boundary Relation expressions use two closed symbols:

```text
PortTrace(PortId)
PortFlux(PortId)
```

The Semantic expression DAG remains shaped. It contains no component-select
nodes manufactured solely for execution. Operator lowering expands roots in
this deterministic order:

1. Relation root order;
2. row-major component multi-index, last axis fastest.

Each lowered scalar input is a
`ScalarSymbolCoordinate { semantic_symbol, component_multi_index }`. The
ordinary scalar SSA engine evaluates every row. Spatial differential nodes
require a discretized Operator lowering and fail at the pointwise scalarizer;
they are not approximated there.

The shared pure typing pass is the only constructor of an opaque
`TypedResidual`. Semantic validation and Operator lowering therefore cannot
disagree through a caller-authored parallel shape array. Root policy is
explicit: Relation roots use `ComponentwiseResidual`, while Event and Guard
activation roots remain invariant scalars. A maximal boundary Connection
provides one derived `Interface` support for its coincident member boundaries;
this is a typing identity, not a new Domain, mesh, or transfer payload.

For two `[2]` Ports, the derived junction contains exactly four scalar rows:
two trace-difference components followed by two flux-sum components.

### Source and hierarchy boundary

The intended source form is closed:

```text
public connector MechanicalBoundary = field_physical(
  trace = velocity: m / s,
  flux = traction: kg / (m * s ^ 2),
  shape = spatial_vector,
  frame = spatial,
  pairing = euclidean_boundary_duality
);

public component BoundaryLaw {
  public support body: volume(ambient_dimension = 2);
  public support wall: boundary(parent = body);
  public port interface: conserving MechanicalBoundary over wall;
}
```

RFC 0034 resolves `wall` to the exact occurrence boundary and parent before
flattening. The support slot disappears. The flattened Port retains only the
exact specialized Connector and exact boundary Domain. Mesh facets, basis
functions, quadrature, transfer maps, and solver layout cannot enter this
path.

### Explicit wire v3

The new vocabulary uses explicit, domain-separated model and transaction
schemas:

```text
eqiora.model-envelope/v3
eqiora.model-transaction-envelope/v3
```

V3 owns exact shaped Fields, boundary Connector Domains, boundary Ports, and
Port trace/flux symbols. Shape is canonically encoded as an extent array.
V1/V2 encoders and decoders reject every V3-only value. Existing V1/V2 bytes
and digests remain fixed. Callers select one version explicitly; decoding does
not sniff input or retry another codec.

## Realization boundary

The Semantic Model owns continuous interface identity, exact quantity types,
support, frame, outward orientation, pairing, continuity, balance, and power
meaning.

Realization owns mesh/facet IDs, quadrature, trace spaces, FEM/FVM selection,
mixed elements and stabilization, mortar/Nitsche, nonmatching maps `P` and
work-conjugate `P*`, monolithic or partitioned execution, iteration controls,
MPI layout, and device residency. Changing only those choices cannot change
the Semantic Model digest.

## Falsifying conformance

The bounded implementation must prove:

- a coincident 2D `[2]` interface is admitted and lowers to exactly four
  analytic scalar junction rows;
- `[2]` and `[3]`, invariant and spatial frames, or distinct exact Connector
  identities do not coerce;
- missing/wrong parent, volume support, boundary-of-boundary support, and
  noncoincident Cartesian boundaries fail before execution;
- package aliases resolving to one exact Connector connect successfully;
- member, fragment, declaration, file, and dependency ordering cannot change
  the maximal set or canonical model identity;
- shape, quantity, support, frame, or pairing changes alter semantic identity;
- mesh, quadrature, discretization, transfer, and coupling execution choices
  do not alter semantic identity; and
- V1/V2 goldens remain byte-identical while rejecting all V3-only payloads.

The registered
[`packages.field-valued-boundary-interface`](../verify/packages/field-valued-boundary-interface/README.md)
case is the conformance root. It compiles one exact dependency package through
the ordinary package path, proves package-alias and source-order invariance,
checks nominal and geometric admission before connection-set union, evaluates
the four scalarized junction rows, records the derived trace/flux/power
observations, and replays the complete accepted Model and Transaction through
wire v3. Pure schema, semantic, compiler, IR, and legacy-wire tests retain the
narrow falsifiers that cannot all inhabit one accepted source model.

## Nonclaims

This RFC does not implement Stokes or elasticity discretization, FSI solve,
moving geometry or ALE, contact, curved/noncoincident embeddings, nonmatching
mesh transfer, mortar/Nitsche, arbitrary frames, dynamic plugins, stream
variables, a universal variable-channel Connector payload, or the durable
result-query identity of an ownerless public exposure eliminated by RFC 0033.
That source/provenance/result-artifact contract is owned independently by
[RFC 0036](0036-physical-exposure-projection-artifacts.md); it does not change
the canonical interface law decided here.

Those features may build on this interface only by preserving the same
meaning → lowered contract → Realization → adapter → evidence path.
