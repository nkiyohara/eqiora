# Julia numerical-oracle route

> **This is ONE independently frozen route.**
> Route-to-route comparison **has not been performed here** and must not be
> inferred from anything in this directory. This session never read the Python
> route, its results, any existing `verify/fluid` implementation or evidence, or
> any production code for this slice.
>
> **Implementation of this slice may not begin** until the integrator compares
> this frozen output with the separately frozen first route under the
> precommitted contract tolerances.

Authored by a fresh, non-implementing Opus 5 session against the frozen
contract commit `f5fd3ce`. Inputs were the frozen contract and public RFCs
[0043](../../../../../rfcs/0043-simplicial-mini-stokes-realization.md),
[0045](../../../../../rfcs/0045-fieldwise-mixed-realization-and-si-congruence.md),
[0047](../../../../../rfcs/0047-mixed-stokes-static-pressure.md),
[0081](../../../../../rfcs/0081-exact-circular-hole-geometry.md) and
[0082](../../../../../rfcs/0082-source-bound-chordal-circular-hole-mesh.md) in
this worktree. Nothing else.

## Coarse-mesh facts that bound every number below

- All **104** mesh vertices lie on the boundary.
- Trace closure fixes **103** velocity vertices; **only the outlet midpoint
  `(2.2, 0.2) m` is free**.
- MINI **bubble velocities remain cell-interior unknowns** on all 104 cells.
- The reported cylinder vector is the **algebraic constrained-vertex force on
  this mesh**. It is **not drag**, not a physically scaled force, and not
  mesh-independent. Nothing here is a PDE-accuracy or mesh-convergence claim.

## Run

```bash
cd verify/fluid/exact-circular-hole-stokes-2d/routes/julia
julia run.jl            # Julia 1.12.6, standard library only, ~45 s, exit 0 on success
```

Deterministic and rerunnable: no RNG, no timestamps, no wall-clock in any
output, BLAS pinned to one thread, no package installation, no cache or
artifact outside `expected/`. Two consecutive runs produce byte-identical
`expected/julia-route-frozen.json`.

Outputs: `expected/julia-route-frozen.json` (frozen values, machine-readable)
and `expected/run-log.txt` (the full check transcript).

## Derivation

### Mesh (RFC 0081 source, RFC 0082 realization) — reconstructed, not consumed

`src/geometry.jl` rebuilds the mesh from the public construction only. No
externally authored mesh file is read or reproduced.

Rays `theta_i = 2 pi i / 50`, `i = 0..49`. Inner vertices
`I_i = c + r (cos theta_i, sin theta_i)`. Outer hits cast from `c` to the
rectangle with the **cast-axis coordinate assigned the exact bound** and only
the transverse coordinate reconstructed, as RFC 0082 requires; the
rounding-sensitive `c + ((bound-c)/d)*d` spelling is not used. For adjacent
rays `i`, `j = i+1 mod 50` the shared quad diagonal is the frozen `O_i--I_j`
and the two cells are `(O_i, O_j, I_j)` and `(O_i, I_j, I_i)`. Both are
**verified positively oriented in that stored order** — no orientation
normalization was needed. Each of the four rectangle corners falls strictly
inside exactly one ray gap and is filled by a one-triangle deterministic fan
`(O_i, C, O_j)`.

Independently rechecked before any assembly (all in `expected/run-log.txt`):

| Fact | Reconstructed |
|---|---|
| circular segments | 50 |
| vertices / cells / boundary facets | 104 / 104 / 104 |
| outer-loop vertices | 54 |
| boundary / interior edges, Euler characteristic | 104 / 104, `V-E+F = 0` |
| inlet / outlet / wall / cylinder facets | 14 / 2 / 38 / 50 |
| boundary partition | every facet covered exactly once |
| cell orientation | all 104 strictly positive, min area `2.1051229574916655e-4 m^2` |
| measured Hausdorff bound | `9.866357858644148e-5 m`, `+` allowance `6.252776074688882e-14` `<= 1e-4` |
| 49 segments | `sagitta(49) = 1.027e-4 m > 1e-4`, so 50 is minimal |
| named-set normals | inlet `(-1,0)`, outlet `(+1,0)`, walls `(0,±1)`, cylinder into the hole |

