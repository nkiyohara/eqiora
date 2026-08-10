# Issue #407 successor evidence contract v1

## Status, authority, and scope

- Contract date: 2026-08-07 (Europe/London).
- Repository base: `b364bec54f4ccc6d25586d8c02c6fcfed3ce1c5f`.
- Accepted decision:
  `/data/nk523/.tmp/stokes-full-demo-contract-decision-v2.md`, SHA-256
  `0f9ba28f7b38cfede35ac78c50f5bfd85018a442aafb88884c13fe7a7693d5fe`.
- Focused acceptance:
  `/data/nk523/.tmp/stokes-full-demo-contract-decision-v2-focused-review.md`,
  SHA-256
  `584ed831469626b0f723d5b879dc95264d4b6ca3c66b72da7cf95a1420767dcb`.
- Frozen Issue identity: **#407 — Freeze bounded-cell Stokes dissipation
  design and dual oracle**.
- Status: successor evidence contract candidate. It freezes the question,
  ownership, future evidence paths, input schema, acceptance predicates, and
  STOP boundary. It does not authorize implementation.

The sole writable path of this lane is this document. This lane writes no
repository path, GitHub state, source fixture, mesh, expected value, tolerance,
test, branch, or implementation.

## Precise proposal and design check

The public question is whether Eqiora can start from an exact circle in a
fixed square, preserve analytic body area exactly while varying two even polar
modes, re-solve an all-Dirichlet steady MINI/P1 Stokes problem for every
admitted trial, expose the complete discrete dissipation gradient, preserve an
immutable private trial history, and corroborate the initial-to-final
dissipation ordering on an independently precommitted refined topology.

Three evidence formulations were considered:

1. A continuous Pironneau-style boundary shape derivative would be
   mathematically natural but would add a two-dimensional continuous-shape
   claim, boundary-density oracle, and remeshing questions that the selected
   product does not need.
2. Independent finite differences alone would be cheap, but would not derive
   the accepted complete discrete reduced-gradient meaning and could agree
   with the same omitted-state or stale-geometry error.
3. A complete finite-dimensional analytic/discrete derivation reconciled with
   a separately implemented non-Eqiora numerical realization directly tests
   the selected MINI/P1 discrete question without widening it.

Option 3 remains selected. It is the narrowest formulation faithful to the
accepted decision and gives the strongest useful disagreement signal for its
cost. Options 1 and 2 are not fallbacks: needing either to close the accepted
claim is a contract STOP and amendment decision.

## Frozen public identity and question

The public title is:

> **Stokes exact-area dissipation shape optimization**

The additive gallery catalog identity is:

```text
optimization.stokes-cell-dissipation
```

The registered executable evidence identity and location are:

```text
optimization.stokes-cell-dissipation-2d
verify/optimization/stokes-cell-dissipation-2d/
```

The pre-existing catalog identity `optimization.cylinder-drag` and Issue #212
remain the deferred Richardson/exterior physical-drag authorities. They are
neither replaced nor promoted by this contract.

The frozen public Python product identities are:

```python
eqiora.fluid.open_stokes_dissipation_design(
    candidate_root=path,
    expected_receipt_sha256=digest,
) -> StokesDissipationDesignProjection

StokesDissipationDesignProjection
StokesDissipationTrialSummary
```

These identities describe one product-specific immutable read projection.
They do not create a general optimizer, study, trial, history, artifact, or
registry API. The complete property vocabulary and compatibility review belong
to the later API envelope; no implementation may infer missing property names
from this evidence contract.

## Frozen scientific model

Let `r_A > 0`, `A_0 = pi r_A^2`, `mu > 0`, and `U > 0`. The fixed outer cell
and fluid domain are

```text
Q        = [-10 r_A, 10 r_A] x [-10 r_A, 10 r_A],
Omega(a) = Q \ K(a).
```

For `a = (a_2, a_4)`, the body is the centred positive polar graph

