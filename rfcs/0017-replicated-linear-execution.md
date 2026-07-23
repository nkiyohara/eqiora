# RFC 0017: Replicated linear execution and fixed-order reductions

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-18

## Summary

Host-local linear solvers execute operator actions and scalar reductions
through one backend-neutral replicated-execution contract; the reproducible
inner product uses a fixed logical partition and ordered final composition
that are independent of the available worker count.

## Motivation

The first threaded slice decorates only `LinearOperator`. This proves that CSR
output rows have unique writers, but every conjugate-gradient inner product
still runs on the calling thread. Extending that shape with a reference-CG
copy inside the Rayon adapter would make the numerical algorithm depend on
placement. Adding a Rayon callback to `LinearProblem` would instead make a
mathematical problem own execution policy.

The missing boundary is smaller and more general. A replicated host execution
knows how to apply one operator to complete resident vectors and how to execute
the scalar reduction selected by the numerical plan. It does not own the
operator, solver algorithm, convergence threshold, or vector layout.

## Proposed design

### Ownership

```text
LinearProblem                  operator + rhs + asserted properties
SolverPlan                     algorithm + tolerance + reduction policy
LinearSolverBackend            numerical algorithm
ReplicatedLinearExecution      operator placement + scalar-reduction placement
SolveReport                    backend + execution + numerical evidence
```

`eqiora-solver` owns the execution trait and a direct serial implementation.
`eqiora-fabric` owns the run-scoped Rayon implementation. Rayon types remain
inside that adapter. Distributed vectors retain their separate ownership,
halo, and collective contracts; this RFC does not relabel them as replicated
vectors.

`LinearSolverBackend::solve` remains the direct serial convenience path. Its
lower-level execution entry point receives an explicit
`ReplicatedLinearExecution`. The reference CG algorithm is implemented once
and calls the execution for every operator action and every norm or inner
product, including independent true-residual acceptance. A threaded decorator
selects the Rayon execution before invoking that same backend implementation.

An execution report is constructed by the execution that actually performed
the actions. It is not attached to a serial report after the solve. A nested
or incompatible execution request fails closed.

### Reproducible inner product

The reproducible `f64` inner product has one Eqiora-owned lowered action:

1. partition both equal-length inputs into consecutive logical chunks of 1,024
   elements;
2. evaluate every chunk left-to-right;
3. retain partials in ascending chunk-index order; and
4. combine partials left-to-right.

The chunk length and both local and final orders are numerical contract, not a
Rayon scheduling hint. The serial implementation evaluates the same lowered
action sequentially. The Rayon implementation evaluates independent partials
in its run-owned pool, collects the indexed results in logical order, and uses
the same ordered final composition. Therefore one and N workers have the same
floating-point expression tree.

Every product, partial, and final sum must remain finite. Shape mismatch,
overflow, unsupported reduction policy, an operator without disjoint row
actions, and worker/pool mismatch return stable diagnostics. Rayon panic or
poison recovery does not authorize a partial result.

The first Rayon execution admits only `ReductionPolicy::Reproducible`.
`Fast` remains a separate benchmark gate because Rayon parallel reductions do
not specify a floating-point composition order. A backend-native fast solver
is not silently reclassified as reproducible.

### Capabilities and provenance

Constructing a threaded solver intersects the underlying backend capability
with the execution's admitted reduction policies. An adapter that lacks the
reproducible policy cannot be wrapped by this first slice. The accepted
`SolveReport` records the underlying numerical backend, Rayon execution ID,
exact worker count, and selected reduction policy independently.

Run-manifest adapter versions identify the implementation release. A future
change to the reproducible expression tree requires an explicit compatibility
decision; changing task granularity alone must not change results.

## Alternatives considered

### Duplicate the conjugate-gradient driver in `eqiora-fabric`

This has low initial implementation cost and can be tuned directly, but it
duplicates convergence, restart, preconditioner, and true-residual logic. The
two algorithms would drift and placement would acquire numerical meaning.
Rejected.

### Store an executor or Rayon pool on `LinearProblem`

This is mechanically compact, but couples mathematical problem identity and
lifetime to one execution placement. It also complicates comparing serial,
threaded, and future device executions over the same problem. Rejected.

### Let Rayon reduce floating-point values directly

This offers the lowest runtime overhead and is appropriate for a future fast
policy. Rayon documents that parallel reduction order is unspecified, so it
cannot establish worker-independent reproducible evidence. Rejected for the
reproducible path.

### Use an exact or compensated superaccumulator immediately

This can provide stronger order independence, but adds a new arithmetic
representation and performance trade-off before a scale case requires it.
The fixed expression tree is sufficient for the present replicated `f64`
oracle. Deferred as a separately evidenced policy.

The selected contract has the strongest alignment with the mathematical
decomposition and the lowest long-term algorithm duplication. Its extra API
surface is exercised immediately by both serial and Rayon executions.

## Compatibility and migration

No Semantic Model, Realization wire, or artifact schema changes. The public
Rust backend trait gains an execution-aware method while retaining the common
serial `solve` call. External backend implementors must deliberately support
or reject non-serial execution; this is acceptable while the crate is 0.x.

The reference path changes to a fixed-chunk reproducible expression tree.
Problems of at most 1,024 entries preserve the former left-to-right expression
exactly. Larger reference vectors may change in their least significant bits;
the new order is thereafter worker-independent and version-governed. Existing
analytic tolerances and independently recomputed residual gates remain the
acceptance authority.

## Verification

The implementation must falsify the design with:

1. a vector longer than two logical chunks whose serial, one-worker Rayon, and
   four-worker Rayon inner products are bit-identical;
2. values chosen so an unordered reassociation would produce a different
   answer;
3. exact serial/one-worker/four-worker CG values and numerical report fields
   for one partitionable SPD operator;
4. canonical Poisson/P1 FEM serial and four-worker agreement, including an
   independently recomputed true residual and exact execution provenance;
5. rejection of an unpartitionable operator, unsupported fast reduction,
   incompatible nested execution, and a backend without reproducible support;
   and
6. workspace formatting, Clippy, tests, dependency layers, and documentation.

Parallel contribution assembly is the next gate. It will reuse stable logical
work identity and ordered accumulation, but it will not be hidden inside the
linear-reduction trait.

## Research basis

- [Rayon `ThreadPool`](https://docs.rs/rayon/latest/rayon/struct.ThreadPool.html)
  executes nested parallel work inside an explicitly owned pool through
  `install`.
- [Rayon indexed parallel iterators](https://docs.rs/rayon/latest/rayon/iter/trait.IndexedParallelIterator.html)
  retain index identity and support ordered collection.
- [Rayon parallel iterators](https://docs.rs/rayon/latest/rayon/iter/trait.ParallelIterator.html)
  explicitly do not specify floating-point reduction order.

These APIs supply placement only. Eqiora continues to own numerical policy and
conformance evidence.

## Security, safety, and governance

The implementation uses safe Rust and a bounded, run-owned thread pool. It
does not mutate Rayon's process-global pool, spawn unbounded work, accept
untrusted code, or cross an FFI boundary. Vector sizes bound task counts and
allocation. Backend or execution panics do not become accepted numerical
evidence.

Changing a supported reduction policy or reproducible expression tree changes
a numerical capability claim and requires review with new conformance
evidence.

## Unresolved questions

- Whether a scale case justifies compensated or exact accumulation as a new
  named policy.
- Which fast-reduction benchmark and NUMA topology first justify widening the
  Rayon capability.
- Whether device executions use the same fixed tree or a device-specific
  reproducible policy with distinct provenance.
