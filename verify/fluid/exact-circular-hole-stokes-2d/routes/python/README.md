# Python analytic oracle route

One of the two mutually independent non-implementing numerical-oracle routes
required by the frozen contract. It derives the affine MINI/P1 Stokes blocks in closed
form, assembles and solves the reduced mixed system at elevated precision, and
freezes every observation the contract names.

Written without reading any production implementation, any existing
`verify/fluid` case, or the Julia route. Inputs are only the frozen contract
text, RFCs 0043 / 0045 / 0047 / 0081 / 0082, and the accepted mesh copy in
[`../../mesh`](../../mesh/README.md).

## Status: one frozen route — the dual oracle gate has NOT passed

RFC 0082 now fixes the shared quad diagonal as `O_i--I_j`, with cells
`(O_i, O_j, I_j)` and `(O_i, I_j, I_i)`. That resolves the ambiguity the earlier
revision of this route returned on, so exactly one mesh is admissible and **every
observation below is frozen** for it under the precommitted contract
tolerances. No formula, expected physics, physical scale or tolerance was changed
after selection.

**This is one route.** The Julia route has not run. No route-to-route agreement
is claimed, measured or implied, and **the dual independent oracle
gate has not passed**. Nothing in this directory may be read as that gate's
result or as permission to begin implementation.

## What this mesh is, and what the cylinder vector is not

The RFC 0082 reference topology is one ray-cast annulus, so:

- **all 104 mesh vertices are boundary vertices**; there are **no interior
  vertices at all**;
- **103 of them are essential** (the closure of the inlet, wall and cylinder
  velocity facets);
- **the only free velocity vertex is the outlet midpoint `[2.2, 0.2] m`**;
- **the MINI bubble velocities remain cell-interior unknowns** — all 208 of them,
  none constrained by any boundary trace, because a bubble vanishes on its own
  cell boundary.

The discrete velocity is therefore almost entirely the prescribed P1 trace plus
the cell bubbles, and the pressure is whatever enforces weak incompressibility of
that nearly fixed field. Measured on this mesh, the probe pressure reaches
`20.61 Pa` against `P = 7.317e-4 Pa` (ratio `2.82e+4`) and the cylinder reaction
`4.617 N/m` against `P L = 3e-4 N/m` (ratio `1.54e+4`).

**The reported cylinder vector is therefore an algebraic constrained-vertex force
on this deliberately coarse mesh.** It is not drag, not a physically scaled
force, not mesh-independent, and not a drag or lift coefficient or a DFG /
Schäfer–Turek benchmark value. This matches the contract's own witness text and its
nonclaims. The assembly itself is sound — see the patch test and the
straight-channel reference check below.

## Equations

On the fluid domain, with `mu = 0.001 kg/(m s)` and zero body force (zero force
potential), find velocity `u` and pressure `p` such that for every admissible
`v` and every `q`

```text
  integral 2 mu sym(grad u) : sym(grad v)
- integral p div(v)                        = integral f . v + integral_outlet t . v
- integral q div(u)                        = 0
```

with the sign convention of RFC 0043: `c(v, p) = -integral p div(v)` and
`c(u, q) = -integral q div(u)`, so the block form

```text
[ K    C ] [u]   [b]
[ C^T  0 ] [p] = [0]
```

is symmetric. Boundary data, from the contract's frozen witness:

- `inlet` (x = 0): `trace(u) + normal(isotropic_lift(g)) = 0` with
  `g(y) = 4 Umax y (H - y) / H^2`, `H = 0.41 m`, `Umax = 0.3 m/s`. With the
  parent-outward normal `[-1, 0]` this prescribes `u = [g(y), 0]`.
- `outlet` (x = 2.2): constant parent-outward traction `[0, 0] Pa`, a natural
  condition. Its facet load is the degree-one midpoint rule
  `b_endpoint = length * traction / 2`, here identically zero, applied to the two
  endpoint P1 bases only — bubble, pressure and interior rows take no facet load.
