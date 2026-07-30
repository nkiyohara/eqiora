# Python exact-cylinder steady-Stokes Result

This case freezes one narrow Python completion path:

```python
eqiora.fluid.solve_exact_cylinder_stokes(
    *,
    model_v7: bytes,
    geometry: RectangleWithCircularHole,
    mesh: CircularHoleChordalMesh,
) -> CircularHoleSteadyStokesResult
```

The caller supplies the Model v7 bytes explicitly. The adapter may admit only
the already accepted model, exact geometry owner, and source-bound chordal
mesh. It is a typed composition of existing scientific contracts, not a
generic Python fluid-authoring or solver API.

## Independent observations

The evidence owner did not inspect or edit the Python implementation. Numeric
observations come from the dual-independent
[`fluid.exact-circular-hole-stokes-2d`](../../fluid/exact-circular-hole-stokes-2d/README.md)
case and its independently agreed Python and Julia routes. This case imports
its six pressure probes, signed fluxes, cylinder constraint force, global
balance, solver tuple, and true-residual acceptance without retuning them.

The source and mesh identities come from the accepted exact-geometry and
source-bound-mesh cases. The model identity is derived independently from the
public ModelEnvelopeV7 rule:

```text
sha256(
  b"eqiora.model-envelope/v7"
  || 0x00
  || canonical semantic content without source_revision
)
```

For the checked model this is
`668fa55e5ab1a46d0b7523e4e3162442ccd7698697c4308604cf4fe9269249de`.
Changing only `source_revision` therefore preserves that semantic digest, but
the frozen application path still rejects the foreign revision before
assembly.

The existing decoder accepts syntactically valid non-canonical JSON and
canonicalizes it. Pretty-printed bytes are consequently a positive replay
witness, not a rejection case: they must produce the same run identity and
equal Result.

## Lineage without output-derived constants

Only the independently known model, exact-source, and mesh digests are literal
acceptance values. The evidence deliberately does not freeze implementation
output for realized geometry, correspondence, the durable chordal binding,
the complete realization, pressure snapshot, or run.

Instead it parses the exposed canonical binding and Run v2 bytes, checks their
closed field sets and semantic links, independently hashes each using its
schema-domain-separated framing, and requires the recomputed values to equal
the public digests. This catches fabricated or stale lineage while avoiding an
oracle copied from the implementation under test.

The Result owns a complete pressure P1 field co-indexed with its 104 support
coordinates. Its support arrays must equal the accepted inner mesh artifact.
The six coordinate-selected pressure probes avoid assuming a local vertex
order beyond that public co-indexing.

## Python ownership boundary

`pressure` reuses Eqiora's immutable `Array`. `coordinates` and `triangles`
are memoized read-only NumPy views, with `float64 (104, 2)` and
`uint32 (104, 3)` layouts. The views remain valid after the Result and its
other owners are deleted. None can be made writeable.

NumPy remains absent while constructing the exact geometry, source-bound mesh,
solving, inspecting scalar metadata, and indexing pressure. Matrix access is
the lazy-import boundary. The contract is exercised both from the installed
package under `python -I` and from an embedded PyO3 public-package load.

## Fail-closed boundary and non-claims

Malformed Model bytes report compatibility diagnostic `EQ0901`. A valid but
foreign source revision, foreign exact owner, swapped authored roles, coarse
mesh, or differently admitted mesh artifact reports validation diagnostic
`EQ0807` before solve.

This case claims no general Model or fluid authoring, velocity or MINI bubble
projection, drag/lift coefficient, generic meshing or solver selection,
visualization, convergence study, transient flow, Navier–Stokes, FSI,
performance, or cross-platform bit identity.

After implementation, run:

```bash
cargo test -p eqiora-python --test python_exact_cylinder_stokes_result
python3 tools/ci/python_package_gate.py
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-exact-cylinder-stokes-result
```
