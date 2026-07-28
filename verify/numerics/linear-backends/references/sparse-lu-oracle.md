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
change a value, a falsifier, a residual, or the digest below. If it believes a
recorded value is wrong, the repository rule is to stop and return the proof
rather than adjust either side to agree.

## Files

| Path | sha256 |
| --- | --- |
| `expected/sparse-lu-contract.json` | `f0968c90ca3bee7eeba6961734099f9b7df62ee43cd0839a118b20c52cb65930` |

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
`sqrt(3)` must reject whatever a solver returns for this system.

## Running it

```bash
python3 verify/numerics/linear-backends/oracle/sparse_lu_oracle.py --summary
```

Python standard library only, no arguments required, deterministic output,
exit status `1` on any failure. `--verbose` lists every check; `--fixture`
selects another fixture; `--expect-digest` enforces the digest above. At the
frozen revision the run reports 144 passing checks, or 145 when the digest is
enforced.

Every value the fixture records is recomputed rather than trusted, so a
corrupted fixture fails rather than silently redefining what is expected. Any
corruption is reported as a named check failure or an explicit abort reason,
never as a traceback.

## Nonclaims

This evidence is mathematics and a transcribed contract. It is not evidence that
any of the following is true, and none of it may be cited as such:

- that Eqiora or the Faer adapter implements `SparseLu`, or implements it
  correctly — no Rust was read, written, or run;
- that a `SparseLu` capability, tag, artifact encoding, or error code exists in
  the tree, or is spelled as transcribed here;
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
