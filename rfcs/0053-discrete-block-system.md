# RFC 0053: Private physics-neutral discrete block system

- Status: Implemented and verified for the private FSI/Stokes/elasticity-pair
  slice and the bounded fixed-domain transient-flow extension;
  [`numerics.physics-neutral-discrete-block-system`](../verify/numerics/physics-neutral-discrete-block-system/README.md),
  [`fluid.fixed-domain-transient-navier-stokes-2d`](../verify/fluid/fixed-domain-transient-navier-stokes-2d/README.md)
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0023](0023-finalized-spatial-linear-handoff.md),
  [RFC 0042](0042-conforming-elasticity-interface-realization.md),
  [RFC 0045](0045-fieldwise-mixed-realization-and-si-congruence.md), and
  [RFC 0050](0050-fixed-reference-monolithic-fsi.md)

## Summary

Eqiora will introduce one small, closed, physics-neutral discrete block
contract between accepted spatial Realization and global algebra. The first
contract is private to `eqiora-numerics`. It gives the existing fluid, solid,
and interface lowerings one shared way to retain exact Field, Relation,
support, space, transformation, and block-incidence facts before producing the
sole `CanonicalCsrSystemView`.

The path is:

```text
accepted Semantic projection + resolved Realization
                    |
                    v
       private discrete block definition
                    |
       ordinary local operators and assembly
                    |
                    v
       one CanonicalCsrSystemView
                    |
        private finalized-linear core
                    |
                    v
  typed physics-specific reconstruction and evidence
```

This is a lowered numerical contract, not canonical Model meaning. It adds no
Semantic Kernel node, weak-form language, package dispatch key, runtime
callback, public provider interface, or durable artifact.

## Motivation

The current spatial paths already agree at the solver boundary, but they reach
it through several near-parallel finalized wrappers. Scalar elliptic,
elasticity, conforming elasticity pairs, MINI Stokes, and fixed-reference FSI
all retain a canonical CSR system, solver policy, vector layout, target,
assembly evidence, and private reconstruction state. Their public wrappers
repeat accessors and solution-reacceptance logic.

The duplication is more consequential before CSR materialization. Fluid,
solid, and FSI finalizers each construct their own layout and directly assemble
a complete system. The captured CSR proves exact algebraic agreement, but it
cannot say which exact Semantic Fields and Relations produced its blocks,
which trace quotient or constraint was applied, or whether a live residual was
silently omitted. Adding another physics tuple in that form would require
another physics-specific layout, finalized wrapper, backend handoff, and
result path.

Conversely, the existing L2 contracts already have clear owners:

- `eqiora-schema` owns expression type, shape, frame, and spatial-support
  meaning;
- `eqiora-realization` owns Field-to-space choices, algebraic constraints,
  trace quotient, time elimination, scaling, solver, target, and schedule;
- `eqiora-assembly` owns anonymous local contributions and deterministic
  scatter;
- `eqiora-solver` owns the sole solver plan, canonical CSR action, agreement
  fingerprint, and accepted solution report; and
- `eqiora-numerics` owns spatial approximation and the composition of those
  contracts.

The new contract must compose those authorities without copying them into a
new universal IR.

## Decision boundary

The first implementation lives in one private `eqiora-numerics` module. It may
use the existing downward L3-to-L2 dependencies, but it creates no new crate
and no same-layer dependency exception. Its types are `pub(crate)` and are not
re-exported by the `eqiora` facade.

Each existing public finalized physics wrapper retains its current name and
typed result. Internally, the wrappers delegate the common algebraic handoff
to one private finalized-linear core. Physics-specific reconstruction remains
ordinary typed Rust state; it is not stored as `Any`, a universal result enum,
a closure, or a trait object.

The closed first slice admits only the facts required by three existing real
consumers:

1. RFC 0050 fixed-reference monolithic FSI, the primary evidence case;
2. coherent-SI simplicial MINI Stokes, including its optional zero-integral
   pressure constraint; and
3. the conforming two-body elasticity pair, including its trace quotient.

The scalar-elliptic and single-body elasticity wrappers may adopt the shared
finalized-linear core as a behavior-preserving refactor, but they do not
justify widening the first block vocabulary. A fourth equation family does
not enter this RFC merely because the private representation could hold it.

## Six-part vocabulary

The private contract has exactly six conceptual parts. Implementations may
combine them into fewer Rust structs, but they must not add a seventh catch-all
metadata map.

