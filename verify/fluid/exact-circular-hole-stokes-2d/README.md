# Exact circular-hole steady Stokes 2D — dual independent oracle

Two independently derived numerical oracles for steady coherent-SI Stokes flow
on the exact circular-hole geometry, packaged together with the one immutable
shared mesh they both had to satisfy, and with the comparison that decides
whether they agree.

This directory began as a pre-implementation oracle: its numerical values were
frozen before production code was written or read. It now also owns the
registered production witness, whose ordinary `eqiora` integration test reads
the frozen mesh and route-A observations directly and checks the complete
Model-to-reaction path against them.

```text
mesh/        the one immutable source-bound copy of the chordal reference mesh,
             and its independent checker
routes/python/   route A — closed-form barycentric cell blocks, exact bubble
             static condensation, dense LU at 40 decimal digits
routes/julia/    route B — explicit 3x3 Gauss-Legendre Duffy quadrature, no
             condensation, dense LU at 256-bit BigFloat, mesh independently
             reconstructed rather than consumed
agreement/   the dual independent oracle gate, and the packaging-fidelity utility
amendment/   the one amendment to the frozen witness tuple, its measurements,
             its falsifiers and the proof that nothing physical moved
```

## Result

| step | command | result |
| --- | --- | --- |
| shared mesh | `python3 mesh/check_mesh.py` | **162 passed, 0 failed** |
| route A | `python3 routes/python/oracle.py --check` | **101 passed, 0 failed** |
| route B | `julia routes/julia/run.jl` | **103 passed, 0 failed** |
| gate | `python3 agreement/compare_routes.py` | **275 passed, 0 failed — PASS** |
| amendment | `python3 amendment/adjudicate.py` | **24 passed, 0 failed — PASS** |
| invariance | `python3 amendment/check_physical_invariance.py` | **35 passed, 0 failed** |
| production | `cargo run -p eqiora-verify -- run --case fluid.exact-circular-hole-stokes-2d` | registered Eqiora execution and falsifiers |

The two routes agree on every frozen physical observation, with the smallest
margin `2.88e+03` times inside its tolerance, and on every geometric selector
exactly — selectors are reconstructed from the shared mesh in exact rational
arithmetic and carry no tolerance. See
[`agreement/README.md`](agreement/README.md) for the full comparison and
[`agreement/expected/agreement-report.json`](agreement/expected/agreement-report.json)
for the frozen record.

**Agreement authorizes implementation against the frozen contract; it does not
verify production by itself.** Production evidence is the separate registered
Cargo test named by `case.toml`. That test reconstructs the ordinary exact
owner, compares the complete mesh inventory, executes the admitted Faer
SparseLU tuple, and applies the physical observations without changing them.

## Provenance

Both routes were authored by separate non-implementing sessions that never
shared context, never read each other's directory, and never read a production
implementation or an existing `verify/fluid` case.

| | route A (Python) | route B (Julia) |
| --- | --- | --- |
| mesh | consumed the shared copy under `mesh/`, revalidated inside the route | reconstructed from the public rule; consumed no mesh file |
| assembly | closed form, no quadrature anywhere | explicit positive `3x3` Gauss-Legendre Duffy loop per cell |
| bubble | eliminated by exact static condensation | kept as an unknown, never eliminated |
| solve | dense LU at 40 decimal digits, cross-checked by an independent LU on the uncondensed system | dense LU at 256-bit BigFloat, residual reapplied cell by cell |
| checks reported at source | 101 route + 162 mesh | 103 |

### What is durable, and what is only a label

