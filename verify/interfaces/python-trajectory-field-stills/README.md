# Python common trajectory field stills

This case freezes the accepted `Trajectory` arms of the first two common
spatial presentation adapters over the `Trajectory`, `FieldRef`, and
`FieldSnapshot` boundaries:

```python
import eqiora.matplotlib as eqplot

trajectory = result.trajectory
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

Here `result` is the common `Result` returned by the explicit
`FixedMeshMonolithic` Plan workflow in the Python modeling guide.

The first admitted consumer is the exact fixed-mesh affine-triangle 2D FSI
trajectory. Both calls admitted by this case take a `Trajectory` — not a
common `Result` — and select their field by exact Model-bound `FieldRef`
identity through `trajectory.state(step).field(field)`. Exact identity is the
`(Model artifact, field)` pair, and two fixtures close it from both sides: an
independent compilation of one source is structurally equivalent yet allocates
fresh semantic field ids, and a committed value edit keeps every semantic field
id while changing only the exact Model artifact. Neither structural
equivalence nor a field id string is an authority, so a `FieldRef` from either
Model is rejected before a Figure exists.

`plot_deformed_field` also has a static `Result` arm without `step`. That arm
selects the bounded mixed-boundary displacement `FieldSnapshot` and exact
generated-Cartesian Q1 `Mesh` through the same common lookups, and is owned by
[`interfaces.python-mixed-boundary-elasticity-demo`](../python-mixed-boundary-elasticity-demo/README.md).
It does not widen this trajectory case to arbitrary single-state spatial
results. A `Result` call that supplies `step`, or a `Trajectory` call that
omits it, rejects before constructing a Figure.

`plot_fixed_reference_fsi` is withdrawn by this slice with no alias, shim, or
deprecation path. The shared adapter surface also contains no `plot_pressure`;
its static `Result` replacement through `plot_scalar_field` belongs to
[`interfaces.python-exact-cylinder-pressure-still`](../python-exact-cylinder-pressure-still/README.md),
not this Trajectory claim.

The predecessor FSI state, result, and one-call solve names are absent as well;
the common `TrajectoryState`, common `Result`, explicit Plan execution, and
typed evidence are their only product successors.

For one subsequent prerelease, `plot_displacement` remains only as an
actionable `DeprecationWarning`-emitting delegation to the structural static
`Result` arm of `plot_deformed_field`. It owns no renderer, accepted input,
scientific value, tolerance, lineage, or evidence in this case and is not an
ordinary presentation API.

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

For the accepted pressure dimension, the Trajectory scalar colorbar remains
the unnamed coherent-SI label `Value [kg·m^-1·s^-2]`. The named `Pressure [Pa]`
label is specific to the static `Result` route owned by the pressure-still case.

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
meshes, arbitrary single-state spatial results, field-name selection, named unit
conversion, derived magnitudes, Studio or notebook hosting, scientific
derivation, media admission, and visual scientific validation are not claimed.
The absence of the withdrawn still and predecessor FSI product names is checked
in the installed runtime, exports, and packaged stub; documentation and example
text are outside the installed profile this case executes. The static-`Result` overload of
`plot_scalar_field` is owned by the exact-cylinder pressure-still case;
The compatibility-only `plot_displacement` name remains outside this case's
accepted input and evidence ownership; its delegation is checked by the
structural sibling case.

The complete bounded executable contract and its pre-committed falsifiers are
in [`case.toml`](case.toml).

Run the registered evidence with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-trajectory-field-stills
```
