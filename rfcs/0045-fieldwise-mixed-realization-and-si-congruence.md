# RFC 0045: Field-wise mixed Realization and coherent-SI congruence

- Status: Accepted and verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0009](0009-realization-graph-v0.md),
  [RFC 0013](0013-realization-and-run-provenance-wire.md),
  [RFC 0037](0037-version-neutral-model-artifact-reference.md),
  [RFC 0043](0043-simplicial-mini-stokes-realization.md), and
  [RFC 0044](0044-packaged-steady-incompressible-newtonian-2d.md)

## Summary

Eqiora will represent a mixed spatial Realization as an exact assignment from
canonical Domain and algebraic-unknown Field identities to numerical bindings,
together with exact Realization-owned constraint identities. A generic L2 congruence
contract records one positive coordinate scale, one positive scale for every
solved Field or algebraic constraint block, and one positive common weak-
functional scale. It therefore defines, without changing Model meaning, the
dimensionless symmetric problem

```text
x        = D x_hat,
A_hat    = D A D / Theta,
b_hat    = D b / Theta.
```

`D` is stored blockwise and expanded only after the selected discrete layout
is known. `Theta` has the physical dimension common to every term of the
symmetric weak functional. The contract admits a mixed operator only when
every block has the dual dimension required for this congruence.

The first adapter is the coherent-SI two-dimensional steady Stokes subset from
RFC 0044. Its public native scaling input is three positive quantities:

```text
L : length,
U : velocity,
P : pressure.
```

The adapter derives, rather than separately accepting,

```text
G     = U / L,       // pressure-gauge multiplier scale
Theta = P U L,       // two-dimensional weak-functional scale
D     = diag(U on velocity, P on pressure, G on gauge).
```

The implementation must assemble the dimensionless congruent operator
directly. It must not materialize one raw `f64` matrix whose rows and columns
silently mix SI dimensions. A small independent verification oracle may form
the conceptual base-SI action and prove that direct dimensionless assembly is
equivalent to `D A D / Theta`.

The generic contracts, canonical Realization v2 persistence, version-neutral
Run reference, equation-aware Stokes adapter, finalized MINI handoff, and
coherent-SI reconstruction are implemented. The registered
`fluid.fieldwise-si-mini-stokes-2d` case joins direct and exact-package
authoring to that path for the bounded fixture specified below.

## Motivation

RFC 0043 verifies a stable nondimensional MINI numerical method, pressure
gauge, symmetric-indefinite operator, and reference MINRES. RFC 0044 separately
verifies a dimensional canonical and exact-packaged steady Stokes law. Joining
those two cases by translating a single global `Space` or by passing SI base-
unit numbers to the nondimensional entry point would leave three important
facts implicit:

1. velocity and pressure select different approximation spaces;
2. the pressure gauge is a Realization-owned algebraic constraint, not Model
   meaning; and
3. a Euclidean residual over unscaled momentum, continuity, and gauge rows is
   not meaningful when those rows have different physical dimensions.

The bridge must make these facts content identity. It must also remain useful
outside fluid mechanics. The L2 contract therefore knows exact resources,
unknown-Field bindings, constraints, block scales, and a weak-functional scale,
but it does not know the words velocity, pressure, MINI, or Stokes. The Stokes
adapter owns that interpretation and derives a complete generic contract.

This preserves the repository's central direction:

```text
Semantic meaning
        |
        v
package-neutral canonical Stokes contract
        |
        v
field-wise mixed Realization + SI scale profile
        |
        v
dimensionless symmetric operator
        |
        v
backend-neutral finalized solve and physical reconstruction
```

No package name, dependency alias, or source declaration order reaches the
numerical adapter.

## Accepted decision and current claim boundary

This RFC accepts exactly these architectural decisions:

- mixed spatial policy is an exact Field-ID assignment rather than one global
  space or a physics-named method tag;
- algebraic normalization is a symmetric block congruence with one common
  weak-functional scale;