### 1. Field and auxiliary blocks

A Field block identifies one exact Semantic Field, whether it is algebraic,
represented but eliminated, or coefficient data. It retains:

- exact Domain and Field identities;
- the existing Field-space binding;
- the existing typed shape, coherent-SI dimension, frame, and spatial support;
- its existing positive congruence scale when scaling is selected.

An auxiliary block is admitted only when an existing typed Realization choice
owns it, initially the multiplier for an exact zero-integral constraint. It is
identified by that constraint and its exact Field, not by a caller-provided
name or ordinal. Represented-but-eliminated displacement remains a physical
Field and is recorded by a transformation; it is not relabelled as an
auxiliary algebraic unknown.

Caller order never selects block order. Field and auxiliary identities are
normalized before identity is computed. The existing physics-specific layout
and `AssemblyMap` remain the sole owners of local-to-global degree-of-freedom
projection. The block contract does not duplicate ranges, especially because
a conforming quotient can make two Semantic Fields share algebraic degrees of
freedom.

### 2. Relation inventory and residual blocks

The Relation inventory retains every exact accepted Relation with its support
and numerical disposition: coefficient definition, residual equation,
represented-state elimination, or normalized boundary treatment. A residual
block separately retains the tested algebraic block and exact origin of one
equation family:

- a Semantic Relation and its inferred residual type and support; or
- an explicit Realization constraint.

A residual block cannot be synthesized from a physics or package name. The
closed conforming interface is not fabricated as an independent row block:
its exact Connection and boundary Relations are owned by the accepted trace
quotient. Every residual Relation and constraint must occur exactly once;
every boundary Relation must be owned by exactly one matching essential,
natural, or conforming-interface treatment.

Algebraic and residual inventories remain separate even though the first
materialized systems are square. This preserves a path to constrained and
mixed formulations without pretending that equation and coefficient spaces
are the same object.

### 3. Contributions

A contribution records canonical incidence from one residual block to one or
more algebraic blocks. The first closed roles are mass, stiffness, mixed
constraint, boundary, algebraic constraint, and right-hand-side load. They are
numerical classifications of an already accepted Relation or Realization
transformation, not new Semantic operators.

Each contribution retains its exact residual owner, participating algebraic
blocks, exact support set, stable logical packet indices, exact assembly target
membership, and the exact ordered Parameter identities present in its
accepted coefficient tapes. Equal current values never merge distinct
Parameter coordinates.

Numerical values still travel through the existing `LocalContribution`,
`AssemblyMap`, `AssemblyPacket`, and ordered `AssemblyBackend` contracts. A
checked `AssemblyWork` adapter verifies every evaluated packet against its
declared packet batch and target membership before scatter. The block layer
neither introduces another dense/sparse matrix type nor adds semantic metadata
to anonymous L2 assembly packets. No contribution contains
a native function pointer, Python callback, opaque opcode, or package name.

### 4. Transformations

Transformations record how the accepted physical and residual inventories
produce the solved algebraic coordinates. The first slice references the
existing typed authorities for:

- essential-value elimination;
- one conforming trace quotient;
- one backward-Euler represented-state elimination; and
- exact boundary-Relation ownership for the quotient.

Congruence scales remain attached to Field and auxiliary blocks, while an
optional zero-integral pressure constraint is represented by its auxiliary and
residual blocks. The block contract records these choices and their exact
identities; it does not redefine their numerical validation. Transformation
order is canonical. A quotient, elimination, scale, or pressure-closure
decision cannot be inferred later from the assembled nonzero pattern.

### 5. Operator facts

Operator facts are closed, validated consequences needed before solver
admission. The first slice retains:

- symmetry and positive-definite or symmetric-indefinite structure;
- pressure/nullspace closure and the presence or absence of an explicit
  constraint;
- represented, algebraic, auxiliary, and eliminated Field inventories; and
- exact block, Relation, support, packet, and target incidence.

An asserted fact must be independently checked by the producing finalizer or
by reapplication of the materialized operator. A fact is not selected because
a requested solver would prefer it. This RFC introduces no new JVP, VJP, or
adjoint claim.

### 6. Materialization

Materialization binds one normalized block definition to exactly one captured
`CanonicalCsrSystemView`. The existing canonical CSR agreement fingerprint
remains the sole exact identity of matrix values, right-hand side, shape, and
asserted linear-operator property. The block layer does not hash or copy those
arrays a second time.