The four RFC 0082 frozen ideal values are reproduced to a relative `2.4e-52 …
2.8e-50` **using the exact decimal radius `1/20`**. Using the binary64 radius
instead shifts `area_deficit(50)` by `2.3e-21 m^2` — exactly twice the
representation error of `0.05` — which is why `ideal_metrics` takes a
`Rational`.

An **index-free geometric digest** (vertices in lexicographic coordinate order,
each cell as its sorted coordinate triple, cell and facet lists sorted) lets the
integrator compare mesh identity across routes without sharing an index
convention:

```
sha256(geometric_digest) = 573f2c9260b2976853c84bc96a4301bc39e52578209ea84bbf679ad3d77ad871
```

### Discretization — distinct from the closed-form route by construction

`src/mini.jl`. Every entry is produced by an **explicit positive 3×3
Gauss-Legendre Duffy quadrature loop on every affine triangle**. No closed-form
cell block, no static condensation, no elimination of the bubble.

- Duffy image of 3-point Gauss-Legendre per unit-square axis: `xi = s`,
  `eta = t(1-s)`, weight `w_a w_b (1-s)/4`. Audited: 9 strictly positive
  points inside the reference triangle, weights summing to `1/2`, **exact
  through total degree four** (the MINI bubble-gradient product degree) and
  **provably inexact at degree five**, so the declared bound is tight.
- Bases at each quadrature point: P1 barycentric `l0, l1, l2` plus the
  **normalized** bubble `b = 27 l0 l1 l2`. Audited: `b = 1` at the barycentre,
  `b = 0` on every edge, reference integral `27/120`, P1 partition of unity.
- Gradients by `grad_x phi = J^{-T} grad_ref phi`; `det J > 0` asserted per cell.
- Symmetric-gradient viscous block, expanded once analytically:

  ```
  2 mu sym(grad(phi_a e_c)) : sym(grad(phi_e e_d))
      = mu [ delta_cd (grad phi_a . grad phi_e) + d_d phi_a  d_c phi_e ]
  ```

  This is the whole difference from the vector-Laplacian falsifier, which drops
  the second term.
- Mixed block `c(v,p) = -int p div(v)`, written into both the momentum column
  and the pressure row, so the assembled system is **exactly symmetric**
  (asserted entrywise, `A[i,j] == A[j,i]`, not to a tolerance).
- Constant-traction P1 facet load `length * traction / 2` at each endpoint.
  Zero here; a probe with unit traction confirms the load reaches **only** the
  two outlet facets' P1 velocity rows and never a bubble, pressure or interior
  row.

### Coherent-SI scaling (RFC 0045 / RFC 0047)

The dimensionless congruent system `A_hat = D A D / Theta` is assembled
**directly** in normalized coordinates `x_hat = (x - [0,0])/L`; no
mixed-dimensional matrix is ever formed.

```
L = H = 0.41 m                U = Umax = 0.3 m/s        P = mu U / L = 0.0007317073170731707 Pa
G = U/L = 0.7317073170731707 1/s                        Theta = P U L
mu_hat = mu U / (P L) = 1.000000000000000044249552e+00
```

`Theta` evaluates in binary64 to `8.999999999999999e-5 W/m`, verified to be
**exactly one ulp below** the mathematical `9e-5`, as the frozen contract states.

Inlet: `trace(u) + normal(isotropic_lift(g)) = 0` with parent-outward `(-1,0)`
gives `u = (g(y), 0)`, `g(y) = 4 Umax y (H-y) / H^2`. At the two `x=0` corners
`g` is exactly zero, so the inlet and wall traces agree there with no
tie-breaking rule. Outlet: constant parent-outward traction `(0,0) Pa`. Walls
and cylinder: `u = 0`. Body force identically zero.

