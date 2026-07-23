# RFC 0002: Reference execution v0

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

The first normative evaluator executes validated scalar `KernelProgram`s with
continuous, exact-periodic, and zero-crossing event Activations. It defines
simultaneous update, causal signal, event localization, and reset semantics
while treating backward Euler and dense Newton as explicitly replaceable
numerical approximations.

## Motivation

A typed graph is not yet a behavioral specification. Optimized runtimes need
a small oracle that answers four questions without backend-specific policy:

1. what a symbol means during a continuous solve;
2. what `Pre` and `Next` mean at coincident periodic ticks;
3. when a causal signal is sampled and held;
4. how a directed zero crossing localizes and commits a reset; and
5. which trajectory a numerical implementation must converge toward.

## Semantic contract

- Execution accepts only an immutable, whole-model-validated `KernelProgram`.
- Exact rational ClockDomain instants determine coincidence; floating-point
  equality never groups periodic Activations.
- All Relations due at one exact instant form one residual system.
- `Field` and `Pre` read the state before that activation microstep.
- `Next` Fields and periodic output Ports are unknowns of the joint system and
  commit atomically after convergence.
- A signal Connection has one output and one or more inputs. Inputs alias the
  source value; a periodically produced output holds between ticks.
- Continuous Relations may not contain `Pre` or `Next`.
- An event guard is armed only from a value strictly outside its zero-tolerance
  band. Resetting onto the guard surface does not immediately retrigger it.
- `Rising`, `Falling`, and `Any` filter the pre-step to post-step crossing.
- Every crossing in a candidate continuous step is localized by repeatedly
  re-solving the same implicit step from its accepted start state. State is not
  fabricated by interpolating endpoint samples.
- The earliest localized instant wins. Events equal within the configured
  model-time tolerance form one residual system; a periodic tick at that same
  instant joins the system before one atomic commit.
- The trajectory records event state immediately before and after reset as two
  samples at the same model time.

The phase order at a periodic instant is:

```text
integrate continuous relations to tick
→ solve all coincident periodic relations
→ atomically commit Next/output values
→ restore continuous algebraic consistency
→ sample trajectory
```

At model time zero, initial continuous consistency is established before a
phase-zero tick, then restored after the tick before the first sample.

The event phase order is:

```text
trial-integrate continuous relations to the candidate endpoint
→ detect directed guard crossings
→ re-solve and bracket the earliest root
→ sample the pre-event state
→ solve all coincident event/tick relations
→ atomically commit Next/output values
→ restore continuous algebraic consistency
→ sample the post-event state
```

## Reference numerical contract

Continuous residuals use backward Euler. Active square systems use dense
Newton with a forward finite-difference Jacobian and partial-pivot Gaussian
elimination. Event localization is bracketed bisection over re-solved implicit
steps. `ReferenceConfig` makes maximum step, residual and event tolerances,
localization limits, and safety limits explicit. These choices define the
current reference approximation, not canonical model meaning.

The v0 path rejects rather than guesses when:

- an initial state or external signal value is absent;
- an active system is non-square;
- a Jacobian is singular or Newton does not converge;
- evaluation produces NaN or infinity;
- guard activation or an unsupported semantic combination is requested.

The bounded scalar-physical extension in [RFC
0031](0031-joint-physical-periodic-reference-execution.md) composes the same
calendar, update, hold, backward-Euler, and dense-Newton rules with RFC 0024
junction residuals. It does not change the v0 meaning above for models without
scalar physical Connections, and legacy conserving markers remain
non-executable.

Repeated event microsteps whose time separation is below the event-time
tolerance terminate with a structured possible-Zeno diagnostic at a configured
limit. This is a safety contract, not a claim to classify every mathematical
Zeno accumulation.

## Alternatives considered

### Explicit Euler

Smaller, but a poor oracle for stiff residual systems and inconsistent with
implicit-by-default modeling. Rejected.

### General solver dependency

Production-quality nonlinear and DAE packages are eventually necessary, but
making one normative would obscure activation semantics and enlarge the first
trusted implementation. Deferred to realization backends.

### Dense finite-difference Newton — selected for v0

Slow and intentionally limited, but transparent, dependency-free, and able to
exercise algebraic loops and nonlinear residuals. Compiler/runtime conformance
can be tested before sparse lowering exists.

## Verification

- Solve a small nonlinear two-variable residual system.
- Prove two coincident periodic Activations observe one shared `Pre` state and
  commit their `Next` Fields simultaneously.
- Execute a dimensioned thermal ODE driven through a causal signal by a
  periodic proportional controller.
- Check held output between ticks and continuous-state sampling at a tick.
- Show backward-Euler error decreases under time-step refinement against the
  analytic constant-input thermal solution.
- Execute a bouncing ball with two coincident event Relations, localize a
  falling height crossing, and commit height/velocity resets atomically.
- Reverse event direction and graph insertion order to prove direction and
  deterministic grouping semantics.
- Terminate deliberate zero-time chatter with a possible-Zeno diagnostic.

## Security and limits

Run configuration has non-zero nonlinear, step, event-localization, and
zero-time-event limits. Expression evaluation rejects non-finite values.
Exact-clock addition is checked and diagnosed rather than wrapping. The
implementation does not execute native callbacks or user source.

## Unresolved questions

- Residual scaling across heterogeneous physical dimensions.
- Structural diagnosis and least-squares semantics for non-square systems.
- Guard/statechart activation and event priority beyond simultaneous grouping.
- Event-time sensitivity, saltation matrices, and differentiable resets.
- Higher-order dense output and mathematical Zeno classification beyond the
  explicit zero-time safety limit.
- Production residual scaling and DAE execution beyond the bounded RFC 0031
  scalar-physical reference composition.
