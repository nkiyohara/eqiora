# Python Gmsh Stokes oracle — independent Route A

This route freezes the mesh-dependent steady-Stokes observations for the exact
Gmsh witness without reading or running Eqiora implementation code. It starts
from base `934493bcb487c1753fb4b3ddffaab88d7150aa7d` and consumes only:

- the already accepted affine MINI/P1 equations, scales, boundary conditions,
  selectors, and tolerance families from
  `../../../exact-circular-hole-stokes-2d/routes/python`;
- the accepted non-implementation mesh seam from commit
  `05254257d98caee8cac924759d01d92c25801169`; and
- official Linux64 Gmsh 4.15.2.

This is Route A only. Route-to-route agreement is a separate decision.

## Frozen input and mesh reconstruction

[`geometry.geo`](geometry.geo) is byte-identical to the sealed mesh-owner GEO.
It enumerates the accepted 50 hole vertices clockwise, then the accepted 54
outer vertices counter-clockwise, with their shortest round-trip binary64
coordinates. It creates all hole points and lines before all outer points and
lines, lists the outer loop before the hole in the plane surface, and supplies
no point characteristic length.

| Input or artifact | SHA-256 |
| --- | --- |
| official `gmsh-4.15.2-Linux64.tgz` | `6c62116e072db29fd1f701fdb9d3d34b46ed5373545063e177b965a008274745` |
| archive `bin/gmsh` | `9dccade5dd1374b28c18af9085d7ce63216cf7ac39d3cefbc0adbfabafba2c7f` |
| `geometry.geo` | `81c96068891d6b506827339cd6fecf07eafcb867c76f01747c35d134167d367e` |
| generated ASCII MSH 4.1 | `ab7340cec1976f713b5c5deab76fc7d554593126f1c1cd68cc021749911a206a` |
| parsed coordinate f64 buffer | `42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d` |
| parsed triangle little-endian u32 buffer | `05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642` |
| accepted Eqiora canonical mesh digest (shared seam) | `5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b` |

The Gmsh settings are `Built-in`, `General.NumThreads=1`,
`Mesh.Algorithm=6`, `Mesh.ElementOrder=1`, `Mesh.SaveAll=1`,
`Mesh.MshFileVersion=4.1`, `Mesh.Binary=0`, and `Mesh.RandomFactor=0`.

`oracle.py` parses `$Entities`, `$Nodes`, and `$Elements` directly. Algebraic
vertex indices are sorted global node tags; triangle element tags are retained.
Each boundary edge is matched to the unique adjacent positive triangle and
oriented so the fluid is on its left. Named boundaries are then reconstructed
from curve entities and endpoint coordinates rather than from assumed element
positions.

| Mesh fact | Value |
| --- | ---: |
| nodes / linear triangles | `662 / 1210` |
| all / boundary / interior edges | `1872 / 114 / 1758` |
| boundary / interior vertices | `114 / 548` |
| inlet / outlet / wall / cylinder edges | `14 / 2 / 48 / 50` |
| Euler characteristic | `0` |
| minimum / maximum cell area | `1.3046519225037137e-5 / 2.3836892726903546e-2 m²` |
| minimum accepted affine-map mean ratio | `0.5236522686855336` |
| maximum GEO-to-MSH coordinate serialization delta | `4.440892098500626e-16 m` |

The coordinate and triangle buffer digests exactly match the independently
accepted mesh seam. That is the index/order mapping used for every observation
below.

## Formulation and numerical route

The reused weak formulation is

```text
 integral 2 mu sym(grad u) : sym(grad v) - integral p div(v) = 0
                        - integral q div(u)                 = 0
```

with `mu=0.001 Pa s`, continuous vector MINI velocity (`P1` plus normalized
cell bubble `beta=27 lambda0 lambda1 lambda2`), and continuous scalar P1
pressure. The inlet trace is
`u=(4 Umax y(H-y)/H²,0)`, with `H=0.41 m` and `Umax=0.3 m/s`; walls and cylinder
are no-slip; body force and outlet traction are zero. The nonempty outlet
traction partition fixes pressure, so no gauge row or zero-integral constraint
is present.

The physical scales are

```text
L = 0.41 m
U = 0.3 m/s
P = mu U/L = 0.0007317073170731707 Pa
G = U/L = 0.7317073170731707 1/s
Theta = P U L = 8.999999999999999e-5 W/m
```

