# Python common trajectory field stills

This case freezes the first two common spatial presentation adapters over the
accepted `Trajectory`, `FieldRef`, and `FieldSnapshot` boundaries:

```python
import eqiora.matplotlib as eqplot

trajectory = eqiora.fsi.solve_fixed_reference_fsi(model).trajectory
pressure = eqplot.plot_scalar_field(
    trajectory,
    step=2,
    field=model.field("fluid_pressure"),
)
geometry = eqplot.plot_deformed_field(
    trajectory,
    step=2,
    field=model.field("solid_displacement"),
    scale=12.0,
)
```

The first admitted consumer is the exact fixed-mesh affine-triangle 2D FSI
trajectory. Both adapters take a `Trajectory` — not an application result — and
select their field by exact Model-bound `FieldRef` identity through
`trajectory.state(step).field(field)`. A field id string is never an authority,
so a `FieldRef` carrying the same id from a structurally equivalent but
different exact Model is rejected before a Figure exists.

`plot_fixed_reference_fsi` is withdrawn by this slice with no alias, shim, or
deprecation path.

## Independent acceptance contract

This contract was frozen before implementation by a Claude lineage that does
not implement the slice. It imports every scientific value, tolerance, and
lineage obligation by reference from
[`interfaces.python-fixed-mesh-trajectory`](../python-fixed-mesh-trajectory/README.md),
[`interfaces.python-fixed-reference-fsi-demo`](../python-fixed-reference-fsi-demo/README.md),
and [`fsi.fixed-reference-monolithic-step-2d`](../../fsi/fixed-reference-monolithic-step-2d/README.md).
It introduces no derived quantity, expected value, tolerance, or unit
conversion of its own.

The registered evidence captures the public renderer call while allowing the
real Agg draw to continue, and extracts the remaining inputs from the returned
Figure. It requires exact equality between:

- renderer `x`/`y` and `trajectory.coordinates` columns zero and one;
- renderer vertex values and the complete unchanged `values("vertex")` block;
- explicit renderer connectivity and `trajectory.cells` restricted to the
  admitted cells, carrying global canonical vertex indices;
- artist color limits and the extrema of the support-restricted values;
- the reference wireframe and `coordinates` on the admitted edges; and
- the deformed wireframe and `coordinates + scale * values` on those edges.

Admitted cells are exactly those whose complete vertex tuple lies in
`support_indices("vertex")`, their sorted unique vertex closure must equal that
support and be nonempty, and the drawn edge set is the sorted unique undirected
edge set of those cells with each edge drawn once. The evidence re-derives both
sets from the accepted connectivity and support membership, never from
coefficient values, so a zero-valued supported entity cannot be dropped.

`solid_velocity` shares its value shape, spatial-cartesian frame, and single
vertex association with `solid_displacement`; only the SI length dimension
separates them. Rejecting it is therefore the sharp falsifier for the
deformation contract rather than a shape check in disguise.

## Installed and headless boundary

Registered evidence obtains this profile from the same complete candidate and
manifest as the base, typing, PyTorch, and JAX profiles, so it shares one
execution with the sibling Matplotlib cases instead of adding a second artifact
build. The focused `tools/ci/python_matplotlib_gate.py` script remains
available for standalone development and is not registered here. The adapters
never call `show`, save a file, or select a backend; their returned Figures are
caller-owned, survive release of the trajectory, and register no pyplot figure.

The image oracle deliberately avoids exact PNG bytes, hashes, dimensions,
pixels, colormaps, fonts, and layout metrics. It requires only a successful
headless draw, a valid decodable PNG, and nonuniform visible content.

## Non-claims

This is not generic Field plotting. Cell-associated scalars, glyph or vector
stills, interface overlays, composite figures, animation, 3D, non-triangle
meshes, single-state spatial results, field-name selection, named unit
conversion, derived magnitudes, Studio or notebook hosting, scientific
derivation, media admission, and visual scientific validation are not claimed.
The absence of the withdrawn still is checked in the installed runtime,
exports, and packaged stub only; documentation and example text are outside the
installed profile this case executes. `plot_pressure` and `plot_displacement`
remain untouched by this claim.

The complete bounded executable contract and its pre-committed falsifiers are
in [`case.toml`](case.toml).

Run the registered evidence with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-trajectory-field-stills
```
