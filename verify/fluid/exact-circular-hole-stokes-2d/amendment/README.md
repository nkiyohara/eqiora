# Witness-tuple amendment: relative tolerance `1e-11` → `1e-6`

The frozen physical witness fixes a solve selection. One element of it is
amended here and nothing else: the relative tolerance, and the residual target
mechanically derived from it.

```text
                       superseded            amended
relative tolerance     1e-11                 1e-6
selected target        1.3239627651209673e-12  1.3239627651209673e-07
                       = max(1e-13, rtol * ||b_hat||_2), ||b_hat||_2 = 0.13239627651209673
```

Unchanged: the mesh, the PDE formulation, every physical observation, every
physical tolerance, every selector, the backend, the algorithm, the
preconditioner, the reduction mode, the absolute tolerance `1e-13` and the
iteration cap `10000`. `check_physical_invariance.py` is the proof, not the
assurance.

```bash
python3 amendment/check_physical_invariance.py     # nothing but the tuple moved
python3 amendment/measure_reapplication_floor.py   # route A measurement
julia    amendment/measure_reapplication_floor.jl  # route B measurement
python3 amendment/adjudicate.py                    # the decision rules
```

## Why the superseded value is returned

The argument originally offered for the amendment was that `1e-11` sat below
the floor imposed by storing the solution in binary64. **That argument is
false and is not used here.** It was measured and refuted: the f64-rounded
elevated-precision solution has reduced residual `1.09e-12` (route A) and
`1.03e-12` (route B), *below* the superseded target, which therefore accepts
it with 18–22 % to spare. There is no representation floor above `1e-11`.

What is true is narrower, and was measured on both routes independently:

| | route A | route B |
| --- | ---: | ---: |
| `‖A_hat fl(x*) − b_hat‖₂`, elevated reapplication | `1.0872217642825674e-12` | `1.0305126233950387e-12` |
| verdict of the superseded target | **accepts** (17.9 % margin) | **accepts** (22.2 % margin) |
| same vector, binary64 reapplication, min over orderings | `1.7435339132457643e-12` | `1.5630028457610753e-12` |
| max over orderings | `2.1680564474484507e-12` | `1.9510962877750815e-12` |
| verdict of the superseded target | **rejects** | **rejects** |

Nine binary64 evaluations of the same vector against the same operator — five
summation orders in route A (natural, reversed, ascending and descending by
magnitude, Kahan-compensated), four in route B (BLAS dense matvec, explicit row
loop forward and reverse, and the full-system path both routes actually use) —
and every one of them rejects what elevated precision accepts.

**The superseded target's verdict on the ideal answer is decided by the
evaluator's arithmetic rather than by the quality of the solution.** That is
the defect, and it is why `1e-11` is returned. A target a solver cannot be told
whether it met is not an acceptance policy.

The ordering-robust statement is the standard `gamma_m` bound, which no
summation order can exceed: `2.2250313504501518e-11` (route A),
`2.3481355…e-11` (route B). Adding the representation term gives a decidability
floor on the target of `2.333754e-11`; the superseded target is `17.6×` below
it.

## Why this decade

The frozen contract already commits to a shape for residual roundoff:

```text
4096 * eps * (1 + ||A_hat||_inf * ||x_hat||_inf + ||b_hat||_inf)   = 6.469510e-05
```

Strip the `4096` safety factor and what remains is the contract's own
one-roundoff scale:

```text
S = eps * ||A_hat||_inf * ||x_hat||_inf = 1.5794700928572167e-08
```

`rtol = 1e-6` gives a target `8.382×` above `S`. `rtol = 1e-7` gives
`0.838×` — it does not clear it. **`1e-6` is the tightest decimal decade whose
target exceeds the contract-shaped one-roundoff scale**, and that is the whole
derivation. `adjudicate.py` checks both halves.

Two things about that scale must be stated plainly rather than left implied.

**It is a bound, not a measurement.** These two routes' operators are far
sparser than it assumes — 2 to 10 nonzeros per row, median 5 — so their actual
worst-row cancellation `max_k Σ_j |A_kj x_j|` is `4.719386e+03`, against
`‖A_hat‖_inf · ‖x_hat‖_inf = 7.113301e+07`, a factor of `15000`. The measured
ordering-robust bound is consequently `677×` tighter than `S`. Judged only
against what these two routes need, the tightest defensible decade would be
`1e-9` (`5.7×` over the measured floor), or `1e-8` with a ten-fold safety
factor. The looser, contract-shaped scale is adopted deliberately, because a
production reapplication path is not readable from this package and the frozen
contract itself declines to assume a favourable sparsity structure. **`1e-6` is
conservative by about three decades relative to what these routes' own
operators require, and that conservatism is the point, not an oversight.**

**The decade is sensitive to one convention.** `S` uses `eps`, because the
frozen contract's allowance is written with `eps`. With the unit roundoff
`u = eps/2` the scale halves to `7.897350e-09`, `rtol = 1e-7` clears it at
`1.68×`, and `1e-7` would be the tightest decade instead. The amendment adopts
the contract's own constant. A reader who prefers `u` should read this as
`1e-7`, one decade tighter, and nothing else in this package changes.

