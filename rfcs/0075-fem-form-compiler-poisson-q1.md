# RFC 0075: FEM form compiler, Cartesian Q1 Poisson slice

- Status: Draft
- Authors: Claude (contract and oracle), Codex (implementation)
- Created: 2026-07-25
- Related RFCs and evidence: RFC 0007 (canonical spatial operators),
  RFC 0020 (local action kernel boundary), RFC 0053 (discrete block system),
  RFC 0056 (pure calculus and support maps),
  [AI-authored platform strategy](../docs/development/ai-authored-platform-strategy.md),
  `verify/numerics/cartesian-poisson-fem-fvm`,
  `verify/differentiation/spatial-poisson-fem-fvm`,
  `verify/numerics/global-matrix-free-action`

## Summary

Derive the Cartesian Q1 Galerkin local matrix, load vector, and paired
residual/JVP/VJP actions for the `org.example.poisson` strong relation from the
typed semantic residual through one private, proof-carrying derivation, and
accept it only against a pre-committed independent oracle.

## Motivation

Physics capabilities currently re-earn their proof machinery. Across
independent physics owners in `eqiora-numerics`, the same structural roles are
re-created: `api.rs` ×7, `assembly.rs` ×4, `acceptance.rs` ×4, `element.rs` ×4,
`newton.rs` ×3 — 22 files and 11,178 lines of candidate repeated surface.
`acceptance.rs` ×4 is the conformance oracle itself, rebuilt per physics.

Under agent authorship the binding constraint is mechanical verifiability. An
agent can safely emit a form; it cannot safely hand-write a physics module with
a bespoke oracle. This RFC tests one hypothesis, on the smallest problem that
can falsify it:

> Can the *compiler* own the proof of its translation class, so that a physics
> instance supplies only witness data?

Poisson is chosen because it has no inf-sup requirement, no gauge, no
stabilization, and an existing registered case with analytic reference — so a
failure here is a failure of the hypothesis, not of the problem.

## Proposed design

### Boundary of this slice

This is an **FEM** lane. A universal weak-form IR is rejected on its merits:
conservative finite-volume face fluxes are method-foreign to a variational form
(RFC 0007). The FVM path is untouched and keeps its existing evidence.

No new crate, no new public type, no new language. The work is a private module
`crates/eqiora-numerics/src/form_compiler.rs`, registered privately from
`src/lib.rs`. Promotion of any contract into `eqiora-ir` is out of scope and
gated on amendment A1's audit-compression test.

### Input is not sufficient as it stands

`RelationDecl::residuals` carries componentwise **strong** residuals. The source
program does not carry test or trial functions, measures, integrals, element
families, quadrature, DOF layout, or weak boundary-term disposition, and field
declarations do not distinguish unknown from coefficient, frozen, or design
role. Supplying that structure fail-closed is the substance of this slice, not
an assumption.

### Execution path

```text
packages/org.example.poisson/src/main.eqi
  -> eqiora-lang AST
  -> eqiora-compiler
  -> eqiora_sem::KernelProgram::typed_relation_residual
  -> scalar-elliptic fail-closed recognizer
  -> private DerivedScalarGalerkinForm + derivation certificate
  -> reference Q1 evaluator
  -> eqiora_assembly::LocalContribution
       |-> assembly packet / CSR / existing solve
       `-> matrix-only LocalLinearActionIr projection
