# Exact circular-hole steady Stokes 2D — dual independent oracle

Two independently derived numerical oracles for steady coherent-SI Stokes flow
on the exact circular-hole geometry, packaged together with the one immutable
shared mesh they both had to satisfy, and with the comparison that decides
whether they agree.

**There is no production implementation of this capability.** None was written,
read or executed at any point in this work. This directory is a
pre-implementation oracle: it exists so that implementation can begin against
values nobody implementing them chose.

```text
mesh/        the one immutable source-bound copy of the chordal reference mesh,
             and its independent checker
routes/python/   route A — closed-form barycentric cell blocks, exact bubble
             static condensation, dense LU at 40 decimal digits
routes/julia/    route B — explicit 3x3 Gauss-Legendre Duffy quadrature, no
             condensation, dense LU at 256-bit BigFloat, mesh independently
             reconstructed rather than consumed
agreement/   the dual independent oracle gate, and the packaging-fidelity proof
```

## Result

| step | command | result |
| --- | --- | --- |
| shared mesh | `python3 mesh/check_mesh.py` | **162 passed, 0 failed** |
| route A | `python3 routes/python/oracle.py --check` | **101 passed, 0 failed** |
| route B | `julia routes/julia/run.jl` | **103 passed, 0 failed** |
| gate | `python3 agreement/compare_routes.py` | **291 passed, 0 failed — PASS** |

The two routes agree on every frozen observation, with the smallest margin
`2.88e+03` times inside its tolerance. See
[`agreement/README.md`](agreement/README.md) for the full comparison and
[`agreement/expected/agreement-report.json`](agreement/expected/agreement-report.json)
for the frozen record.

**Agreement authorizes implementation against the frozen contract. It does not
verify production.** Nothing here has been run against Eqiora, and this case is
not registered: it carries no `case.toml`, so the repository gate does not
select it. Registration, the capability matrix and the roadmap are the
integrator's, not this packaging step's.

## Provenance

Both routes were authored by separate non-implementing sessions that never
shared context, never read each other's directory, and never read a production
implementation or an existing `verify/fluid` case.

| | route A (Python) | route B (Julia) |
| --- | --- | --- |
| source branch | `agent/cylinder-stokes-oracle-python` | `agent/cylinder-stokes-oracle-julia` |
| final source commit | `e8bbd44b49315f0d4ee723ec73df53a4f8f6f2f0` | `dccbc318744c7cec4f6b73f5ba0fe60880af7583` |
| that worktree's contract commit | `cd8faa0` | `f5fd3ce` |
| mesh | consumed the shared copy under `mesh/`, revalidated inside the route | reconstructed from the public rule; consumed no mesh file |
| assembly | closed form, no quadrature anywhere | explicit positive `3x3` Gauss-Legendre Duffy loop per cell |
| bubble | eliminated by exact static condensation | kept as an unknown, never eliminated |
| solve | dense LU at 40 decimal digits, cross-checked by an independent LU on the uncondensed system | dense LU at 256-bit BigFloat, residual reapplied cell by cell |
| checks reported at source | 101 route + 162 mesh | 103 |

The packaging base is `dea1fd138fce92fd0127f5df9155b675159a58c3`. RFC 0081 and
RFC 0082 — the entire public contract both routes derived against — are
**byte-identical** at `dea1fd1`, `cd8faa0` and `f5fd3ce`; the three commits
differ only in agent-routing documentation and in each worktree's own route
files. Neither route author read the other's contract revision, and neither
derived against different contract text.

The two routes cross-check each other's mesh without sharing an index
convention: route B publishes an index-free geometric digest
`573f2c9260b2976853c84bc96a4301bc39e52578209ea84bbf679ad3d77ad871` over
lexicographically ordered coordinates, and the gate compares counts, the named
partition, the quad diagonal and the ordered cell pair on both sides.

## What packaging changed, and the proof that it changed nothing else

Public-release hygiene rejects repository-numbered tracking prose — a bare
tracker word followed by a number, and the bare hash-number references that go
with it. Both source routes carried such prose throughout their comments,
documentation and frozen JSON strings. Packaging replaced all of it with stable
contract and RFC wording, then reran every deterministic generator so each
self-hash describes the packaged source. That rerun necessarily changes the
frozen files' bytes and digests.

| file | source digest | packaged digest |
| --- | --- | --- |
| `mesh/mesh.json` | `2ec74b9f481a60b460c9bb8096821cd73eeb7e17ef18a7ae67828e605d17a8f2` | `ada2d08cde5b4e6bd13c97d3b76a45cad810d8eb7acf0f0edc82cd605acd2b39` |
| `mesh/falsifier-wrong-diagonal.json` | `1574239edbfddb510d144271d58beea9195c6b4a222c73f3c153ab54c9162dea` | `eccb5642eab811cee1cad0cee8749f7f2a64d16ab300b041fa4efcbe7b61cd2f` |
| `routes/python/result.json` | `4037d358e613a016e21e04c9ff8fffa2475f056fb55a2f88c4c5828d957abfd7` | `f3c6579fb45879bb1861e203b9da470be42cfdd56f9dfe02b484b98c814bf685` |
| `routes/julia/expected/julia-route-frozen.json` | `2ad9a041c75906055b3a4ae3a2f2e05f3964cb5e62276a21e19f354586254dff` | `069becd79acb16cd358d2511a2ce2e407e1bdc68af2628fabd2734f4d03b8459` |