- `walls` and `cylinder`: `trace(u) = [0, 0] m/s`.
- Essential vertices are the closure of the velocity facets. The two corners
  `[2.2, 0]` and `[2.2, 0.41]` are on both a wall facet and an outlet facet, so
  they stay fixed while their outlet facet still contributes its full-system
  traction action. The two corners `[0, 0]` and `[0, 0.41]` are on both an inlet
  and a wall facet; the route checks the two prescriptions agree, and they do
  exactly, because `g(0) = g(H) = 0`.

Pressure reference is `BoundaryTraction` (RFC 0047): the nonempty outlet traction
partition fixes the constant mode, so there is **no gauge row, no multiplier and
no `ZeroIntegral` constraint**. This is asserted structurally, not numerically.

## Closed-form cell blocks

Every integrand on an affine triangle is a barycentric monomial, so

```text
integral_T l0^a l1^b l2^c dA = 2 |T| a! b! c! / (a+b+c+2)!
```

evaluates all of them exactly. **There is no quadrature loop anywhere in this
route.** With `g_i = grad(lambda_i)` constant, `sum_i g_i = 0`, and the
normalized bubble `beta = 27 l0 l1 l2`:

| Block | Closed form |
| --- | --- |
| viscous, row `(l_i, e)` col `(l_j, d)` | `mu \|T\| ( delta_de g_j . g_i + g_j[e] g_i[d] )` |
| viscous, bubble/bubble `(e, d)` | `mu (81 \|T\| / 20) ( delta_de tr(M) + M[e][d] )`, `M[e][d] = sum_i g_i[e] g_i[d]` |
| viscous, P1/bubble | `0` |
| coupling, P1 `(l_j, d)` with `l_m` | `-(\|T\| / 3) g_j[d]` |
| coupling, bubble `(d)` with `l_m` | `+(9 \|T\| / 20) g_m[d]` |

Derivations. Expanding `sym(grad u) : sym(grad v)` for `u = phi e_d`,
`v = psi e_e` gives `(1/2)(delta_de grad(phi).grad(psi) + d_e phi d_d psi)`, so
`a(u,v) = mu integral (delta_de grad(phi).grad(psi) + d_e phi d_d psi)`. The
vector Laplacian keeps only the first term. The P1/bubble viscous coupling
vanishes because it is proportional to `sum_i g_i = 0`. For the bubble/bubble
term, `integral m_i m_j` is `|T|/90` when `i = j` and `|T|/180` otherwise, and
`sum_i g_i = 0` collapses it to `(27^2/180) |T| sum_i g_i[e] g_i[d]`. For the
bubble coupling, `integral l_m m_i` is `|T|/60` when `i = m` and `|T|/30`
otherwise, and `sum_{i != m} g_i = -g_m`.

These are exactly what the contract's degree-four `3 x 3` Gauss–Legendre Duffy rule
integrates: every integrand above has total degree at most four, so the frozen
quadrature is exact and agrees with these closed forms identically, not
approximately.

## Degrees of freedom

| Block | Layout | Count |
| --- | --- | ---: |
| velocity P1 | `2 * vertex + component` | 208 |
| velocity bubble | `208 + 2 * cell + component` | 208 |
| pressure P1 | `416 + vertex` | 104 |
| **full system** | momentum rows then continuity rows | **520** |
| prescribed | 103 essential vertices x 2 | 206 |
| **reduced system** | no gauge row | **314** |

## Solve

- **Primary** — exact static condensation of the bubble block. Bubble rows couple
  only to their own cell's bubble and pressure unknowns and carry no load, so
  `u_b = -Kb^-1 Cb p` exactly; `Kb = mu (81|T|/20)(tr(M) I + M)` is SPD and
  inverted in closed form. The condensed `106 x 106` system is solved by dense LU
  at 40 decimal digits, then the bubbles are reconstructed.