### Solve and residuals

520 full DOFs (208 P1 velocity + 208 bubble + 104 pressure), 206 essential,
**314 reduced**, no gauge row or column. Solved by dense LU with partial
pivoting at **256-bit BigFloat**. The reduced true residual and the weak
pressure-row residual are then computed by **`apply_full`, a second
cell-by-cell reapplication of the operator that never touches the assembled
matrix**; the two applications agree to `2.5e-73`.

## Frozen results

Full precision (25 significant digits) in
`expected/julia-route-frozen.json`; abbreviated here.

### Velocity at the barycentre of the selected cell (m/s)

The selected cell is the one whose **physical barycentre** minimizes squared
distance to the target, with an exact tie broken by the lexicographically
sorted vertex-coordinate triple. **Every selection was unique** (no tie). The
barycentre is reported beside each value because on this deliberately coarse
mesh it is far from the target.

| target (m) | selected barycentre (m) | `u_x` | `u_y` |
|---|---|---|---|
| `(0.10, 0.20)` | `(0.1001314216447587, 0.19791111277392828)` | `8.801138571928350e-2` | `-4.377542327224644e-3` |
| `(0.20, 0.30)` | `(0.2044040267077555, 0.30326755761427576)` | `1.690143015268948e-1` | `-9.024390638457480e-3` |
| `(0.30, 0.20)` | `(0.3198475226050584, 0.11550768456009686)` | `9.623743076579622e-2` | `1.125182753674584e-1` |
| `(1.00, 0.20)` | `(0.8998685783552413, 0.20208888722607177)` | `1.326922903707496e-1` | `-7.663108263206533e-4` |
| `(2.00, 0.20)` | `(2.061054339220387,  0.06666666666666667)` | `2.367220892434223e-1` | `5.088790553945741e-2` |

At a barycentre every barycentric coordinate is `1/3` and the normalized bubble
is exactly `1`, so `u = mean(vertex velocities) + bubble coefficient`.

### P1 pressure (Pa)

| probe | vertex (m) | `p` | exact ties |
|---|---|---|---|
| `cylinder_min_x` | `(0.15000000000000002, 0.2)` | `2.061189714291363e+1` | 1 |
| `cylinder_max_x` | `(0.25, 0.2)` | `1.115216508530620e-1` | 1 |
| `cylinder_min_y` | `(0.19686047402353435, 0.15009866357858642)` | `1.103786740720071e+1` | **2** |
| `cylinder_max_y` | `(0.19686047402353435, 0.2499013364214136)` | `1.031573013017810e+1` | **2** |
| `outer_near_inlet_mid` | `(0.0, 0.20000000000000004)` | `1.978039033264140e+1` | 1 |
| `outer_near_outlet_mid` | `(2.2, 0.2)` | `-4.836168726748482e-2` | 1 |

**Read the tie handling carefully.** `50` is even, so no ray points at `90` or
`270` degrees and the extremal-`y` cylinder vertex is genuinely two-fold
degenerate: rays `37`/`38` and `12`/`13` give **bitwise equal** `y` in binary64.
The contract's lexicographic rule resolves both to the smaller `x`. The two
candidates carry **materially different** pressures, so the frozen JSON records
**both**:

```
cylinder_min_y  (0.19686047402353435, 0.15009866357858642) -> 11.03786740720071    <- selected
                (0.20313952597646565, 0.15009866357858642) -> 10.02565092736277
cylinder_max_y  (0.19686047402353435, 0.2499013364214136)  -> 10.31573013017810    <- selected
                (0.20313952597646567, 0.2499013364214136)  ->  9.34071852973703
```