```text
rho_a(theta) = r_A
  * (1 + a_2 cos(2 theta) + a_4 cos(4 theta))
  / sqrt(1 + (a_2^2 + a_4^2)/2),
theta in [0, 2 pi).
```

The complete design set and start are

```text
|a_2| + |a_4| <= 1/2,
a_start = (0, 0).
```

The analytic area identity, not polygonal mesh area, is

```text
1/2 integral_0^(2 pi) rho_a(theta)^2 dtheta = A_0.
```

On `Omega(a)`, solve

```text
-div sigma(u,p) = 0,
 div u           = 0,
 sigma(u,p)      = -p I + 2 mu epsilon(u),
 epsilon(u)      = (grad u + grad u^T)/2,
```

with

```text
u = 0       on the body boundary,
u = U e_x   on every side of the outer square,
integral_Omega p dA = 0.
```

There is no body force, inlet, outlet, traction boundary, pressure outlet,
periodicity, inertia, or physical time. `n_K` is the body-outward normal and
`n_Omega = -n_K` on the body; no force integral is an observable.

The continuous quantity being approximated is

```text
E(a) = 2 mu integral_Omega epsilon(u):epsilon(u) dA,
```

with units W/m. `J(a) = E(a)/(mu U^2)` may be displayed as a dimensionless
normalization. Neither quantity is drag.

The executable objective is the complete fixed-topology MINI/P1 discrete
functional `E_h(a)`. Its accepted derivative is only `grad_a E_h`: the
discretize-then-differentiate derivative of analytic boundary regeneration,
harmonic interior motion, element maps, the precommitted quadrature, the
Stokes residual and gauge, the state solve, and dissipation evaluation with
respect to `(a_2, a_4)`. It is not a continuous shape derivative, a remesh
derivative, or a Pironneau/Richardson boundary-density oracle.

## Geometry, state, and history ownership

One body-fitted affine-triangle reference topology and one distinct refined
topology are independently precommitted. Each contains genuine fluid-interior
vertices, a fixed outer boundary, fixed ordered body-angle samples, explicit
body/fluid membership, and exact topology identity.

For every reference-history trial:

- body vertices are regenerated from the analytic `rho_a` at the fixed
  ordered angles;
- outer vertices remain fixed;
- fluid-interior vertices follow the accepted P1 harmonic-coordinate seam;
- connectivity stays fixed;
- coefficient admission, orientation, mesh quality, harmonic residual, and
  correspondence pass before the Stokes solve; and
- the exact Geometry revision, Mesh state and topology, correspondence,
  Model, Realization, Run, Result, derivative artifact, and trial identity
  remain associated.

No remeshing occurs within a derivative, line search, or accepted history.
The distinct refined topology independently re-evaluates the initial circle
and accepted final design. Publication requires the accepted-final
dissipation to remain strictly below the initial dissipation on that topology.
This is ordering corroboration, not a mesh-independent optimum claim.

The native product owner alone owns the canonical full history. Every attempted
trial is immutable and records its order, accepted parent, coefficients,
identity chain, objective and area observations with units, derivative
identity/status, search direction and step, sufficient-decrease inputs and
outcome, validity flags, and exactly one terminal disposition:

```text
accepted
outside design set
invalid geometry
invalid mesh
solve failure
derivative failure
insufficient decrease
budget exhaustion
```

The accepted sequence is an ordered subsequence of the complete trial
sequence. Rejected trials remain present and ordered. Design history is never
a physical `Trajectory`. Python, Marimo, the viewer, and renderers receive only
the native-owned immutable projection; they do not parse a history receipt or
recompute scientific meaning.

## Source map contract

The future source map has the exact repository path

```text
verify/optimization/stokes-cell-dissipation-2d/references/pironneau-source-map.md
```

It must bind every source-derived statement to a page, section, equation, or
quoted figure caption in the inspected publisher artifact:

- O. Pironneau, *On optimum profiles in Stokes flow*, JFM 59(1), 117--128
  (1973), DOI `10.1017/S002211207300145X`;
