# RFC 0042: Conforming elasticity interface realization

- Status: Accepted; bounded implementation verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0018](0018-ordered-assembly-execution.md),
  [RFC 0023](0023-finalized-spatial-linear-handoff.md),
  [RFC 0035](0035-field-valued-boundary-interfaces.md),
  [RFC 0039](0039-canonical-isotropic-elasticity-2d.md), and
  [RFC 0041](0041-complete-exterior-port-families.md)

## Summary

Eqiora admits one exact two-body elasticity network whose live field-valued
physical Connection joins coincident complete Cartesian sides. The two bodies
remain distinct semantic Domains and lower independently to the existing
package-neutral isotropic-elasticity contract. A separate pair lowerer proves
that their only live bindings are the two ends of one binary, opposite-side,
coincident interface.

One bounded Realization generates an independent Cartesian Q1 mesh for each
body and constructs an exact topological bijection between the two interface
vertex sets. Paired vertices map to one quotient displacement degree of
freedom. Thus:

```text
trace continuity       = identity of the assembled unknown
weak traction balance  = both body operators scatter into the same row
```

The implementation introduces no merged semantic Domain, merged mesh,
penalty, multiplier, Nitsche term, mortar space, or package-specific executor.

## Semantic boundary

The existing field-physical Connector remains authoritative for trace/flux
shape, dimension, frame, pairing, nominal compatibility, exact boundary
support, and parent-outward orientation. The Semantic Kernel already admits
distinct Boundary identities when their Cartesian point sets coincide. This
RFC does not change that meaning.

Body-local elasticity lowering retains each live boundary as:

```text
PortBinding { connection, port }
```

The owning Port must belong to the exact local boundary law. A peer Port may
belong to a distinct coincident Boundary; coincidence and Connector identity
are proved by the ordinary physical-junction admission, not reimplemented by
the numerical lowerer.

The pair lowerer admits exactly:

1. two two-dimensional Cartesian elasticity bodies;
2. one body-local live side on each body;
3. one ordinary conserving Connection containing exactly those two Ports;
4. one common normal axis and opposite `Upper`/`Lower` sides;
5. adjacent interiors and an identical tangential interval; and
6. no unconsumed Model node or additional live binding.

It orders the bodies geometrically: the negative-coordinate body is first and
exposes its `Upper` side; the positive-coordinate body is second and exposes
its `Lower` side. Declaration order, package alias, instance name, Connection
member order, and allocated identity do not determine this order.

The result remains method-neutral. Semantic coincidence does not itself imply
matching nodes or select monolithic rather than partitioned execution.

## Realization-owned interface quotient

The bounded Realization selects two generated Cartesian meshes with equal
cell count per axis, continuous two-component Q1, one common Gauss rule,
replicated `f64`, one offline host worker, and the existing SPD `SolverPlan`.

It proves the interface map from topology:

- the negative mesh's upper normal coordinate equals the positive mesh's
  lower normal coordinate exactly;
- both tangential coordinate arrays are exactly equal; and
- increasing tangential multi-index gives a total vertex bijection.

No coordinate tolerance or nearest-neighbour search participates. The map is
an immutable numerical witness:

```text
local vertex on body 0 ─┐
                        ├─ quotient global vertex
local vertex on body 1 ─┘
```

Body-local vertex identities and meshes remain available for reconstruction.
Only the global algebraic map identifies the paired trace vertices. External
essential closure is then computed over quotient vertices; at least one
external complete essential side must anchor the coupled system as a whole.
An individual body need not be anchored independently.

For `n x n` cells on each half-domain, the map has `n + 1` pairs and

```text
global vertices = 2 (n + 1)^2 - (n + 1) = (n + 1)(2n + 1).
```

With two components and the left exterior side fixed, the reduced system has
`4n(n + 1)` rows.

## Assembly and finalized evidence

Every ordinary body cell still evaluates the existing elasticity
`LocalContribution`. One existing `AssemblyPlan` produces four targets:

1. the reduced quotient system used by CG;
2. the complete quotient system used for external reaction;
3. the complete cut system for the negative body; and
4. the complete cut system for the positive body.

Each cell packet maps into the first two targets and its own body-local target.
This preserves one generic assembly contract while retaining enough
independent evidence to falsify an incorrect interface.

