# Python exact-cylinder pressure still

This case freezes one optional presentation adapter:

```python
import eqiora.matplotlib as eqplot

figure = eqplot.plot_pressure(result)
figure.savefig("pressure.png")
```

The input must be the accepted
`eqiora.fluid.CircularHoleSteadyStokesResult`. The adapter passes its complete
co-indexed pressure and support mesh to Matplotlib; it does not accept raw
arrays or reconstruct scientific meaning.

## Independent acceptance contract

The evidence contract was frozen before implementation. It imports the
accepted Model, geometry, mesh, solve, pressure, and lineage obligations from
[`interfaces.python-exact-cylinder-stokes-result`](../python-exact-cylinder-stokes-result/README.md)
without changing any scientific value or tolerance.

The installed-wheel test captures the public triangular renderer call while
allowing the real Agg draw to continue. It requires exact equality between:

- renderer `x` and `result.coordinates[:, 0]`;
- renderer `y` and `result.coordinates[:, 1]`;
- explicit renderer connectivity and `result.triangles`;
- renderer vertex values and `result.pressure.numpy(copy=False)`; and
- renderer color limits and the Result's pressure extrema.

The renderer uses vertex-associated Gouraud shading only as presentation of
the already accepted P1 coefficients. It does not smooth, average, shift,
nondimensionalize, derive, integrate, or validate pressure.

## Installed and headless boundary

Registered evidence obtains this profile from the same complete candidate and
manifest as the base, typing, PyTorch, and JAX profiles. Its closed Matplotlib
check group installs the optional extra in an isolated environment, fixes the
reviewed release, removes `DISPLAY`, isolates Matplotlib configuration, and
selects Agg from the evidence environment. The focused
`tools/ci/python_matplotlib_gate.py` script remains available for standalone
development but is not a second registered artifact build. The adapter itself
does not call `show`, save a file, or select a backend. Its returned Figure is
owned by the caller.

The ordinary `eqiora` import remains Matplotlib-free, and Matplotlib remains an
optional distribution requirement. Importing `eqiora.matplotlib` without the
extra returns an actionable installation error.

The image oracle deliberately avoids exact PNG bytes, hashes, compression,
dimensions, fonts, ticks, antialiasing, or pixel values. It requires only a
successful headless draw, valid decodable PNG, canvas-consistent positive
dimensions, and nonuniform visible content.

## Non-claims

This is not a generic Result, Field, or raw-array plotting API. It claims no
velocity, vectors, contours, streamlines, probes, interactive behavior, 3D,
trajectory animation, media admission, publication styling, deterministic
image bytes, or scientific evidence from visual similarity.

Run the registered evidence with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-exact-cylinder-pressure-still
```