Before materialization, validation proves an exact packet partition, target
membership, block/Relation/support incidence, transformation ownership, and
closure inventory. Materialization then checks the state-owned
`AssemblyReport` shape and operator property before binding the block identity
to the existing CSR fingerprint. Method-native full systems, body-cut systems,
reaction recovery, Field reconstruction, and physics evidence remain private
state adjacent to the materialization. They are not additional solver inputs.

## Identity separation

The first slice deliberately has no durable `DiscreteBlockSystemEnvelopeV1`
and no public block digest. Four identities remain distinct:

1. Semantic identity is the exact Model revision and exact Domain, Field,
   Relation, Connection, and Parameter identities.
2. Realization identity is the exact mesh, spaces, constraints, quotient,
   time step, scaling, and numerical policy already owned by the resolved
   Realization.
3. Private block identity is structural equality of the normalized six-part
   definition. Stable logical packet indices and target membership are
   included; declaration/insertion order, runtime completion order, worker
   count, backend, and reconstruction storage are excluded.
4. Algebraic agreement identity is the existing
   `CanonicalCsrAgreementFingerprintV1`; execution and verification identity
   remains in `SolverPlan`, `AssemblyReport`, `SolveReport`, and Run evidence.

The materialization retains both the private normalized definition and the
existing CSR agreement fingerprint. Equality of CSR fingerprints alone does
not erase distinct Semantic or Realization identity. Conversely, two worker
schedules completing the same stable logical packets in different orders do
not create distinct mathematical systems.

No filesystem path, source name, package/provider name, renderer identifier,
backend type, thread count, device ordinal, or timing enters private block
identity. Exact floating-point arrays are normalized and validated by the
existing canonical CSR constructor; this RFC does not invent a second float
encoding.

Parameter identity forwarding across Component bindings remains owned by
[RFC 0055](0055-component-parameter-terms.md). This RFC preserves the exact
Parameter coordinates it receives and never substitutes value equality, but it
does not claim that all current package bindings already produce the desired
shared coordinate or a coefficient-sensitive FSI adjoint.

## Common finalized-linear core

The private finalized-linear core owns:

- the block materialization association, when the path has adopted RFC 0053,
  and the sole shared `CanonicalCsrSystemView`;
- the existing `SolverPlan`;
- the currently admitted vector layout and target;
- the common normal-orientation, plan, iteration, producer topology, verifier
  topology, residual-target, and independently reapplied true-residual checks.

The exact `AssemblyReport` remains with physics-specific reconstruction state
and is checked when the block materialization is attached to the core.

Every typed public wrapper delegates `linear_problem` and solution acceptance
to this core before consuming its physics-specific reconstruction state. This
closes the current asymmetry in which the general finalized spatial wrappers
recheck solver plan and topology while the low-level FSI wrapper rechecks only
shape and residual. FSI's incompressibility, kinematics, interface action, and
energy acceptance still run afterward and remain independent of generic
linear acceptance.

The core is not a public generic `FinalizedOperatorProblem<R, E>`. Such a type
would expose reconstruction implementation parameters, stabilize the current
`Target` shape immediately before graph-shaped execution work, and spend a
public API promise without an external consumer.

## Three-consumer proof

The first implementation is accepted only when the same private vocabulary is
used by all three consumers without changing their existing numerical claims.

### Fixed-reference FSI

The FSI definition contains fluid velocity, fluid pressure, and solid velocity
unknown blocks; represented-but-eliminated solid displacement; exact fluid
momentum, incompressibility, solid momentum, and kinematic Relations; the
conserving interface Connection and its exact boundary Relations; fluid and
solid fused mass/spatial contribution batches; the conforming velocity
quotient; backward-Euler elimination; and gauge-free pressure closure by the
complete operator.

Its materialized CSR and every existing RFC 0050 solution, interface, residual,
kinematic, and energy oracle must remain unchanged.

### Coherent-SI MINI Stokes

The Stokes definition contains exact velocity and pressure Field blocks, the
exact momentum and incompressibility Relations, and either the accepted
zero-integral constraint block or the admitted boundary-determined pressure
closure. It proves that the vocabulary handles a mixed saddle system and an
auxiliary unknown without relying on an FSI name.

