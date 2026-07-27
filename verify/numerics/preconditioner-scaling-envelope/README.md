# Preconditioner scaling envelope

This case is a **falsifier for the stable preconditioner vocabulary**, not an
implementation of a new one. It exists to satisfy the second, disjunctive entry
condition of the deferred gates in
[the library and accelerator strategy](../../../docs/development/library-and-accelerator-strategy.md):
investigation may start when the current method demonstrably breaks a
**pre-declared** resource, convergence, or robustness envelope, and the breach
is the falsifier.

The "Declared envelope" section below was written before any iteration count was
measured, and its thresholds have not been altered since. One thing did change
after the first run: the **probe**. The first probe turned out to measure
nothing, for a reason recorded in full under "Run 1". Replacing a void
instrument is not the same as moving a line, and both runs are kept here so the
difference is auditable.

## Declared envelope

*Declared 2026-07-26, before any measurement. Unchanged since.*

### Solver controls, identical at every level and in both runs

`SolverPlan(ConjugateGradient, relative = 1e-10, absolute = 1e-14,
maximum_iterations = 50 000)`, with `PreconditionerPolicy::Identity` and
`PreconditionerPolicy::Jacobi` as the two compared series. Also held fixed:
`f64` scalar, replicated layout, host serial target, offline schedule,
`Reproducible` reduction, the Eqiora reference CG backend, and full Dirichlet
elimination on all six faces.

### Refinement sequence

`cells_per_axis n in {4, 8, 16, 32}`, so `h = 1/n` and each step is one exact
halving of `h`.

| `n` | Q1 FEM free unknowns `(n-1)^3` | TPFA FVM unknowns `n^3` |
| --- | --- | --- |
| 4 | 27 | 64 |
| 8 | 343 | 512 |
| 16 | 3 375 | 4 096 |
| 32 | 29 791 | 32 768 |

### Validity conditions on the measurement itself

A run that fails one of these is **void** — reported as "measured nothing",
never as adequacy and never as a breach.

- **V1 — relative-dominated stopping.** `residual_target = max(absolute,
  relative * ||b||)` must exceed the absolute floor at every level, so the same
  relative residual reduction is requested at every refinement.
- **V2 — uncensored counts.** No solve may reach the iteration cap.
- **V3 — non-degenerate Krylov probe.** The right-hand side must not lie in a
  low-dimensional invariant subspace of the operator, because a Krylov method
  terminates in as many steps as that subspace has dimensions, *independently of
  the mesh*, which would counterfeit scalability. Checked as: identity-CG must
  exceed 3 iterations at the coarsest level, and the count must not be constant
  across the sequence.

V3 was added after Run 1 and is the reason Run 1 is void. It was declared before
Run 2 was measured.

### What "adequate" means

Let `it(n)` be accepted CG iterations at `cells_per_axis = n`, let
`rho_k = it(2n_k) / it(n_k)` be the per-halving growth ratio, and let `s` be the
least-squares slope of `log2(it)` regressed on `log2(n)` over the four declared
levels.

Theory fixes the two ends of the scale. For a second-order elliptic operator
the condition number is `kappa = O(h^-2)`, so CG needs `O(sqrt(kappa)) = O(h^-1)`
iterations: `s -> 1` and `rho -> 2`, iterations roughly double per halving of
`h`. A scalable method — the class AMG belongs to — holds iterations near
constant: `s -> 0` and `rho -> 1`. Jacobi rescales the diagonal; it does not
change the `O(h^-2)` conditioning of the operator, so it is expected to move the
constant, not the order.

**ADEQUATE** (the envelope holds; the gate stays shut) iff, for Q1 FEM with
`Jacobi` — the strongest preconditioner in the stable vocabulary:

- **A1** `max_k rho_k <= 1.4`, and
- **A2** `s <= 0.5`.

**BREACH** (the envelope is broken; the falsifier fires) iff, for the same
series, all three hold:

- **B1** the terminal ratio `rho(16 -> 32) >= 1.8`;
- **B2** the fitted slope `s >= 0.85`;
- **B3** the total growth `it(32) / it(4) >= 5.0` (theory predicts `2^3 = 8`).

Between the two is a declared **indeterminate band**. Landing in it is neither
adequacy nor a breach: it is reported as "no breach, the gate stays shut". The
band is declared deliberately so that a marginal result cannot be argued into a
breach after the fact.

The cell-centred TPFA FVM series is measured and recorded with the same
statistics, but it is **corroborating observation only**. The breach predicate
is evaluated on the Q1 FEM `Jacobi` series alone, and that was fixed here rather
than chosen later from whichever series looked stronger.

### Declared discriminator, and a prediction stated in advance

**D1** `|s_Jacobi - s_Identity| <= 0.10` on the FEM series would show that the
stable vocabulary changes the constant but not the order.