If the other route's libm breaks the `y` tie instead of preserving it, the two
routes will select different vertices and disagree by ~1 Pa on a
`1.66e-13 Pa` tolerance. That would be a **selector** disagreement, not a
numerical one. This route measured the tie to be robust: perturbing every
`cos`/`sin` by one ulp in an alternating pattern leaves all selectors unmoved
(see *Stability*). The integrator should still compare the tie structure
explicitly, not just the selected value.

### Signed fluxes (m²/s)

Facet length times endpoint-average velocity dotted with the
adjacent-fluid-cell-derived parent-outward unit normal.

```
inlet    = -8.149573099927538e-2      (into the domain)
outlet   = +8.149573099927538e-2
walls    =  0            cylinder =  0        (exactly, no-slip)
inlet + outlet = -2.4e-76             (limit 1e-8)
```

### Reactions (N/m, per unit out-of-plane thickness)

```
cylinder constraint force ON THE FLUID   = (-4.617062540501678e+0,  3.952008400301015e-2)
fluid force ON THE CYLINDER  (negation)  = ( 4.617062540501678e+0, -3.952008400301015e-2)

all-essential constrained reaction       = ( 1.04e-76, -1.36e-76)
integrated body force                    = ( 0, 0)      (zero force potential)
integrated applied traction              = ( 0, 0)      (outlet traction is (0,0) Pa)
componentwise sum                        = ( 1.04e-76, -1.36e-76)     (limit 1e-10)
```

The API convention is the constraint force **on the fluid**; the fluid force on
the cylinder is its componentwise negation and any consumer must label it as
such. Both are frozen, both labelled. **This vector is not drag** and no
comparison against any external cylinder-flow reference is made or implied: it
is the algebraic constrained-vertex force on a mesh where 103 of 104 velocity
vertices are prescribed.

### Residuals and structure

```
selected residual target      1.3239627651209673e-07   (max(1e-13, 1e-6*||b_hat||_2), rtol amended)
roundoff allowance            6.46950959525234e-05     (4096 eps (1 + ||A||inf ||x||inf + ||b||inf))
true reduced residual (2)     5.43e-73                 independently reapplied
weak pressure-row residual(2) 1.62e-75                 no gauge column to exclude
||A_hat||inf = 2525.169952486783   ||x_hat||inf = 28169.592761981967
pressure reference = BoundaryTraction; gauge rows 0, columns 0, multipliers 0,
ZeroIntegral constraints 0, traction partition 2 facets      (exact, not tolerance-based)
```

Supplementary (not a contract observation): pressure integral
`1.728499492285226 Pa m^2`.

## Stability of the frozen values

| perturbation | velocity | pressure | flux | reaction | selectors |
|---|---|---|---|---|---|
| 256 → 384 bit working precision | 0 | 0 | 0 | 0 | unmoved |
| vertex/cell renumbering + cell rotation | 0 | 0 | 0 | 0 | unmoved |
| ±1 ulp on every `cos`/`sin` | `1.11e-16` | `1.39e-17` | `0` | `8.33e-17` | unmoved |
| `mu_hat := 1` exactly | `0` | `1.78e-15` | `0` | `8.88e-16` | unmoved |
| route-to-route tolerance | `6.20e-11` | `1.66e-13` | `2.48e-11` | `8.00e-14` | — |

Every perturbation stays four to six orders inside the precommitted tolerance,
so neither libm divergence nor a different-but-defensible spelling of `mu_hat`
can explain a route-to-route disagreement. The reindexing row is the required
invariance demonstration: geometry and connectivity are preserved while every
vertex index, cell index and stored cell rotation change.

## Falsifiers

All exercised; see `expected/run-log.txt`. Deviations are in physical units;
"detected" means beyond the route-to-route tolerance above or a structural
rejection.