- **Cross-check** — an independent dense LU on the *uncondensed* `314 x 314`
  reduced system. The two routes agree to `4.6e-39`.
- Neither calls Eqiora. Both leave the reduced matrix exactly symmetric (checked
  entrywise, max asymmetry exactly `0`).

The contract's solver selection (`eqiora.reference`, MINRES, Identity, Reproducible,
`f64`, rtol `1e-11`, atol `1e-13`, `<= 10000` iterations) is recorded and used to
compute the acceptance target, but this route solves directly rather than
iterating: its business is the exact answer the production path must reach.

## Reaction sign

The reported `cylinder` value is the **constraint force on the fluid**, the
existing API convention. The **fluid force on the cylinder is its componentwise
negative** and any consumer must label it as such. Both are emitted, named, in
`result.json`. The route asserts the orientation: the fluid force on the cylinder
must act along `+x`, and it does.

## Frozen observations

All values are the binary64 spellings emitted in `result.json`; the internal
arithmetic runs at 40 decimal digits.

**MINI velocity at the barycentre of the selected cell** (at the barycentre all
`lambda_i = 1/3` and `beta = 1`, so the value is the mean of the three P1 vertex
velocities plus the bubble coefficient):

| Target `m` | Cell | `u_x` m/s | `u_y` m/s |
| --- | ---: | --- | --- |
| `[0.1, 0.2]` | 51 | `0.08801138571928349` | `-0.004377542327224645` |
| `[0.2, 0.3]` | 25 | `0.16901430152689484` | `-0.00902439063845748` |
| `[0.3, 0.2]` | 91 | `0.09623743076579622` | `0.11251827536745837` |
| `[1.0, 0.2]` | 1 | `0.1326922903707496` | `-0.0007663108263206535` |
| `[2.0, 0.2]` | 103 | `0.2367220892434223` | `0.05088790553945741` |

**P1 pressure:**

| Probe | Vertex | Position `m` | `p` Pa | Ties |
| --- | ---: | --- | --- | ---: |
| `cylinder_min_x` | 25 | `[0.15000000000000002, 0.2]` | `20.611897142913634` | 1 |
| `cylinder_max_x` | 0 | `[0.25, 0.2]` | `0.111521650853062` | 1 |
| `cylinder_min_y` | 37 | `[0.19686047402353435, 0.15009866357858642]` | `11.03786740720071` | 2 |
| `cylinder_max_y` | 13 | `[0.19686047402353435, 0.2499013364214136]` | `10.315730130178096` | 2 |
| `outer_nearest_x_low_mid` | 77 | `[0.0, 0.20000000000000004]` | `19.780390332641403` | 1 |
| `outer_nearest_x_high_mid` | 50 | `[2.2, 0.2]` | `-0.04836168726748482` | 1 |

**Signed fluxes**, each facet contributing `length * endpoint-average velocity .
parent-outward unit normal` with the normal taken from the adjacent fluid cell:

| Quantity | Value `m^2/s` |
| --- | --- |
| inlet | `-0.08149573099927537` |
| outlet | `+0.08149573099927537` |
| sum | `1.7219155529623352e-41` (limit `1e-8`) |
| continuous reference `-2 Umax H / 3` | `-0.08199999999999999` |

**Cylinder reaction and global balance**, `N/m`:

| Quantity | x | y |
| --- | --- | --- |
| constraint force **on the fluid** | `-4.617062540501679` | `0.03952008400301018` |
| fluid force **on the cylinder** | `4.617062540501679` | `-0.03952008400301018` |
| all-essential reaction + body force + traction | `-5.345112862320582e-41` | `1.140769053837547e-40` |

Integrated body force and integrated applied traction are both exactly `[0, 0]`.
The componentwise balance limit is `1e-10 N/m`.

**Residuals and pressure reference:**