The finalized handoff owns the sole reduced CSR source and the opaque quotient
and cut reconstruction state. After ordinary independent solution acceptance,
it reconstructs both body fields, sums external constrained reactions from the
complete quotient residual, and evaluates each cut residual `K_i u_i - f_i`.
The interface rows of those two cut residuals are reported explicitly as weak
interface actions. A free-row mask is retained because a trace vertex that
also lies on an external essential side has a cut residual containing an
inseparable support reaction; only free interface rows are reported as
coupling-equilibrium evidence. They are not inferred from postprocessed stress
samples.

The reduced matrix remains symmetric positive definite under the admitted
coercive material and global anchoring gates, so the existing conjugate-
gradient contract remains valid.

## Falsifying verification

The registered
[`solid.conforming-elasticity-pair-2d`](../verify/solid/conforming-elasticity-pair-2d/README.md)
case owns this bounded claim. It uses the unit square split at `x = 1/2`, with
`mu_L = 3`, `mu_R = 6`, `lambda = 0`, and a conservative body force `[6, 0]`
on both halves. Its continuous exact displacement has unequal interface
strains `1/2` and `1/4`, but equal stress and opposite outward traction.

The case must prove:

- direct source and exact `Eqiora.Solid.LinearElasticity@0.3.0` flattening
  produce equal reduced, quotient-full, and both body-full systems;
- the interface quotient removes every duplicated trace vertex exactly once;
- reconstructed interface traces are bit-identical;
- the exact global Q1 interpolation identities are
  `L2^2 = h^4/192` and `H1_seminorm^2 = 5h^2/96`;
- body-local weak interface resultants are `[3, 0]` and `[-3, 0]` at every
  refinement;
- independent raw recovered tractions are `[3 + 3h, 0]` and
  `[-3 + 3h, 0]`, so their sum converges as `[6h, 0]` rather than being
  mislabeled algebraic balance;
- both body-force resultants are `[3, 0]`, the external reaction is `[-6, 0]`,
  and global balance closes; and
- non-binary Connections, same-side interfaces, additional uninterpreted live
  Port Relations, unanchored coupled systems, and one-point reduced
  integration fail closed.

Lower-level quotient tests separately exercise both Cartesian axes and reject
nonmatching tangential vertex arrays before assembly. They do not widen the
registered generated-matching-mesh claim.

## Alternatives considered

### Merge the two meshes

Rejected. It obscures body and material identity and makes reconstruction and
future independent discretizations harder. The quotient expresses exactly the
needed numerical identity without altering either mesh resource.

### Add Lagrange multipliers

Rejected for this conforming slice. Multipliers introduce an additional space
and an indefinite system where shared Q1 trace nodes already provide exact
continuity.

### Use penalty or Nitsche coupling

Rejected for this slice. These are valuable nonmatching-interface choices but
require stabilization, facet quadrature, parameter, and evidence contracts
that are absent from exact matching-node continuity.

### Put interface DOFs in the Semantic Model

Rejected. Node placement and quotient identity are approximation choices and
therefore belong to Realization, not physical meaning.

### Generalize immediately to arbitrary subdomain graphs

Rejected. A generic graph abstraction would be speculative before a second
topology or method needs it. This RFC names and verifies the exact pair it
implements.

## Compatibility

This RFC changes no Semantic Kernel node, Model wire, package schema,
Connection meaning, mesh schema, assembly API, solver plan, or package release.
It adds a pair-specific package-neutral lowerer, numerical quotient witness,
finalized handoff, and solution evidence. Existing single-body elasticity
entry points retain their behavior and continue to reject live bindings.

## Nonclaims

This RFC does not implement or claim:

- arbitrary multi-domain graphs or multiple interfaces;
- nonmatching, partial, curved, or embedded interfaces;
- mortar, Nitsche, penalty, multiplier, or partitioned coupling;
- simplex, unstructured, adaptive, mixed, discontinuous, or high-order spaces;
- three-dimensional, nonlinear, finite-strain, dynamic, contact, or fracture
  mechanics;
- Stokes, Navier--Stokes, fluid packages, or fluid-structure interaction;
- distributed assembly, threaded execution, GPU execution, or multi-node
  coupled solve;
- durable vector-field, interface-action, stress, or traction artifacts; or
- differentiation of the coupled solve or interface geometry.