There is no backward-stability argument here, and the proposition that offered
one is refuted: the normwise backward error of `fl(x*)` (Rigal–Gaches) is
`2.5014948607585717e-21`, which is `4.4e-05` of a unit roundoff. No
backward-stability floor exists anywhere near either target.

## What the amended gate does not decide

### It does not imply the physical tolerances, and no target could

`measure_reapplication_floor.py` constructs, without any solver, a vector whose
reduced residual equals a given target **exactly** — the direction comes from
inverse iteration on the oracle's own operator, so it is the worst direction
available — and reports the physical error it still carries.

| target | constructed residual | worst pressure-probe shift | × the `3.658737e-10 Pa` production tolerance |
| --- | ---: | ---: | ---: |
| superseded `1.323963e-12` | exact | `1.552598e-10 Pa` | `0.42` |
| amended `1.323963e-07` | exact | `2.720699e-05 Pa` | `74362` |

So the superseded target *did* incidentally imply the pressure tolerance, and
the amended one does not. That loss is real and is recorded rather than
minimised. It costs nothing here, because physical acceptance in this case has
never rested on the residual gate: it rests solely on the dual-derived
observations and balances, which are unchanged.

More importantly, the two roles are **mutually exclusive on this witness**:

```text
evaluation-decidability needs   target >  2.333754e-11
implying the pressure tolerance needs target <= 3.119952e-12
                                                disjoint by 7.48x
```

No relative tolerance whatsoever makes the residual gate both decidable
independently of evaluation rounding and strong enough to imply the pointwise
pressure tolerance. The gap is a property of the operator — `cond_2` is
`4.64e+09` — not of the tolerance table. **The residual gate can only be a
solver-health and stopping threshold on this witness.** That is a measured
result, and it is the reason the amendment's character is what it is.

### It does not repair the strict product predicate

The strict predicate `true_residual <= residual_target` is untouched here and
remains a non-claim. It is also **not made satisfiable** by this amendment.
Route B's non-implementing f64 MINRES analogue reaches a best true residual of
`2.467383e-05` over 20000 unstopped iterations; the amended target is
`1.324e-07`, `186×` smaller. Measured across decades, with the recurred
residual driving the stopping test:

| rtol | target | stops at | of cap | true residual there | pressure error |
| --- | ---: | ---: | ---: | ---: | ---: |
| `1e-11` | `1.323963e-12` | 9401 | 94.0 % | `2.467956e-05` | `9.3653e-08 Pa` |
| `1e-10` | `1.323963e-11` | 8423 | 84.2 % | `2.467956e-05` | `9.3350e-08 Pa` |
| `1e-9` | `1.323963e-10` | 7811 | 78.1 % | `2.467956e-05` | `6.8068e-08 Pa` |
| `1e-8` | `1.323963e-09` | 6704 | 67.0 % | `2.467955e-05` | `6.6847e-08 Pa` |
| `1e-7` | `1.323963e-08` | 6299 | 63.0 % | `2.467956e-05` | `7.2412e-08 Pa` |
| **`1e-6`** | **`1.323963e-07`** | **5832** | **58.3 %** | **`2.467925e-05`** | **`3.3316e-07 Pa`** |
| `1e-5` | `1.323963e-06` | 5289 | 52.9 % | `2.471485e-05` | `9.5377e-06 Pa` |

Three things follow, all measured. The true residual is **flat** — it has
already floored by iteration ~5300, so no decade in this range satisfies the
strict predicate; the smallest that would is about `1.9e-4`. Liveness is a
**continuum**, not a cliff: every decade terminates inside the cap, so liveness
alone does not select `1e-6` — it only shows the superseded value was closest
to the cap, at 94 %. And the pressure error is over its tolerance at **every**
decade, moving by a factor of 3.6 while the target moves five, so tightening
the target buys almost no physical accuracy.

The row for `1e-6` is the run now frozen in route B's document. The other rows
were measured by the same non-implementing analogue and are recorded here only;
they are advisory, not oracle inputs, and not production measurements.

### It loosens two derived bounds inside this package

Because the frozen contract derives them from the selected target, amending the
target loosens them mechanically:

- the routes' own true-residual limit, `target + roundoff_allowance`, moves from
  `6.469509727648616e-05` to `6.482749222903548e-05` — a 0.2 % change;
- the agreement gate's weak pressure-row limit,
  `target + 4096·eps·(1 + weak_norm + target)`, moves from `2.233457e-12` to
  `1.323972e-07` — **five decades looser**.

No frozen residual changes: the weak norms are `3.14e-40` and `1.62e-75` and
clear both bounds by more than 32 orders of magnitude. But the gate's power to
reject a *mutated* weak residual is correspondingly coarser, and two mutation
probes recorded in [`../agreement/README.md`](../agreement/README.md) — weak
norms at `2.2334574668971316e-12` and at `1e-11` — would now be accepted where
they were previously rejected. That is a real reduction in falsifier strength
and it is stated, not absorbed.