| Quantity | Value |
| --- | --- |
| independently reapplied true reduced residual (dimensionless) | `8.193525269074018e-38` |
| weak pressure-row residual (dimensionless) | `3.1420923017657965e-40` |
| solver-selected target | `1.3239627651209673e-12` |
| roundoff allowance `4096 eps (1 + \|\|A\|\|_inf \|\|x\|\|_inf + \|\|b\|\|_inf)` | `6.469509595252338e-05` |
| pressure reference | `BoundaryTraction`, 2 traction facets |
| gauge row / multiplier / `ZeroIntegral` | none, none, none (exact structural assertions) |
| reduced system rows | 314 |

The two `cylinder_min_y` / `cylinder_max_y` probes are **exact two-way ties in
binary64**. The contract's lexicographic rule resolves them, and `result.json` emits
both tied candidates, because they carry materially different pressures:
`11.0379` vs `10.0257 Pa` for min y and `10.3157` vs `9.3407 Pa` for max y. The
rule must therefore be applied to the *stored* binary64 coordinates, never to
recomputed ones — a one-ulp difference in a reimplementation would silently
select the other vertex and shift the probe by about `1 Pa`.

## What is checked — 101 checks, 0 failures

Beyond the structural revalidation of both mesh files inside the route,
independent of `check_mesh.py`:

- **Patch test, the decisive one.** `u = (x + y, -(x + y))` is divergence free and
  lies exactly in P1, hence exactly in MINI; `sym(grad u) = diag(1, -1)`, so with
  `p = 2 mu` the exact traction on any `x = const` face is `(2 mu - p, 0) = 0`.
  The discrete solution must reproduce it exactly. It does, **on this very mesh**,
  to `1.5e-40` in velocity and `3.8e-40` in pressure, with bubbles vanishing to
  `5.2e-41` and the constant pressure fixed at `0.002 Pa` by the traction
  partition alone. This is an exact algebraic identity, not a convergence claim,
  and it fails if the viscous block, either coupling sign, the bubble block, the
  essential closure, the parent-outward normal or the natural-boundary handling
  is wrong. The vector-Laplacian variant fails it by `1.3e-3 Pa`, because its
  natural condition `mu du/dn - p n = 0` cannot be met by a field with a
  rotational part.
- **Congruence (RFC 0045).** Directly assembling the dimensionless operator on
  normalized coordinates `x_hat = (x - [0,0]) / L` with `mu_hat = mu U /(P L) = 1`
  reproduces `D A D / Theta` coefficientwise to `2.1e-40` relative.
- **Scales.** `P = mu U / L` and `G = U / L` match the contract's binary64 spellings
  exactly. `Theta = P U L = mu Umax^2` is checked within one ulp: the contract's
  `0.00009 W/m` is the exact-decimal reading, and *every* binary64 evaluation
  lands one ulp lower at `8.999999999999999e-05`. This changes nothing at `1e-10`.
- Flux balance, componentwise momentum balance, true and weak-continuity
  residuals under target plus allowance, exact reduced-matrix symmetry, the
  lexicographic resolution of every tied pressure probe, the claim-boundary
  topology facts above, and the structural pressure-reference assertions.

`result.json` carries a quantitative ledger: every bounded check records its
measured magnitude beside its limit, not just a boolean.

### Falsifiers

All eight detected, without changing any computed value:

| Falsifier | Category | Detected by | Margin |
| --- | --- | --- | ---: |
| vector-Laplacian viscosity | formulation | frozen probes; also fails the patch test | `2.6e+10 x` tol |
| pressure/velocity coupling sign reversed | formulation | frozen probes | `6.2e+10 x` tol |
| inlet/outlet membership swapped | boundary data | frozen flux and reaction oracle | `5.6e+10 x` tol |
| bubble normalization 1 instead of 27 | formulation | barycentre velocity recovery | `2.9e+07 x` tol |
| bubble unknowns omitted | formulation | rank of the reduced system: **4 of 106**, deficiency 102 | structural |
| inlet normal reversed | boundary data | signed-flux balance `0.163` vs limit `1e-8` | `1.6e+07 x` |
| reaction sign mislabelled | reporting | reaction-orientation assertion | `6.2e+10 x` tol |
| **wrong quad diagonal** | mesh contract | frozen velocity, pressure and reaction | see below |

