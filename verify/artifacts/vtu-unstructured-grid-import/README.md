# VTU one-Piece ASCII unstructured-grid import verification

This case fixes the first bounded VTK XML `UnstructuredGrid` input to one
serial `.vtu` file containing one `Piece`, four planar `Float64` XYZ points,
and two positively oriented `VTK_TRIANGLE` cells. Selection uses exact XML
structural element paths:

- `[0,0]` selects the sole `Piece`;
- `[0,0,0,0]` selects `PointData/temperature`, one `Float64` scalar per
  point; and
- `[0,0,1,0]` selects `CellData/flux`, one two-component `Float64` vector per
  cell.

Association and `Name` are checked after structural selection and retained as
display/provenance metadata. They are not lookup keys or selection identity.
The normalized geometry retains Points DataArray path `[0,0,2,0]`; topology
retains the composite Cells path `[0,0,3]`, because connectivity, offsets, and
types are admitted together as one topology payload.

[`fixtures/unit-square-tri3-ascii.vtu`](fixtures/unit-square-tri3-ascii.vtu)
is the unedited ASCII output of VTK Python 9.5.2's maintained
`vtkXMLUnstructuredGridWriter`. The writer's optional `RangeMin`, `RangeMax`,
and nested `InformationKey` values are retained as reference-tool syntax but
are not accepted mesh or Field meaning. Selection does not depend on names or
those derived ranges.

[`expected/summary.json`](expected/summary.json) is an independently readable
expected-content table. It records exact points, connectivity, offsets, VTK
cell types, selectors, and Field values rather than copying a future Eqiora
artifact representation. [`expected/source.sha256`](expected/source.sha256)
fixes the original fixture bytes. No placeholder normalized mesh, Field,
manifest, or accepted-artifact digest is recorded before the importer fixes
those identities.

The Rust integration test constructs its own typed mesh and Field oracle and
does not deserialize this human-readable summary. This keeps the executable
oracle independent of the table intended for reviewer inspection. The test
also renames a Field while preserving its structural path: accepted mesh and
Field artifacts remain identical, while source identity, manifest identity,
and display provenance change.

## Regeneration

The checked-in recipe uses only the official VTK Python package and pins the
generator version that emitted the fixture:

```bash
cd verify/artifacts/vtu-unstructured-grid-import
uv run --with vtk==9.5.2 python references/generate_fixture.py
sha256sum -c expected/source.sha256
```

Regeneration is an explicit fixture-maintenance operation. A VTK upgrade may
change non-semantic XML metadata or formatting and therefore the source-byte
digest; review those changes independently from accepted mesh/Field content.

## Claim boundary

This evidence is limited to ASCII, one structurally selected Piece, planar
affine Tri3 cells, one structurally selected point scalar, and one structurally
selected two-component cell vector. It does not
claim Tet4 fixture evidence, inline/appended binary, compression, multiple
pieces or `.pvtu`, mixed/high-order/curved cells, temporal collections,
export, implicit path resolution, or general VTK support.

Once the adapter and public workflow target are present, run:

```bash
cargo test --locked -p eqiora --features vtu --test vtu_unstructured_grid_import
cargo run --locked -p eqiora-verify -- run \
  --case artifacts.vtu-unstructured-grid-import
```