## Falsifiers and must-accept probes

`adjudicate.py` applies 24 rules. The load-bearing ones:

**Must-accept.** The corrected probe is `fl(x*)` under *both* evaluation modes,
not a representation-floor claim:

1. the superseded target accepts `fl(x*)` under elevated reapplication, in both
   routes — this is what refutes the original rationale;
2. the amended target accepts `fl(x*)` under elevated reapplication and under
   every measured binary64 ordering, in both routes, by `≥ 6.1e+04×`.

**Must-reject.**

3. the superseded target rejects `fl(x*)` under every measured binary64
   ordering, in both routes — the verdict flip;
4. the superseded target lies below the ordering-robust decidability floor in
   both routes;
5. `rtol = 1e-7` fails to clear the contract-shaped one-roundoff scale, so the
   chosen decade is the tightest that does;
6. a constructed vector meeting the amended target exactly violates the
   pressure tolerance by `74362×`, so the gate does not imply physical
   acceptance;
7. the decidability floor exceeds the implication ceiling, so no rtol can serve
   both roles.

**Invariance.** `check_physical_invariance.py` compares each frozen document
against a pre-amendment record generated while the superseded documents were
still in the tree. `complement_sha256` digests the whole document with the
amendment-allowlisted leaves removed; `physical_sha256` digests the named
physical subtrees. Both are unchanged in both documents, so nothing outside the
allowlist moved — the superseded files are not needed to prove it.

## Non-claims

1. **Not a physical accuracy claim.** Measured, twice: a vector meeting the
   amended target exactly can be `74362×` outside the pressure tolerance, and
   the frozen f64 analogue is over that tolerance at every decade tested.
2. **No representation-floor claim.** The superseded target is *above* the
   f64-rounded solution's elevated-precision residual, in both routes.
3. **No backward-stability claim.** `eta(fl(x*)) = 2.50e-21`, `4.4e-05 u`.
4. **Not the tightest decade the measurements alone would support.** Against
   these routes' measured evaluation bound that would be `1e-9`, or `1e-8` with
   a ten-fold factor. `1e-6` follows the contract-shaped bound instead, and is
   about three decades conservative.
5. **Not convention-free.** With `u = eps/2` in place of the contract's `eps`,
   the same derivation selects `1e-7`.
6. **Does not repair the strict product predicate**, which is unchanged and
   unsatisfiable on this witness at every decade from `1e-11` to `1e-4`.
7. **Not a general solver default.** Case-local, this witness, this operator.
8. **No production implementation was read, executed or consulted**, here or
   anywhere in this package. The MINRES numbers are route B's non-implementing
   Julia analogue, never an oracle input.
9. **The `4096·eps` agreement allowance is unchanged** and remains an oracle
   route finite-precision self-check. It is not promoted to product acceptance.

## Environment and method

Linux `6.11.0-1014-lowlatency` x86-64. CPython `3.12.3`, mpmath `1.3.0`, Julia
`1.12.6`, ruff `0.15.13`. These numbers describe this machine and predict
nothing about hosted CI.

Route A assembles closed-form MINI/P1 blocks and solves by dense LU; the
amendment measurement runs it at 60 decimal digits, above the route's own 40, so
the reapplication is strictly more accurate than the solution being tested. Its
residual at the *unrounded* solution is `2.04e-57`. Route B assembles by
explicit `3×3` Gauss–Legendre Duffy quadrature with the bubbles never
condensed and solves at 256-bit `BigFloat`, residual `1.31e-73`. Both then round
their own `x_hat` to binary64 and reapply.

The rounding is certified decided: the closest any component comes to a
rounding tie is `1.28e-03 ulp` (route A), so a solve error of `1e-57` cannot
change a rounded bit. Both routes' `‖b_hat‖₂`, `‖x_hat‖_inf` and `‖A_hat‖_inf`
reproduce the frozen documents bit-for-bit, which is what establishes that the
measured object is the frozen one.

While preparing this amendment the route A measurement was additionally run at
100 decimal digits and route B at 512 bits, and `rho_elevated` was bit-identical
in every reported digit at both. Those runs used the same route sources at a
raised precision and are **not** what the committed scripts do — the committed
scripts run at 60 digits and 256 bits — so that reproduction is reported as
preparation evidence rather than as a check this package re-runs.

## What was not checked

1. **The production reapplication path.** It is not readable from here. The
   decade derivation therefore uses the contract-shaped bound rather than these
   routes' measured sparsity, and the resulting margin is stated above.
2. **The true lattice minimum** of `‖A_hat x − b_hat‖₂` over binary64 `x`. That
   is a closest-vector problem and is not solved. Every measurement here is at
   the single point `fl(x*)`; no claim is made that no other binary64 vector
   achieves a smaller residual.
3. **Any hosted measurement.** Everything here is local.
4. **The registered repository gate.** This case still carries no `case.toml`;
   see the case [`README.md`](../README.md).