- coordinate, Field-block, and constraint-block scales are Realization
  identity and never Model meaning;
- the first Stokes surface accepts only positive `L`, `U`, and `P`, from which
  it derives `G`, `Theta`, and every block of `D`;
- production assembly emits the dimensionless congruent operator directly;
  mixed-unit raw algebra exists only as bounded verification reasoning; and
- the field-wise durable shape is a new Realization wire generation rather
  than a reinterpretation of v1.

The registered bridge now joins those decisions into one bounded executable
claim. RFC 0043 still owns general MINI convergence/stability evidence and RFC
0044 still owns byte-level direct/package semantic normalization; neither is
silently widened by the affine bridge fixture.

## Normative generic L2 contract

### Exact resource identity

A field-wise mixed Realization is closed over one exact canonical Model
artifact and semantic revision. It records:

- every realized Domain by its exact typed identity;
- every algebraically solved Field by its exact typed identity and space;
- every algebraic constraint by an exact Realization-owned identity;
- the Domain on which each Field and integral constraint acts; and
- the complete numerical plan, target, layout, and schedule.

Field names are diagnostic metadata only. Lookup by name, insertion order,
package provider, dependency alias, or structural guessing is forbidden.
Bindings are sorted canonically by typed identity in durable encoding.

Every algebraic unknown required by the admitted lowerer must be bound exactly
once. Canonical Fields used only as immutable coefficient data remain in the
linked Model and lowerer; they do not acquire an empty numerical treatment or
allocate solve degrees of freedom. An unrecognized unknown, missing binding,
duplicate binding, or unconsumed constraint fails before mesh construction.

### Unknown-Field bindings and mixed spaces

Each binding contains one solved Field identity and one numerical basis family.
The common realized Domain is a separate exact identity. A space describes
basis and conformity; it carries no physical meaning. Field shape determines
how the scalar basis is replicated or scalarized. The first admitted mixed
combination is deliberately narrow:

```text
velocity         simplex P1 plus one normalized cell bubble, two components
pressure         continuous simplex P1, scalar
force potential  retained canonical coefficient data; no unknown allocation
```

The combination is selected by the Stokes adapter after checking the exact
canonical roles. The generic contract does not add a universal `MixedSpace`
tag whose name substitutes for the actual field assignments.

### Constraint identity

A constraint is a Realization resource. Its durable key is the exact Model and
Domain context plus its kind and target Field identity. The first kind is

```text
ZeroIntegral {
  field: pressure,
}
```

On one connected pressure domain this constraint introduces exactly one
scalar multiplier block. The multiplier is not a Field, Parameter, pressure,
source, stabilization coefficient, or semantic Relation. Its identity and
scale nevertheless enter Realization content because they change the
algebraic problem.

A disconnected admitted mesh cannot reuse one global mean constraint. The
first adapter rejects it before assembly rather than silently adding or
omitting multipliers.

### Coordinate scale

Each realized spatial Domain has one exact positive finite coordinate scale.
It is a quantity with length dimension and is part of Realization identity.
For the first Cartesian Stokes adapter, normalized coordinates use the exact
canonical lower corner as origin and `L` as the common reference length:

```text
x_hat = (x - x_lower) / L.
```

`L` need not equal every side length. The resulting normalized Domain may be
rectangular. Anisotropic coordinate scales, curvilinear charts, moving
coordinates, and mesh-dependent local normalization require later contracts;
the first version must not infer them.

The coordinate scale is numerical policy. Changing it changes Realization
identity, never canonical Model bytes or physical geometry.

### Symmetric block congruence

Let the solved and auxiliary coefficient blocks be indexed by `i`. The
Realization stores one positive finite quantity `s_i` per block and one
positive finite `Theta`. Conceptually,

```text
D = block_diag(s_0 I, s_1 I, ..., s_n I).
```

