# SparseLu exact-rational oracle

Pre-committed evidence for [Issue #126](https://github.com/nkiyohara/eqiora/issues/126),
frozen before any `SparseLu` implementation exists.

## Authoring boundary

This oracle and its fixture were written by an agent that does not implement the
slice. While authoring them the agent read only `AGENTS.md`, Issue #126, and the
existing files in this case directory. It read no Rust source, ran no Rust, and
saw no candidate implementation. Nothing outside
`verify/numerics/linear-backends/**` was changed.

The implementing agent may wire this fixture into Rust evidence. It may not
change a value, a falsifier, a residual, a tolerance, or the digest below. If it
believes a recorded value is wrong, the repository rule is to stop and return the
proof rather than adjust either side to agree.

The [acceptance threshold](#acceptance-threshold) was added in a second commit by
the same non-implementing author, after the implementing agent stopped: the first
freeze carried no tolerance, and the implementer may not author the one its own
work is judged against. That amendment was written under the same boundary — no
Rust read, none run, no candidate implementation seen, no measured residual
consulted — and it changed no matrix, right-hand side, solution, falsifier, or
residual already frozen.

## Files

| Path | sha256 |
| --- | --- |
| `expected/sparse-lu-contract.json` | `2555229a72984bf922655dad70b8050d70a7271af6bfd85a81ca9a3be4e8bcec` |

The digest freezes the fixture, not the checker. `oracle/sparse_lu_oracle.py`
carries no digest of itself; it recomputes the fixture digest on every run and
enforces it only when `--expect-digest` is supplied.

## Mathematical facts versus contract expectations

The fixture separates these deliberately and the distinction is load-bearing.

- `mathematics` holds statements about two explicit matrices that are true
  independently of Eqiora. The oracle re-derives every one of them from the
  stored CSR arrays in exact rational arithmetic and fails on disagreement.
- `contract_expectations` holds statements about the Eqiora and Faer slice,
  transcribed from the frozen design in Issue #126. They carry
  `"proved_by_this_oracle": false`. The oracle checks only their internal
  consistency and shape — tuple axes, disjointness of the positive and negative
  sets, tag stability, kebab-case encoding. Whether Eqiora honours them is
  established by Rust evidence this author did not write.

Enum spellings in `contract_expectations` are those given in Issue #126 and in
the authoring brief. This author did not read the Rust enums, so the fixture
fixes the tuple *shape* and the tag and encoding *values*; it is not evidence
about Rust identifier spelling.

## Principal witness

Square canonical CSR, `n = 5`, `nnz = 14`, zero-based sorted unique column
indices, no structurally empty row, full diagonal, `det A = 64`.

```text
      [ 5  -4   0  -3   0]        b = [16, -1, 7, -9, 1]
      [-1   3   2   0   0]        x = [1, -2, 3, -1, 2]           A x   = b
  A = [ 0  -1   1   0   1]        y = [9/2, 5, -2, 1/2, 3/2]      A^T y = b
      [ 0   0  -2   3   0]
      [-1   0   0   2   2]
```

`A` is neither structurally nor numerically symmetric. The pattern has five
entries with no transpose partner — `(0,3)`, `(2,4)`, `(3,2)`, `(4,0)`, `(4,3)`
— and among the pairs that are structurally symmetric the values still differ,
for instance `A[0][1] = -4` against `A[1][0] = -1`.

Every component of `x` differs from the corresponding component of `y`; the
componentwise difference is `[-7/2, -7, 5, -3/2, 1/2]`. A storage-orientation
error is therefore visible in every component rather than in a subset, and
cannot be masked by a coincidence in one entry.

`A`, `b`, `x` and `y` are all dyadic, so each is exactly representable in
binary64 and a Rust consumer can compare against them without a tolerance. This
is why the determinant was chosen to be a power of two.

Both `det A != 0` and an independent Gauss–Jordan solve are checked, so `x` and
`y` are *the* solutions rather than merely solutions. The determinant is
computed by Bareiss fraction-free elimination, a different route from the solver
used to confirm uniqueness.

### Initial guesses

`already_satisfied` is exactly `x`, so its true residual is exactly zero and it
is accepted under any positive tolerance without factorization. `not_satisfied`
is the zero vector, whose residual is `b` with squared norm `388`.

### Falsifiers

Each is active: the oracle constructs the wrong route and checks that the
resulting true residual is exactly the recorded value and strictly positive.

| Id | Detects | Squared residual |
| --- | --- | --- |
| `csr-read-as-csc` | CSR arrays consumed as column-major storage | `650` |
| `transpose-route-returns-normal-solution` | transpose route returning `x` | `386` |
| `rhs-permuted` | right-hand side reordered against the rows | `934` |
| `one-based-column-indices` | one-based indices in either direction | `1455` |
| `omitted-off-diagonal` | the dropped `(0,3)` entry, value `-3` | `9` |
| `wrong-solution` | any returned vector that is not `x` | `27` |

Three of these deserve a note.

`csr-read-as-csc` is not an assertion about a residual alone. The oracle rebuilds
a matrix by reading `row_ptr`, `col_idx` and `values` as column-major arrays and
checks that the result equals `A^T` elementwise, which is the actual mechanism of
the confusion, then checks that this misread system's unique solution is `y`.

`one-based-column-indices` bites in both directions because the pattern contains
both column `0` and column `n - 1`: reading the stored zero-based indices as
one-based underflows on three entries, and reading one-based indices as
zero-based overflows on two. It also covers the lenient consumer that discards
the out-of-range entries — that decode leaves every row structurally nonempty,
so it looks plausible, and only arithmetic exposes it. Its matrix is singular and
its residual against `x` is `1455`.

`omitted-off-diagonal` drops the entry that has no transpose partner. The reduced
matrix stays nonsingular, determinant `58`, so a consumer that omits the entry
returns a well-formed but wrong vector that differs from `x` in every component,
rather than failing visibly.

## Rank-deficient witness

Square canonical CSR, `n = 3`, `nnz = 7`, no structurally empty row, singular.

```text
       [2  5/2  0]        b = [1, 1, 5]
  A' = [1   0   3]        row 2 = row 0 + row 1 exactly
       [3  5/2  3]
```

The proof of rank deficiency is threefold and each part is re-derived: the
determinant is exactly zero; the `2x2` minor on rows and columns `{0,1}` has
determinant `-5/2`, forcing rank at least two, so the rank is exactly two; and
the nonzero right null vector `[-15, 12, 5]` and left null vector `[1, 1, -1]`
are each annihilated exactly.

The right-hand side is inconsistent. Because the rank is two, the left null space
is one-dimensional and spanned by `w = [1, 1, -1]`, so for every vector `z`

```text
  min ||b - A' z||_2^2 = (w . b)^2 / ||w||_2^2 = 9 / 3 = 3
```

No vector attains a zero residual. This lower bound is exact and rational, so it
is the fixture's statement about failing closed: any acceptance tolerance below
`sqrt(3)` must reject whatever a solver returns for this system. The threshold
chosen below is far below it.

## Acceptance threshold

The first freeze deliberately carried no tolerance, on the grounds that a
consumer should derive its own from the exact squared residuals. That left the
implementing agent unable to proceed: it may wire a pre-committed oracle but may
not author the tolerance its own implementation is judged against. This section
is the amendment. The threshold is chosen here, by the same non-implementing
author, against the exact witnesses and against nothing else — no implementation
was read, run, or consulted, and no candidate residual was measured.

### The choice

```text
  accept xhat  when  ||b - A xhat||_2 <= atol + rtol * ||b||_2
  rtol = 0 exactly            atol = 2^-30            atol^2 = 2^-60
```

`rtol` is exactly zero because `||b||_2 = sqrt(388)` is irrational: any nonzero
relative term makes the threshold irrational and no inequality below stays
decidable in exact arithmetic. With it removed the threshold is a rational, and
squaring it keeps every comparison against a recorded squared residual rational
too, so no consumer ever needs a square root.

`atol` is fixed by one rule stated before any bound is consulted — **the largest
power of two strictly below `1e-9`**, since `2^-30 < 1e-9 <= 2^-29`. The rule
picks the value; the two bounds below are then checked against it rather than
used to choose it, which is what keeps the threshold from being tuned to either
an implementation or a falsifier. Being a power of two it is dyadic, so a
consumer compares against exactly the number proved about here.

### The two walls

| | Bound | Value | Relation to `atol` |
| --- | --- | --- | --- |
| below | backward-error envelope `R` | `3375/2^47`, about `2.4e-11` | `atol / R = 131072/3375`, above `38` |
| above | least wrong route, squared | `9` | `9 / atol^2 = 9 * 2^60`, above `1e19` |

`R = 3 n^3 rho u ||A||_inf ||x||_inf` follows the standard backward-error
analysis of LU with partial pivoting (Higham, *Accuracy and Stability of
Numerical Algorithms*, 2nd ed., Thm 9.4): `(A + dA) xhat = b` with
`|dA| <= gamma_{3n} |L||U|`, bounding `|L| <= 1` and `max |U| <= rho max |A|`.
Every factor is deliberately worst case — `rho` is the worst-case partial-pivoting
growth `2^(n-1) = 16` where this matrix grows by about one, and `n^2` crudely
bounds `|| |L||U| |xhat| ||_inf`. `R` bounds what rounding *can* produce; it does
not predict what any implementation *does* produce, which for this system is
nearer `1e-15`. A threshold `38` times above `R` therefore cannot fail a
backward-stable binary64 factorization on rounding alone.

The upper wall is the smallest squared residual over all six falsifiers and the
rejected initial guess, which is the `9` of `omitted-off-diagonal`. The oracle
proves the strict inequality for each route separately rather than leaning on
the minimum, and proves a margin factor of `1e18` for each. The threshold is
thus not merely sufficient against the frozen routes: it sits eighteen orders
below the weakest of them, so it also catches errors far subtler than any
falsifier while still clearing rounding by a factor of `38`.

### Componentwise solution error

Residual acceptance and solution accuracy are different statements, so the
componentwise ceiling is derived rather than assumed. From
`xhat - x = A^-1 (A xhat - b)` and `||v||_inf <= ||v||_2`, any accepted `xhat`
satisfies

```text
  max_i |xhat_i - x_i| <= ||A^-1||_inf * atol = (147/64) * 2^-30 = 147 * 2^-36
```

`A` is nonsingular with `det A = 64`, so `A^-1` is exact and rational and
`||A^-1||_inf = 147/64`. The recorded ceiling is `2^-28`, the tightest power of
two the residual tolerance provably implies: `2^-28` covers `147 * 2^-36` and
`2^-29` does not. A consumer may assert it componentwise against the exact
solution without introducing a second tolerance.

### Where the numbers live

The separation the fixture already drew is preserved and the amendment respects
it in both directions.

- `mathematics.principal.acceptance` and `mathematics.rank_deficient.acceptance`
  hold the threshold and every inequality about it. These are statements about
  two explicit matrices and one chosen rational, true independently of Eqiora,
  and the oracle re-derives `||A||_inf`, `||x||_inf`, `A^-1`, `||A^-1||_inf`, the
  envelope and the least wrong route from the stored CSR arrays before comparing.
- `contract_expectations.test_plan` holds the plan an implementation is expected
  to declare — `SparseLu`, `General`, `Identity`, `Fast`, `F64`,
  `maximum_iterations = 1`, `rtol = 0`, `atol = 2^-30` — over four cases: the
  principal positive solve, the initial guess that is accepted early, the
  initial guess that must not be, and the singular system that must fail closed.
  It inherits `"proved_by_this_oracle": false`. The oracle checks its shape, its
  internal consistency, and that every tolerance, squared target and ceiling it
  declares is *exactly* the rational proved about under `mathematics`, so no
  number can be wired that was never analysed and no analysed number can be
  silently replaced. Nothing else about it is established here.

## Running it

```bash
python3 verify/numerics/linear-backends/oracle/sparse_lu_oracle.py --summary
```

Python standard library only, no arguments required, deterministic output,
exit status `1` on any failure. `--verbose` lists every check; `--fixture`
selects another fixture; `--expect-digest` enforces the digest above. At this
revision the run reports **224 passing checks, 35 of them exact rational
inequalities**, or 225 checks when the digest is enforced. The previous freeze
reported 144 and 145.

The inequality count covers every ordering comparison between exact rational
magnitudes. Each goes through one helper that rejects an inexact operand outright
rather than reporting it as a disagreement, so a count of 35 means 35 comparisons
decided in exact arithmetic and none decided in floating point. Comparisons that
are not between magnitudes — a rank below a dimension, a nonzero determinant, a
non-negative tag — are checks but are deliberately outside the count.

Every value the fixture records is recomputed rather than trusted, so a
corrupted fixture fails rather than silently redefining what is expected. Any
corruption is reported as a named check failure or an explicit abort reason,
never as a traceback.

### What the amendment was mutation-checked against

76 single-field mutations of the fixture were run through the oracle. All 76 were
detected. They drove **79 of the 80 newly added checks, and all 35 inequalities,
to a named failure** — including every plan field (`maximum_iterations -> 2`,
`Identity -> Jacobi`, `Fast -> Reproducible`, `F64 -> F32`, `General -> SPD`,
`rtol -> 2^-30`), every tolerance decoupling between `test_plan` and
`mathematics`, both directions of the forward-error ceiling (`2^-29` fails to
cover the implied bound, `2^-27` is no longer tightest), a threshold loosened
until it accepts the weakest falsifier, one tightened below the rounding
envelope, and the early-exit branch flag moved to the wrong case. The pre-existing
falsifier evidence was mutated too and still fails, so nothing here weakened it.

The single check no fixture mutation can reach is
`acceptance.inverse-is-a-genuine-inverse`: `A^-1` is derived from the matrix, so
mutating the matrix moves both sides together. It is an oracle self-check rather
than a fixture assertion, and it was confirmed load-bearing by fault-injecting a
wrong `exact_inverse` into the oracle, which fails it.

## Nonclaims

This evidence is mathematics and a transcribed contract. It is not evidence that
any of the following is true, and none of it may be cited as such:

- that Eqiora or the Faer adapter implements `SparseLu`, or implements it
  correctly — no Rust was read, written, or run;
- that a `SparseLu` capability, tag, artifact encoding, or error code exists in
  the tree, or is spelled as transcribed here;
- that Eqiora's acceptance predicate has the form assumed here, that it exposes a
  relative tolerance, an absolute tolerance, an iteration cap or an initial guess
  under any spelling, or that `maximum_iterations = 1` describes anything Eqiora
  currently does. The plan is a transcribed expectation; only its binding to the
  analysed rationals is checked;
- that any implementation attains the threshold. The backward-error envelope is
  an upper bound on what binary64 rounding can produce, derived symbolically. No
  residual was measured, on this or any machine, and the envelope is not a
  timing, profile, or environment claim;
- that `2^-30` is the only defensible threshold. It is one choice, fixed by a
  stated rule and shown to clear both walls; the walls are the evidence, the
  choice is not;
- anything about performance, timing, memory, fill-in, ordering, pivoting
  strategy, conditioning, or scale;
- anything about transposed solving as an Eqiora capability. The transpose
  solution `y` exists here only as a discriminator that makes storage-orientation
  confusion observable; Issue #126 lists transposed problems among its nonclaims;
- anything about threading, distribution, MPI, CUDA, reproducible reduction,
  Jacobi preconditioning, non-`f64` scalars, or cross-build bitwise
  reproducibility;
- singularity *diagnosis*. The rank-deficient witness supports failing closed on
  one specific inconsistent singular system, not a general detector.