```

`LocalContribution` is the output target because it owns a local matrix **and**
RHS. `LocalLinearActionIr` is insufficient alone: Poisson needs a source term,
which RFC 0020 deliberately excludes from local action.

### Admitted subset, closed

The derived plan represents exactly:

- `coefficient * dot(grad(test), grad(trial))`;
- `source * test`;
- exact relation and field identities;
- complete homogeneous essential boundary discharge.

Any other structure **fails**. No generic callback, no escape hatch, no
backend identifier becomes mathematical meaning.

### Admission gates

All must pass before assembly; each fails closed.

| Gate | Required output |
| --- | --- |
| Static semantics | Replayed typed residual: every node carries dimension, shape, frame, and nominal support; every root matches relation scope. Consumes `KernelProgram::typed_relation_residual`. |
| Role assignment | Explicit inventory of unknowns, coefficient/frozen fields, parameters, volume relations, and boundary relations. Every semantic node consumed exactly once; ambiguity and leftover relations fail. |
| Derivation certificate | Closed versioned rule IDs binding exact source relation and node identities to each weak term: test pairing, negative-divergence integration by parts, volume terms, complete boundary disposition. |
| Realization compatibility | Exact test and trial space, reference cell, local DOF ordering, geometry map, quadrature policy. Declared quadrature exactness is recorded, never treated as inferred proof. |
| Bounded compilation | Limits on DAG nodes, derivative order, integral terms, quadrature points, local DOFs, temporaries, and generated work. |

Passing every gate makes the form **compiler-admissible**. That is not a
capability claim. Only the registered case below makes it verified.

### Reused contracts

`DiscreteSpace` / `HypercubeQ1Space` for basis tabulation; `AffineGeometryMap`
and `QuadratureRule` for geometry and integration; `ScalarSpatialExpression`
with its `evaluate_jvp` / `evaluate_vjp` for derivative actions;
`LinearizedRelation` as the existing linearization boundary;
`LocalContribution` as output. The public `ScalarEllipticCartesianModel` API is
preserved unchanged.

### Bounded claim

The exact `org.example.poisson` strong relation lowers through a private
proof-carrying 2D Cartesian Q1 Galerkin form, produces local matrix and RHS
contributions and paired residual/JVP/VJP actions on the CPU reference path,
and agrees with independent analytic, assembled, convergence, and conservation
oracles.

### Nonclaims

Source-level weak forms; arbitrary expressions; natural, Robin, or mixed
boundary conditions; vector, mixed, or nonlinear forms; simplex, high-order, or
adaptive spaces; native or JIT code generation; a public form IR; CUDA or MPI
execution; and any performance property.

### Stop condition

If the admitted subset requires a special case at essentially every term, stop
and report rather than widening toward a general weak-form IR. Proceed to a
second consumer only if this plan deletes or simplifies a hand-written residual
or derivative path **without weakening any oracle**.

## Alternatives considered

**A public `eqiora-form-compiler` crate with a general weak-form IR.** Rejected.
It spends the abstraction budget before the hypothesis is tested, requires a
public compatibility promise for a design with zero consumers, and reopens the
universal-variational-form question that RFC 0007 settled on FVM grounds. A
private module can be deleted; a published IR cannot.

**Continue hand-written vertical slices per physics.** Rejected as the default,
retained as the fallback. It is mathematically safe and has produced correct
results, but it re-earns the oracle per physics — 22 files of repeated role
surface at four physics — and under agent authorship the review cost of that
growth is the binding constraint. If this slice fails its stop condition, this
alternative is what Eqiora continues with, and that outcome is informative.

**Generate from the existing `calculus/` proof seam.** Rejected for this slice.
That module is a bounded tensor-calculus and support-map checker whose standard
operators are only `SymmetricPart` and `IsotropicLift`; it has no quadrature,
basis, or DOF concept and explicitly excludes general weak forms. Extending it
would enlarge a central proof surface before the hypothesis is tested.

## Compatibility and migration

No public API changes. `ScalarEllipticCartesianModel` keeps its signature.

Migration is shadow-first: the derived evaluator is routed through
`CartesianEllipticCell::evaluate` for exactly the Q1 slice, and the existing
closed-form path is retained as test-only shadow code. Every local contribution
and the final CSR and RHS are compared against the current path before the old
path is considered for removal. Numerical agreement alone does not authorize
removal — the derivation certificate and mutant gates must also hold.

`lower_cartesian_q1_diffusion_local_action` continues to project the matrix
portion to `LocalLinearActionIr`, unchanged in meaning.

## Verification

The oracle below is **pre-committed**: it was derived analytically from the
bilinear basis before any implementation was read, and per amendment A4 the
implementing agent may not author, tune, or relax it.

### Reference values

Node ordering is counter-clockwise from the lower-left corner of
`[x0, x0+hx] x [y0, y0+hy]`. With `alpha = hy/(6 hx)`, `beta = hx/(6 hy)`:

```
K = k * ( alpha * [[ 2,-2,-1, 1],     +  beta * [[ 2, 1,-1,-2],
                   [-2, 2, 1,-1],                [ 1, 2,-2,-1],
                   [-1, 1, 2,-2],                [-1,-2, 2, 1],
                   [ 1,-1,-2, 2]]                [-2,-1, 1, 2]] )