Its dimensionless congruence, canonical CSR, physical reconstruction, gauge or
boundary-pressure evidence, and balance must remain unchanged.

### Conforming elasticity pair

The elasticity-pair definition contains two exact displacement Fields and
their volume Relations plus one exact conserving Connection, two body-local
spatial contribution families, essential elimination, and one conforming
trace quotient. It proves that the vocabulary handles a positive-definite
multi-Domain system without pressure, mixed spaces, or time elimination.

Its quotient system, reconstructed body Fields, opposite interface actions,
reactions, and balance must remain unchanged.

## Falsifying verification

The registered
[`numerics.physics-neutral-discrete-block-system`](../verify/numerics/physics-neutral-discrete-block-system/README.md)
case executes the ordinary public finalization paths while focused internal
tests inspect the private definition. Together they prove:

- block-builder insertion order cannot change normalized block identity;
- direct and packaged sources retain exact Semantic identity while continuing
  to produce the same accepted algebra and physics evidence;
- every exact Field, Relation, interface Connection, constraint, and auxiliary
  unknown received from the closed recognizers is represented exactly once;
- equal-valued distinct Parameters remain distinct coordinates in the block
  definition, while no claim is made beyond
  [RFC 0055](0055-component-parameter-terms.md);
- wrong support, scale, mesh, or Realization identity fails before
  materialization, while the existing closed recognizers remain authoritative
  for schema shape, dimension, frame, and support typing;
- a contribution outside its admitted row/column incidence, a duplicate or
  missing packet, wrong target membership, or non-finite local value fails
  before an accepted CSR escapes;
- removing or duplicating fluid/solid mass, spatial, or constraint
  contributions changes algebra and is rejected by the existing independent
  physics oracles;
- trace-quotient, backward-Euler, congruence, essential-elimination, or
  pressure-closure drift fails before execution;
- a solution with substituted solver plan, orientation, producer topology, or
  verifier topology is rejected by every wrapper, including FSI; and
- FSI, Stokes, and elasticity-pair accepted CSR fingerprints, solutions,
  reconstructions, and registered balance evidence remain unchanged through
  the shared path.

The registered case is a structural numerical claim. It does not turn private
block types into a stable API or durable artifact.

## Bounded transient-flow extension

The bounded fixed-domain transient-flow extension is the first nonlinear,
multi-step consumer of the same private vocabulary. It extends the closed
implementation only where an accepted typed Realization requires new
structure:

- the existing backward-Euler derivative transformation now owns the exact
  represented previous state and step duration;
- `EnergySkewConvection` records the deliberate weak-form transformation from
  the conservative Semantic Relation, while registered evidence retains and
  checks the exact conservative-to-skew defect;
- advection is one closed contribution role with exact residual, Field,
  support, Parameter, packet, and target incidence; and
- the explicit transient Realization revision replaces the default-policy
  identity used by older default paths.

The block system does not become a nonlinear operator graph. Direct nonlinear
residual evaluation and analytic linearization remain owned by the
method-specific local operator. At every Newton point, the checked assembly
backend binds the normalized block identity to the exact materialized CSR and
validates that binding before the linearization escapes. Step count remains a
Run directive and is absent from both Semantic and Realization identity.

The registered
[`fluid.fixed-domain-transient-navier-stokes-2d`](../verify/fluid/fixed-domain-transient-navier-stokes-2d/README.md)
case falsifies semantic near-misses, inconsistent initial states, insufficient
quadrature, corrupt analytic Jacobians, and nonlinear nonconvergence. It also
checks every analytic Jacobian column against centered differences of a
directly reassembled residual. This is a bounded extension of the private
contract, not a public weak-form API, a general nonlinear IR, or a claim about
ALE, moving meshes, turbulence, or arbitrary time methods.

## Boundary with the curated facade

[RFC 0054](0054-curated-facade-and-control-plane.md) owns public-facade
curation and the generated Rust/Studio/Python control-plane protocol. This RFC
precedes it so facade curation can inspect a working internal boundary instead
of standardizing a speculative shape.

The original discrete-block slice therefore:

- adds no `eqiora` facade export, public provider SDK item, JSON Schema,
  TypeScript/Zod type, Python model, command, or DTO;
- sends no matrix, block contribution, mesh, or Field array through the
  control plane;
- preserves every existing public finalized physics wrapper and its current
  typed result; and
- leaves any future read-only provider view to an explicit facade API-budget
  decision backed by at least two external consumers.