Component and degree-of-freedom expansion is deterministic from the finalized
layout. Callers cannot supply a free per-DOF array, because that would obscure
field identity and permit mesh ordering to enter policy.

For a symmetric dimensional weak system `A x = b`, the dimensionless problem
is

```text
A_hat x_hat = b_hat,
A_hat = D A D / Theta,
b_hat = D b / Theta,
x     = D x_hat.
```

Since `D` is real, positive, and diagonal by blocks, congruence preserves
symmetry and inertia. A symmetric-indefinite assertion therefore remains
valid across the boundary. A left-only or right-only transformation is not
this contract.

The equation-aware adapter requires every conceptual matrix block and load block to have
dimensions

```text
dimension(A_ij) = dimension(Theta / (s_i s_j)),
dimension(b_i)  = dimension(Theta / s_i).
```

Zero structural blocks need no invented physical unit, but their row and
column identities remain fixed. Generic L2 validates positive scales and exact
block coverage; the adapter validates these dual dimensions before any raw
scalar enters a solver.

### Residual and reconstruction

Solver acceptance uses only the dimensionless residual

```text
r_hat = A_hat x_hat - b_hat
      = D (A x - b) / Theta.
```

The implementation must not concatenate dimensional residual blocks and call
their raw Euclidean norm a physical or dimensionless residual. Independent
evidence may additionally report each normalized block residual.

Reconstruction multiplies every solved and auxiliary block by its exact scale
and returns physical SI values associated with the original Field or
constraint identity. Scale profiles are numerical coordinates, not output
units. Two valid scale profiles for the same Model may produce different
dimensionless systems and Realization digests but must reconstruct the same
accepted physical solution within the declared numerical tolerance.

## Normative Stokes adapter

### Admitted canonical meaning

The first adapter accepts exactly the package-neutral canonical contract from
RFC 0044:

```text
-div(2 mu symmetric_part(grad(u)) - isotropic_lift(p))
  - grad(q) = 0,
div(u) = 0.
```

It retains the exact two-dimensional Cartesian volume, velocity, pressure,
force-potential Fields, positive dynamic viscosity, force-potential scalar
tape, and complete homogeneous velocity trace. It does not dispatch on the
fluid package identity. Direct-flat and exact-package authoring must enter the
same adapter through their ordinary canonical lowerings.

### Native scale profile

The public Stokes scale profile contains only:

```text
L > 0 with dimension length,
U > 0 with dimension velocity,
P > 0 with dimension pressure.
```

All values must be finite. The adapter derives:

```text
coordinate scale       L,
velocity block scale   U,
pressure block scale   P,
gauge block scale      G = U / L,
weak-functional scale  Theta = P U L.
```

For an intrinsic two-dimensional weak form, `Theta` has dimension power per
out-of-plane length. The case reports force resultants per unit out-of-plane
thickness. It does not silently manufacture a three-dimensional thickness.

The numerical values of `L`, `U`, and `P` are conditioning choices. The
adapter does not require `P = mu U / L`; instead, the dimensionless viscous
coefficient retains the physically correct ratio `mu U / (P L)`. Requiring a
particular balance would confuse one useful characteristic scaling with
semantic validity.

### Direct dimensionless assembly

Production assembly evaluates normalized geometry, canonical prescribed data,
and scale ratios inside the local operator and emits `A_hat` and `b_hat`
directly. The ordered local-contribution and reduced/full assembly contracts
remain unchanged. Neither finalized CSR owns a mixed-unit interpretation.

For verification only, a bounded oracle independent of the SI scaling adapter
may reuse the already verified MINI local assembly on the physical mesh,
assemble the conceptual coherent-SI weak blocks in base-unit scalar values,
and require

```text
A_hat z = D (A (D z)) / Theta
```

for deterministic probes with nonzero velocity, pressure, and gauge entries.
The oracle is not the production path, a second Realization, or permission for
general code to materialize dimensionally heterogeneous matrices.