| falsifier | outcome |
|---|---|
| wrong quad diagonal `O_j--I_i` | detected — vel `2.06e-2`, pre `6.47e-1`, reac `2.75e-2` |
| vector-Laplacian instead of `2 mu sym(grad u)` | detected — vel `4.04e-2`, pre `9.42`, reac `2.17` |
| unnormalized bubble vs. the `27x` evaluation convention | detected — vel `4.39` |
| pressure/velocity coupling sign reversed (both blocks) | detected — pre `4.12e+1` |
| coupling sign reversed on the momentum row only | detected — pre `4.12e+1`, **and exact CSR symmetry is lost**, so it rejects before the solve |
| dropped bubble unknowns | detected — **reduced system becomes exactly singular** (inf-sup enrichment removed) |
| swapped inlet/outlet membership | detected — vel `5.99e-2`, pre `2.06e+1`, flux `2.00e-2`, reac `1.51` |
| reversed inlet normal in the boundary data | detected — vel `4.73e-1`, pre `4.12e+1`, flux `1.63e-1`, reac `9.23` |
| reversed normal in the flux observation | detected — `\|inlet+outlet\| = 1.63e-1 > 1e-8` |
| omitted cylinder facets | detected — partition covers 54 of 104 facets; solution moves by `4.62` N/m |
| zero traction substituted for cylinder no-slip | detected — **0 constrained cylinder vertices**, named reaction inadmissible |
| stale correspondence from a refined/renumbered mesh | detected — `n=52` gives 108 facets; 3 frozen cylinder indices are not chords |
| radial hit coinciding with a rectangle corner (`n=64`) | rejects at reconstruction |
| gauge row added with a nonempty traction partition | detected — `gamma_hat = -8.78e-1`, pressure probes shift `1.42e+1 Pa` |

Each falsified run recovers its reaction by reapplying **its own** defective
operator, not the frozen one, so a falsifier is never flattered by a mismatched
reaction. Three of these deserve their exact scope stated rather than a bare
"detected":

- **Unnormalized bubble.** Replacing `27 l0 l1 l2` by `l0 l1 l2` *consistently*
  is a pure basis rescaling and is a **null transformation on the discrete
  field** — pressure, flux and reaction move by exactly zero, as measured. What
  is detectable, and what this falsifier exercises, is the realistic defect:
  assembling with the unnormalized bubble while the barycentre evaluation keeps
  the RFC 0043 convention that the coefficient *is* the barycentre value. That
  moves the velocity probes by `4.39 m/s`.
- **Coupling sign reversal.** A *consistent* reversal of both the momentum
  column and the pressure row is equivalent to `p -> -p`: velocity, flux and
  reaction are provably unchanged and measure exactly zero deviation. Only the
  pressure probes catch it, by `41.2 Pa`. Reversing the momentum row alone
  additionally destroys exact CSR symmetry, which the RFC 0043 symmetry
  assertion rejects before any solve.
- **Reversed inlet normal in the boundary data.** The discrete divergence
  theorem makes `inlet_flux + outlet_flux = 0` an *algebraic identity* for any
  solution, so this defect does **not** break the flux balance. It is caught by
  the frozen probes, fluxes and reaction instead. Only reversing the normal used
  in the flux *observation* breaks the balance. Both variants are exercised
  separately.

## Internal method audit (not a claim)

`src/audit.jl` solves a fixture whose exact solution lies **exactly** in the
MINI/P1 space, so it is a genuine exactness test rather than a convergence
test: on `(0,2) x (0,1)` with `u = (x, -y)`, `p = 2 mu`, zero body force and
zero parent-outward traction on `x = 2` (since `sigma n = (2 mu - p, 0) = 0`).
All 19 audit checks pass: velocity, bubbles (zero), pressure, exact matrix
symmetry, outflow flux `= 2`, total reaction `= 0`, and the analytic per-side
reactions `(0, ±4 mu Lx) = (0, ±12)`.

A plane-Poiseuille profile is deliberately **not** used: it is quadratic in `y`,
is not in the MINI velocity space, and could only be checked asymptotically —
which is exactly the PDE-convergence claim the frozen contract forbids. **No capability,
convergence or accuracy claim is made by this audit.**