All affine cell blocks are exact barycentric integrals. Cell bubbles are
statically condensed. The resulting `1760 x 1760` sparse system uses SciPy
SuperLU only to produce correction directions; the solution, every residual,
and every update are accumulated at 60 decimal digits. Iteration stops only
after the elevated-precision condensed defect is below
`1e-48 (1 + ||b||₂)`. Three correction steps leave a physical condensed defect
of `1.2113556391021507e-61`.

An exact patch field `u=(x+y,-(x+y)), p=2 mu` is reproduced on this mesh with
maximum velocity, pressure, and bubble errors `1.49e-59`, `3.11e-60`, and
`1.06e-60`. This checks the full accepted block signs, symmetric-gradient
viscosity, essential closure, and natural outlet condition; it is not a
convergence result.

The production solver intent remains Faer 0.24.4 SparseLU on the symmetric
indefinite f64 operator with independent fixed-order true-residual acceptance.
The elevated-precision route supplies expectations, not a production-backend
measurement.

## Frozen observations

### Stable MINI velocity probes

Each selector minimizes squared distance from the target to a triangle
barycentre, with lexicographic geometry as the exact tie-break. All five are
unique and have positive separation from the runner-up.

| target m | element tag | node tags | barycentre m | velocity m/s | selection gap m² |
| --- | ---: | --- | --- | --- | ---: |
| `[0.1,0.2]` | `624` | `[253,251,254]` | `[0.09793337988219507,0.20177374283660562]` | `[0.142132439521373,0.015007293004304013]` | `1.5888025164267582e-5` |
| `[0.2,0.3]` | `294` | `[178,176,363]` | `[0.19694114554517952,0.2995191378539526]` | `[0.3726363883034526,0.010313584478627322]` | `2.3461882548857374e-5` |
| `[0.3,0.2]` | `771` | `[318,278,319]` | `[0.2948887445042576,0.20638624237896663]` | `[0.10859853497962277,-0.019034808196661847]` | `3.324401528312503e-5` |
| `[1.0,0.2]` | `250` | `[400,149,503]` | `[0.9761231491450053,0.1708051423499906]` | `[0.296498184726562,0.0005914373991058375]` | `1.8317270918674013e-3` |
| `[2.0,0.2]` | `1041` | `[151,597,598]` | `[1.9591009670192756,0.1914645002659279]` | `[0.31270649667725553,0.0037804493525137903]` | `5.632210530709504e-4` |

### Pressure extrema and stable geometric probes

The global P1 nodal pressure minimum and maximum are respectively
`0.0004742049675737538 Pa` at node tag `71`, position `[2.2,0.2] m`, and
`0.06959832738138942 Pa` at node tag `1`, position `[0.15,0.2] m`.

| selector | node tag | position m | pressure Pa | tied tags | gap |
| --- | ---: | --- | ---: | --- | ---: |
| cylinder minimum x | `1` | `[0.15,0.2]` | `0.06959832738138942` | `[1]` | `3.942649342761062e-4 m` |
| cylinder maximum x | `26` | `[0.25,0.2]` | `0.019333181397105` | `[26]` | `3.942649342761062e-4 m` |
| cylinder minimum y | `39` | `[0.1968604740235343,0.1500986635785864]` | `0.04389626088659296` | `[38,39]` | `7.869738849791974e-4 m` |
| cylinder maximum y | `13` | `[0.1968604740235343,0.2499013364214136]` | `0.045165230577321865` | `[13,14]` | `7.869738849790864e-4 m` |
| outer nearest inlet midpoint | `98` | `[0.0,0.2]` | `0.062148654204247` | `[98]` | `6.383644743431994e-4 m²` |
| outer nearest outlet midpoint | `71` | `[2.2,0.2]` | `0.0004742049675737538` | `[71]` | `4.0e-2 m²` |

The two cylinder y-extrema are exact coordinate ties in the parsed binary64
mesh. The accepted lexicographic rule selects the lower-x candidate and the
result records all tied node tags.

### Flux, cylinder reaction, and global momentum

| Quantity | x | y |
| --- | ---: | ---: |
| cylinder constraint force on fluid, N/m | `-0.006384200476069211` | `-0.00006344553664047762` |
| fluid force on cylinder, N/m | `+0.006384200476069211` | `+0.00006344553664047762` |
| all constrained reaction + body + traction, N/m | `7.368560570709604e-63` | `-6.624108059442036e-62` |

Signed inlet, outlet, and net fluxes are
`-0.08149573099927537`, `+0.08149573099927537`, and
`4.861730685829017e-62 m²/s`. The continuous parabolic inlet reference is
`-0.08199999999999999 m²/s`; the difference is the already accepted P1
boundary interpolation, not a mass defect.