This package was assembled on **merged main `84b78c5`** ("Specify the circular
reference quad diagonal"), which is durable and reachable from `origin/main`.
It is recorded for orientation only: nothing here reads or resolves it.

The other commit identifiers in this directory are not durable, for two
different reasons.

- **The two source worktrees were throwaway, and their branches were never
  pushed.** `e8bbd44` and `dccbc31` are the two final route commits, and
  `cd8faa0` and `f5fd3ce` the contract revisions those worktrees stood on.
  These are *informative local-session labels* and may resolve to nothing in
  any other clone.
- **`dea1fd1` is neither a source-session label nor the packaging base.** It
  was the exact review head of the separate RFC 0082 quad-diagonal
  clarification branch — a branch that was pushed for review and deleted after
  it squash-merged, so `dea1fd1` may equally fail to resolve elsewhere. Its
  durable form is `84b78c5`, whose tree is identical to it. Route B's own
  document repeats the earlier "packaging base" wording for `dea1fd1`; that
  document is left as its author wrote it, and this section is the correction.

Nothing in this package, in the agreement gate, or in any default repository
check reads any of these identifiers or needs them to exist.

What a future reader reproduces from instead:

- **the packaged bytes**, and their sha256 values, which the gate recomputes on
  every run and records in
  [`agreement/expected/agreement-report.json`](agreement/expected/agreement-report.json);
- **the contract text and digests embedded in the packaged documents**,
  including the shared mesh's own source digest;
- **rerunning each route in place** — `routes/python/oracle.py --check`
  reproduces `result.json` byte for byte, and `routes/julia/run.jl` reruns
  byte-identical;
- **rerunning the gate**, which regenerates its frozen report byte for byte.

None of that depends on a temporary worktree or on resolving a git object.

RFC 0081 and RFC 0082 — the entire public contract both routes derived against —
were confirmed **byte-identical** at both worktree contract revisions and at
`dea1fd1` when the package was assembled, and they carry the same bytes at the
merged base `84b78c5`; those revisions differed only in agent-routing
documentation and in each worktree's own working files under this directory.
Neither route author read the other's contract revision, and neither derived
against different contract text. The durable form of that statement is the
contract text carried inside the packaged documents, not the labels.

The two routes cross-check each other's mesh without sharing an index
convention: the gate reconstructs every cell barycentre, both boundary vertex
sets and all eleven geometric selectors from `mesh/mesh.json` in exact rational
arithmetic, and compares counts, the named partition, the quad diagonal and the
ordered cell pair on both sides. Route B additionally publishes an index-free
geometric digest
`573f2c9260b2976853c84bc96a4301bc39e52578209ea84bbf679ad3d77ad871` over
lexicographically ordered coordinates of **its own** reconstructed mesh; see the
non-claim below for why that digest is not, and cannot be, reproduced from the
shared mesh file.

## What packaging changed, and the proof that it changed nothing else

Public-release hygiene rejects repository-numbered tracking prose — a bare
tracker word followed by a number, and the bare hash-number references that go
with it. Both source routes carried such prose throughout their comments,
documentation and frozen JSON strings. Packaging replaced all of it with stable
contract and RFC wording, then reran every deterministic generator so each
self-hash describes the packaged source. That rerun necessarily changes the
frozen files' bytes and digests.

The source column below is history: those files lived in the unpushed source
worktrees and are not part of this package. The packaged column is what this
directory contains and what every check here recomputes.

| file | source digest (historical) | packaged digest |
| --- | --- | --- |
| `mesh/mesh.json` | `2ec74b9f481a60b460c9bb8096821cd73eeb7e17ef18a7ae67828e605d17a8f2` | `ada2d08cde5b4e6bd13c97d3b76a45cad810d8eb7acf0f0edc82cd605acd2b39` |
| `mesh/falsifier-wrong-diagonal.json` | `1574239edbfddb510d144271d58beea9195c6b4a222c73f3c153ab54c9162dea` | `eccb5642eab811cee1cad0cee8749f7f2a64d16ab300b041fa4efcbe7b61cd2f` |
| `routes/python/result.json` | `4037d358e613a016e21e04c9ff8fffa2475f056fb55a2f88c4c5828d957abfd7` | `f3c6579fb45879bb1861e203b9da470be42cfdd56f9dfe02b484b98c814bf685` |
| `routes/julia/expected/julia-route-frozen.json` | `2ad9a041c75906055b3a4ae3a2f2e05f3964cb5e62276a21e19f354586254dff` | `069becd79acb16cd358d2511a2ce2e407e1bdc68af2628fabd2734f4d03b8459` |

The three source digests the two authors reported were recomputed here rather
than trusted, and all three matched exactly.

`agreement/check_packaging_fidelity.py`, in its **historical source-pair mode**,
is the argument that nothing but prose moved. It walks each source document and
its packaged counterpart in parallel and requires identical key sets **in
identical order**, identical array lengths, identical leaf types, and every
non-string leaf bit-identical — floats compared by `repr` and by sign of zero,
with non-finite values rejected outright. Run once, against all four files,
while the source worktrees still existed:

| file | numeric / boolean leaves, all bit-identical | strings | strings changed |
| --- | ---: | ---: | ---: |
| `mesh/mesh.json` | 1074 | 136 | 2 (`role_detail`, `purpose`) |
| `mesh/falsifier-wrong-diagonal.json` | 1074 | 136 | 1 (`role_detail`) |
| `routes/python/result.json` | 343 | 220 | 10 |
| `routes/julia/expected/julia-route-frozen.json` | 185 | 169 | 1 (`statement`) |

`PACKAGING FIDELITY: PASS`. The ten changed strings in `result.json` are eight
prose fields (`frozen_scope`, `dual_independent_oracle_gate.pending_route`,
`dual_independent_oracle_gate.statement`, `scales.Theta_note`,
`dimensions.note`, `limitations[0]`, `limitations[2]`, `limitations[5]`) plus
`mesh.accepted.sha256` and `mesh.wrong_contract_falsifier.sha256`, which are the
recorded digests of the two regenerated mesh files. Route A's pinned mesh
digests were updated to match; that pin is the only non-prose edit packaging
made to a script, and the table above is why it is safe.

Every algorithm, precommitted value, probe, tolerance, sign convention,
selector and numeric observation is unchanged. No JSON key was renamed —
including `Theta_issue_spelling_W_m`, which the hygiene checker does not flag
and whose renaming would have been a structural change to a frozen document.
Route B's 47 `hp` decimal strings, its 25-significant-digit record of every
frozen value, are all unchanged.

Each route's own frozen document still states that the dual independent oracle
gate had not passed. That was each route's true statement about itself, in
isolation, before this comparison existed. Packaging deliberately did **not**
rewrite it: the routes' self-descriptions are their authors', and the gate
result lives in `agreement/` where it can be read as the separate act it is.

That source-pair mode is history and is not needed again. The utility's
**default, no-argument mode is self-contained**: it recomputes the sha256 of
every document the frozen agreement report says it compared, requires each to
equal the digest recorded there, and rejects any non-finite numeric leaf in any
frozen JSON. It reads nothing outside this directory and resolves no git object.

## What was verified here, and in which environment

Every command below ran in the foreground to completion. Environment: Linux
`6.11.0-1014-lowlatency` x86-64, CPython `3.12.3`, mpmath `1.3.0`, Julia
`1.12.6`, ruff `0.15.13`. These numbers describe this machine and predict
nothing about hosted CI.

| command | result | wall clock |
| --- | --- | ---: |
| `python3 mesh/check_mesh.py` | 162 passed, 0 failed | `0.10 s` |
| `python3 routes/python/oracle.py --check` | 101 passed, 0 failed; `result.json` reproduced byte for byte | `57.5 s` |
| `julia routes/julia/run.jl` | 103 passed, 0 failed, exit 0; rerun three times, `julia-route-frozen.json` and `run-log.txt` byte-identical each time | `~41 s` each |
| `python3 agreement/compare_routes.py` | 275 passed, 0 failed — PASS | `< 1 s` |
| `python3 agreement/compare_routes.py --check` | report reproduced byte for byte under three `PYTHONHASHSEED` values | `< 1 s` |
| `python3 agreement/check_packaging_fidelity.py` | `PACKAGE INTEGRITY: PASS`, all five recorded digests match | `< 1 s` |
| `ruff check` and `ruff format --check` on the packaged Python | `All checks passed`, `8 files already formatted` | `< 1 s` |
| `python3 tools/ci/check_public_release_tree.py .` | no repository-local leaks | `< 1 s` |
| `python3 tools/ci/check_docs.py .` | links and entry points valid | `< 1 s` |
| `cargo fmt --all -- --check` | clean, exit 0 | `3.6 s` |
| `cargo xtask check-architecture` | file sizes, public surface, glob re-exports, dependency graph and RFC numbering all clean | `~10 s` |

The mesh files were not regenerated in this run: `mesh/build_mesh.py` is not an
input to the gate, and `check_mesh.py` revalidates the frozen bytes in place.
Both mesh digests are unchanged from the packaged values above.

### Gate state when the oracle was frozen

`python3 tools/ci/local_verify.py fast --plan` derives the case id
`fluid.exact-circular-hole-stokes-2d` from this directory path and plans three
commands: `cargo fmt --all -- --check`, `cargo run -p eqiora-verify -- run --case
fluid.exact-circular-hole-stokes-2d`, and `tools/ci/check_docs.py .`. The first
and third were run and pass. **The second cannot run**: it was run and exits
`1` with `unknown verification case ID`, because there was no `case.toml` at
that pre-implementation revision. That was the correct fail-closed result then.
The integrated slice now supplies the manifest and production target; current
gate results are recorded with the integrating change, not retroactively
attributed to this historical oracle run.

The gate was additionally shown to be decisive rather than decorative.
Twenty-eight mutations of the frozen inputs — values pushed past tolerance in
each of the four families, a missing probe, an extra probe, reordered probes, a
relabelled probe, a relabelled route, a broken reaction negation, two renamed
unit keys, a non-finite value, an introduced gauge row, a changed pressure
reference, a changed facet count, reordered tie candidates, a changed DOF count,
a changed scale, a broken boundary partition, a changed quad diagonal, a probe
vertex and a tie candidate each moved one ulp off the shared mesh, a barycentre
replaced by another cell's, and each route's true reduced residual pushed one
ulp past its own target-plus-allowance bound — were each rejected with a
non-zero exit. Eight further mutations appended a byte to either route document,
the shared mesh or the gate's own source, and each was rejected both by
`compare_routes.py --check` and by the packaging-fidelity package check. All
thirty-six ran on throwaway copies outside the repository.

The weak pressure-row residuals were mutated against the bound the gate applied
at the time, which borrowed each route's recorded *true-residual* allowance. A
later cross-provider review established that the frozen contract names the
existing *pressure-row* allowance for that residual instead, and that the
bounded test admitted negative norms and negative allowances. The gate now
applies `weak_norm <= target + 4096 * eps * (1 + weak_norm + target)`, the
existing no-gauge production formula, and requires every residual, target,
allowance and limit to be finite and nonnegative. **No route value, mesh value,
selector, tolerance family, solver selection or claim changed** — the frozen
weak norms `3.14e-40` and `1.62e-75` pass the corrected `2.23e-12` bound by more
than `27` orders of magnitude. Thirty-five residual and guard mutations and four
must-accept probes were run against the corrected gate; see
[`agreement/README.md`](agreement/README.md) for both the formulas and that
mutation set. The four route and mesh documents are byte-identical to the
packaged values above throughout.

## Not verified here

1. **The oracle routes do not verify production by themselves.** The registered
   Cargo target is the production evidence and must pass the repository gate.
2. **Hosted and non-local environments are not implied by a local run.** Record
   the exact completed local gate and any hosted result beside the integrating
   change.
3. **Neither route's falsifier set was re-derived here.** Each route's own
   falsifiers were rerun as part of rerunning that route, and both routes report
   them all detected; this packaging step did not author or re-derive any of
   them.
4. **Byte identity of the mesh across independent reconstructions is not
   claimed**, and RFC 0082 does not claim it either: the transverse coordinates
   come from the platform `libm`, so a production inventory comparison must be
   tolerance-based rather than bitwise. This was measured here rather than
   assumed. Rebuilding route B's canonical index-free digest text from
   `mesh/mesh.json` reproduces its structure exactly — the same 313 canonical
   lines, the same vertex, cell and facet ordering, the same boundary-side
   labels — but **13 of those lines differ**, because 2 of the 104 vertices
   differ between the shared mesh and route B's own reconstruction: an inlet
   `y` of `0.012187498836501692` against `0.01218749883650172` (`2.78e-17 m`,
   16 ulp) and `0.09004906956144595` against `0.09004906956144597`
   (`1.39e-17 m`, 1 ulp). So route B's digest
   `573f2c92…` **cannot** be reproduced from the shared mesh file, and no check
   here claims it can. Making one pass would require inventing a coordinate
   tolerance for a digest, which is exactly what this package refuses to do.
   Neither differing vertex is a probe selector, and both routes' probe
   barycentres still map to the same shared-mesh cells exactly.
5. **Route A's `reference_channel.py` is a diagnostic**, not part of the frozen
   route and not a claim. It was not rerun during packaging.
6. **Lineage, artifact digests and replay-validated authored-region
   correspondence** are production digest guards with no numerical content and
   are unreachable from standalone numerical routes. Neither route covers them.
7. The `n = 8/16/32/64` refinement family was **not** run as a convergence
   study; `n = 52` and `n = 64` appear only as falsifier fixtures.

## Coarse-mesh facts that bound every number in this directory

The reference topology is one ray-cast annulus, and that dominates the
magnitudes far more than the physics does:

- all **104** mesh vertices are boundary vertices; there are **no interior
  vertices at all**;
- **103** of them are essential — the closure of the inlet, wall and cylinder
  velocity facets;
- **the only free velocity vertex is the outlet midpoint `[2.2, 0.2] m`**;
- the MINI **bubble velocities remain cell-interior unknowns** on every cell,
  all 208 of them, because a bubble vanishes on its own cell boundary.

So the discrete velocity is almost entirely the prescribed P1 trace plus the
cell bubbles, and the pressure is whatever enforces weak incompressibility of
that nearly fixed field. The probe pressure reaches `20.61 Pa` against
`P = 7.317e-4 Pa`, and the cylinder reaction `4.617 N/m` against
`P L = 3e-4 N/m`.

**The reported cylinder vector is an algebraic constrained-vertex force on this
deliberately coarse mesh.** It is not drag, not a physically scaled force, not
mesh-independent, and not a drag or lift coefficient or a DFG / Schäfer–Turek
benchmark value. No PDE-convergence, accuracy, Navier–Stokes, Reynolds-number,
transient, vortex-shedding, curved-element, production-mesher, 3D or performance
claim is made anywhere in this directory.

## The measured advisory the integrator must weigh before implementing

Route B measured, and this packaging step preserves without conversion, that the
frozen solve selection may not be able to meet the frozen production tolerances
on this witness. Nothing was relaxed and nothing was reinterpreted.

Using a **Julia f64 Paige-Saunders MINRES analogue** of the frozen tuple
(identity preconditioner, rtol `1e-6` as amended, atol `1e-13`, cap 10000) on
this witness:

- MINRES reaches the recurred target at iteration `5832` of `10000`, leaving
  `41.7 %` cap headroom;
- its **true** residual floors at `2.4679e-5` — with the stopping test disabled
  the best true residual over 20000 iterations is `2.467954e-5` at iteration
  6100, while the recurred estimate keeps collapsing, which is total loss of
  Lanczos orthogonality. The floor is unchanged by the amendment: it is already
  reached before any of these stopping points;
- the **pointwise** production tolerances are missed: pressure by `911x`
  (`3.33e-7 Pa` against `3.66e-10 Pa`) and reaction by `127x` (`1.91e-8 N/m`
  against `1.50e-10 N/m`). Velocity and flux pass, and both global balances
  pass;
- a **dense f64 LU** — explicitly *not* the frozen selection — meets every
  tolerance with roughly a `10^4` margin, so the gap is attributable to the
  iterative selection on a `cond_2 = 4.64e+9` operator rather than to the
  tolerance table or the mesh.

This is **advisory and not production evidence**. It is not the registered
`eqiora.reference` backend, not its `Reproducible` reduction, and not a hosted
measurement. It indicates feasibility rather than settling it, and it does not
relax the frozen production contract. The decision it informs is the
integrator's.

## The one amendment to the frozen witness

The frozen tuple's relative tolerance was amended from `1e-11` to `1e-6`, and
with it the mechanically derived residual target, from
`1.3239627651209673e-12` to `1.3239627651209673e-07`. Nothing else changed:
`check_physical_invariance.py` digests each frozen document with the
amendment-allowlisted leaves removed and requires the result to equal a record
taken before the amendment, and both documents match.

The superseded value was returned because its verdict on the f64-rounded
elevated-precision solution **flips with the reapplication precision** — both
routes accept that vector in elevated precision and reject it under every
binary64 summation order measured. A target that cannot be evaluated to a
stable verdict is not an acceptance policy.

The residual gate is a solver-health and stopping threshold only. On this
witness it provably cannot be anything else: evaluation-decidability needs a
target above `2.33e-11` while implying the pressure tolerance needs one at or
below `3.12e-12`, and those ranges are disjoint by `7.5x`. Physical acceptance
rests solely on the dual-derived observations and balances, which are
unchanged. See [`amendment/README.md`](amendment/README.md) for the derivation,
the falsifiers and the full non-claims — including that this amendment does
**not** make the strict `true_residual <= target` predicate satisfiable.