## Measured finding the integrator must weigh before implementation

The frozen solve selection may not be able to meet the frozen production
tolerances on this witness. Nothing was relaxed; this is reported as measured.

Implementation-independent properties of the frozen witness + frozen scale
profile + RFC 0082 mesh:

```
cond_2(A_hat_reduced) = 4.635683511e9
|lambda| in [5.438264899e-7, 2.521007622e3], 104 negative / 210 positive
||x_hat||_inf = 2.816959e4        (the pressure block dominates: p reaches ~2.8e4 * P)
```

Measured with a **Julia f64 Paige-Saunders MINRES, identity preconditioner,
rtol 1e-6 (amended from 1e-11), atol 1e-13, cap 10000** — an *analogue* of the
frozen tuple, run
locally. It is **not** the registered `eqiora.reference` backend, not its
`Reproducible` reduction, and not a hosted measurement, so it indicates
feasibility rather than settling it. The implementation was validated on a
well-conditioned system (`cond 1.7e4`), where its recurred and independently
reapplied true residuals agree to four digits.

| | iterations | recurred residual | true residual | max pressure error | max reaction error |
|---|---|---|---|---|---|
| MINRES + Identity | 5832 / 10000 | `1.3102e-7` | **`2.4679e-5`** | `3.33e-7 Pa` | `1.91e-8 N/m` |
| f64 dense LU (*not* the frozen selection) | — | — | `1.55e-12` | `2.31e-14 Pa` | `9.42e-15 N/m` |
| production tolerance `floor + 5e-7 * scale` | — | — | — | `3.66e-10 Pa` | `1.50e-10 N/m` |

So, on this witness:

1. MINRES reaches the *recurred* target at iteration 5832 of the 10000 cap,
   leaving `41.7 %` headroom. Under the superseded rtol `1e-11` it reached the
   target only at iteration 9401, leaving under 7 %.
2. Its **true** residual floors at `2.4679e-5`. Rerunning with the stopping
   test disabled and the operator independently reapplied every 100 iterations
   for 20000 iterations, the best true residual seen is `2.467954e-5` at
   iteration 6100 — it does not improve afterwards, while the recurred estimate
   keeps collapsing (to `3.1e-28` by iteration 20000). That is total loss of
   Lanczos orthogonality, so the recurred estimate stops describing the true
   residual. The contract's *residual* criterion is nevertheless satisfied,
   because the frozen roundoff allowance `4096 eps (1 + ||A||inf ||x||inf +
   ||b||inf)` evaluates to `6.470e-5` here, and the residual is measured
   against target-plus-allowance rather than against the target.
3. The **pointwise** production tolerances are missed: pressure by `911x` and
   reaction by `127x`. Velocity and flux pass, and both global balances pass.
   They were missed under the superseded rtol too, by `256x` and `160x`, so
   this is a property of the iterative selection on this operator rather than
   of the amended tolerance.
4. A direct f64 factorization meets every tolerance with a `10^4` margin, so
   the gap is attributable to the iterative selection on a `4.6e9`-conditioned
   operator, not to the frozen tolerance table or the mesh.

The frozen contract states that if the reference path cannot meet the tolerances the
slice returns with the measured argument instead of relaxing the claim, and it
excludes sparse-LU integration and new solver policy from this slice. That
decision is the integrator's; this route only supplies the measurement.

## Exact omissions

Deliberately **not** covered here; each needs production code or the other
route.

1. **Route-to-route comparison.** Not performed. No agreement is claimed.
2. **Source-revision guard.** "Same-named hand-authored polygon or a chordal
   owner from another exact source rejects before assembly" is a production
   digest guard with no numerical content; unreachable from a standalone
   numerical route.