```

Unit square, exact rationals, `6*K`:

```
[ 4 -1 -2 -1 ]
[-1  4 -1 -2 ]
[-2 -1  4 -1 ]
[-1 -2 -1  4 ]
```

The registered case must use a **non-square, non-unit, off-origin** cell:
`x0=0.25, y0=0.5, hx=0.25, hy=0.5, k=1`. `hx != hy` separates the two derivative
contributions; `x0,y0 != 0` prevents a source bug from hiding behind symmetry.

```
K = [ 0.83333333333333326 -0.58333333333333326 -0.41666666666666663  0.16666666666666666]
    [-0.58333333333333326  0.83333333333333326  0.16666666666666666 -0.41666666666666663]
    [-0.41666666666666663  0.16666666666666666  0.83333333333333326 -0.58333333333333326]
    [ 0.16666666666666666 -0.41666666666666663 -0.58333333333333326  0.83333333333333326]
```

Exact load for `f = 2 pi^2 sin(pi x) sin(pi y)`, computed by separable analytic
integration rather than quadrature:

```
F = [0.42549571438121403, 0.47482060177589164, 0.2710258553802215, 0.24287139083576761]
```

Exact load for constant `f = 1`: `hx*hy/4 = 0.03125` at every node.

### Structural invariants

Every row of `K` sums to zero; `K` is symmetric with positive diagonal; the load
vector sums to the exact cell integral of the source; `alpha` and `beta` depend
only on the aspect ratio.

### Mutant set

Each mutant must be rejected by a **named** gate. A mutant that survives means
the gate set is incomplete and is itself a reportable defect.

| # | Mutation | Rejecting gate |
| --- | --- | --- |
| M1 | Flip the integration-by-parts sign | derivation certificate; sign disagreement against `K` |
| M2 | Drop one essential boundary relation | role assignment — every node consumed exactly once |
| M3 | Duplicate a boundary relation on one domain | role assignment — ambiguity fails rather than silently resolving |
| M4 | Swap `alpha` and `beta` | non-square cell comparison; invisible on a square cell |
| M5 | Swap test/trial gradient indices | structural transpose check; numerically invisible for this symmetric operator |
| M6 | One-point quadrature for the source term | manufactured-load comparison; declared exactness is not proof |
| M7 | Scale the flux coefficient | element comparison; also breaks the balance residual |
| M8 | Reclassify `source_scale` as an unknown | role assignment |
| M9 | Corrupt one Jacobian column | JVP against independently rebuilt centered differences |
| M10 | Return the primal residual as the VJP | VJP against a scalar directional derivative — **not** JVP/VJP duality, which is self-referential |
| M11 | Omit the source term in assembly | conservation: boundary reactions plus integrated source |
| M12 | Off-by-one in local-to-global DOF mapping | shadow comparison of final CSR and RHS |

M5 and M10 are precisely the mutants a generated-versus-generated check would
pass. They are why a hand-derived oracle must exist independently.

### Registered evidence

New: `verify/numerics/compiled-cartesian-poisson-q1-2d/`,
`crates/eqiora/tests/compiled_package_poisson_form.rs`, and focused element
tests in `eqiora-numerics`.

The case must compile the actual package through the ordinary explicit Q1
Realization path; compare the non-unit affine cell against the values above;
shadow-compare every local contribution and the final CSR and RHS; replay the
four-level continuous-L2 convergence floor of 1.9 and the relative balance limit
of 2e-11; compare JVP and VJP against independently rebuilt centered
differences; reject every mutant above; and explicitly run
`numerics.cartesian-poisson-fem-fvm`,
`differentiation.spatial-poisson-fem-fvm`, and
`numerics.global-matrix-free-action`.

## Security, safety, and governance

No unsafe code, no new trust boundary, no irreversible action. Compilation is
bounded in DAG size, derivative order, quadrature points, local DOFs, and
temporaries, so a malformed or adversarial package cannot induce unbounded work.

Governance: the implementing agent must not author, tune, or relax the oracle,
expected values, tolerances, or falsifiers for its own implementation
(amendment A4). Wiring a pre-committed fixture is permitted; owning the evidence
content is not. The integrator role is per slice and runs the affected closure.

## Unresolved questions

- Which second physics consumer best falsifies the audit-compression claim —
  the elasticity patch, or the thermal slab? The elasticity patch exercises
  vector fields and is the stronger test, but is a larger step.
- Should the derivation certificate be persisted in the artifact wire, or
  remain an in-process proof? Persisting it creates a compatibility promise
  that this slice does not need.
- What is the exact quadrature policy record when declared exactness exceeds
  the integrand degree, and does it belong in the Realization or the
  certificate?
- Whether the shadow path is deleted at the end of this slice or after the
  second consumer. Deleting it early removes the strongest available oracle.
