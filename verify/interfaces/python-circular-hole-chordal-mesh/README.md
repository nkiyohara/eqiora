# Exact-cylinder external Gmsh 4.15.2 mesh

This case freezes one deliberately narrow installed-Python path:

```text
exact circular-hole Geometry
  -> MeshRequest
  -> eqiora.meshing.resolve
  -> external Gmsh CLI exactly 4.15.2
  -> immutable MeshPlan containing the inspected mesh
  -> eqiora.meshing.generate
  -> immutable Mesh
```

The plan provider is exactly `eqiora.gmsh-cli/4.15.2`. Executable resolution
uses the explicit `EQIORA_GMSH` path when present and otherwise `gmsh` on
`PATH`. Missing executables, launch failures, nonzero exits, malformed or
unsupported output, and every nonexact version fail as structured validation.
None may fall back to the former reference spoke mesh.

## Independently owned positive witness

The ordinary positive path runs before provider-failure probes. It retains the
exact source digest
`b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`
and the existing request and boundary receipt:

```text
maximum_boundary_error = 1e-4 m
minimum_mean_ratio = 1e-5
maximum_boundary_facets = 50
circle_segments = 50
```

The evidence owner started from commit
`934493bcb487c1753fb4b3ddffaab88d7150aa7d`, constructed the pre-existing
accepted Rust chordal owner, and traversed its canonical `PlanarRegion` hole
first and outer loop second. Coordinates were emitted with Rust's
shortest-roundtrip binary64 spelling. The Gmsh recipe has no point mesh-size
argument and fixes:

- Built-in straight lines and one planar surface;
- Algorithm 6, element order 1, and all elements saved;
- ASCII MSH 4.1, `Mesh.RandomFactor = 0`; and
- one thread.

The derived GEO SHA-256 is
`81c96068891d6b506827339cd6fecf07eafcb867c76f01747c35d134167d367e`.
Independently installed official Linux64 and PyPI Gmsh 4.15.2 produced the
same local MSH bytes, and an immediate clean replay was byte-identical. Their
local SHA-256 was
`ab7340cec1976f713b5c5deab76fc7d554593126f1c1cd68cc021749911a206a`.
That hash records this Linux derivation; it is not a raw-MSH portability claim.

The pre-existing bounded MSH 4.1 importer and mesh envelope project those
bytes to 662 vertices and 1,210 positively oriented linear triangles. The
former 104-vertex/104-triangle spoke artifact is therefore not the accepted
positive path.

## Independent quality derivations

The accepted two-dimensional `AffineMapQuality` for a cell with local
Jacobian `J` is

```text
q = 2 |det(J)| / ||J||_F^2.
```

One derivation imported the frozen MSH through the pre-existing Rust importer
and mesh constructor. A separate Python numerical pass decoded the MSH node
blocks and triangle blocks, rebuilt each local Jacobian in importer order, and
evaluated the formula directly. They agreed exactly in binary64 on:

```text
minimum_mean_ratio          = 0.5236522686855336
minimum_signed_measure_scale = 2.6093038450074273e-5
```

The installed-Python test repeats the coordinate-array calculation and
requires the achieved quality to satisfy the requested `1e-5` gate. A `0.75`
request is a precommitted rejection probe, not a tuned production target.
Reconstructing the circle with Gmsh-side trigonometric expressions changes
interior Delaunay coordinates and is an explicit non-authoritative route.

## Frozen public mesh projection

For the exact one-thread Linux witness, the imported mesh projection has:

```text
canonical byte length       42,388
raw canonical SHA-256       9d3c6211e6832aa5a5f7e99fa210058ff1b76eab7f1e99aaa7033c282d6e2dd2
domain-separated Mesh digest 5962836788fa785fd0761813c542e9078523796409787d86ad8a006dfef5b62b
f64 coordinate-buffer SHA-256 42ea585f3facdc21fadf66435f37f1127bf926e6159c5ff1e4a345ba7268db3d
u32 triangle-buffer SHA-256   05a68c5630e68ed091e7da3bff07516a9ddf9345bc8319db108ac4004a7c6642
```

`Mesh.digest` retains the established domain separation:

```text
sha256(
  b"eqiora.simplicial-mesh-envelope/v1"
  || 0x00
  || canonical_json
)
```

The NumPy projections are memoized, C-contiguous, and irreversibly read-only.
The test also requires repeated `generate` calls to publish independent array
storage with identical content.

## Conformity and authored correspondence

An independent edge-incidence pass finds exactly 114 boundary edges and
classifies every one against the accepted chordal geometry:

```text
cylinder = 50
inlet    = 14
outlet   = 2
walls    = 48
fluid    = 1,210 triangles
```

The public selection counts must match that complete partition. A second
same-coordinate source swaps only the authored inlet and outlet names. Its
mesh identity remains unchanged, while correspondence-derived public counts
become `inlet=2` and `outlet=14`. This rejects hard-coded standard-name
membership and keeps the MSH file from deciding authored meaning.

The plan inspection occurs during `resolve`. A forwarding executable records
the positive Gmsh calls and is then made unlaunchable; two calls to `generate`
must still publish the exact inspected mesh without another external launch.

## Failure closure

After the positive witness, focused probes require structured
`eqiora.ValidationError` for:

- absent Gmsh on both explicit and `PATH` routes;
- an explicit executable that cannot launch, even when valid Gmsh is on
  `PATH`;
- exact-version discovery followed by a nonzero mesh-generation exit;
- malformed bytes or an unsupported MSH version after a successful exit;
- older, newer, suffixed, or multiline version reports;
- a 49-facet work limit, a `0.75` quality requirement, an unknown selection,
  or a plan replayed against foreign exact Geometry.

The explicit-path positive probe places a wrong-version `gmsh` on `PATH` and
still requires the explicit 4.15.2 executable. The complementary positive
probe removes `EQIORA_GMSH` and resolves through `PATH`.

## Claim boundary

This is one current exact-cylinder straight-chordal planar geometry, linear
two-dimensional triangles, ASCII MSH 4.1, and external Gmsh exactly 4.15.2.
It does not claim arbitrary geometry, another provider or version, 3D, curved
elements, bundled or downloaded Gmsh, raw-MSH or cross-platform byte identity,
persistence, performance, general production meshing, a Model, solve, Result,
visualization, or physical validation.

Run the registered evidence in an environment that supplies the exact
executable:

```bash
EQIORA_GMSH=/absolute/path/to/gmsh-4.15.2 \
  python3 tools/ci/python_package_gate.py
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-circular-hole-chordal-mesh
```