[RFC 0054](0054-curated-facade-and-control-plane.md) subsequently established a
curated stable/transitional facade inventory. The transient-flow extension
adds one transitional prepared application service through that budget. It
exposes no block vocabulary: it owns the exact Model, Realization,
authenticated mesh, and an owned solver-adapter handle with an exact capability
snapshot, then returns typed trajectory evidence. The public
run operation accepts no substitute adapter, mutable capability drift fails
closed, and the trajectory retains the executing adapter identity. The facade's curated `numerics` namespace omits
the lower-level bridge; the public `eqiora-numerics` form itself consumes one
indivisible mesh envelope rather than separate artifact identity and mesh data.

No current control-plane DTO carries a matrix, block contribution, mesh, or
Field array. Studio and Python do not reconstruct lowering meaning. A later
execution provider that truly needs block-level access must justify an
Eqiora-owned provider contract separately; the existing CUDA FSI slice
consumes the finalized CSR, not the private definition.

## Alternatives considered

### Consolidate only the finalized CSR wrappers

Rejected as the complete solution. A shared finalized-linear core removes
real duplication and is part of this RFC, but by itself it cannot detect an
omitted Relation, explain block identity, or prevent each new physics tuple
from inventing another pre-CSR layout.

### Add a new L2 block-IR crate

Rejected for the first slice. Such a crate would either duplicate
`eqiora-realization` space/constraint/quotient types or require new same-layer
dependency exceptions among IR, realization, meshing, and assembly. There is
not yet an independent L2 consumer that justifies that compatibility and layer
cost.

### Extend `eqiora-ir` into a universal weak-form representation

Rejected. Scalar/operator IR owns lowered mathematical actions, while this
contract composes an accepted spatial Realization with mesh, spaces,
constraints, and assembly. A universal form compiler is materially larger and
is bounded separately by
[RFC 0056](0056-pure-calculus-and-support-maps.md).

### Publish a generic operator trait with callbacks

Rejected. Trait-object or closure-defined block actions have no closed
canonical identity, cannot be replayed without executable code, and invite
native or Python callbacks into numerical inner loops. The first slice uses
owned closed data and existing local-action contracts.

### Use a public generic finalized wrapper

Rejected. `FinalizedOperatorProblem<Reconstruction, Evidence>` would expose
physics-private type parameters and stabilize more surface than execution
adapters need. Typed physics wrappers over one private core preserve both
ergonomics and deletion freedom.

## Compatibility

This RFC changes no Model, Transaction, Realization, artifact, package, Run,
Studio, or Python wire. It adds no Semantic Kernel entity or public enum
variant. Existing canonical CSR fingerprints and accepted physical results are
compatibility oracles for the refactor.

The first implementation may add private modules and remove duplicated private
wrapper fields and validation functions. Existing public wrapper names and
accessors remain source-compatible. Any later durable block artifact, public
provider view, matrix-free implementation, or graph-shaped execution binding
requires its own compatibility decision.

## Nonclaims

This RFC does not implement or claim:

- a universal weak-form, tensor-calculus, symbolic-equivalence, or support-map
  language;
- a public block-system API, provider SDK, plugin ABI, dynamic registry, or
  control-plane schema;
- a durable block-system artifact, canonical block byte encoding, or public
  block digest;
- arbitrary mixed, high-order, discontinuous, nonmatching, mortar, Nitsche,
  contact, or adaptive spaces;
- a production matrix-free operator, GPU assembly, distributed assembly,
  field-split preconditioner, Schur complement, or solver graph;
- nonlinear or transient Navier--Stokes beyond the bounded fixed-domain 2D
  MINI/P1 backward-Euler reference, multiple FSI steps, partitioned FSI, ALE,
  remeshing, or topology change;
- new primal/JVP/VJP actions, coefficient-sensitive FSI differentiation,
  adjoints, or shape sensitivity;
- Parameter identity forwarding owned by
  [RFC 0055](0055-component-parameter-terms.md);
- pure calculus and general support maps owned by
  [RFC 0056](0056-pure-calculus-and-support-maps.md);
- graph-shaped Realization, deployment, or execution plans owned by
  [RFC 0058](0058-portable-realization-and-execution-graphs.md); or
- stable facade and generated client protocols owned by
  [RFC 0054](0054-curated-facade-and-control-plane.md).
