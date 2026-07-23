# RFC 0039: Canonical two-dimensional isotropic elasticity realization

- Status: Accepted; bounded implementation verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0009](0009-realization-graph-v0.md),
  [RFC 0018](0018-ordered-assembly-execution.md),
  [RFC 0020](0020-local-action-kernel-boundary.md),
  [RFC 0023](0023-finalized-spatial-linear-handoff.md),
  [RFC 0037](0037-version-neutral-model-artifact-reference.md), and
  [RFC 0038](0038-canonical-tensor-structure-operators.md)
- Follow-up package application: [RFC 0040](0040-occurrence-bound-field-slots.md)

## Summary

Eqiora admits one exact two-dimensional, small-strain, isotropic elasticity
subset from an ordinary canonical Relation network and realizes it as
continuous Cartesian Q1 finite elements, ordered local contributions, CSR,
and conjugate gradient execution.

The Semantic Model owns two continuum Fields on one exact Cartesian volume:

- a `[2]` `SpatialCartesian` displacement `u`; and
- an invariant scalar conservative-load potential `q`.

One pointwise Relation defines `q` from a supported scalar spatial expression.
The balance Relation is

```text
-div(
  2 * mu * symmetric_part(grad(u))
  + lambda * isotropic_lift(div(u))
) - grad(q) = 0.
```

Every Cartesian side has an explicit homogeneous displacement-trace Relation.
Method-neutral lowering proves that exact closed structure and lowers the
definition of `q` to one immutable scalar spatial tape. It does not select a
mesh, basis, quadrature, sparse layout, solver, or execution target.

One exact reference Realization then selects a generated 2D Cartesian mesh,
componentwise Q1 space, two-point tensor-product Gauss quadrature, the existing
local-contribution and ordered-assembly contracts, replicated `f64` CSR, and a
host-serial conjugate-gradient solve. The two semantic Fields do not imply a
mixed numerical method: `q` is explicitly prescribed by its canonical
Relation and is eliminated by method-neutral lowering before displacement
assembly.

The original slice deliberately proves that path from a direct Model. The
subsequent RFC 0040 application provides the same balance Relation through the
public
`Eqiora.Solid.LinearElasticity.IsotropicBalanceWithPotential2d` Component.
That package owns only the reusable support and Field obligations, Lamé
Parameters, and balance meaning; the enclosing root still owns the load and
boundary closure. The packaged application passes this RFC's unchanged
package-neutral lowerer and numerical path, so it extends the evidence without
rewriting the bounded decision made here.

## Motivation

RFC 0038 closed the canonical tensor operations needed to state isotropic
elasticity without a physics-specific Kernel node. It deliberately stopped
before spatial realization. A numerical example built directly from Lamé
coefficients and a Q1 stiffness routine would not close that gap: it could
silently use different tensor, support, sign, or load semantics from the
accepted Model.

This RFC closes the smallest execution slice that can falsify that separation:

```text
canonical two-Field Model v4
        |
        | typed, name-independent semantic recognition
        v
method-neutral elasticity model
        |
        | resolved Realization v1
        v
2D Cartesian Q1 local operators
        |
        v
ordered AssemblyMap -> CSR -> CG -> accepted Run v2 lineage
```

The original slice is intentionally a direct Model, not a Model Package.
Packaging the verification root would test exact distribution again but would
not create a reusable material library. At the time this RFC was accepted, the
public Component surface could bind scalar Parameters, Ports, and spatial
supports, but could not cleanly bind a volume Field or spatial load
definition. Turning the same source into a Component would therefore have
forced it to own the displacement, load, and fixed boundary topology together:
a benchmark fixture wearing a material-package name. RFC 0040 later supplied
the missing occurrence-bound Field seam and enabled a narrower reusable
balance Component without changing this original verification root.

## Canonical model subset

### Spatial support and Fields

The lowerer admits exactly one two-dimensional Cartesian box Domain and its
four exact axis-aligned boundary Domains. It requires exactly two continuum
Fields defined on the box:

```text
u : displacement, dimension m, shape [2], frame SpatialCartesian
q : load potential, dimension Pa, shape [], frame Invariant
```

The exact Field identities are taken from the committed `KernelProgram`.
Source declaration names, file names, insertion order, and allocation order
are not recognition keys.

The scalar potential is a Model quantity because `grad(q)` is the conservative
body-force field. It is not a solver callback, mesh array, Realization
parameter, or global support-free scalar. A unique Relation on the same volume
has the exact pointwise form

```text
q - q_hat(coordinate, scalar_parameters) = 0,
```