- publisher artifact SHA-256
  `a3845478ce7bb336480a4d2cdd630afde19c02812037bca3fb766ce5f139ef2e`;
- section 2, especially (2.1), for the fixed outer boundary, moving no-slip
  body, prescribed outer velocity, steady Stokes state, and
  symmetric-gradient dissipation lineage;
- (2.2) only for Pironneau's qualified dissipation/drag discussion, never as
  a finite-cell force equality;
- section 4 for first-variation and fixed-volume lineage; and
- section 5 for descent discussion and the unbounded-domain caveat.

The map must label the following as Eqiora benchmark specializations rather
than source statements: two spatial dimensions, the square factor `10`, the
two-mode normalized polar family, the coefficient diamond, MINI/P1
discretization, exact reference/refined topology, harmonic interior motion,
the complete discrete reduced derivative, optimizer constants, finite-
difference sequence, history format, and every expected value or tolerance.

Richardson 1995, DOI `10.1098/rspa.1995.0103`, appears only in an exclusion
entry: exterior two-dimensional flow, equivalent/effective-radius drag, and
constant-surface-vorticity claims remain deferred. Its unavailable full text
supplies no formula, coefficient, value, or tolerance here.

The source map also lists these accepted repository contracts as reusable
mechanisms, never as new scientific oracles:

- `fluid.exact-circular-hole-stokes-2d`;
- `geometry.exact-circular-hole-geometry`;
- `geometry.circular-hole-chordal-reference-mesh`;
- `fsi.fixed-topology-ale-monolithic-2d`;
- `differentiation.spatial-shape-optimization`;
- `interfaces.python-exact-cylinder-stokes-result`;
- `interfaces.python-exact-cylinder-pressure-still`; and
- the accepted #239 common gallery admission/media path.

## Single sealed-input authority

Both independent derivation lanes consume exactly one byte-identical input
artifact at this external staging path:

```text
/data/nk523/.tmp/issue407-stokes-dissipation-sealed-inputs-v1.json
```

After acceptance, its repository destination is exactly:

```text
verify/optimization/stokes-cell-dissipation-2d/references/sealed-inputs.json
```

The input artifact does not yet exist. A fresh-context non-implementer evidence
owner must author it after this contract is accepted, make it mode `0444`, and
publish its SHA-256 before either derivation starts. A partial file, placeholder
value, directory digest, mutable symlink, or separately supplied side input is
not sealed input.

The JSON must contain, in one canonical object, all of the following before it
may be sealed:

1. this contract path, SHA-256, and accepted-review SHA-256;
2. the accepted decision and focused-review paths and hashes above;
3. the exact Pironneau artifact identity and source-map obligations;
4. exact numerical values and coherent-SI units for `r_A`, `mu`, and `U`;
5. the fixed equations, boundary labels, gauge, profile formula, design-set
   predicate, and start design identities;
6. complete reference and refined topology bytes or immutable content-addressed
   file identities, including exact vertex/facet/cell counts, connectivity,
   ordered boundary-angle samples, membership, and correspondence;
7. the harmonic-coordinate input and acceptance predicates;
8. the MINI/P1 basis, assembly, element-map, quadrature, gauge, linear/nonlinear
   solver, convergence, state-residual, orientation, and mesh-quality
   conventions and limits;
9. the complete derivative observation plan: coordinate and directional
   probes, independently regenerated plus/minus geometries, finite-difference
   steps, and comparison predicates;
10. reduced-descent and history inputs: direction rule, sufficient-decrease
    rule, backtracking constants, coefficient/geometry/mesh admission order,
    stationarity predicate, terminal priority, and iteration/attempt budget;
11. the exact refined-ordering designs and association predicates;
12. raw input-size caps and a deterministic implementation-independent
    abstract-work bound for both evidence routes;
13. the units, comparison forms, acceptance bands, and tolerances that the
    registered oracle will require, but no expected numerical output; and
