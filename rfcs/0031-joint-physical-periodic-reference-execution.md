# RFC 0031: Joint scalar-physical and exact-periodic reference execution

- Status: Accepted; bounded reference slice implemented and verified
- Authors: Eqiora contributors
- Created: 2026-07-18
- Depends on: RFC 0002, RFC 0021, RFC 0022, RFC 0024

## Summary

The reference interpreter may execute one bounded Model-v2 network in which a
closed scalar-physical subsystem with continuous state reads an exact-periodic,
held causal signal, without changing the Semantic Kernel, scalar conserving
connection meaning, package identity, or deployment scheduling.

## Motivation

The existing contracts deliberately stop on opposite sides of the required
composition. [RFC 0002](0002-reference-execution-v0.md) defines continuous and
exact-periodic scalar execution, atomic `Next` commit, and causal sample-and-hold,
but rejects conserving across/through execution. [RFC
0024](0024-scalar-conserving-connection-semantics.md) defines nominal scalar
physical Domains, canonical junction residuals, and a static affine solve, but
its Phase 1 execution boundary rejects derivatives, causal Ports, periodic
Activation, and mixed physical/signal Relations.

Neither boundary should acquire a motor-specific exception. The missing
contract is a small, deterministic reference composition that can falsify the
general architecture:

```text
exact Model Packages
  -> deterministic component elaboration
  -> one flat Model-v2 Relation network
  -> continuous scalar-physical reference solve
  -> exact-periodic signal update and hold
  -> accepted trajectory and separate package/Run lineage
```

The canonical DC-motor-plus-discrete-controller case is the first evidence, not
the definition of the executor. Motor, controller, electrical, and rotational
concepts remain ordinary package declarations. No new kernel node, source-only
semantic shortcut, standard-package branch, or backend callback is introduced.

## Relationship to existing authority

This RFC composes four existing contracts and does not reinterpret them:

- [RFC 0002](0002-reference-execution-v0.md) remains authoritative for exact
  ClockDomain coincidence, `Pre`/`Next`, atomic periodic commit, held causal
  outputs, reference backward Euler, and bounded dense Newton.
- [RFC 0021](0021-component-hierarchy-and-instantiation.md) remains
  authoritative for typed component interfaces and deterministic flattening.
- [RFC 0022](0022-exact-package-identity-and-resolution.md) remains
  authoritative for exact offline package identity, source verification, and
  package-compilation provenance.
- [RFC 0024](0024-scalar-conserving-connection-semantics.md) remains
  authoritative for nominal scalar physical Domains, Port orientation,
  physical unknown order, junction residual construction, and Model v2.

RFC 0024 Phase 1 remains a valid static affine capability. This RFC adds a
separate reference-execution admission path; it does not silently widen the
Phase 1 affine lowerer or its solver claim.

## Proposed design

### Admitted semantic shape

The first joint reference path accepts only a fully validated, immutable
`KernelProgram` reconstructed or emitted as Model v2. Its admitted connected
slice has:

- finite scalar `f64` values in coherent SI coordinates;
- one closed scalar-physical subsystem under RFC 0024 closure, possibly
  containing more than one nominal scalar physical Domain;
- continuous Relations containing scalar Fields, `Derivative`, `Across`,
  `Through`, Parameters, model time, and causal signal input values;
- exact-periodic Relations containing `Pre`, `Next`, Parameters, model time,
  and causal signal input and output values under RFC 0002;
- one output and one or more inputs on every causal signal Connection;
- at most one admitted ClockDomain, with an exact rational period and phase
  when present; and
- no externally supplied signal values after initial admission.

A continuous Relation may couple several nominal physical Domains and may read
a causal input Port. Reading the signal does not make it conserving: the input
aliases its unique output and contributes no junction equation or physical
power term. A periodic Relation may update discrete Fields and signal outputs,
but may not contain `Across`, `Through`, or `Derivative` in this slice.

The path rejects conserving markers, unconnected scalar physical Ports,
multiple membership, signal/conserving interchange, Event or Guard Activation,
spatial operators, vector/tensor values, and any symbol outside the closed
sets above. Every participating physical Relation has exactly one continuous
Activation. Periodic Relations and physical Relations remain distinct even
when they meet through a causal signal.