Prediction recorded before measurement: on a uniform Cartesian mesh with a
constant coefficient and full Dirichlet elimination, **every** surviving Q1 free
vertex is interior, so the assembled stiffness diagonal is a single constant.
Jacobi is then exactly a scalar rescaling `M = c I`; `M^-1 A = A / c` spans the
same Krylov space as `A`, so Jacobi-CG and Identity-CG are expected to produce
identical or near-identical iteration counts on the FEM series. The TPFA series
is expected to differ because boundary-adjacent cells carry extra facet
contributions and its diagonal is therefore *not* constant.

If that prediction holds it is not a defect of the measurement. It sharpens the
finding: the best preconditioner in the stable vocabulary is a no-op on this
operator class, so the growth being measured is the growth the stable
vocabulary cannot touch.

### Resource envelope

- **R1** the complete measurement (2 methods x 2 preconditioners x 4 levels)
  completes within 600 s wall on a single host core in the unoptimized `test`
  profile.
- **R2** `n = 64` (250 047 FEM unknowns) is declared **outside** this case's
  resource envelope for a debug-profile gate and is not measured. No statement
  in this case extrapolates past `n = 32`.

### Declared mapping from a breach to a gate

Fixed in advance, so that the conclusion is not shaped by the numbers. The
strategy names AMG, restarted GMRES, and field split as three *distinct*
contracts, each needing its own envelope:

| Deferred contract | What a breach here can support |
| --- | --- |
| AMG construction and provenance | **Yes, if B1–B3 fire.** An iteration count growing like `h^-1` on an SPD operator, uncorrected by every preconditioner in the stable vocabulary, is exactly the deficiency a scalable multilevel hierarchy addresses. |
| Restarted GMRES | **No.** CG already has a three-term recurrence with `O(1)` vector memory and no restart or orthogonalization policy to tune. A GMRES envelope must be declared on a **nonsymmetric or indefinite** operator and must breach an **orthogonalization-memory or cost** envelope. An SPD iteration-count breach is silent about it. |
| Field split | **No.** This operator has one field and one algebraic block. A field-split envelope must be declared on a **multi-field or saddle-point** operator, where the coupled block structure — not mesh refinement — is what breaks the current method. |

## Run 1 — void probe, recorded rather than discarded

The first probe reused the registered manufactured cube
[`cartesian-poisson-3d-fem-fvm/models/poisson.eqi`](../cartesian-poisson-3d-fem-fvm/models/poisson.eqi)
unchanged, whose source is the single mode
`3 pi^2 sin(pi x) sin(pi y) sin(pi z)`.

Observed: **1 iteration at every level** for Q1 FEM under both policies and for
TPFA FVM under `Identity` — flat, from `n = 4` to `n = 32`.

That is not scalability. On a uniform tensor-product mesh with homogeneous
Dirichlet data, the discrete operator's eigenvectors are exactly the separable
sine modes sampled on the grid. The load vector of a *single* such mode is
therefore an eigenvector of the operator, `A b = lambda b`, and conjugate
gradients solves an eigenvector right-hand side exactly in one step at every
refinement. The measured "1, 1, 1, 1" is a Krylov finite-termination artifact of
the manufactured solution, not a property of the preconditioner.

Two things follow. First, Run 1 is void under V3 and supports no conclusion in
either direction. Second — and this is a reusable finding — the registered
single-mode manufactured Poisson case **cannot be used as a solver-stress
probe** by anyone, because its right-hand side is spectrally degenerate by
construction. That degeneracy is invisible in its own convergence and
conservation claims, which are about discretization accuracy rather than about
the Krylov spectrum.

The one series in Run 1 that was not flat, TPFA under `Jacobi`, is explained by
the same mechanism in reverse: TPFA boundary-adjacent cells carry extra facet
contributions, so its diagonal is not constant, and diagonal scaling destroys
the eigenvector alignment.

## Run 2 — the declared probe

*Thresholds carried over unchanged from Run 1. The probe itself was chosen
after Run 1 was voided, and its two coarsest levels were executed before it was
finalized — see the disclosure at the end of this section. This section is
therefore not a pre-registration in the auditable sense, and the case manifest
records it as asserted rather than established.*

Run 2 changes exactly one thing: the source. It is replaced by a **constant**
source in [`models/constant-source-poisson.eqi`](models/constant-source-poisson.eqi):

```text
-div(grad(u)) = 1   on (0, 1)^3
u = 0 on the complete boundary
```

The choice is made on a stated principle, not by search. A constant source is
the textbook probe for exactly this experiment, it carries no tunable knob, and
its spectral content is provably broad: the assembled load vector is exactly
`b_i = h^3` on every free unknown (for Q1, because `integral(phi_i) = h^3` at an
interior vertex; for TPFA, because the cell source is `f * h^3`), and the
constant vector expands over the sine basis with coefficients proportional to
`1 / (j k l)` across **all** odd triples. It is not confined to any
low-dimensional invariant subspace, which is what V3 requires.

No exact solution is needed, because this case measures iteration counts rather
than accuracy, and every recorded count already belongs to a solve whose
independently recomputed true residual was accepted against the requested
target.