Margins are against the looser *production* tolerance, so they hold a fortiori
against the route-agreement tolerance.

#### The wrong quad diagonal, and why flux cannot catch it

Feeding the excluded `I_i--O_j` split
([`../../mesh/falsifier-wrong-diagonal.json`](../../mesh/falsifier-wrong-diagonal.json))
through the identical route, with every vertex, boundary facet and named set
unchanged:

| Quantity | max abs difference | route tolerance | ratio | rejected? |
| --- | ---: | ---: | ---: | --- |
| velocity | `2.056e-2 m/s` | `6.20e-11` | `3.3e+08` | yes |
| pressure | `6.471e-1 Pa` | `1.66e-13` | `3.9e+12` | yes |
| reaction | `2.747e-2 N/m` | `8.00e-14` | `3.4e+11` | yes |
| **signed flux** | `2.44e-41 m^2/s` | `2.48e-11` | `9.8e-31` | **no** |

It is rejected by the same margins against the production tolerance
(`1.4e+05` / `1.8e+09` / `1.8e+08`). Read the last row: **the signed fluxes and
the global momentum balance are identical on both meshes**, because both carry
the same 104 vertices and the same 104 boundary facets and those observations
depend only on the P1 boundary trace. The difference lives entirely in interior
connectivity. A flux-only check would accept the wrong mesh; only the probes and
the reaction separate them. The route asserts this asymmetry in both directions.

One other falsifier deserves a precise statement rather than a number.
**Rescaling the bubble basis is a change of basis**: `beta' = l0 l1 l2` spans the
same MINI space, so the solved velocity *field* is unchanged, and this route
verifies that it is. What breaks is the recovery convention —
`beta(barycentre) = 1` holds only for the 27-normalization — so an implementation
that changes the normalization while still reading the bubble coefficient as the
barycentre enrichment misses the frozen velocity probes by the factor 27 on the
enrichment part. That is the honest detection, and it is the one this route
asserts.

### Reindexing invariance

Permuting `vertex v -> (7v + 13) mod 104` and `cell c -> (11c + 5) mod 104`, and
rotating every triple, changes every index while preserving the geometry
bit-for-bit. Re-solving and re-observing moves no observation by more than
`7.4e-39`, and every geometric selector picks the identical coordinates.

## Straight-channel reference check

[`reference_channel.py`](reference_channel.py) runs the *same* assembly on a
straight `2.2 x 0.41 m` channel with no hole, the same viscosity, the same
parabolic inlet, the same no-slip walls and the same zero outlet traction, and
compares against the analytic Poiseuille drop `8 mu Umax Lx / H^2 = 0.031409875
Pa`, centreline speed `Umax` and flux `2 Umax H / 3`. Rerun in the environment
recorded below, in `40.2 s` total:

| Mesh | `dp` Pa | ratio to analytic | `u_centre` m/s | flux `m^2/s` |
| --- | --- | ---: | --- | --- |
| 6x3 | `0.028754102` | `0.915448` | `0.269689` | `0.0728889` |
| 10x5 | `0.029753872` | `0.947278` | `0.289153` | `0.07872` |
| 14x7 | `0.029988809` | `0.954757` | `0.294528` | `0.0803265` |

All three converge toward the analytic values as the mesh refines. This is the
magnitude corroboration for the large RFC 0082 numbers above: the assembly is sound,
and the contract magnitudes are a property of the RFC 0082 reference topology, which has
no interior velocity vertices at all. `reference_channel.py` is a **diagnostic**;
it is not part of the frozen route, not a claim of this slice, and it carries no
pass/fail threshold — the patch test inside `oracle.py` is the hard check. Its
file is unchanged from the previous revision.

