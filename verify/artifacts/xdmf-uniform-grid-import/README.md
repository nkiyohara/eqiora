# XDMF Uniform-grid import verification

This case parses one XDMF 3 metadata document into a bounded typed import
plan. The admitted document contains one `Domain`, one two-dimensional
`Uniform` grid, positively oriented `Tri3` topology, `XY` geometry, one
node-centered scalar, and one cell-centered two-component vector. Every
`DataItem` references a dataset in one checked-in HDF5 source.

The adapter never opens a path or performs network I/O. The test supplies a
caller-owned resolver with the complete HDF5 source bytes and the requested
typed arrays. Normalized topology, geometry, and field values are rebuilt
through Eqiora's shared mesh, field, provenance, and artifact contracts.

Fresh artifact production and persisted replay are separate. Replay accepts an
independently decoded expected manifest plus expected mesh and ordered fields,
then fresh-derives the import and requires complete equality before issuing an
opaque handle. Falsifiers cross-wire request identities, same-shaped resolved
values, and accepted artifacts; reject malformed XML lexical constructs,
DTD/entity and XInclude input; exhaust parser and decoded-array limits; and
reject wrong scalar types, shapes, dimensions, order, or truncated metadata.
Source-byte mutation instead produces a distinct fresh manifest and fails
replay against the prior manifest. A resolver may rebind same-shaped values to
the correct requests and thereby create a different valid fresh import; these
checks do not prove that it honestly decoded the supplied source bytes.

This evidence does not claim native HDF5 parsing, resolver honesty, implicit
filesystem or network resolution, temporal collections, export, mixed or
high-order cells, curved geometry, or general XDMF support.

Run:

```bash
cargo test --locked -p eqiora --features xdmf --test xdmf_uniform_grid_import
cargo run --locked -p eqiora-verify -- run \
  --case artifacts.xdmf-uniform-grid-import
```