Joint admission reuses RFC 0024's topological closure, nominal-domain checks,
physical-coordinate order, and junction construction. It does not call the
Phase 1 expression-admission gate and then bypass its failure: this RFC owns the
explicitly wider `Field`/`Derivative`/causal-input vocabulary above. One typed,
immutable projection records the result before execution:

```text
JointReferencePlan {
    differential_fields
    algebraic_fields
    discrete_fields
    continuous_signal_outputs
    held_signal_outputs
    physical_unknowns
    continuous_relations
    junction_residuals
    periodic_activation_groups
}
```

Every member is a checked reference to the original `KernelProgram`; the plan
does not copy expressions, generate hidden Fields, or mutate the model. Its
constructor performs complete admission and returns no partial plan.

### Nominal physical domains

RFC 0024 nominal typing is unchanged. Every member of one conserving
Connection must reference the same exact Domain ID; equality of across and
through dimensions is insufficient. A Relation may couple Ports belonging to
different nominal Domains, such as electrical and rotational Domains, but it
does not merge those Domains and does not permit their Ports in one Connection.

The compiler and interpreter never infer a physical family from dimensions or
package names. In particular, two same-dimension rotational connector
declarations remain incompatible unless they share the same elaborated nominal
Domain identity.

### Canonical state and unknown order

The joint plan is constructed only after whole-model validation and RFC 0024
physical closure. All sets below use typed canonical IDs, never source order,
package insertion order, or hash-map iteration.

The retained state layout is:

1. differential and algebraic Fields, sorted by canonical Field ID;
2. discrete Fields, sorted by canonical Field ID; and
3. held causal output Ports, sorted by canonical Port ID.

Physical values are algebraic execution coordinates rather than Fields. Their
order is exactly RFC 0024 order: scalar physical Ports by canonical Port ID,
with `Across(port)` immediately followed by `Through(port)`.

At one backward-Euler continuous step, the Newton unknown order is:

1. differential Field values, sorted by canonical Field ID;
2. algebraic Field values, sorted by canonical Field ID;
3. continuously determined causal output Ports, sorted by canonical Port ID;
   then
4. physical subsystems by canonical subsystem ID, each using its RFC 0024
   physical unknown order.

For a differential Field `x`, `Derivative(x)` is the derived coordinate
`(x_next - x_previous) / h`; it is not an independent step unknown. Algebraic
Fields have no fabricated derivative value.

At initial consistency and post-tick restoration, differential Field values
are fixed. The unknown order is:

1. derivatives of differential Fields, sorted by Field ID;
2. algebraic Fields, sorted by Field ID;
3. continuously determined causal output Ports, sorted by Port ID; then
4. physical subsystems and unknowns in the same order as a continuous step.

At one exact-periodic activation set, the unknown order remains the RFC 0002
order:

1. `Next(field)` coordinates, sorted by Field ID; then
2. due causal output Ports, sorted by Port ID.

No phase may reorder these canonical coordinates for convenience. A solver may
use an internal permutation only if it restores this order before evaluation,
commit, trajectory sampling, or evidence emission.

### Canonical residual order

Continuous-step, initial-consistency, and restoration residuals use one order:

1. participating continuous Relations by canonical Relation ID, retaining the
   stored root order inside each Relation; then
2. RFC 0024 Junction groups by canonical Connection ID, with every across root
   followed by the through root exactly as RFC 0024 specifies.

A continuous physical Relation appears once even when it couples several
nominal Domains or reads a signal. Junction roots remain owned by Connections
and acquire no independent Activation.

Periodic residuals are separate. All Relations due at the exact instant are
ordered by canonical Relation ID and retain their stored root order. Junction
roots are not copied into the periodic solve; physical consistency is restored
in the following phase.

Each phase must be nonempty and square under its declared unknown and residual
orders. Missing ownership, duplicate membership, an under- or overdetermined
phase, or a singular/nonconvergent reference Newton solve fails closed. The
reference path performs no structural index reduction or least-squares solve.

### Initial consistency

Model-declared initial Field values and the runtime's explicit zero initial
values for held output Ports are finite guesses, not an accepted initial
condition. At model time zero the interpreter:

1. solves continuous initial consistency with differential Fields fixed and
   derivative, algebraic-Field, and physical coordinates unknown;
2. records no trajectory sample yet;
3. executes a phase-zero periodic activation set when exact ClockDomain
   semantics make one due;
4. atomically commits every `Next` Field and signal output; and
5. restores continuous physical/algebraic consistency before the first sample.