where `q_hat` lowers through the existing dimension-checked scalar spatial
expression contract. Constants, scalar Parameters, in-volume coordinates,
closed scalar arithmetic, and supported unary mathematics are admitted only as
that existing contract permits. No finite-difference derivative or Python
callback is introduced: coordinate JVPs of the immutable tape evaluate
`grad(q_hat)` at quadrature points.

### Constitutive and balance meaning

For constant scalar Parameters `mu` and `lambda`, define

```text
epsilon(u) = symmetric_part(grad(u))
sigma(u)   = 2 mu epsilon(u) + lambda isotropic_lift(div(u)).
```

The lowerer recognizes the typed expression structure and retains both
constant coefficient expressions as immutable tapes, including exact
Parameter identities at the canonical revision. It accepts either ordering of
the two additive stress terms but does not perform general symbolic
equivalence, guess missing factors, or infer a material law from names.

The balance root must be exactly

```text
-div(sigma(u)) - grad(q) = 0.
```

Its result is a `[2]` spatial Cartesian residual with force-per-volume
dimension on the exact box. The coefficient gate is the coercivity condition
for the adopted two-dimensional constitutive law:

```text
mu > 0
lambda + mu > 0.
```

Both coefficients must be finite. This is an intrinsic two-dimensional law;
it is not a plane-stress or plane-strain reduction from a three-dimensional
material.

### Boundary meaning

Each of the four exact boundaries must own one Relation with

```text
trace(u) = 0.
```

The residual remains shaped and componentwise. Missing, duplicate, natural,
nonzero, or unrelated boundary Relations fail this lowerer. Homogeneous
essential elimination is a Realization operation over already accepted Model
meaning; the zero value and exact boundary identity are not supplied by the
mesh.

The closed benchmark has no Port. A Port denotes a reusable connection
boundary and would add nominal connector, exposure, and connection-set
semantics that this boundary-value problem does not exercise.

## Method-neutral lowering

The lowered elasticity model contains only facts proved from the Semantic
Model:

- exact Domain, displacement-Field, and load-potential-Field identities;
- exact physical Cartesian bounds;
- finite coercive `mu` and `lambda` values;
- the immutable spatial tape defining `q`; and
- proof that every box side has homogeneous displacement trace.

It contains no mesh density, Q1 basis, quadrature points, local matrix, sparse
format, solver tolerance, preconditioner, worker count, or target. The
lowering is fail-closed: a third Field, a second box, an unconstrained `q`, an
unsupported load expression, a different stress composition, incomplete
boundary closure, a non-continuous Activation, different continuum
Representation identities, or any unrelated Kernel node produces a diagnostic
rather than an approximate model.

This reference lowerer consumes one explicit closed normal form. Algebraically
equivalent spellings such as `q_hat - q = 0`, moving `grad(q)` inside the outer
negation, placing `lambda` inside `isotropic_lift`, or adding coordinate terms
that cancel identically remain valid canonical Models but are not normalized
into this execution subset. Keeping one stated form is preferable to a growing
collection of matcher permutations; future symbolic normalization should be a
separate shared compiler contract.

Material data can therefore affect Model identity only. Mesh, quadrature,
solver, and placement can affect Realization identity only. Verification must
show both directions: changing a numerical choice changes the Realization
without changing Model bytes, while changing a Lamé coefficient invalidates
an old Realization-to-Model reference.

## Q1 realization

### Space and degree-of-freedom identity

The accepted reference space is continuous tensor-product Q1 on a generated
Cartesian mesh. One canonical displacement component is attached to each
vertex. Algebraic degree-of-freedom identity is the checked pair

```text
(vertex identity, component in 0..2),
```

with component varying fastest inside one vertex. This ordering is numerical
identity, not a decomposition of the canonical Field into unrelated scalar
Fields.

On one quadrilateral cell there are four nodes and eight displacement degrees
of freedom. Basis gradients are transformed through the existing affine
geometry map. The bilinear form is

```text
a(u, v) = integral(
  2 mu epsilon(u) : epsilon(v)
  + lambda div(u) div(v)
).
```

The load is

```text
l(v) = integral(v dot grad(q_hat)).
```

Two Gauss points per reference axis exactly integrate the affine-geometry Q1
stiffness polynomial and provide the declared deterministic load quadrature.
The local operator returns one ordinary `LocalContribution`; it does not
define an elasticity-specific assembly API.

### Assembly and solve

Assembly uses the existing `AssemblyMap`, indexed work, ordered backend, and
CSR contracts. It produces both:

- the reduced free-displacement system after homogeneous essential
  elimination; and
- the complete uneliminated residual action used for reaction recovery.

The reduced matrix is symmetric positive definite for this exact closed
Dirichlet problem and the admitted coefficient gate. The reference path uses
replicated `f64` storage and conjugate gradient through an ordinary
`SolverPlan`. Independently recomputed true residual acceptance remains the
solver gate; a convergence reason alone is insufficient.