The three source digests the two authors reported were recomputed here rather
than trusted, and all three matched exactly.

`agreement/check_packaging_fidelity.py` is the argument that nothing but prose
moved. It walks each source document and its packaged counterpart in parallel
and requires identical key sets **in identical order**, identical array lengths,
identical leaf types, and every non-string leaf bit-identical — floats compared
by `repr` and by sign of zero, with non-finite values rejected outright. Run
against all four files with the source worktrees still present:

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

## What was verified here, and in which environment

Every command below ran in the foreground to completion. Environment: Linux
`6.11.0-1014-lowlatency` x86-64, CPython `3.12.3`, mpmath `1.3.0`, Julia
`1.12.6`, ruff `0.15.13`. These numbers describe this machine and predict
nothing about hosted CI.

| command | result | wall clock |
| --- | --- | ---: |
| `python3 mesh/build_mesh.py` | regenerated both mesh files | `0.07 s` |
| `python3 mesh/check_mesh.py` | 162 passed, 0 failed | `0.10 s` |
| `python3 routes/python/oracle.py` | 101 passed, 0 failed | `55.0 s` |
| `python3 routes/python/oracle.py --check` | 101 passed, 0 failed; `result.json` reproduced byte for byte | `54.8 s` |
| `julia routes/julia/run.jl` | 103 passed, 0 failed, exit 0; rerun byte-identical | `40.8 s`, `39.9 s` |
| `python3 agreement/compare_routes.py` | 291 passed, 0 failed — PASS | `< 1 s` |
| `python3 agreement/compare_routes.py --check` | report reproduced byte for byte under three `PYTHONHASHSEED` values | `< 1 s` |
| `python3 agreement/check_packaging_fidelity.py` | PASS, prose-only | `< 1 s` |
| `ruff check` and `ruff format --check` on the packaged Python | `All checks passed`, `8 files already formatted` | `< 1 s` |
| `python3 tools/ci/check_public_release_tree.py .` | no repository-local leaks | `< 1 s` |
| `python3 tools/ci/check_docs.py .` | links and entry points valid | `< 1 s` |
| `cargo fmt --all -- --check` | clean, exit 0 | `3.7 s` |

### The repository gate does not select only available work here

`python3 tools/ci/local_verify.py fast --plan` derives the case id
`fluid.exact-circular-hole-stokes-2d` from this directory path and plans three
commands: `cargo fmt --all -- --check`, `cargo run -p eqiora-verify -- run --case
fluid.exact-circular-hole-stokes-2d`, and `tools/ci/check_docs.py .`. The first
and third were run and pass. **The second cannot run**: there is no `case.toml`
here, because this is a pre-implementation oracle rather than accepted evidence.
Adding one so a gate would select an unimplemented capability would misstate
what exists, so it was not added, and no claim of a passing repository gate is
made.

The gate was additionally shown to be decisive rather than decorative:
twenty-one mutations of the frozen inputs — values pushed past tolerance in each
of the four families, a missing probe, an extra probe, reordered probes, a
relabelled probe, a relabelled route, a broken reaction negation, two renamed
unit keys, a non-finite value, an introduced gauge row, a changed pressure
reference, a changed facet count, reordered tie candidates, a changed DOF count,
a changed scale, a broken boundary partition and a changed quad diagonal — were
each rejected with a non-zero exit, on throwaway copies outside the repository.

## Not verified here

1. **No production implementation.** None exists, none was read, none was run.
   Agreement authorizes implementation; it is not implementation evidence.
2. **The repository gate was not run for this case.** With no `case.toml` there
   is nothing for `local_verify.py` to select, and adding one to make a gate
   select an unimplemented capability would misrepresent it. Registration is the
   integrator's step.
3. **Neither route's falsifier set was re-derived here.** Each route's own
   falsifiers were rerun as part of rerunning that route, and both routes report
   them all detected; this packaging step did not author or re-derive any of
   them.
4. **Cross-platform byte identity of the mesh is not claimed**, and RFC 0082
   does not claim it either: the transverse coordinates come from the platform
   `libm`, so a production inventory comparison must be tolerance-based rather
   than bitwise.
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
(identity preconditioner, rtol `1e-11`, atol `1e-13`, cap 10000) on this
witness:

- MINRES reaches the recurred target only at iteration `9401` of `10000`;
- its **true** residual floors at `2.4680e-5` — with the stopping test disabled
  the best true residual over 20000 iterations is `2.467954e-5` at iteration
  6100, while the recurred estimate keeps collapsing, which is total loss of
  Lanczos orthogonality;
- the **pointwise** production tolerances are missed: pressure by `256x`
  (`9.37e-8 Pa` against `3.66e-10 Pa`) and reaction by `160x` (`2.40e-8 N/m`
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