## Run

```bash
python3 verify/fluid/exact-circular-hole-stokes-2d/mesh/check_mesh.py               # 162 checks
python3 verify/fluid/exact-circular-hole-stokes-2d/routes/python/oracle.py          # 101 checks, rewrites result.json
python3 verify/fluid/exact-circular-hole-stokes-2d/routes/python/oracle.py --check  # fail if result.json would change; writes nothing
python3 verify/fluid/exact-circular-hole-stokes-2d/routes/python/reference_channel.py   # diagnostic
```

Dependencies: Python `>= 3.12` and `mpmath >= 1.3.0`; everything else is standard
library. Both are declared and checked at startup.

Environment these timings were measured in — they are local, vary a little run to
run, and say nothing about any other environment: Linux 6.11.0-1014-lowlatency
x86-64, CPython 3.12.3, mpmath 1.3.0. `oracle.py` about 55 s, `oracle.py --check`
about 60 s, `reference_channel.py` about 40 s, `check_mesh.py` and `build_mesh.py`
about 0.1 s each. Every command above ran to completion in this environment:
nothing timed out and no auxiliary route was skipped.

| File | SHA-256 |
| --- | --- |
| `stokes.py` | `61b5f70b419360576b14e95d217442ed975af4b030e4a4ac49cfc74899d95d78` |
| `oracle.py` | `650136022b026efb32c37bd010ff20c97716acb3f9d7952aeda1fdaa20ad64d0` |
| `reference_channel.py` | `b36e9a69408e704123b5424a7abe0842a78c12bba3e9112af71531615844f8cc` |
| `result.json` (29357 bytes) | `f3c6579fb45879bb1861e203b9da470be42cfdd56f9dfe02b484b98c814bf685` |
| `../../mesh/mesh.json` | `ada2d08cde5b4e6bd13c97d3b76a45cad810d8eb7acf0f0edc82cd605acd2b39` |
| `../../mesh/falsifier-wrong-diagonal.json` | `eccb5642eab811cee1cad0cee8749f7f2a64d16ab300b041fa4efcbe7b61cd2f` |

All calculation lives in the scripts. `result.json` is output, never input;
`oracle.py --check` requires it to be reproduced byte for byte, and it was.

These are the **packaged** digests. Packaging replaced repository-numbered
tracking prose with stable contract and RFC wording and reran `oracle.py`, so
these digests differ from the ones carried in the source worktree. All 343
numeric and boolean fields of `result.json`, and its full member order, are
bit-identical to the source; the ten changed strings are the prose fields plus
the two recorded mesh digests. See [`../../README.md`](../../README.md).

## Limitations

1. **One route only.** The Julia route has not run, no route-to-route agreement
   is claimed, and the dual independent oracle gate has not passed.
2. **The recovered pressure and reaction are far above their physical scales**,
   by about four orders of magnitude, and the cylinder vector is an algebraic
   constrained-vertex force rather than drag. See the second section above for
   the measured ratios and the reason.
3. **Cross-platform mesh bytes are not claimed** (RFC 0082 does not claim them
   either), so the production inventory comparison must be tolerance-based rather
   than bitwise.
4. **The tied pressure probes must be resolved on the stored coordinates**, as
   described above.
5. **The RFC 0082 ideal closed forms need the exact decimal radius `1/20`**; the
   binary64 spelling of `0.05` shifts them by one relative half-ulp.
6. Steady Stokes only. No Navier–Stokes, Reynolds number, transient behaviour,
   vortex shedding, drag or lift coefficient, DFG or Schäfer–Turek validation,
   PDE mesh convergence, curved element, production mesher, 3D, or performance
   claim is made.
