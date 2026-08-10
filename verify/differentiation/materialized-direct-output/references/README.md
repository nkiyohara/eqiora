# Materialized direct-output reference contract

## Authority and authoring boundary

This is the preimplementation, non-implementer-owned reference contract for
one generic host-local `f64` materialized direct-output differentiation path.
It was frozen on repository revision
`1b0f7056b505e76b1a5c4b2d31ef1fa4c40a7c82`, before the two public entries or
their evidence implementation existed. The implementing agent may wire this
contract but may not change, tune, or relax its meanings, bounds, projections,
or falsifiers. A claimed inconsistency stops the implementation and returns
the proof to an independent reference owner.

The controlling authorities are:

- the sealed direct-solver composition decision v2, SHA-256
  `4e59db8e841137e81a1b014afc1ade8239b8dd4c9cfd839f19a5f5b8eebc26ac`;
- its accepted focused fresh review, SHA-256
  `ede0a38fb3635ae2c08b9c7792f3735a012fba175438de8212c28e787b8bae36`;
  and
- `verify/numerics/linear-backends/expected/sparse-lu-contract.json`,
  SHA-256
  `666309634cca3d6be5d16d8e90e6ad01d0b92694cbb70fd03acce38ef8e98780`.

The JSON is immutable mathematical input. This case must verify its digest and
load its exact rational values; it must not copy or edit the fixture or widen
the existing `numerics.linear-backends` claim. The two decision files are
coordination authority, not repository inputs and are not copied into the
case.

## Exact generic witness

From `mathematics.principal` in the bound JSON, use exactly

```text
      [ 5  -4   0  -3   0]        b = [16, -1, 7, -9, 1]^T
      [-1   3   2   0   0]        x = [1, -2, 3, -1, 2]^T
  A = [ 0  -1   1   0   1]        y = [9/2, 5, -2, 1/2, 3/2]^T
      [ 0   0  -2   3   0]
      [-1   0   0   2   2]

  A x = b
  A^T y = b
  det(A) = 64
```

`A` is the fixture's zero-based, sorted, unique, 14-entry canonical CSR
matrix, not a separately authored dense matrix. The display above is only its
exact rational mathematical rendering. It is nonsingular and neither
structurally nor numerically symmetric.

Give the canonical *primal* source the structurally distinct values

```text
q := [0, 0, 0, 0, 0]^T
w := [0, 0, 0, 0, 0]^T.
```

Then `A w = q` exactly by the additive identity, and `det(A) != 0` makes `w`
the unique accepted state for this source. Every component of the derivative
RHS `b` is nonzero, so `q` differs from `b` in every component. Neither `q`
nor `w` is a fitted numerical result.

Use one well-typed Model Parameter `p`, with accepted value `p_* = 0` and the
sealed direction `dp = 1`. Its typed semantic identity must be the same at
relation construction, direction submission, and gradient return; no chosen
numeric or string identifier is oracle content. Define the exact linear
relation and scalar output by

```text
R(w, p) := A w - b p
J(w, p) := b^T w.
```

At `(w, p) = (0, 0)`, the primal residual is exactly zero,
`R_w = A`, `R_p = -b`, `-R_p dp = b`, `J_w = b`, and `J_p = 0`.
The canonical source therefore retains `q`, while both derivative problems
have the distinct RHS `b`.

The expected forward and adjoint statements are kept as exact projections of
the already frozen rationals:

```text
dw = x                    because A x = b
lambda = y                because A^T y = b
dJ[dp] = b^T x
dJ/dp = J_p - R_p^T y = b^T y
b^T x = y^T b             by A x = b and A^T y = b.
```

The evidence must evaluate the displayed sums from the bound rational vectors.
It must not store their reduced scalar as another expected literal. It first
applies the fixture's existing residual and componentwise solution bounds to
the recovered normal and transpose vectors, then checks the returned output
tangent and Parameter gradient through the displayed projection identities.
There is no second output tolerance.

## Existing plan and bounds

Use exactly the fixture-bound direct plan and no substitute:

```text
algorithm              SparseLu
operator property      General
preconditioner         Identity
reduction policy       Fast
scalar                  f64
maximum iterations      1
relative tolerance      0
absolute tolerance      2^-30
componentwise ceiling   2^-28
```

The absolute-residual predicate is the fixture predicate. The same submitted
derivative RHS and oriented relation action govern the backend true-residual
check and the independent differentiation replay. The componentwise ceiling
is the existing fixture-derived ceiling, not a newly selected tolerance.

