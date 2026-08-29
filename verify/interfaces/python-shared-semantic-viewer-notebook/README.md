# Installed-wheel shared semantic viewer Notebook host

This case verifies the V3 host boundary of the shared viewer without adding a
scientific claim. The canonical application is
[`examples/python/shared_semantic_viewer_marimo.py`](../../../examples/python/shared_semantic_viewer_marimo.py).
It constructs one exact 2-by-2 metre planar rectangle with current public
`GeometryGraph`, realizes a 4-by-3 `CartesianMesher` Mesh, and supplies those
accepted objects to `View` from one non-editable installed wheel.

## Independent positive path

The CPython 3.13 Notebook candidate installs the wheel's exact `viewer` extra
and Marimo 0.23.16. It copies only the canonical application into a clean
consumer, launches it under `python -I`, and drives the independently managed
Chromium host with the shared Playwright harness. The DOM oracle requires the
accepted carrier names, 20 vertices and 12 cells independently implied by a
4-by-3 Cartesian grid, the private scene marker, the read-only canvas, camera
controls, and an exact Mesh-owned `left` selection. It observes no scientific
coefficient, pixel identity, colour, camera value, or renderer triangulation.

The application and wheel carry the JavaScript/CSS. Runtime traffic must stay
on exact loopback; an external renderer or asset request rejects. The host
lifecycle uses the candidate's existing owned-process cleanup and mutation
checks. Focused Python tests separately prove lazy optional imports,
deterministic text fallback, immutable widget payload, exact owner rejection,
and `View.close()`. The maintained renderer spike separately proves browser
resource disposal.

## Claim boundary

This proves one installed Linux x86-64 ordinary-GIL CPython 3.13 / Marimo
0.23.16 path for accepted planar Geometry and quadrilateral Mesh composition.
V0--V2 product tests own the private scene, selection, and vertex/cell scalar
field behavior. The private transport is replaceable and is not an artifact.

It does not claim JupyterLab host execution, Studio or Cloud integration, 3D,
vector/tensor/trajectory display, animation, contours, streamlines, derived
science, image identity, performance, large-data behavior, or production
scale.

Run the registered aggregate with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-shared-semantic-viewer-notebook
```
