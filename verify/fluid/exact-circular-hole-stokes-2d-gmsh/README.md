# Exact Gmsh 4.15.2 circular-hole steady Stokes witness

This case closes one narrow Linux x86-64 path: the exact source digest
`b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`
is realized by `eqiora.gmsh-cli/4.15.2` as Mesh
`5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b`
with 662 vertices and 1,210 affine triangles, then the existing MINI/P1 steady
Stokes formulation is solved by Faer 0.24.4 sparse LU.

The scientific expectations come from two frozen routes that did not read or
run Eqiora's implementation. Route A uses elevated-precision closed-form
affine blocks with static bubble condensation. Route B uses Julia-native
positive quadrature, the full uncondensed system, refined sparse LU, and a
separate refined sparse QR check. Both consume the same independently accepted
GEO/MSH seam. [`agreement/check.py`](agreement/check.py) checks their exact
input, topology, partition, and selector receipts before comparing all shared
velocity, pressure, flux, and reaction observations.

The two routes agree without changing the established tolerances:

| family | maximum route difference | existing tolerance | margin |
| --- | ---: | ---: | ---: |
| velocity | `1.4051260155412137e-16 m/s` | `6.2e-11 m/s` | `4.41e5` |
| pressure | `1.457167719820518e-16 Pa` | `1.6634146341463415e-13 Pa` | `1.14e3` |
| signed flux | `2.7755575615628914e-17 m²/s` | `2.48e-11 m²/s` | `8.94e5` |
| reaction | `6.195044477408373e-18 N/m` | `8e-14 N/m` | `1.29e4` |

The production projection uses the existing, looser floor-plus-`5e-7` scale
tolerances: `1.50002e-7 m/s`, `3.6587365853658537e-10 Pa`,
`6.15002e-8 m²/s`, and `1.5002e-10 N/m`. The flux-closure bound remains
`1e-8 m²/s`; the componentwise momentum-closure bound remains `1e-10 N/m`.
No tolerance was fitted to this mesh or to Eqiora output.

The registered science evidence runs the route-agreement checker. The existing
installed Python Result case executes the public exact-cylinder path against
the exact Gmsh provider and checks the mesh identity and counts, six pressure
probes, signed inlet/outlet flux, cylinder constraint force, momentum closure,
and the existing true-residual acceptance. Its supplementary Rust/PyO3 test
checks the same physical projection at the native boundary. The Matplotlib and
Marimo cases consume that accepted Result. Missing or wrong Gmsh behavior
remains owned by `interfaces.python-circular-hole-chordal-mesh` and is not
duplicated here.

Run the route agreement before the registered production evidence:

```bash
cargo run --locked -p eqiora-verify -- run \
  --case fluid.exact-circular-hole-stokes-2d-gmsh
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-exact-cylinder-stokes-result
```

This is one exact Linux x86-64 Gmsh 4.15.2 witness. It does not change or
widen the older 104-triangle Studio/core reference case and makes no arbitrary
geometry, alternate-provider, 3D, curved, adaptive, convergence,
cross-platform-byte, performance, Navier–Stokes, transient, drag, lift, or
coefficient claim.