Resource bounds are exactly dimension five, one typed Model Parameter, one
scalar output, one finite derivative RHS per call, and one direct
factor-and-solve attempt per non-early-exit call. Normal and adjoint calls may
refactorize independently. There is no factor reuse, prepared lifecycle,
performance, fill, memory, cache, allocation, or wall-clock requirement.

## Mandatory ordinary positive path

The evidence executes this order, and both positive results complete before a
falsifier counts:

1. Verify the bound fixture digest, load its exact `A`, `b`, `x`, `y`, and
   bounds, and establish `A w = q` exactly before derivative work.
2. Construct `AssembledLinearizedRelation::from_canonical` at `(w,p)=(0,0)`
   with the one typed Model Parameter and the complete dense one-column action
   `R_p = -b`.
3. Pair `J` with that relation through
   `new_with_canonical_state_jacobian`, supplying exactly
   `relation.state_jacobian()`. Shape, finiteness, property, and accepted-point
   checks must pass before solving.
4. Call unchanged `forward_output_sensitivity` with `dp=1` and the exact plan.
   The direct boundary must observe coefficient source `A`, canonical source
   RHS `q`, derivative problem RHS `b`, and `Normal` orientation. It factors
   the original coefficients, reports `Normal`, recovers `x` under the bound
   residual/componentwise checks, passes relation JVP replay against `b`, and
   returns the exact projection `b^T x` under the rule above.
5. Call unchanged `adjoint_output_gradient` with the exact plan. The direct
   boundary must again observe coefficient source `A`, canonical source RHS
   `q`, and derivative problem RHS `b`, now with `Transposed` orientation. It
   factors the original coefficients, reports `Transposed`, uses the
   transpose action of that factor without constructing an explicit `A^T`
   CSR source, recovers `y` under the bound residual/componentwise checks,
   passes relation VJP replay against `b`, and returns `b^T y` under the rule
   above.
6. Establish the exact forward/adjoint projection equality shown above. A
   negative result cannot rescue a failure of either ordinary call.

The canonical association is the exact source supplied by
`relation.state_jacobian()` for this accepted pair. This binds the relation and
solve meanings without claiming that pointer equality, CSR byte equality,
shape, or a matching numerical solution establishes semantic lineage.

## Mandatory non-vacuous falsifiers

Each falsifier begins only after the ordinary positive pair has completed. It
must pass all unrelated shape, finiteness, property, plan, source-presence, and
orientation-admission gates, reach the named direct boundary, and fail at the
targeted check. An earlier unrelated denial does not count.

### Derivative RHS replaced by the canonical primal RHS

Submit the admitted ordinary normal problem `(A, b, Normal)` with canonical
source RHS `q`. Its zero initial vector is tested against the derivative RHS
`b`; the fixture freezes that squared residual as `388`, so it is not an early
success. After that decision, mutate only provider factor-and-solve RHS
selection to use `state_jacobian.right_hand_side() = q` instead of
`problem.right_hand_side() = b`.

The mutant must factor the original `A` and reach its solve. Since `A` is
nonsingular it returns zero for `q`. The backend's final normal true residual
is evaluated against the actual derivative problem `b`, has the already frozen
squared value `388`, and rejects it against `2^-60`. If control nevertheless
reaches differentiation, relation JVP replay against `b` must also reject it.
An initial shortcut using `q`, or a shape/source/property/orientation failure,
does not exercise this falsifier.

### Transpose route returns the normal solution

Submit the admitted ordinary transposed problem `(A, b, Transposed)`. Mutate
the provider result at the transposed solve boundary so that it returns the
fixture's normal solution `x` while preserving the submitted source, RHS, and
reported `Transposed` orientation. The falsifier must reach differentiation's
independent VJP replay; it must not count an earlier source or orientation
denial. The fixture freezes

```text
||b - A^T x||_2^2 = 386,
```

so relation VJP replay rejects the result against `2^-60`. This probe proves
the replay boundary; it is not permission for a correct backend to waive its
own oriented true-residual check.

### Same-shape foreign canonical source

Let `P` be the exact cyclic row permutation recorded by the fixture's
`rhs-permuted` falsifier,

```text
(P v)_i = v_[1,2,3,4,0][i],
```

and construct the foreign canonical coefficient source `B` by `P B = A`
(`B = P^-1 A`). It is a valid nonsingular 5x5 CSR source with the same
`General` property, and it captures the same structural primal pair
`B w = q = 0`. It is not the relation's `A`.