The reduced operator is asserted as `SymmetricIndefinite`. The first exact
solver tuple remains the RFC 0043 reference choice:

```text
backend         eqiora.reference
algorithm       MinimumResidual
preconditioner  Identity
reduction       Reproducible
scalar          f64
```

Tolerance and iteration limits are exact `SolverPlan` content. Conjugate
gradient, a general-operator substitution, or an unimplemented MINRES
preconditioner fails capability admission rather than falling back.

## Versioned artifact boundary

The existing `eqiora.realization-envelope/v1` remains frozen. Its one global
space and v1 solver vocabulary are not reinterpreted as field-wise policy.
The new shape requires `eqiora.realization-envelope/v2` with private wire DTOs
decoded through validated constructors.

V2 contains:

- exact version-neutral Model artifact reference and semantic revision;
- sorted Domain, unknown-Field/space, and constraint identities;
- exact coordinate and symmetric block scale quantities;
- exact `Theta` quantity;
- mesh, quadrature, layout, solver, target, and schedule policy; and
- content-addressed imported mesh/layout inputs when selected.

Quantity wire values use coherent-SI scalar values plus exact SI exponent
vectors. Unknown wire members are denied and counts are bounded before
allocation. Typed resource membership and dimensions are revalidated against
the linked Model and admitted lowerer before numerical lowering.

A sealed version-neutral Realization reference may let existing Run-manifest
v2 derive the common Model, revision, Realization digest, layout, target, and
reduction facts from either accepted envelope version. This API refactor must
not change existing Run-manifest v2 bytes or make a manifest an execution
attestation. Exact package-to-execution lineage remains a separately validated
edge.

## Registered falsifying verification

The verified registered case is `fluid.fieldwise-si-mini-stokes-2d`.

### Fixture

The fixture reuses the exact
`Eqiora.Fluid.Incompressible@0.1.0::SteadyStokesWithPotential2d` release and
provides direct-flat and exact-package roots for

```text
Omega = (0 m, 4 m) x (0 m, 2 m),
mu    = 6 Pa s,
q     = 3 Pa (x / (4 m) - 1/2),
u     = 0,
p     = q.
```

Hence

```text
grad(q)                    = (0.75 Pa/m, 0),
integral_Omega p           = 0,
integrated body force      = (6 N/m, 0),
complete boundary reaction = (-6 N/m, 0).
```

The affine pressure is represented exactly by P1. This bridge case therefore
checks identity, scaling, algebra, reconstruction, gauge, and balance rather
than adding a second MINI convergence claim. RFC 0043 remains the independent
manufactured convergence and stability evidence.

Two nontrivial valid profiles are verified:

```text
profile A: L = 4 m, U = 0.5 m/s, P = 0.75 Pa
           G = 0.125 1/s, Theta = 1.5 W/m

profile B: L = 4 m, U = 1.0 m/s, P = 1.5 Pa
           G = 0.25 1/s, Theta = 6.0 W/m
```

They must retain identical Model bytes and reconstruct equivalent physical
fields while producing distinct Realization identities.

### Acceptance

The registered evidence must require:

- ordinary canonical role recognition for both direct and exact-package
  authoring, with each result bound to its own exact Semantic identities; the
  separate RFC 0044 case retains byte-level verification-private identity
  normalization rather than duplicating it here;
- exact Field-ID-bound velocity and pressure spaces, retained canonical
  force-potential data, and one exact mean-pressure constraint;
- byte-identical v2 decode/re-encode and domain-separated digest replay;
- a linked Run-manifest v2 whose bytes retain their existing schema meaning;
- exact symmetry of the finalized dimensionless CSR;
- coefficientwise equivalence between direct dimensionless assembly and
  `D A D / Theta`, including the complete reduced velocity, pressure, gauge,
  and right-hand-side blocks;
- independently recomputed dimensionless true-residual acceptance;
- zero physical velocity, affine physical pressure, zero pressure mean, and
  zero physical gauge multiplier within declared scaled tolerances;