14. the minimum mutant identities and required rejection stage frozen below.

This contract intentionally chooses none of the values in items 4 and 6--13.
In particular it does not choose mesh bytes or counts, solver or quadrature,
finite-difference steps, optimizer constants or budget, expected values, or
tolerances. Tolerances are precommitted in the future sealed input; expected
numerical outputs are independently produced by both routes and frozen only by
accepted reconciliation. Omitting any required input keeps both derivation and
implementation stopped. Adding or changing an input after either route starts
invalidates both route artifacts; the input must be versioned, resealed, and
both routes restarted from fresh context.

Neither derivation may receive Eqiora implementation output, candidate output,
writer scratch, an unsealed fixture, or an additional scientific input. The
only other readable authorities are this accepted contract, its accepted
decision/review chain, the source artifact named above, and the accepted
repository contracts at the exact base.

## Isolated analytic/discrete derivation

The exact external output path is

```text
/data/nk523/.tmp/issue407-stokes-dissipation-analytic-discrete-route-v1.md
```

Its exact repository destination is

```text
verify/optimization/stokes-cell-dissipation-2d/references/analytic-discrete-route.md
```

One fresh-context non-implementer owns this path and no other #407 evidence
output. The writer reads neither implementation, implementation plans, the
numerical-route scratch/output, nor reconciliation scratch. The completed
single-file artifact is sealed mode `0444` with SHA-256 before the numerical
route is revealed.

This route must:

- distinguish every Pironneau statement from every Eqiora specialization;
- derive the polar-area identity and both coefficient derivatives;
- derive the exact finite-dimensional MINI/P1 Stokes residual, pressure gauge,
  discrete dissipation, and complete reduced adjoint/JVP/VJP derivative for
  the sealed topology, element maps, harmonic motion, and quadrature;
- establish sign, body/fluid normal, coherent-unit, and optional `J`
  normalization conventions;
- compute the sealed positive-path and mutant observations without using
  Eqiora output;
- report the sealed sufficient-decrease, derivative-comparison, history,
  refined-ordering, and terminal predicates exactly as applied;
- report formulas, numerical values, residuals, hashes, commands,
  environment, and resource use needed for reproduction; and
- state the continuous-shape, force/drag, remesh, optimality, and portability
  nonclaims.

It must return `STOP`, with the smallest disagreement argument, if the sealed
question is underdetermined, internally inconsistent, exceeds its sealed
resource bound, or requires a continuous boundary derivative. It never edits
the sealed input or selects a looser tolerance.

## Isolated independent numerical realization

The exact external output path is

```text
/data/nk523/.tmp/issue407-stokes-dissipation-independent-numerical-route-v1.md
```

Its exact repository destination is

```text
verify/optimization/stokes-cell-dissipation-2d/references/independent-numerical-route.md
```

A different fresh-context non-implementer owns this path and no other #407
evidence output. The writer reads neither Eqiora implementation/output,
analytic-route scratch/output, nor reconciliation scratch before sealing. It
may use an independently written Julia, Python, boundary-integral, or mixed-FEM
realization, but may not import Eqiora arrays, ordering, meshes, results, or
executable formulas. The completed single-file artifact is sealed mode `0444`
with SHA-256 before either route is revealed to the other.

Starting only from the sealed input, this route must independently:

- regenerate every analytic body geometry and harmonic/interior mesh state;
- solve the same all-Dirichlet square-cell Stokes problem and zero-mean gauge
  on both precommitted topologies;
- compute state residuals and `E_h` for the start, derivative probes, attempted
  trials, and refined-ordering designs;
- compute centred coefficient differences over the sealed step sequence plus
  the sealed independent directional comparisons;
- execute at least one ordinary valid nonzero decreasing trial before any
  mutant verdict is counted;
- preserve and digest the full accepted/rejected trial order and exact
  associations;
- run the minimum mutants below, including stale-parent-state and false-
  refinement substitutions; and