The reference capability is exact to generated Cartesian, continuous
Galerkin, spatial dimension two, replicated `f64`, and one host worker. It is
not an alias for the scalar-elliptic capability and does not widen that
capability's field-shape claim.

### Identity lineage

The canonical tensor vocabulary requires explicit Model v4. The resulting
version-neutral Model artifact reference may feed unchanged Realization v1 and
Run v2 identity lineage under RFC 0037. The reference Run may remain
output-less: the in-memory displacement and reactions gate the evidence, but
this RFC does not invent a durable vector-field or stress-result artifact.

No package compilation or package execution binding participates in the
original direct-Model slice. The complementary RFC 0040 package application
records its package lineage independently.

## Falsifying verification

The registered
[`solid.isotropic-elasticity-2d`](../verify/solid/isotropic-elasticity-2d/README.md)
case owns the bounded claim.

### Local operator

For one affine unit cell, the case must prove:

1. the eight-by-eight local stiffness is symmetric;
2. both rigid translations and the infinitesimal rigid rotation have zero
   energy to scaled floating-point tolerance;
3. pure shear and uniform dilatation match the analytical strain energies;
4. cross-component blocks contain nonzero coupling and, together with the
   exact affine patch reactions, prevent accidental reduction to two scalar
   diffusion problems; and
5. noncoercive or non-finite Lamé coefficients fail before assembly.

### Assembly patch

A two-by-two affine Cartesian patch receives algebraic nodal values from an
exact affine displacement. Q1 reconstruction must reproduce its constant
gradient and strain, the interior assembled residual must vanish under the
constant stress, and boundary resultants must agree with that stress. The
patch vector is a lower-level operator/assembly oracle; it does not author or
claim a nonzero public vector boundary condition in the canonical language.

### Manufactured solution

On the unit square let

```text
s_x = sin(k x)
s_y = sin(k y)
q   = q0 (s_x^2 + s_y^2 - 4 s_x^2 s_y^2)
Psi = q0 / (2 k^2 (lambda + 2 mu)) s_x^2 s_y^2
u*  = -grad(Psi),
```

with `k = pi / m`. Then `u*` vanishes on the complete boundary and

```text
q = (lambda + 2 mu) Delta(Psi),
div(sigma(u*)) = -grad(q).
```

The exact Model represents `u`, `q`, the pointwise definition of `q`, the
balance, and all four boundary Relations. A mesh sequence must show monotone
continuous displacement L2 error and an observed order of at least 1.9 over
the declared asymptotic levels.

### Componentwise equilibrium

Reaction recovery uses the complete uneliminated residual. For each Cartesian
component independently,

```text
boundary_reaction[i] + integral(grad(q)[i]) = 0
```

must hold to the registered relative tolerance. Because the symmetric
manufactured load has zero net force, the same evidence target also uses a
minimal affine-potential falsifier with a nonzero componentwise integral; zero
reaction and zero load cannot satisfy that check accidentally.

### Realization and artifact separation

The case must additionally prove that:

- Model v4 canonical bytes and digest are unchanged across at least two valid
  mesh or solver plans;
- those numerical changes alter Realization identity;
- Model v4, Realization v1, and Run v2 replay through their explicit bounded
  decoders; and
- a changed Model coefficient, Model digest, semantic revision, or
  Realization digest fails exact lineage replay.

## Why the original slice did not include an elasticity package

An exact package containing only this root Model would have been a legitimate
way to distribute the benchmark, but it would have added no reusable physics
abstraction. It would also have combined package resolution with the first
vector PDE execution claim, making failures harder to localize. Deferring that
composition until the hierarchy could bind existing Fields was therefore an
intentional sequencing decision, not a claim that elasticity could not be
packaged.

RFC 0040 subsequently provided the typed occurrence-bound Field seam without
transferring mesh arrays. The public
`Eqiora.Solid.LinearElasticity.IsotropicBalanceWithPotential2d` Component now
owns one two-dimensional volume support slot, displacement and load-potential
Field slots, the two Lamé Parameters, and the isotropic balance Relation. The
root owns the exact volume and four boundary Domains, the two continuum Fields,
the load-definition Relation, and all four homogeneous trace Relations.

The registered
[`solid.packaged-isotropic-balance-2d`](../verify/solid/packaged-isotropic-balance-2d/README.md)
case proves that this division is semantic rather than an executor shortcut:

- occurrence elaboration is equivalent to the explicit flat Model under a
  complete deterministic identity normalization;
- dependency aliases and declaration, binding, and file order cannot change
  the flattened meaning;
