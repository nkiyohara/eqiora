# Python rich Mesh semantic selection

This case verifies the N2 extension of the installed bare-Mesh notebook view.
For the one accepted 50-chord Gmsh 4.15.2 circular-hole Mesh, Rust replays the
source Geometry and its accepted Geometry↔Mesh correspondence and projects the
closed `cylinder`, `inlet`, `outlet`, `walls`, and `fluid` membership inventory
into the private wheel-local presentation payload.

The payload records each selection's exact dimension, canonical Mesh entity
indices, and owner-provided vertex closure. Python and TypeScript independently
check its transport digest, correspondence identity, closed ordering, entity
ordering, topology closure, complete boundary partition, and complete fluid
cell inventory before a view is published. Labels select only an already
authenticated record; they never determine membership.

The existing JupyterLab 4.6.2 and marimo 0.23.16 host path exposes a client-only
dropdown, exact entity-count summary, canonical-index inspector, and an overlay
whose primitive closure matches the selected membership. Switching one view
does not change another view, send a model write, mutate the Python Mesh, or
alter any accepted identity.

This is not arbitrary Geometry or Mesh display, a public viewer or selection
schema, a raw-name query, selection algebra, picking/editing, a field or result
projection, persisted widget state, scientific pixel evidence, or a
production-scale renderer.

```bash
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-rich-mesh-selection-display
```