The admitted model must produce a complete finite consistent point within the
configured reference tolerance. Failure is diagnostic. A deterministic,
explicit Newton-guess policy may seed an unknown with zero, but that guess is
never substituted for a successful consistency solve or published as an
accepted coordinate.

This proves consistency only for the admitted bounded system. It is not a
general DAE-index analysis or consistent-initialization algorithm.

### Continuous integration and exact ticks

Between ticks, every periodic output retains its last committed value. Signal
inputs alias that held output and are known inputs to the continuous physical
solve. One accepted interval follows this order:

```text
backward-Euler solve of continuous Fields + physical coordinates to next instant
  -> if no tick is due: sample the accepted continuous state
  -> if the exact clock is due:
       solve all coincident periodic Relations from one shared pre-tick state
       -> atomically commit Next Fields and causal outputs
       -> restore continuous physical/algebraic consistency at the same time
       -> sample the post-restoration state
```

The continuous step is clipped to the next exact rational tick converted under
RFC 0002's checked model-time calendar. Floating-point proximity never creates
or separates a coincident activation set. All `Pre` reads in the set observe
the same pre-tick state; no Relation observes a partially committed controller
update.

The reference trajectory contains the initial post-restoration sample and one
post-restoration sample at each requested/tick/accepted output instant under an
explicit sampling policy. It does not expose internal Newton iterates as model
states.

Field samples retain the existing trajectory contract. At the same accepted
boundaries, the result also exposes each scalar `PhysicalUnknown` as a
dimensioned `PhysicalSample`. This observation reads the accepted algebraic
coordinates; it creates no hidden Field, persistent state, model identity, or
durable trajectory wire. It exists so conformance evidence can evaluate the
original constitutive and generated junction residuals without reconstructing
unobservable solver state.

Event/tick coincidence is outside this RFC. RFC 0002 event grouping remains a
separate implemented slice and is not implicitly composed with scalar physical
execution here.

### Reference numerical tolerance and coherent SI

All values presented to the reference solver are the numerical coordinates of
coherent SI quantities. The first joint solver applies no automatic equation,
variable, nominal-value, or unit-family scaling. Its dense Newton convergence
test uses the existing explicitly configured absolute and relative tolerances
on the fixed-order raw coherent-SI residual vector.

This unscaled norm is deliberately a transparent oracle, not a
unit- or equation-rescaling-invariant numerical policy. Multiplying an equation
by a constant can change reference Newton behavior even when its mathematical
zero set is unchanged. Production residual scaling and robust nonlinear/DAE
solvers remain Realization concerns.

Registered evidence must additionally declare componentwise acceptance
tolerances with the dimension of each observed residual or balance. A single
dimensionless statement such as "residual below epsilon" may not be used to
compare voltage, torque, current, and angular-momentum equations as if their
units were interchangeable. No hidden normalization may be added only to make
the verification pass.

### Sign convention, power, and stored energy

RFC 0024 orientation remains authoritative: `Through(port)` is positive from
the junction into the owning Relation. For a scalar physical Port `p`, power
entering its component is

```text
P(p) = Across(p) * Through(p).
```

Component power is the ordered sum over its physical Ports. A two-terminal
electrical component therefore receives
`(v_positive - v_negative) * i_positive` when its through conservation equation
sets `i_negative = -i_positive`. A one-port rotational component receives
`angular_velocity * torque_into_component`. A motor delivering shaft power has
negative mechanical through power under this convention; no motor-specific
sign exception is permitted.

Electromechanical coupling Relations must make transduced electrical and
mechanical power cancel under their declared parameter relation. Resistive and
load terms are nonnegative dissipation under the accepted orientation. Energy
stored in admitted inductive and inertial states is evaluated from ordinary
Fields and Parameters; it is not a hidden kernel quantity.

The verification case checks both instantaneous typed power identities and a
time-discrete energy balance that explicitly includes the selected backward-
Euler approximation. It must not label backward-Euler numerical dissipation as
physical loss or claim exact continuous-time energy conservation from a finite
step trajectory.

The causal controller signal carries information, not physical power. This RFC
does not assign energetic meaning to a signal Port.

### Model time is not execution scheduling

ClockDomain period and phase are canonical model meaning. Worker count, thread
placement, task priority, deadline, RTOS policy, queue order, and backend launch
order are absent from this reference semantic plan and remain Realization or
execution provenance.

