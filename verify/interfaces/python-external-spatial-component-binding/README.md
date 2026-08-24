# Python external spatial Component binding

This case installs the built wheel, loads its component-only
`steady-flow-past-cylinder.eqi`, authors the exact rectangle-with-circular-hole
Geometry in Python, resolves `fluid`, `inlet`, `outlet`, `walls`, and
`cylinder` once into revision-bound `GeometrySelection` values, and passes
those values plus explicit coherent-SI Parameters to `eqiora.bind_component`.

The source contains no box, coordinate bounds, circle centre, or radius.
Python derives `channel_height` from `geometry.bounds`. The native adapter
accepts no raw selection names, authenticates every handle against the exact
Geometry revision, and delegates support/Parameter typing, hierarchy
expansion, lowering, transaction commit, semantic admission, and canonical
artifact reconstruction to the existing Rust owners. The returned value is
the ordinary immutable common `Model`.

The positive path runs before missing support, foreign/stale handle,
volume/boundary swap, raw-name, extra Parameter, and non-finite Parameter
rejections. The installed test never reads the packaged Model JSON.

This case stops at Model construction. Exact-cylinder steady-Stokes Plan/Run
admission remains blocked by #522 and is not evidence here. There is no new
scientific value, tolerance, result, persisted binding schema, general
package loader, arbitrary Geometry, or support algebra claim.

Run:

```bash
mise run pr -- --case interfaces.python-external-spatial-component-binding
```
