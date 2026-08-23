# Julia route — Gmsh 4.15.2 MINI/P1 Stokes oracle

This is one fresh, non-implementing numerical route for steady Stokes flow on the
accepted exact-source Gmsh mesh. It started from exact base
`934493bcb487c1753fb4b3ddffaab88d7150aa7d` and then consumed only the independently
accepted mesh-evidence seam
`05254257d98caee8cac924759d01d92c25801169`. It did not read or run Eqiora's
implementation, an implementation fixture, another Gmsh Stokes route, or the older
Python Stokes route.

The route is intentionally numerical and Julia-native: it parses ASCII MSH 4.1,
assembles the uncondensed MINI/P1 system by positive quadrature, solves the full
sparse reduced system with refined LU, and cross-checks every physical observation
with independently refined sparse QR. It uses Julia 1.12.6 and standard libraries
only.

## Frozen input

The source is the exact DFG channel `[0, 2.2] x [0, 0.41] m` minus the radius
`0.05 m` circle centred at `(0.2, 0.2) m`. Its accepted exact-source digest is
`b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`.

The mesh owner emitted the accepted 50-chord region's binary64 coordinates with
Rust shortest-roundtrip spelling. The GEO traverses the hole points and lines
first, the outer points and lines second, and supplies the outer loop then the hole
to `Plane Surface`. No `Point` has a characteristic length. The exact options are
Built-in, `General.NumThreads=1`, `Mesh.Algorithm=6`, `ElementOrder=1`,
`SaveAll=1`, MSH 4.1 ASCII, and `RandomFactor=0`.

| Frozen input or projection | SHA-256 |
| --- | --- |
| official `gmsh-4.15.2-Linux64.tgz` | `6c62116e072db29fd1f701fdb9d3d34b46ed5373545063e177b965a008274745` |
| extracted official executable used here | `9dccade5dd1374b28c18af9085d7ce63216cf7ac39d3cefbc0adbfabafba2c7f` |
| sealed GEO | `81c96068891d6b506827339cd6fecf07eafcb867c76f01747c35d134167d367e` |
| regenerated and sealed MSH | `ab7340cec1976f713b5c5deab76fc7d554593126f1c1cd68cc021749911a206a` |
| Julia boundary correspondence map | `8eafb74f5727d720ac6b8e67d4413c07687636ada7cb72af78194634475b1d83` |
| frozen Julia record | `5e6ce308919d87d8c56e3261e09b519bb9e33a98657e321b0fe0747fc8fe3d23` |

The regular official executable reported exactly `4.15.2` and regenerated an MSH
byte-identical to the sealed input. The accepted mesh owner's domain-separated
Eqiora Mesh digest is
`5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b`.
This route cites that ownership digest but does not recompute Eqiora's canonical
wire projection.

## Topology and selection map

The Julia MSH parser independently obtains:

| Quantity | Value |
| --- | ---: |
| vertices / triangles | 662 / 1,210 |
| boundary / interior vertices | 114 / 548 |
| boundary / interior edges | 114 / 1,758 |
| Euler characteristic | 0 |
| minimum signed measure scale | `2.6093038450074273e-5` |
| minimum affine mean ratio | `0.5236522686855336` |

GEO curves `1:50` are the cylinder, `70:71` the outlet, `91:104` the inlet,
and `51:69,72:90` the walls. Those 50/2/14/38 curves map to 50/2/14/48
boundary facets. Gmsh subdivides wall curves 67, 69, 72 and 74 into two facets,
and curves 68 and 73 into four; every other curve has one facet. The groups are
pairwise disjoint and cover all 114 facets exactly once.

There are 4,406 full DOFs: 1,324 P1 velocity, 2,420 cell-bubble velocity, and
662 P1 pressure. The closure of inlet, walls, and cylinder fixes 113 vertices
(226 velocity DOFs); the only free boundary vertex is the outlet midpoint
`(2.2, 0.2)`. The reduced system has 4,180 DOFs. Pressure uses
`BoundaryTraction`: there is no gauge row, column, multiplier, or zero-integral
constraint.