No conversion from `ClockDomain` to `ExecutionSchedule` is introduced. The
same canonical model and trajectory contract must remain valid when deployment
policy changes, subject to the numerical conformance of a separately admitted
executor.

### Package and Run lineage

The root and reusable components resolve through ordinary RFC 0022 exact
Model Package identities. There is no built-in electrical, rotational, motor,
or controller registry branch. Compiler canonical declarations are verified
against every exact source bundle before elaboration, and the selected root
flattens through RFC 0021 into the ordinary Model-v2 graph.

Package compilation and execution evidence remain separate artifacts. Only
after the trajectory, residuals, initialization, and power checks are accepted
may the case construct a `RunManifestV1` and bind it through
`PackageRunBindingV1`. That lineage edge proves exact content linkage, not that
execution occurred or that the numerical evidence is correct. Typed
Realization and `RunManifestV2` package lineage are outside this RFC.

## Determinism and limits

Meaning and canonical identity are independent of source declaration order,
source-file order, exact dependency alias spelling after alpha-normalization,
graph insertion order, and internal map insertion. Exact source identity still
changes when source bytes or normalized bundle paths change; semantic and
source identities must not be conflated.

The implementation retains fixed canonical order for Fields, Ports, Relations,
Connections, residual roots, and coincident periodic Activations. The reference
arithmetic order is fixed within one implementation. Numerical acceptance uses
declared tolerances and does not claim cross-architecture bit identity.

Existing package, parser, graph, expression, hierarchy, transaction, and model
limits apply before joint-plan construction. The reference configuration also
bounds accepted steps, exact tick advances, Newton iterations, event-related
limits retained by the shared interpreter, and expression evaluation. The
active equation count must equal the deterministically constructed unknown
count before Newton iteration can be accepted.

Limit violations, non-finite input or output, a non-advancing step/calendar,
an unsupported semantic value, or a non-square phase fail before trajectory or
Run-lineage publication. Additional joint-plan-specific count and byte limits
remain future hardening rather than an implicit claim of this first slice.

## Alternatives considered

### Rewrite the motor as causal blocks

This avoids transient conserving execution but makes causality and manually
derived state equations the source of truth. It cannot falsify nominal
connectors, junction residuals, or the causal/acausal boundary. Rejected.

### Add a motor-specific runtime

This is the smallest demo but creates a second semantic implementation and
cannot generalize to another electromechanical component. Rejected.

### Co-simulate separate plant and controller executors

This postpones the shared implicit system and introduces time negotiation,
rollback, and exchange policy before one-process semantics are proven.
Deferred to a distinct co-simulation contract.

### Require a production DAE backend first

A production solver will be necessary for scale and robustness, but it would
mix adapter admission with the missing semantic phase order. The deterministic
reference path is the smaller oracle. Production DAE execution remains a
separate graduation gate.

### Automatically scale heterogeneous residuals

Scaling is valuable, but an implicit heuristic would make a new numerical
policy part of the semantic reference case. The first path instead exposes raw
coherent-SI behavior and explicit tolerances. A later typed scaling policy
belongs to Realization. Selected for v1.

## Compatibility and migration

This RFC adds no kernel node, expression symbol, Connection kind, package wire,
Model wire, transaction wire, or Run wire. The admitted model requires the
existing Model v2 scalar-physical values. Model v1 and its structural conserving
markers retain their exact bytes, digest, and non-executable physical meaning.

Existing RFC 0002 continuous/periodic models and RFC 0024 static affine models
keep their current behavior. The reference interpreter constructs the joint
plan automatically when admitted scalar-physical Connections are present;
unsupported mixed models fail rather than falling back to a superficially
similar executor. The static affine lowerer remains a separately selected,
narrower specialization and rejects dynamic symbols.

Internal APIs may gain a typed joint-plan/result contract during this pre-alpha
implementation. Any durable trajectory or joint-run wire requires its own
versioned artifact decision; `RunManifestV1` linkage alone is not such a wire.

## Verification

The evidence suite combines one registered exact-packaged ideal DC motor,
lumped rotational load/inertia, and exact-periodic scalar controller with the
prerequisite RFC 0002, 0021, 0022, and 0024 conformance tests. Together they
must:

1. replay the complete exact package graph, semantic/source identities,
   compilation record, Model-v2 bytes, and Model digest offline;