Before committing to Run 2, the two coarsest levels were executed to confirm V3
only: `n = 4` gave 4 identity-CG iterations and `n = 8` gave 14, so the count is
neither degenerate nor constant. No threshold was touched at that point, and
the terminal ratio and fitted slope that decide B1–B3 were not yet observable.

## Observed — the envelope is breached

Recorded in [`expected/iterations.csv`](expected/iterations.csv). Every count
belongs to a solve whose independently recomputed true residual was accepted
against the requested target, and V1, V2, and V3 all held.

| Series | `n = 4` | `n = 8` | `n = 16` | `n = 32` |
| --- | --- | --- | --- | --- |
| Q1 FEM, `Identity` | 4 | 14 | 25 | 52 |
| Q1 FEM, `Jacobi` | 4 | 14 | 25 | 52 |
| TPFA FVM, `Identity` | 4 | 16 | 44 | 90 |
| TPFA FVM, `Jacobi` | 4 | 19 | 42 | 87 |

Primary series, Q1 FEM with `Jacobi`, against the declared predicate:

| Declared | Threshold | Observed | Verdict |
| --- | --- | --- | --- |
| **B1** terminal ratio `rho(16 -> 32)` | `>= 1.8` | **2.08** | fires |
| **B2** fitted slope `s` | `>= 0.85` | **1.19** | fires |
| **B3** total growth `it(32) / it(4)` | `>= 5.0` | **13.0** | fires |
| **A1** `max_k rho_k` | `<= 1.4` for adequacy | 3.5 | fails adequacy |
| **A2** `s` | `<= 0.5` for adequacy | 1.19 | fails adequacy |
| **D1** `\|s_Jacobi - s_Identity\|` | `<= 0.10` | **0.00** | holds |

**The declared envelope is breached, at the finest declared refinement step.**
All three breach predicates fire together, and the result is not in the
indeterminate band.

The advance prediction is confirmed exactly. Q1 FEM `Jacobi` and `Identity`
agree iteration-for-iteration at all four levels — 4, 14, 25, 52 in both series
— because every free vertex is interior and the assembled diagonal is a single
constant, so Jacobi is precisely a scalar rescaling and leaves the Krylov space
untouched. On TPFA, where the diagonal genuinely varies at boundary-adjacent
cells, `Jacobi` moves the finest count only from 90 to 87: a 3% improvement in
the constant against a fitted slope of 1.45. The stable vocabulary changes the
constant and not the order.

Two honest qualifications on the numbers:

- The coarsest level is **pre-asymptotic**. At `n = 4` the FEM system has 27
  unknowns, so CG's exact finite termination bounds the count from below and
  `it(4) = 4` is artificially small. That inflates both the first ratio (3.5)
  and the fitted slope (1.19 against a theoretical 1.0). The two finest ratios,
  1.79 and 2.08, bracket the theoretical 2.0 without relying on that level, and
  the breach does not depend on it: B1 is evaluated on `16 -> 32` alone.
- Nothing here is extrapolated past `n = 32`, per R2.

R1 held: the complete 16-solve measurement took 510 s wall, inside the declared
600 s. That figure is a **resource-envelope observation, not a performance
claim** — single core of a 12th Gen Intel Core i7-12700, unoptimized `test`
profile, `rustc` 1.97.1. It is dominated by assembly, which the public API
re-runs for each preconditioner at each level.

## What the breach licenses, and what it does not

Applying the mapping declared in advance:

- **AMG construction and provenance — evidence supports opening the gate.** The
  measured deficiency is exactly the one a scalable multilevel hierarchy exists
  to remove: iteration count rising like `h^-1` on an SPD operator, with every
  preconditioner in the stable vocabulary provably unable to touch the order.
  What this licenses is *investigation*, under the strategy's unchanged rule
  that the falsifier and the construction/provenance policy are built first and
  a candidate enters the stable vocabulary only after passing them. It does not
  assert that AMG would in fact fix this growth; that is the next experiment,
  not this one.
- **Restarted GMRES — no support.** This measurement is entirely inside a
  symmetric positive-definite operator solved by CG, which already has a
  three-term recurrence and `O(1)` vector storage. Restart length and
  orthogonalization policy — the whole content of that contract — have nothing
  to act on here. That gate needs its own envelope on a nonsymmetric or
  indefinite operator, breaching an orthogonalization-memory or cost budget.
- **Field split — no support.** The operator has one field and one algebraic
  block. Nothing in this case exercises block coupling, so it cannot speak to a
  solver graph over semantic or algebraic blocks. That gate needs its own
  envelope on a multi-field or saddle-point operator, where block structure
  rather than mesh refinement is what breaks the current method.

## Claim boundary

This case claims a *convergence-envelope breach*, nothing more. It does not
implement, admit, or benchmark any preconditioner outside `Identity` and
`Jacobi`; it does not claim AMG would in fact fix the growth; it does not make a
performance claim; and it records no timing, because a debug-profile wall time
on one host is not a performance observation.

Run:

```bash
cargo test -p eqiora-numerics --test preconditioner_scaling_envelope
cargo run -p eqiora-verify -- run --case numerics.preconditioner-scaling-envelope
```