- componentwise physical reaction plus body-force balance;
- direct/package equality of dimensionless algebra, reconstructed physical
  fields, and reported evidence; and
- distinct profile-A/profile-B Realization digests with equivalent recovered
  physical results.

Quantitative tolerances belong to the case manifest and executable expected
contract after the first implementation measures the deterministic reference
path. This RFC does not freeze unmeasured constants as architectural truth.

### Required falsifiers

The case and lower-level tests must reject or detect:

- missing, duplicate, unrelated, or unknown Field bindings;
- a force-potential Field incorrectly added to the algebraic unknown inventory;
- scalar/vector shape, frame, support, Representation, or physical-dimension
  mismatch;
- an unsupported velocity/pressure space pair;
- a missing, duplicate, or non-pressure mean constraint;
- a disconnected mesh under one global gauge;
- zero, negative, non-finite, or dimensionally invalid `L`, `U`, or `P`;
- missing, forged, independently supplied, or incorrectly derived `G` or
  `Theta`;
- a per-DOF scale array whose ordering is not derived from block identity;
- left-only scaling, right-only scaling, or inverse physical reconstruction;
- an implementation that reports a mixed-dimensional raw residual norm;
- quadrature below the MINI degree-four assembly requirement;
- loss of finalized CSR symmetry;
- conjugate gradient, `General` operator substitution, or unsupported MINRES
  preconditioning;
- Model digest, semantic revision, Domain identity, Field identity, mesh
  artifact, or scale-profile drift during replay;
- unchanged Realization identity after a scale change; and
- Run reduction, worker topology, or Realization-digest drift.

No failed path may produce accepted solution evidence or a run binding that
purports to attest it.

## Alternatives considered

### Nondimensionalize the Stokes PDE before the generic Realization boundary

This naturally exposes Reynolds-like ratios and can be convenient in a
fluid-only code. It makes the artifact derive its meaning from a particular
physics equation, however, and leaves field identity, auxiliary constraints,
and non-fluid mixed systems without a common contract. Rejected as the L2
boundary. The Stokes adapter still performs equation-aware scale derivation
before emitting the generic congruent operator.

### Assemble one raw mixed-unit `f64` matrix and scale it afterward

This mirrors the mathematical formula literally and is useful as a small
independent oracle. In production it gives an ordinary scalar array a false
single-unit interpretation and permits consumers to compute meaningless raw
residual norms. Rejected for execution. Direct dimensionless assembly is the
normative path.

### Independent row and column equilibration

`R^-1 A D` is more general and can improve conditioning for nonsymmetric
problems. Unless the row and column scales are duals of one common functional,
it does not preserve symmetry and cannot retain the exact MINRES property.
Rejected as canonical scaling for this symmetric mixed slice. Backend-owned
equilibration may later be modeled as a distinct preconditioner with its own
evidence.

### Put reference scales or pressure gauge in the Semantic Model

Both choose numerical coordinates or a representative of a continuous
equivalence class. Changing either must not change physical meaning or package
identity. Rejected; they belong to Realization.

### Add one global mixed-space enum

A `MiniStokes` tag would be concise but would hide which exact Field receives
which space and would not compose with a third solved Field.
Rejected in favor of the closed Field-ID assignment.

### Extend Realization envelope v1 in place

Optional field maps and scales would change or ambiguously reinterpret frozen
v1 meaning. Rejected. V2 is an explicit wire generation, while a sealed
version-neutral reference shares only the identity facts needed downstream.

## Staged implementation

### Stage 1: Pure L2 contracts (implemented)

- Add exact Domain/unknown-Field bindings and Realization-owned constraint types.
- Add positive dimensioned coordinate, block, and weak-functional scales.
- Validate closed identity inventories and exact block-scale coverage; leave
  equation-specific dual-dimension checks to the admitted adapter.
