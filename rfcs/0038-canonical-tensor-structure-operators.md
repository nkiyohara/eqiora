# RFC 0038: Canonical tensor structure operators and explicit wire v4

- Status: Accepted; bounded semantic slice implemented and verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0007](0007-canonical-spatial-operators.md),
  [RFC 0035](0035-field-valued-boundary-interfaces.md),
  [RFC 0037](0037-version-neutral-model-artifact-reference.md)

## Summary

Eqiora adds two physics-neutral tensor structure operators to the canonical
Relation expression DAG:

```text
symmetric_part(T) = (T + transpose(T)) / 2
isotropic_lift(s) = s I_d
```

`symmetric_part` accepts exactly a spatial Cartesian `[d,d]` tensor on a
Cartesian volume. `isotropic_lift` accepts exactly an invariant scalar on a
Cartesian volume and obtains `d` from that support, producing a spatial
Cartesian `[d,d]` tensor. Both preserve physical dimension and exact nominal
support.

Source lowering preserves both operations in the canonical DAG. Hierarchical
source checking and committed semantic validation invoke the same
identity-parametric typing functions when they own the required support
identities; component scalarization accepts only the resulting opaque typing
proof. Explicit Model and Transaction wire v4 inherit the closed v3 vocabulary
and add only these two expression nodes. V3 remains closed and byte-stable.

## Motivation

The existing `grad` and `div` operators already express the differential
structure of a multidimensional continuum relation. They could not, however,
form a symmetric gradient or lift an invariant scalar into an isotropic
second-order tensor without one of three undesirable shortcuts:

- add an elasticity-specific Kernel node;
- encode tensor coordinates as unrelated scalar equations; or
- defer mathematical meaning to a chosen finite-element implementation.

The two missing operations are structural, not constitutive. For example, an
isotropic small-strain balance can now remain one ordinary implicit Relation:

```text
-div(
  2 * mu * symmetric_part(grad(displacement))
  + lambda * isotropic_lift(div(displacement))
) = 0
```

This expression states continuous meaning only. It does not choose a material
package, mesh, element, weak form, assembly policy, solver, or result schema.

## Canonical semantics

Let an expression type contain physical dimension, exact value shape,
component frame, and optional nominal spatial support.

### Symmetric part

`symmetric_part(T)` is admitted only when:

- `T` is supported on a Cartesian volume of ambient dimension `d`;
- `T` has exact shape `[d,d]`; and
- `T` has the `SpatialCartesian` component frame.

Its result has the same dimension, shape, frame, and support as `T`. For exact
component indices,

```text
result[i,j] = 0.5 * (T[i,j] + T[j,i]).
```

A nonsquare tensor, a tensor whose extent differs from the ambient dimension,
an invariant frame, a boundary support, or no support fails typing.

### Isotropic lift

`isotropic_lift(s)` is admitted only when:

- `s` is supported on a Cartesian volume of ambient dimension `d`; and
- `s` is an invariant scalar.

Its result preserves the dimension and support of `s`, has exact shape
`[d,d]`, and has the `SpatialCartesian` frame. For exact component indices,

```text
result[i,j] = s, if i = j;
result[i,j] = 0, otherwise.
```

The off-diagonal zero carries the same physical dimension as `s`. A global
Parameter is therefore not lifted directly; a supported scalar expression
such as `div(displacement)` supplies the required nominal volume.

### One typing authority

The pure typing functions are identity-parametric. Semantic validation supplies
Domain identities from the committed graph, while the hierarchical source
checker supplies resolved source identities. Neither layer reimplements shape,
frame, or support rules. Flat source lowering preserves the closed operators
and SI dimensions but deliberately leaves whole-graph shape, frame, and support
admission to `KernelProgram`; returning an uncommitted Transaction is not a
semantic-admission claim.

The operators are ordinary expression nodes, not new Kernel node kinds.
Expression identity includes the operator and operand in the existing ordered
DAG. Component scalarization lowers only their pointwise tensor structure:
direct-and-transposed coordinates for `symmetric_part`, and diagonal scalar or
dimensioned zero for `isotropic_lift`. Spatial `grad` and `div` realization
remains a separate lowering obligation.

## Source surface

The source language exposes the exact unary names `symmetric_part` and
`isotropic_lift`. They use the same typed call path as the existing closed
spatial vocabulary. No free-form operator registry, implicit transpose, or
physics-specific shorthand is introduced.