### Residual

Dimensionless scaling reapplies the complete uncondensed `4180`-row reduced
operator, including all `2420` reconstructed bubble rows.

| Quantity | Value |
| --- | ---: |
| `||b_hat||₂` | `6.138485578780151` |
| selected target `max(1e-6 ||b_hat||₂,1e-13)` | `6.138485578780151e-6` |
| f64 reapplication allowance | `2.2912119699006733e-9` |
| final acceptance limit | `6.1407767907500515e-6` |
| true reduced residual | `3.0625239066692896e-58` |
| weak pressure-row residual | `6.41068006636691e-61` |

The allowance is the accepted fixed formula

```text
4096 eps_f64 (1 + ||A_hat||inf ||x_hat||inf + ||b_hat||inf)
```

with `||A_hat||inf=26.45127993219957` and
`||x_hat||inf=95.11771408789888` for this mesh.

## Tolerances and why they are not fitted

The table is reused unchanged from the accepted Stokes case:

```text
tolerance(kind, relative) = floor(kind) + relative * physical_scale(kind)
```

| family | floor | scale | Route A/B agreement (`2e-10`) | production (`5e-7`) |
| --- | ---: | ---: | ---: | ---: |
| velocity | `2e-12 m/s` | `0.3 m/s` | `6.2e-11 m/s` | `1.50002e-7 m/s` |
| pressure | `2e-14 Pa` | `7.317073170731707e-4 Pa` | `1.6634146341463415e-13 Pa` | `3.6587365853658537e-10 Pa` |
| flux | `2e-13 m²/s` | `0.123 m²/s` | `2.48e-11 m²/s` | `6.15002e-8 m²/s` |
| reaction | `2e-14 N/m` | `3e-4 N/m` | `8e-14 N/m` | `1.5002e-10 N/m` |

No new observation was used to choose or widen a tolerance. The flux closure
bound remains `1e-8 m²/s` and the componentwise momentum closure bound remains
`1e-10 N/m`. Constant pressure is in P1, so weak continuity with `q=1` gives
the discrete net-flux identity. Summing the P1 velocity partition gives the
global reaction/body/traction identity. The measured elevated-precision
closures are therefore roundoff witnesses; the fixed limits remain decisive
orientation and completeness guards.

## Falsifiers

The ordinary positive route and patch test complete before mutants run.

- A one-byte MSH mutation misses the exact artifact digest.
- Reversing the first triangle is rejected by strict positive orientation.
- Removing one cylinder chord is rejected by the exact 50-facet receipt.
- Reversing only the inlet normal produces net flux
  `0.16299146199855075 m²/s`, exceeding `1e-8`.
- Mislabeling the cylinder constraint reaction as fluid-on-cylinder produces
  maximum error `0.012768400952138423 N/m`, `8.51e7` production tolerances.
- Swapping inlet and outlet membership misses all four frozen families:
  velocity by `5.90e5`, pressure by `1.72e8`, flux by `2.32e6`, and reaction
  by `1.12e7` production tolerances.

## Run and reproduced checks

The official Linux64 executable needs its normal runtime libraries available.
With the executable and a home-backed scratch directory:

```bash
python3 oracle.py \
  --gmsh /absolute/path/to/gmsh-4.15.2-Linux64/bin/gmsh \
  --work-dir /home-backed/path/eqiora-gmsh-stokes-route-a

python3 oracle.py --check \
  --gmsh /absolute/path/to/gmsh-4.15.2-Linux64/bin/gmsh \
  --work-dir /home-backed/path/eqiora-gmsh-stokes-route-a
```

The frozen environment was Linux x86-64, CPython 3.12.3, mpmath 1.3.0, NumPy
2.1.3, and SciPy 1.16.1. The standard run reports `56 passed, 0 failed` and
generates [`result.json`](result.json) byte-identically under `--check`. A
separate 80-decimal-digit rerun produced binary64-identical velocity, pressure,
flux, and cylinder-reaction values. `ruff check` and `ruff format --check` pass
for both Python files.

## Not checked or claimed

- Eqiora implementation, implementation fixtures, and production output were
  neither read nor executed.
- The Faer SparseLU production path and hosted environments were not run.
- Route B agreement has not been evaluated here.
- Raw MSH byte portability beyond this official Linux64 witness is not claimed.
- This is not mesh convergence, PDE benchmark validation, drag/lift
  coefficient validation, Navier–Stokes, transient, curved-element, 3D,
  performance, or general-mesher evidence.