- Add the first simplex P1-bubble and positive Duffy quadrature policy
  vocabulary without adding fluid names to generic crates.

Stage 1 has no numerical-execution claim.

### Stage 2: Realization v2 and lineage (implemented)

- Add bounded canonical v2 encoding and replay.
- Keep every v1 canonical byte regression unchanged.
- Introduce the sealed version-neutral Realization reference needed by
  unchanged Run-manifest v2 wire semantics.
- Reject Model, revision, resource, scale, layout, target, and reduction drift.

Stage 2 proves identity and policy persistence, not a Stokes solve.

### Stage 3: Stokes adapter and finalized handoff (implemented)

- Map the RFC 0044 canonical roles to exact unknown-Field bindings.
- Derive `G`, `Theta`, and generic `D` from positive `L`, `U`, and `P`.
- Lower the canonical force-potential tape in normalized coordinates.
- Assemble reduced and full dimensionless systems directly.
- Reuse the RFC 0043 reference MINRES and reconstruct coherent-SI evidence.

No package dispatch or backend-specific type may enter this adapter.

### Stage 4: Registered evidence (implemented)

- Add `fluid.fieldwise-si-mini-stokes-2d` with the exact fixture above.
- Prove direct/package numerical equality and two-profile physical invariance.
- Exercise all listed falsifiers.
- Update capability and roadmap claims only after the registered local gate
  passes.

Completion of Stage 4 establishes only the bounded end-to-end
canonical/package SI Stokes execution claim stated by the registered case.

## Compatibility and migration

This RFC changes no Semantic Kernel node, Model wire, Transaction wire,
package schema, package release, Connection meaning, or existing numerical
case. Realization envelope v1 and Run-manifest v2 canonical bytes remain
unchanged. Existing scalar and elasticity plans continue to use their current
contracts.

There is no automatic migration from v1 to v2. A caller must explicitly bind
the exact Fields and constraints and choose a complete scale profile. Guessing
field roles from names, copying one global v1 space to every Field, or deriving
scales from current floating-point values is forbidden.

## Security, safety, and governance

All dimensioned values are finite and resource-bounded before layout
expansion. Checked arithmetic guards block and degree counts. Durable decoding
denies unknown wire members and reconstructs every quantity, typed identity,
space, constraint, and plan through validated constructors. The numerical
adapter then proves Model membership and physical dimensions. Imported mesh
and layout artifacts retain their independent content validation.

Scale values can strongly affect conditioning even when mathematically valid.
An accepted scale profile proves dimensional consistency and identity, not
good performance. Solvers must still independently verify their true
dimensionless residual, and evidence must report the exact scale profile.

Changing the v2 digest domain, identity derivation, scale formula, constraint
meaning, or the fields included in canonical bytes requires explicit RFC
review. Adding automatic scale selection or backend-private equilibration
requires separate policy and falsifying evidence.

## Nonclaims

This RFC does not implement or claim:

- a general mixed variational language or arbitrary constraint graph;
- automatic characteristic-scale selection, equilibration, condition-number
  optimization, or scale-quality guarantees;
- a universal per-DOF scaling array or backend-private scale identity;
- natural, open, traction, slip, periodic, or nonzero velocity boundaries;
- general vector body forces or nonconservative forcing;
- Taylor--Hood, stabilized equal-order, discontinuous, curved, adaptive,
  higher-order, or three-dimensional Stokes spaces;
- disconnected fluid domains or multiple pressure nullspaces;
- Navier--Stokes advection, transient flow, turbulence, multiphase flow, or
  ALE;
- fluid Ports, trace transfer, structural mechanics, monolithic or
  partitioned FSI;
- production MINRES, block or Schur preconditioning, faer MINRES, parallel
  linear solve, MPI, CUDA, or distributed field reconstruction;
- performance, strong-scaling, weak-scaling, or robustness evidence;
- durable general velocity/pressure result artifacts; or
- execution attestation, merely because a Run manifest records identity and
  provenance.