3. **Replay-validated authored-region correspondence, artifact digests, Model /
   Realization / Run lineage, `region_entity_set_entities` resolution.** This
   route derives named sets from the reconstructed geometry directly. It
   exercises *analogues* of the stale-correspondence falsifier only.
4. **Production vertex/cell/facet index order.** Unknown to this route and not
   claimed. All selectors are geometric; the index-free digest above is the
   intended cross-route mesh comparison key.
5. **RFC 0082 corner-reuse branch.** A radial hit within the classification
   tolerance of a corner is a *rejection* here, not an implemented reuse path.
   The `n = 50` witness provably never triggers it (asserted); `n = 64` does,
   and is shown to reject.
6. **`n = 8/16/32/64` refinement family.** Not run as a convergence study.
   The frozen contract forbids manufacturing a PDE-convergence claim on those variants;
   `n = 52` and `n = 64` appear only as falsifier fixtures.
7. **The registered `eqiora.reference` MINRES.** Section above is a Julia
   analogue, explicitly labelled advisory.
8. **Capability matrix, `verify/` manifest, roadmap, RFC index.** Untouched:
   integrator-owned, and this route is not yet an accepted capability.
9. **The repository gate was not run.** `cargo` is not installed in this
   worktree, so `python3 tools/ci/local_verify.py fast|affected` cannot execute
   here at all. No claim of a passing repository gate is made. Two facts the
   integrator should know before running it:
   - `local_verify.py` derives the case id **`fluid.exact-circular-hole-stokes-2d`**
     from this directory path, but no `case.toml` is registered there — this
     route is a pre-implementation oracle, not accepted evidence, and
     registering it is the evidence maintainer's step, not this session's.
   - The commit touches **only**
     `verify/fluid/exact-circular-hole-stokes-2d/routes/julia/**` and no Rust,
     registry, RFC, manifest or shared mesh path.

## Provenance

- Contract commit: `f5fd3ce` ("Specify circular reference quad diagonal").
- Julia `1.12.6`, standard library only (`LinearAlgebra`, `Printf`, `SHA`).
  No `Project.toml`, no `Manifest.toml`, no downloaded package.
- Checks: **103 total, 103 passed, 0 failed**.
- `sha256(expected/julia-route-frozen.json)` is printed at the end of every run
  and recorded in `expected/run-log.txt`, together with the sha256 of every
  source file, so a reader can confirm the frozen file belongs to this source.
- Packaging replaced repository-numbered tracking prose with stable contract and
  RFC wording and reran `run.jl`, so the digests in `expected/run-log.txt` are
  the packaged ones and differ from those the source worktree carried. All 185
  numeric and boolean fields of `expected/julia-route-frozen.json`, all 47 of
  its 25-digit `hp` decimal strings, and its full member order are bit-identical
  to the source; of its 169 strings the one that changed is `statement`. The
  index-free geometric mesh digest
  `573f2c9260b2976853c84bc96a4301bc39e52578209ea84bbf679ad3d77ad871` is
  unchanged, because it is computed from the reconstructed geometry alone. See
  [`../../README.md`](../../README.md) for the packaging proof and the source
  digests. `f5fd3ce` is this worktree's revision of the contract commit; RFC
  0081 and RFC 0082 are byte-identical there and at the packaging base
  `dea1fd1`.

## Layout

```
run.jl              driver: audit, reconstruct, lower, solve, observe, falsify, freeze
src/oracle.jl       module entry point
src/geometry.jl     RFC 0081 source + RFC 0082 chordal mesh reconstruction, metrics, digest
src/mini.jl         Duffy quadrature, MINI/P1 bases, assembly, independent reapplication, solve
src/witness.jl      frozen witness lowering, physical reconstruction, geometric selectors
src/audit.jl        internal method audit (exact patch fixture)
src/falsify.jl      mesh checks, structural assertions, deviation metric, gauge falsifier
src/minres.jl       advisory f64 MINRES analogue (never produces a frozen value)
expected/julia-route-frozen.json
expected/run-log.txt
```