- report values, residuals, hashes, commands, environment, resource use, and
  nonclaims sufficient for reproduction.

A finite channel, exterior solver, frozen-state calculation, same-polygon
perturbation, or Eqiora-produced topology is a different question and must
return `STOP`. Exceeding the sealed abstract-work bound also returns `STOP`;
the route may stream or deterministically recompute intermediates unless the
sealed public observation requires retention.

## Ordinary positive evidence

Both routes must complete the same ordinary path before a negative probe can
count:

1. validate the sealed circle and both distinct topology identities;
2. regenerate the circle geometry, bind correspondence, and admit the
   reference mesh;
3. solve the all-Dirichlet state with the zero-mean gauge and obtain finite
   residuals, pressure, and `E_h`;
4. derive or independently compute the complete two-coordinate discrete
   gradient;
5. regenerate and re-solve every sealed plus/minus coefficient perturbation
   and pass coordinate and directional comparisons;
6. attempt the sealed backtracking sequence and accept at least one nonzero
   trial satisfying all admission and sufficient-decrease predicates;
7. retain at least one rejected trial as well as the complete accepted
   subsequence and identity lineage;
8. reach exactly one honest terminal disposition; and
9. re-evaluate the initial and accepted-final designs on the distinct refined
   topology and preserve strict final-below-initial `E_h` ordering.

A mutant rejected before this ordinary path becomes usable is vacuous. It is
reported as `NOT EXERCISED`, not killed.

## Minimum mutant and falsifier contract

The reconciled evidence and later registered production case must reject all
of the following at the named semantic boundary:

| Mutant | Required rejection |
| --- | --- |
| omit or corrupt the sign of the area-normalization denominator | analytic area/profile check before meshing |
| swap `a_2`/`a_4` or use the wrong harmonic angle | regenerated geometry and coordinate/directional derivative comparison |
| apply body expansion using the fluid-normal sign | analytic/discrete sign comparison |
| change any outer side from `U e_x` to inlet/outlet/traction meaning | boundary-contract admission before solve |
| omit or duplicate the pressure gauge | gauge/residual admission before objective use |
| omit the factor implied by `2 mu epsilon:epsilon` | objective formula/unit and independent value comparison |
| hold geometry, element maps, quadrature, or state fixed in the derivative | complete reduced-gradient comparison |
| perturb the deformed polygon instead of regenerating `rho_a` | exact geometry identity before finite-difference comparison |
| substitute a parent state, Run, or Result into a regenerated child | exact child Geometry/Mesh/Run/Result association **and independently replayed child-state residual before objective or acceptance** |
| alias/substitute the reference topology as refined, or swap initial/final refined associations | distinct topology identities and correct design/objective association before ordering |
| accept an invalid mesh because its objective decreases | geometry/mesh admission before solve or sufficient decrease |
| delete, overwrite, or reorder a rejected trial | immutable complete-history order/digest check |
| relabel budget exhaustion as stationarity | terminal predicate and disposition check |

The bold stale-state and false-refinement boundaries are mandatory production
falsifiers, not observations that may be discharged only in an external
oracle script. For stale state, association alone is insufficient: a replayed
residual on the exact child geometry and mesh must reject the substituted
state before objective or acceptance is read. For false refinement, numerical
ordering alone is insufficient: distinct topology identity and exact
initial/final association must pass before the ordering comparison.

## Reconciliation ownership and output

Only after both route files are sealed may a third fresh-context non-writer
receive the sealed input and the two route artifacts. That reconciler owns only

```text
/data/nk523/.tmp/issue407-stokes-dissipation-reconciliation-v1.md
```

whose repository destination is

```text
verify/optimization/stokes-cell-dissipation-2d/references/reconciliation.md
```

The reconciler compares, at minimum:

- source-versus-specialization classification;
- geometry, area, normal, sign, gauge, and unit conventions;
- primal state residual and dissipation observations;
- both gradient coordinates and every sealed directional observation;
- sufficient-decrease decisions and full history order/associations;
- each non-vacuous mutant verdict and rejection stage; and
- distinct-topology initial/final refined ordering.

The reconciler does not average results, choose a preferred route, widen a
band, relax a tolerance, change an input, repair either route, or inspect
implementation output. Exact or banded comparisons use only the predicates
precommitted in the sealed input. Any unresolved disagreement produces
`STOP`, preserves both arguments, and leaves implementation stopped.

An `ACCEPT` reconciliation freezes the exact route hashes and the agreed
oracle observations for integration into

```text
verify/optimization/stokes-cell-dissipation-2d/expected/oracle.json
```

by the evidence/integration owner. The implementer may wire that accepted
artifact but may not author, tune, or relax it. Reconciliation acceptance is
not candidate acceptance and establishes no performance or optimum claim.

## #396 dependency closure and #407 dependency boundary

Before this contract can be integrated, #396 must carry exactly this bounded
dependency header:

```text
Parent issue: #312
Depends on: #238, #239
Consumes accepted cases:
  geometry.exact-circular-hole-geometry
  geometry.circular-hole-chordal-reference-mesh
  interfaces.python-exact-cylinder-stokes-result
  interfaces.python-exact-cylinder-pressure-still
  interfaces.python-trajectory-field-stills
```

#396 has no direct or transitive #245 dependency. It displays selected
immutable design projections through accepted Geometry, Mesh, Result, Field,
still, and presentation nouns. It derives no pressure, dissipation, area,
derivative, trial acceptance, or sampling meaning, creates no second Result
authority, and does not reinterpret design trials as physical time. Broader
geometry-to-solve authoring remains in #245 and is outside this chain.

#407 consumes the accepted cases listed in the source-map section and the
dependency-closed #396 statement only as a future presentation dependency. It
has no #165, #212, #245, #388, force, wake, or drag prerequisite. #396 may run
in parallel because it owns a disjoint non-calculating presentation seam; it
is not needed to derive or reconcile this evidence.

## Bounds

The scientific bounds fixed now are:

- two spatial dimensions and one out-of-plane-unit-length interpretation;
- one fixed square with half-width `10 r_A`;
- one centred, positive, regular two-even-mode polar body;
- exactly two design coordinates with `|a_2| + |a_4| <= 1/2`;
- steady inertia-free incompressible Stokes flow;
- Dirichlet velocity on the body and all four outer sides;
- one zero-mean pressure gauge;
- one fixed reference topology during the complete derivative/history and one
  distinct refined topology for ordering corroboration;
- one ordinary path containing at least one accepted nonzero step and one
  rejected trial; and
- exactly the terminal disposition vocabulary above.

Evidence resource bounds are mandatory but deliberately unresolved here. The
sealed-input owner must precommit raw byte/count caps and deterministic
abstract work for geometry generation, topology validation, solves, derivative
probes, attempted trials, retained observations, and mutant execution. Live
allocation, allocator behavior, worklist lifetime, or wall-clock is not an
oracle unless a later public claim explicitly adds it.

## Nonclaims

This contract and its future evidence do not claim:

- Richardson exterior flow, profile, effective/equivalent radius, drag value,
  or constant-vorticity condition;
- physical force, lift/drag coefficient, or equality of dissipation and
  force;
- continuous shape calculus, continuous stationarity, boundary-density
  correctness, or remesh differentiation;
- source-profile reproduction or an approximation to exterior flow;
- mesh-independent objective, gradient, stationarity, or optimum;
- a global or unique optimum, convergence outside the exact accepted history,
  or robustness outside the two-coordinate family;
- arbitrary CAD, splines, topology, translation, rotation, scale, odd/sine
  modes, or outer-cell optimization;
- transient flow, Navier--Stokes inertia, wake physics, turbulence, 3D, or FSI;
- performance, portability across optional backends, or a resource-residency
  guarantee;