2. preserve canonical semantic meaning under dependency-alias, file,
   declaration, graph-insertion, and internal-map permutations while retaining
   the distinct exact-source identity rules;
3. reject same-dimension but different nominal physical Domains and reject
   every signal/conserving interchange;
4. prove deterministic Field/physical unknown and Relation/junction residual
   orders independently of source order;
5. establish a complete finite initial-consistency point before sampling;
6. exercise a phase-zero or later tick, shared `Pre`, atomic `Next`/output
   commit, held output between ticks, and post-tick physical restoration;
7. compare the accepted current, angular-speed, and controller trajectories
   against an independent analytic solution or a demonstrably tighter
   high-accuracy reference with explicit state and convergence tolerances;
8. re-evaluate every original Relation and generated junction residual under
   dimensioned componentwise tolerances;
9. check electromechanical transduction power, nonnegative declared
   dissipation, and the backward-Euler-aware stored-energy balance under the
   RFC 0024 sign convention;
10. show ClockDomain changes alter model meaning while execution scheduling is
    absent from that identity and cannot be supplied as a clock;
11. construct package-compilation-to-`RunManifestV1` lineage only after
    numerical acceptance and reject changed compilation, Model, revision,
    output, Run, or binding identities; and
12. fail atomically on non-square phases, inconsistent initialization,
    non-finite values, nonconvergence, and the limits exercised by the case.

The registered case directly owns the exact three-package composition,
alias/file/declaration/insertion invariance, nominal and connection-kind
rejections, phase-zero update/hold/restoration, dimensioned physical frame,
independent trajectory and balance oracle, accepted lineage path, and one
deterministic semantic-step-limit failure. RFC 0002 periodic tests own shared
`Pre` across coincident Relations and general phase failures; package binding
tests own identity-substitution rejection; the prerequisite semantic tests own
the remaining fail-closed invariants. The case does not duplicate those lower
contract tests merely to inflate one integration fixture.

Passing this case closes one bounded falsification target. It does not promote
any broader product area without its own registered evidence.

## Security, safety, and governance

Package source and artifact bytes remain untrusted under RFC 0022. Joint-plan
construction reads only an immutable validated `KernelProgram` and performs no
network access, native loading, build scripting, dynamic plugin discovery, or
workspace writes. Reference execution invokes no user callback or package code.

All mutation is execution-local until a complete accepted trajectory exists.
Failure returns diagnostics without publishing a partial model revision,
trajectory identity, Run manifest, or package/Run binding. Exact package
provenance does not establish publisher trust or execution attestation.

## Nonclaims

This RFC does not claim:

- a broad electrical, rotational, motor, drive, or control component library;
- nonlinear magnetic saturation, hysteresis, commutation, switching power
  electronics, friction discontinuities, backlash, or topology change;
- vector/tensor, aggregate, stream, expandable, fluid, multibody, or
  overconstrained connectors;
- arbitrary DAE index, structural index reduction, production nonlinear/DAE
  solving, adaptive integration, or backend-history restart;
- Event/Guard coincidence with physical execution, statecharts, Stateflow
  semantics, mode changes, history states, or Zeno classification;
- buses, variants, model references, rate-transition blocks, fixed-point
  arithmetic, lookup tables, or a PID/control catalog;
- multiple ClockDomains, C or Rust code generation, generated-code
  equivalence, multirate code generation, real-time schedulability, RTOS
  priority, or deadline guarantees;
- Simulink or Simscape import, interoperability, UI, component breadth, or
  product equivalence;
- typed Realization or `RunManifestV2` package lineage;
- MPI, GPU, distributed, device-resident, or performance execution; or
- dynamic plugins, registry discovery, signatures, or publisher trust.

The words "hybrid" and "sampled" in the verification case mean only the
continuous/exact-periodic semantic composition above. They do not imply a
general hybrid scheduler or state-machine runtime.

## Unresolved questions

- The first typed residual-scaling Realization policy for heterogeneous
  physical equations.
- A production residual-native DAE projection for the same canonical model.
- Durable trajectory and typed joint-run provenance beyond generic
  `RunManifestV1` output identities.
- Event/tick/physical coincidence, mode-dependent topology, and hybrid
  trajectory differentiation.
- Task lowering from semantic ClockDomains to independently versioned
  deployment schedules.