- the existing name-independent method-neutral lowerer and Q1/CSR/CG path run
  unchanged, including when the provider package is renamed;
- the packaged and explicit Models produce identical solutions over the
  four-level `4, 8, 16, 32` convergence sequence, with the registered L2 and
  H1 convergence gates;
- a separate affine-potential case preserves the nonzero integrated force
  `[1, 0]` and componentwise boundary-reaction balance; and
- exact package compilation, Model v4, Realization v1, Run v2, and package
  execution-binding lineage all replay independently.

Neither package owns Q1, mesh generation, quadrature, assembly, CSR, solver,
target, or schedule choices. Static traction interfaces, boundary collections
or partitions, field-valued physical Ports, and broader solid or coupled
physics remain separate contracts.

## Alternatives considered

### Put isotropic elasticity in a Kernel node

Rejected. RFC 0038 already supplies the physics-neutral tensor operations.
One elasticity node would create a second constitutive semantics and make
extensions such as anisotropy depend on Kernel growth.

### Scalarize displacement before semantic validation

Rejected. Two unrelated scalar Fields lose exact vector shape, frame, and
tensor composition and cannot falsify cross-component stiffness coupling.

### Solve `q` as a second Q1 unknown

Rejected for this slice. The canonical `q` Field is exactly prescribed by a
pointwise Relation. Giving it independent algebraic degrees of freedom would
turn an eliminable load definition into an unrequested mixed discretization
and introduce a space/stability decision absent from the Model.

### Put Lamé coefficients or load callbacks in Realization

Rejected. They change the continuous Relation and therefore Model identity.
Realization may choose how an accepted spatial tape is evaluated, not replace
its material or forcing meaning.

### Publish a verification-specific Component package

Rejected for the original slice. It would either have hidden the displacement
and load inside an unusable closed Component or exposed an interface whose
Field semantics had not yet been defined. The later RFC 0040 application is
not that whole-root fixture: it binds root-owned Fields into a reusable balance
Relation and leaves load and boundary closure outside the package.

### Generalize immediately to 3D, simplex, mixed, or nonlinear solids

Rejected. Each changes a distinct contract: dimension, reference topology,
space pairing, constitutive linearization, or nonlinear solve. The exact 2D Q1
slice is sufficient to falsify the current semantic-to-numerical boundary.

## Failure modes

The method-neutral lowerer fails before numerical allocation for:

- anything other than one exact 2D Cartesian box;
- anything other than one length-valued `[2]` spatial displacement and one
  pressure-valued scalar potential on the same continuum Representation;
- any non-continuous load, balance, or boundary Activation;
- a missing, duplicate, ambiguous, or unsupported potential definition;
- a missing or structurally different balance Relation;
- non-finite `mu` or `lambda`, `mu <= 0`, or `lambda + mu <= 0`;
- missing, duplicate, nonhomogeneous, or non-trace boundary closure; or
- any extra Kernel node that a whole-model execution would otherwise ignore.

The Realization fails before assembly or solve for:

- a non-2D mesh, non-Cartesian mesh, non-Q1 space, or non-Gauss rule;
- inconsistent geometry and quadrature reference cells;
- checked entity, local-width, global-DOF, packet, or allocation overflow;
- non-finite geometry, load, local coefficients, or assembled entries;
- a Realization referencing a different Model identity or semantic revision;
  or
- a solver or target outside the exact admitted capability.

## Compatibility

This RFC adds a fail-closed numerical lowerer and one exact capability. It does
not change the Semantic Kernel, RFC 0038 operator meaning, Model v4 grammar,
generic Realization plan fields, assembly contracts, CSR schema, or solver
contracts. Models outside the exact subset remain valid canonical Models; they
are simply not accepted by this specialized reference lowerer.

## Nonclaims

This RFC does not implement or claim:

- within its original direct-Model slice, package compilation or execution
  binding; the complementary bounded package application is owned by RFC 0040;
- a broad elasticity constitutive library or general solid component
  hierarchy;
- public physical Ports, traction coupling, fluid-structure interaction, or
  mesh transfer;
- plane stress, plane strain, shell, beam, or a reduction from 3D;
- three-dimensional execution;
- traction or mixed essential/natural boundary conditions;
- nonzero public vector boundary authoring or execution;
- unstructured, simplex, adaptive, mixed, high-order, or discontinuous spaces;
- incompressible or nearly incompressible mixed methods;
- anisotropic, heterogeneous, nonlinear, finite-strain, plastic, contact, or
  damage laws;
- dynamic elasticity, inertia, damping, time integration, or wave propagation;
- distributed, threaded, GPU, or multi-node execution;
- stress, strain, vector-field, or reaction artifact schemas; or
- design differentiation, adjoints, or topology/shape optimization.