## Formulation and numerical method

The already accepted equations and scales are reused unchanged:

```text
-div(2 mu sym(grad(u)) - p I) = 0,       div(u) = 0,
mu = 0.001 Pa s,  L = H = 0.41 m,  U = Umax = 0.3 m/s,
P = mu U / L = 0.0007317073170731707 Pa,
Theta = P U L = 8.999999999999999e-5 W/m.
```

Velocity is continuous `(P1 + 27 lambda0 lambda1 lambda2)^2`; pressure is
continuous P1. Every local entry comes from the accepted positive degree-four
`3x3` Gauss-Legendre Duffy rule. The bubbles remain explicit unknowns—there is
no static condensation. Inlet velocity is
`(4 Umax y (H-y)/H^2, 0)`; walls and cylinder are no-slip; body force and outlet
traction are zero.

The primary values use SuiteSparse LU followed by two refinement steps. A
separate SuiteSparse QR solve with two refinement steps agrees within
`1.36e-16 m/s`, `5.56e-17 Pa`, `5.56e-17 m^2/s`, and
`1.28e-18 N/m` after also including the larger deviations from a complete
vertex/cell/facet reindexing. The smallest existing tolerance margin is
`2.996e3`. Residuals are not read from either factorization's recurrence: a
second cell/quadrature loop applies the mixed operator directly from the solved
fields.

## Frozen values

Velocity is evaluated at the barycentre of the cell geometrically nearest each
target, with the sorted coordinate triple as exact tie key. All selections are
unique; the smallest squared-distance gap to the next cell is
`1.588802516426744e-5 m^2`.

| target (m) | selected barycentre (m) | velocity (m/s) |
| --- | --- | --- |
| `(0.1, 0.2)` | `(0.09793337988219507, 0.20177374283660565)` | `(0.14213243952137308, 0.01500729300430398)` |
| `(0.2, 0.3)` | `(0.19694114554517952, 0.2995191378539526)` | `(0.37263638830345264, 0.010313584478627463)` |
| `(0.3, 0.2)` | `(0.29488874450425756, 0.20638624237896663)` | `(0.10859853497962273, -0.019034808196661753)` |
| `(1.0, 0.2)` | `(0.9761231491450052, 0.17080514234999064)` | `(0.2964981847265619, 0.000591437399105796)` |
| `(2.0, 0.2)` | `(1.9591009670192758, 0.19146450026592787)` | `(0.3127064966772555, 0.0037804493525137846)` |

P1 pressure probes and global vertex extrema are:

| selector | selected vertex (m) | pressure (Pa) |
| --- | --- | ---: |
| cylinder minimum x / global maximum | `(0.15, 0.2)` | `0.06959832738138935` |
| cylinder maximum x | `(0.25, 0.2)` | `0.01933318139710498` |
| cylinder minimum y, lexicographic tie winner | `(0.1968604740235343, 0.1500986635785864)` | `0.04389626088659285` |
| cylinder maximum y, lexicographic tie winner | `(0.1968604740235343, 0.2499013364214136)` | `0.04516523057732172` |
| outer nearest inlet midpoint | `(0.0, 0.2)` | `0.062148654204246964` |
| outer nearest outlet midpoint / global minimum | `(2.2, 0.2)` | `0.00047420496757375326` |

The two y extrema each have an exact two-way coordinate tie. The non-selected
minimum-y candidate `(0.2031395259764656, 0.1500986635785864)` has pressure
`0.042434082813404064 Pa`; the non-selected maximum-y candidate
`(0.2031395259764657, 0.2499013364214136)` has
`0.04101749471784515 Pa`. The nearest untied coordinate gap is
`7.869738849790864e-4 m`. The global maximum and minimum pressure gaps are
`2.7416201650938554e-4 Pa` and `1.0854406283514599e-3 Pa`.

Signed fluxes are:

```text
inlet    -0.08149573099927537 m^2/s
outlet   +0.08149573099927535 m^2/s
walls     0 exactly
cylinder  0 exactly
net      -2.7755575615628914e-17 m^2/s
```

The cylinder constraint force **on the fluid** is

```text
(-0.006384200476069209, -6.344553664048034e-5) N/m.
```

The fluid force on the cylinder is its componentwise negation. The complete
essential constraint force, and therefore reaction + body + traction momentum
closure, is `(-6.195044477408373e-18, 1.5987211554602254e-18) N/m`.

The independently reapplied true reduced residual is
`2.7331758424307547e-14`; the weak pressure-row residual is
`1.3023613209636815e-16`. The selected target is
`6.138485578780151e-6`, the existing true-residual roundoff allowance is
`2.2912119699006716e-9`, and the corrected weak-row allowance is
`9.095002846930392e-13`. The normwise infinity backward error is
`1.7635084791063768e-18`.

## Tolerances and falsifiers

No new tolerance family was invented. Route-to-route comparisons use the
accepted `absolute floor + 2e-10 * existing physical scale` values:

| family | tolerance |
| --- | ---: |
| velocity | `6.2e-11 m/s` |
| pressure, including extrema | `1.6634146341463415e-13 Pa` |
| signed flux | `2.48e-11 m^2/s` |
| reaction | `8.0e-14 N/m` |

Selectors, tie multiplicities, topology, pressure-reference structure, and
input digests are exact. The existing flux-balance and momentum-closure limits
remain `1e-8 m^2/s` and `1e-10 N/m`.

All falsifiers run only after the ordinary positive path. Algorithm 5 changes
the mesh to 689 vertices / 1,264 triangles and MSH digest `c46cdd5c...`.
Replacing the symmetric gradient with a vector Laplacian moves observations by
up to `2.53e-3 m/s`, `1.81e-3 Pa`, and `3.88e-5 N/m`. Reversing the pressure
coupling sign moves pressure by `0.139 Pa`; swapping inlet/outlet moves velocity
by `0.0884 m/s`, pressure by `0.0630 Pa`, flux by `0.143 m^2/s`, and reaction
by `1.69e-3 N/m`. Reversing the inlet normal in the flux observation breaks
balance by `0.163 m^2/s`. A suffixed `4.15.2-nox` version is rejected before
meshing.

## Run

The route consumes the mesh owner's sealed GEO and MSH as read-only shared
inputs and independently reruns the exact official executable:

```bash
cd verify/fluid/exact-circular-hole-stokes-2d-gmsh/routes/julia
LD_LIBRARY_PATH=/path/to/runtime-libraries \
GMSH=/path/to/gmsh-4.15.2-Linux64/bin/gmsh \
GMSH_ARCHIVE=/path/to/gmsh-4.15.2-Linux64.tgz \
GMSH_GEO=/path/to/accepted-owner-region.geo \
GMSH_MSH=/path/to/accepted-owner-region.msh \
julia --startup-file=no run.jl
```

This run produced 47 passed checks, zero failed, and reproduced both frozen
files byte-for-byte. `--freeze` is reserved for the oracle owner; it refuses to
write if any check fails.

## Research ledger and nonclaims

Current best formulation: uncondensed direct quadrature plus refined sparse LU,
with refined sparse QR and reindexing as independent numerical views. A package
FEM stack was rejected because its hidden basis and boundary conventions are
harder to audit. Reusing closed-form/static-condensed local blocks was rejected
because it would weaken independence from the accepted Julia route.

The cited Eqiora canonical mesh and buffer digests are not recomputed here. No
Eqiora implementation, production fixture, other Stokes route, hosted job, or
production backend was read or run. This is not a PDE-convergence, curved-mesh,
cross-platform byte-identity, performance, Navier-Stokes, or transient claim.
The cylinder vector is an algebraic constrained-vertex force on this one mesh;
it is not drag, lift, a coefficient, or a mesh-independent physical value.