Bind this foreign source at the otherwise ordinary pair boundary. Shape,
finiteness, property, plan, and normal orientation all pass. Direct solving of
`B z = b` is equivalent to `A z = P b`, so it reaches the provider and returns
the exact `rhs-permuted.wrong_vector` already frozen in the fixture. The
backend residual against `B z = b` passes. Differentiation then replays the
solution against the accepted relation `A z = b`; the fixture freezes that
squared residual as `934`, so the post-solve JVP replay rejects it against
`2^-60`.

This probe distinguishes relation action from same-shape canonical material.
It does not define a general source-mutation taxonomy or make row permutation
an accepted production operation.

## Failure conditions and STOPs

Fail closed before publishing a derivative result for the earliest applicable
condition: malformed/non-finite data; an unaccepted primal point; source,
relation, or output layout mismatch; property substitution; absent canonical
source; derivative RHS/source-RHS confusion; lost or relabelled orientation;
explicit transpose materialization; provider/plan or iterative fallback;
factorization or finite-result failure; backend oriented-residual failure;
relation JVP/VJP replay failure; or a foreign, stale, reassembled, or
deep-cloned-for-convenience source whose accepted relation association is not
established.

Stop and return to the contract owner rather than editing this reference if:

- `q` and `b` are not distinct in execution;
- any falsifier cannot reach its frozen boundary or is rejected only by an
  unrelated earlier gate;
- another expected scalar, RHS, accepted state, output, gradient, or tolerance
  is proposed without the policy-required independent derivation;
- either frozen public entry cannot express this witness without another
  public type, field, enum variant, diagnostic, wire, or solver-plan value;
- the exact canonical source, derivative RHS, property, and orientation cannot
  coexist safely in one request; or
- implementation needs a local solve, explicit transpose source, Stokes
  symmetry shortcut, prepared factor owner, or wider path/API.

## Nonclaims

This contract does not establish an Eqiora implementation, a passing case, or
support in any backend other than the later bounded faer reference path. It
does not establish Stokes E2, the separate numerics-to-differentiation L3
dependency, quadrature, a scientific formula/value/tolerance, or any gallery
result. It does not widen the historical normal-only `numerics.linear-backends`
case.

It makes no claim for matrix-free input, complex scalars, conjugate transpose,
devices, distributed execution, another solver algorithm, multiple RHS,
factor reuse, prepared factors, persistence, performance, or general
materialization. It adds no Model, Geometry, Mesh, Realization, Run, Result,
wire, schema, artifact, provider identity, or durable source identity.

The borrowed canonical association proves only that this accepted pair was
constructed with the relation's exact source. It does not infer semantic
lineage from pointer/`Arc` identity, allocation sharing, bytes, shape,
properties, solution agreement, or a digest. Application provenance remains a
separate owner. The later private Stokes shared-`Arc` handoff is outside this
generic contract.

## Research ledger

**Current best formulation.** The native linear relation `R(w,p)=A w-b p`
with `J(w,p)=b^T w` uses the accepted matrix/vector objects directly, keeps the
primal `q=0` structurally distinct from derivative `b`, and makes normal,
transpose, and RHS ownership observable without another scalar oracle.

**Rejected alternatives.** Reusing `b` as the primal source would let the
motivating wrong-RHS-owner implementation pass. A nonzero primal pair would
add oracle data without more discrimination. A prepared factor, explicit
transpose CSR, provider-specific differentiation path, or Stokes-symmetry
shortcut would widen the contract without improving this generic witness.

**Open questions.** Only independent acceptance of this reference, subsequent
implementation evidence, and the already separated RFC/quadrature/Stokes
successors remain. They are not answered here.

**Smallest next experiment.** After this reference is accepted, run the two
ordinary 5x5 calls first, followed by the wrong-source-RHS,
wrong-normal-on-transpose, and foreign-source probes. No performance or reuse
experiment can change this contract.

**Red-team note.** A provider can return numerically plausible answers while
reading the wrong RHS, and a same-shape source can pass its own backend
residual while disagreeing with the relation. The zero/nonzero RHS split and
independent relation replay make those two false successes observable.

## Nonchecks

This reference author did not inspect or run a future implementation, add a
fixture/checker/expected scalar/tolerance, execute Eqiora, run a gate, amend an
RFC or case manifest, or change any source, registration point, capability
claim, dependency, or lockfile. The bound JSON was read without modification;
its existing exact values and bounds were not re-derived or reclassified as a
new claim.