- a public receipt/history wire or reusable optimizer, history, study, trial,
  artifact, registry, differentiation, or mesh-motion abstraction; or
- implementation or publication authority from this contract alone.

The terminal design is the **accepted final iterate**. It is called stationary
only if the independently precommitted stationarity predicate passes, and it
is never called an optimum under this contract.

## STOP decisions

Keep derivation, reconciliation, implementation, candidate production, and
publication stopped when any of these holds:

- this contract lacks focused fresh-context acceptance at its exact SHA;
- the sealed-input artifact is missing, incomplete, mutable, unhashed, or
  differs between routes;
- any mesh byte/count, solver/quadrature choice, finite-difference step,
  optimizer constant/budget, expected value, band, or tolerance is selected
  after either derivation sees output;
- a route reads implementation, candidate, writer scratch, or the other
  route before both are sealed;
- the source map attributes an Eqiora specialization to Pironneau or imports a
  Richardson value from unavailable full text;
- either route cannot complete the ordinary positive path within the sealed
  bounds;
- either route needs a continuous shape derivative, remeshing, physical
  force, a general public abstraction, or a second scientific input;
- a formula, sign, scale, unit, gauge, gradient, history decision, mutant
  rejection, or refined ordering does not reconcile;
- a mutant passes, is killed only by an unrelated earlier denial, or the stale-
  state/false-refinement boundary is weaker than frozen above;
- the refined topology is not identity-distinct or reverses the claimed
  accepted-final-below-initial ordering;
- any trial is lost or stale state survives to objective/acceptance;
- title, catalog, case, API, or #396/#407 dependency identity drifts;
- a finite-cell observation is labelled drag, Richardson, exterior, optimal,
  or mesh independent; or
- implementation would require changing the frozen scientific question or
  tuning accepted evidence.

A STOP returns the exact disagreement or missing obligation to the contract or
evidence owner. It does not relax a predicate, silently narrow evidence,
average routes, or authorize speculative implementation.

## Successor handoff

If this contract receives exact-SHA focused acceptance, the next decision-
changing action is one non-implementer sealed-input lane. Only after its
complete JSON is sealed may the analytic/discrete and independent numerical
routes start in parallel at their exact isolated paths. Their completion
unlocks one reconciliation lane. Implementation remains stopped until
reconciliation accepts and Envelope 1 integrates the contract, source map,
sealed inputs, both routes, reconciliation, and oracle through the registered
case identity.

## Research ledger

**Current best formulation.** The accepted bounded square, exact-area
two-mode profile, all-Dirichlet steady MINI/P1 Stokes dissipation problem with
a complete discrete reduced gradient, immutable native history, and distinct-
topology ordering corroboration.

**Rejected or superseded.** Richardson exterior drag for the first gallery;
finite-cell reaction relabelled drag; a continuous boundary derivative;
finite differences as the only derivative meaning; remesh-general gradients;
and a general optimizer/history schema.

**Open questions owned by sealed evidence.** Exact physical scale, reference
and refined topology, harmonic/mesh predicates, solver, gauge realization,
quadrature, derivative steps and directions, optimizer constants and budget,
stationarity predicate, resource caps, values, bands, and tolerances.

**Next evidence.** Focused contract review, then one sealed-input precommit,
two isolated independent routes, and one non-writer reconciliation.

**Red-team note.** The most plausible false success is a visually improving
history whose derivative freezes part of the state, whose child reuses its
parent Result, or whose “refined” check aliases the reference topology. Exact
child residual replay and topology/design identity are therefore mandatory
before objective, acceptance, or ordering can count.

## Nonchecks

- No source PDF was opened in this lane; its accepted hash and source mapping
  were inherited from the accepted decision.
- No mesh, Stokes solve, derivative, optimization, candidate, fixture, oracle
  value, tolerance, or evidence output was produced.
- No repository, GitHub, test, or gate state was read or changed beyond the
  repository governance/contract documents needed to author this external
  contract.