The registered fixture deliberately uses the isotropic linear-elasticity form
above because it composes both operators with existing `grad` and `div`. The
fixture is a semantic falsifier, not an executable solid model.

## Explicit Model and Transaction wire v4

The enlarged expression vocabulary uses two explicit, domain-separated
schemas:

```text
eqiora.model-envelope/v4
eqiora.model-transaction-envelope/v4
```

V4 inherits v3's shaped Field and field-valued boundary-interface grammar and
adds only the closed expression variants:

```text
{"op":"symmetric-part","value":operand_index}
{"op":"isotropic-lift","value":operand_index}
```

Model v1, v2, and v3 and Transaction v1, v2, and v3 remain closed. Their
encoders reject either new node, their decoders reject v4, and changing only a
v4 schema tag to v3 cannot make the payload admissible. Existing v3 canonical
bytes and domain-separated digests do not change.

Callers select v4 explicitly. Decoding does not sniff content, try older
versions, or migrate bytes. Model and Transaction v4 retain their separate
digest domains and existing distinction between semantic content identity and
ordered edit identity. Existing decoder resource limits and typed replay
barriers apply before graph mutation.

`ModelEnvelopeV4` implements the sealed version-neutral Model artifact
contract from RFC 0037. This makes v4 eligible for downstream identity linkage
without changing Realization or Run schemas. The registered claim here stops
at exact Model/Transaction v4 replay; it does not claim v4 execution lineage.

## Alternatives considered

### Add a canonical elasticity node

Rejected. Isotropic elasticity is one constitutive composition of general
tensor operations. A physics node would couple canonical meaning to one law
and create a parallel semantic hierarchy.

### Add a general transpose and identity-tensor language now

Rejected for this slice. Those operations may become useful, but a general
tensor algebra needs index, contraction, frame, and diagnostics decisions that
are not justified by this bounded requirement. The two closed operations have
complete shapes and component semantics today.

### Encode components as separate scalar Relations

Rejected. That loses the exact tensor shape and frame before validation and
makes component ordering part of author intent rather than deterministic
lowering.

### Extend wire v3 in place

Rejected. A v3 decoder must continue to mean the grammar it originally
accepted. Explicit v4 preserves deterministic compatibility and prevents a
new expression from acquiring an old digest domain.

## Compatibility and migration

This is an additive in-memory expression change and a new explicit artifact
generation before 1.0. Existing models without the new nodes keep their prior
meaning and may still use v1, v2, or v3 as appropriate. A model containing
either tensor structure operator must select v4; there is no implicit upgrade
or downgrade.

V4 may encode the complete v3 subset, but equal semantic meaning encoded in
v3 and v4 has different artifact identity because schema generation is part of
the digest domain. The version-neutral reference preserves that distinction.

## Verification

The registered
[`language.canonical-tensor-operators`](../verify/language/canonical-tensor-operators/README.md)
case must prove:

1. the 2D source fixture lowers to one shaped canonical Relation containing
   both tensor structure nodes;
2. source lowering preserves the canonical operators and dimensions, while the
   public identity-parametric typing contract and committed semantic validation
   enforce exact shape, frame, dimension, and volume support; nonsquare,
   wrong-frame, unsupported, and global-scalar uses fail closed;
3. component scalarization implements the exact symmetric and diagonal
   coordinate rules with deterministic root/component ordering;
4. explicit Model and Transaction v4 canonical bytes and digests replay
   exactly through typed constructors; and
5. v1/v2/v3 encoders and decoders reject the new nodes, a forged v3 schema
   fails.

The evidence target is intentionally one language/semantic/artifact slice. It
does not solve the displayed equation. A separate `eqiora-artifact`
`legacy_v3_golden` regression freezes the pre-v4 v3 byte count and digest; it
supports the compatibility decision without being folded into this registered
case's evidence target.

## Security and safety

Both operations are total only over statically admitted finite shapes. Ambient
dimension comes from a validated Cartesian volume, not an untrusted extent
sidecar. Artifact decoding remains bounded before allocation or graph mutation,
and exact version selection prevents permissive fallback to another grammar.

## Nonclaims

This RFC does not implement an elasticity Model Package, constitutive material
library, boundary conditions, mesh, weak form, local operator, assembly,
linear/nonlinear solve, displacement/stress result, patch test, convergence
study, FEM/FVM execution, or FSI. It does not define general transpose, tensor
products, arbitrary contractions, index notation, curvilinear frames, mixed
variance, or user-defined tensor functions. It does not migrate legacy wire
bytes or widen their accepted vocabularies.
